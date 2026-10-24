use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{SyncSender, TrySendError},
};

use ratatui::{
    buffer::Buffer,
    layout::{Rect, Size},
};
use ratatui_image::{Resize, ResizeEncodeRender, picker::Picker, protocol::StatefulProtocol};

use crate::image_render::worker::{DecodedImage, ImageJob, ImageOutput, ImageWork};

const BYTE_LIMIT: usize = 64 * 1024 * 1024;
static NEXT_REQUEST: AtomicU64 = AtomicU64::new(1);

/// LRU-evicting cache of decoded images and their protocol renderers for one view.
pub struct ImageCache {
    map: lru::LruCache<PathBuf, ImageEntry>,
}

struct ImageEntry {
    request: u64,
    cancel: Arc<AtomicBool>,
    max_dim: u32,
    bytes: usize,
    visible: bool,
    state: State,
}

enum State {
    Loading(bool),
    Ready(Box<CachedImage>),
    Error,
}

impl Drop for ImageEntry {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Protocol ownership moves to the bounded worker during resize/encoding.
pub struct CachedImage {
    protocol: Option<Box<StatefulProtocol>>,
    key: PathBuf,
    request: u64,
    cancel: Arc<AtomicBool>,
    tx: SyncSender<ImageJob>,
    retry: bool,
}

impl CachedImage {
    pub fn size_for(&self, resize: Resize, size: Size) -> Size {
        self.protocol
            .as_ref()
            .map_or(size, |protocol| protocol.size_for(resize, size))
    }
}

impl ResizeEncodeRender for CachedImage {
    fn needs_resize(&self, resize: &Resize, size: Size) -> Option<Size> {
        self.protocol
            .as_ref()
            .and_then(|protocol| protocol.needs_resize(resize, size))
    }
    fn resize_encode(&mut self, resize: &Resize, size: Size) {
        let Some(protocol) = self.protocol.take() else {
            return;
        };
        let job = ImageJob {
            key: self.key.clone(),
            request: self.request,
            cancel: self.cancel.clone(),
            work: ImageWork::Encode {
                protocol,
                resize: resize.clone(),
                size,
            },
        };
        match self.tx.try_send(job) {
            Ok(()) => self.retry = false,
            Err(error) => {
                let job = match error {
                    TrySendError::Full(job) | TrySendError::Disconnected(job) => job,
                };
                if let ImageWork::Encode { protocol, .. } = job.work {
                    self.protocol = Some(protocol);
                }
                self.retry = true;
            }
        }
    }
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        if let Some(protocol) = &mut self.protocol {
            protocol.render(area, buf);
        }
    }
}

impl ImageCache {
    pub fn new(limit: usize) -> Self {
        Self {
            map: lru::LruCache::new(NonZeroUsize::new(limit.max(1)).expect("limit >= 1")),
        }
    }

    pub(crate) fn set_limit(&mut self, limit: usize) {
        self.map
            .resize(NonZeroUsize::new(limit.max(1)).expect("limit >= 1"));
    }

    /// Request an image for display. If the key is absent, sends a decode job
    /// to the worker. Call `install_decoded` when the result arrives.
    pub fn request(&mut self, key: PathBuf, max_dim: u32, tx: &SyncSender<ImageJob>) {
        if self
            .map
            .peek(&key)
            .is_some_and(|entry| entry.max_dim != max_dim)
        {
            self.map.pop(&key);
        }
        if !self.map.contains(&key) {
            self.map.put(
                key.clone(),
                ImageEntry {
                    request: NEXT_REQUEST.fetch_add(1, Ordering::Relaxed),
                    cancel: Arc::new(AtomicBool::new(false)),
                    max_dim,
                    bytes: 0,
                    visible: true,
                    state: State::Loading(false),
                },
            );
        }
        let entry = self.map.get_mut(&key).expect("image entry inserted");
        entry.visible = true;
        // Touch existing entry; a saturated queue retries on the next visible frame.
        if matches!(entry.state, State::Loading(false)) {
            match tx.try_send(ImageJob {
                key,
                request: entry.request,
                cancel: entry.cancel.clone(),
                work: ImageWork::Decode(max_dim),
            }) {
                Ok(()) => entry.state = State::Loading(true),
                Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Disconnected(_)) => entry.state = State::Error,
            }
        }
    }

    pub(crate) fn accepts(&self, image: &DecodedImage) -> bool {
        self.map
            .peek(&image.key)
            .is_some_and(|entry| entry.request == image.request)
    }

    /// Install a completed decode result and build the protocol renderer.
    pub fn install_decoded(
        &mut self,
        img: DecodedImage,
        picker: &Picker,
        tx: &SyncSender<ImageJob>,
    ) -> Option<String> {
        if !self.accepts(&img) {
            return None;
        }
        let entry = self.map.get_mut(&img.key).expect("image belongs to cache");
        let error = match img.result {
            Ok(ImageOutput::Decoded(image)) => {
                entry.bytes = image.as_bytes().len();
                // Created lazily from decoded + picker; `None` while decode pending.
                entry.state = State::Ready(Box::new(CachedImage {
                    protocol: Some(Box::new(picker.new_resize_protocol(image))),
                    key: img.key,
                    request: entry.request,
                    cancel: entry.cancel.clone(),
                    tx: tx.clone(),
                    retry: false,
                }));
                None
            }
            Ok(ImageOutput::Encoded(protocol)) => {
                if let State::Ready(cached) = &mut entry.state {
                    cached.protocol = Some(protocol);
                    cached.retry = false;
                }
                None
            }
            Err(error) => {
                entry.state = State::Error;
                entry.bytes = 0;
                Some(format!("Image rendering failed: {error:#}"))
            }
        };
        while self.map.iter().map(|(_, entry)| entry.bytes).sum::<usize>() > BYTE_LIMIT {
            self.map.pop_lru();
        }
        error
    }

    /// Get a mutable reference to the protocol for rendering, if ready.
    pub fn get_proto(&mut self, key: &PathBuf) -> Option<&mut CachedImage> {
        self.map
            .get_mut(key)
            .and_then(|entry| match &mut entry.state {
                State::Ready(image) => Some(image.as_mut()),
                _ => None,
            })
    }

    pub(crate) fn is_pending(&self) -> bool {
        self.map.iter().any(|(_, entry)| match &entry.state {
            State::Loading(_) => true,
            State::Ready(image) => image.protocol.is_none() || image.retry,
            State::Error => false,
        })
    }

    pub(crate) fn begin_frame(&mut self) {
        for (_, entry) in &mut self.map {
            entry.visible = false;
        }
    }

    pub(crate) fn end_frame(&mut self) {
        let unused: Vec<_> = self.map.iter().filter_map(|(key, entry)| {
            let pending = matches!(&entry.state, State::Loading(_)) || matches!(&entry.state, State::Ready(image) if image.protocol.is_none() || image.retry);
            (!entry.visible && pending).then(|| key.clone())
        }).collect();
        for key in unused {
            self.map.pop(&key);
        }
    }

    pub(crate) fn cancel_pending(&mut self) {
        let keys: Vec<_> = self
            .map
            .iter()
            .filter_map(|(key, entry)| match &entry.state {
                State::Loading(_) => Some(key.clone()),
                State::Ready(image) if image.protocol.is_none() || image.retry => Some(key.clone()),
                _ => None,
            })
            .collect();
        for key in keys {
            self.map.pop(&key);
        }
    }

    pub(crate) fn invalidate(&mut self, path: &Path) {
        self.map.pop(path);
    }
    pub(crate) fn clear(&mut self) {
        self.map.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eviction_cancels_work_and_keeps_recent_entries() {
        let mut cache = ImageCache::new(2);
        let (tx, rx) = std::sync::mpsc::sync_channel(16);
        cache.request("a.png".into(), 100, &tx);
        let first = rx.recv().unwrap();
        cache.request("b.png".into(), 100, &tx);
        let second = rx.recv().unwrap();
        cache.get_proto(&"a.png".into());
        cache.request("c.png".into(), 100, &tx);
        assert_eq!(cache.map.len(), 2);
        assert!(!first.cancel.load(Ordering::Relaxed));
        assert!(second.cancel.load(Ordering::Relaxed));
    }

    #[test]
    fn saturated_queue_retries_and_resolution_change_rejects_stale_results() {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let mut cache = ImageCache::new(2);
        cache.request("a.png".into(), 100, &tx);
        cache.request("b.png".into(), 100, &tx);
        assert!(matches!(
            cache.map.peek(&PathBuf::from("b.png")).unwrap().state,
            State::Loading(false)
        ));
        let old = rx.recv().unwrap();
        cache.request("a.png".into(), 2048, &tx);
        assert!(old.cancel.load(Ordering::Relaxed));
        assert!(!cache.accepts(&DecodedImage {
            key: old.key,
            request: old.request,
            result: Err(anyhow::anyhow!("old"))
        }));
        rx.recv().unwrap();
        cache.request("b.png".into(), 100, &tx);
        assert!(matches!(
            cache.map.peek(&PathBuf::from("b.png")).unwrap().state,
            State::Loading(true)
        ));
    }

    #[test]
    fn errors_finish_requests_and_pixel_budget_evicts_lru_images() {
        let (tx, rx) = std::sync::mpsc::sync_channel(16);
        let picker = Picker::halfblocks();
        let mut cache = ImageCache::new(32);
        cache.request("bad.png".into(), 2048, &tx);
        let job = rx.recv().unwrap();
        assert!(
            cache
                .install_decoded(
                    DecodedImage {
                        key: job.key,
                        request: job.request,
                        result: Err(anyhow::anyhow!("invalid image"))
                    },
                    &picker,
                    &tx
                )
                .is_some()
        );
        assert!(!cache.is_pending());
        cache.request("bad.png".into(), 2048, &tx);
        assert!(rx.try_recv().is_err());
        for i in 0..5 {
            cache.request(format!("{i}.png").into(), 2048, &tx);
            let job = rx.recv().unwrap();
            cache.install_decoded(
                DecodedImage {
                    key: job.key,
                    request: job.request,
                    result: Ok(ImageOutput::Decoded(image::DynamicImage::new_rgba8(
                        2048, 2048,
                    ))),
                },
                &picker,
                &tx,
            );
        }
        assert!(
            cache
                .map
                .iter()
                .map(|(_, entry)| entry.bytes)
                .sum::<usize>()
                <= BYTE_LIMIT
        );
        assert!(!cache.map.contains(&PathBuf::from("0.png")));
    }
}

#[cfg(test)]
mod visibility_tests {
    use super::*;
    #[test]
    fn offscreen_unsent_requests_do_not_keep_the_ui_polling() {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let mut cache = ImageCache::new(32);
        cache.request("old.png".into(), 100, &sender);
        let old = receiver.recv().unwrap();
        cache.request("waiting.png".into(), 100, &sender);
        cache.begin_frame();
        cache.request("visible.png".into(), 100, &sender);
        cache.end_frame();
        assert!(old.cancel.load(Ordering::Relaxed));
        assert!(!cache.map.contains(&PathBuf::from("waiting.png")));
        assert!(cache.map.contains(&PathBuf::from("visible.png")));
    }
}
