//! Public-only card covers share bounded decoding and the application's native image protocol.
use super::{layout_options::ImageFit, value::Value};
use crate::{app::App, storage::metadata::VaultMetadata};
use anyhow::{Result, bail};
use ratatui::{
    Frame,
    layout::{Rect, Size},
    style::{Color, Style},
    widgets::{Paragraph, Wrap},
};
use ratatui_image::{
    Resize, StatefulImage,
    picker::Picker,
    protocol::StatefulProtocol,
    sliced::{SignedPosition, SlicedImage, SlicedProtocol},
};
use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    time::SystemTime,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    id: String,
    remote: bool,
    generation: u64,
    size: u64,
    modified: Option<SystemTime>,
    shape: Option<(u32, u32)>,
    slice: Option<(u16, u16)>,
}
enum Entry {
    Pending(Arc<AtomicBool>),
    Ready(Box<StatefulProtocol>),
    Sliced(Box<SlicedProtocol>),
    Error(String),
}
impl Drop for Entry {
    fn drop(&mut self) {
        if let Self::Pending(cancelled) = self {
            cancelled.store(true, Ordering::Relaxed);
        }
    }
}
struct Response {
    key: Key,
    image: Result<image::DynamicImage>,
}
pub(super) struct Cover<'a> {
    pub value: &'a Value,
    pub host: &'a str,
    pub fit: ImageFit,
    /// Full cover height and rows hidden above the viewport.
    pub clip: Option<(u16, u16)>,
}
pub(super) struct PublicImages {
    cache: lru::LruCache<Key, Entry>,
    tx: SyncSender<Response>,
    rx: Receiver<Response>,
    active: Arc<AtomicUsize>,
    generation: u64,
}
impl Default for PublicImages {
    fn default() -> Self {
        let (tx, rx) = mpsc::sync_channel(4);
        Self {
            cache: lru::LruCache::new(NonZeroUsize::new(64).expect("cover capacity is positive")),
            tx,
            rx,
            active: Arc::new(AtomicUsize::new(0)),
            generation: 0,
        }
    }
}
impl PublicImages {
    pub fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.cache.clear();
    }
    #[cfg(test)]
    pub(super) fn stats(&self) -> (usize, usize, usize) {
        (
            self.cache.len(),
            self.active.load(Ordering::Relaxed),
            self.cache
                .iter()
                .filter(|(_, entry)| matches!(entry, Entry::Ready(_) | Entry::Sliced(_)))
                .count(),
        )
    }
    fn poll(&mut self, picker: &Picker) -> bool {
        let mut changed = false;
        while let Ok(response) = self.rx.try_recv() {
            if self.cache.contains(&response.key) {
                let entry = match response.image {
                    Ok(image) => {
                        if let Some((width, height)) = response.key.slice {
                            match SlicedProtocol::new(picker, image, Some(Size::new(width, height)))
                            {
                                Ok(protocol) => Entry::Sliced(Box::new(protocol)),
                                Err(error) => Entry::Error(format!("{error:#}")),
                            }
                        } else {
                            Entry::Ready(Box::new(picker.new_resize_protocol(image)))
                        }
                    }
                    Err(error) => Entry::Error(format!("{error:#}")),
                };
                self.cache.put(response.key, entry);
                changed = true;
            }
        }
        changed
    }
    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        cover: Cover<'_>,
        metadata: &VaultMetadata,
        app: &App,
    ) {
        let Cover {
            value,
            host,
            fit,
            clip,
        } = cover;
        if area.is_empty() || matches!(value, Value::Null) {
            return;
        }
        let text = value.to_text().display();
        if let Some(color) = hex_color(&text) {
            frame.render_widget(Paragraph::new("").style(Style::default().bg(color)), area);
            return;
        }
        let source = match cover_source(value, host, metadata) {
            Ok(source) => source,
            Err(error) => {
                diagnostic(frame, area, &format!("Cover: {error}"), app);
                return;
            }
        };
        if !app.config.features.images.is_enabled() {
            diagnostic(frame, area, "Images disabled", app);
            return;
        }
        let Some(picker) = &app.image_picker else {
            diagnostic(frame, area, "Image protocol unavailable", app);
            return;
        };
        self.poll(picker);
        let (id, remote, size, modified) = match source {
            CoverSource::Public(id) => {
                let file = metadata
                    .file(&id)
                    .expect("cover belongs to public snapshot");
                (id, false, file.size, file.modified)
            }
            CoverSource::Remote(url) => (url, true, 0, None),
        };
        let font = picker.font_size();
        let dimensions = || {
            let width = u32::from(area.width) * u32::from(font.width.max(1));
            let height = u32::from(clip.map_or(area.height, |(height, _)| height))
                * u32::from(font.height.max(1));
            let scale = 256.0 / f64::from(width.max(height).max(1));
            (
                (f64::from(width) * scale).round().max(1.0) as u32,
                (f64::from(height) * scale).round().max(1.0) as u32,
            )
        };
        let shape = if fit == ImageFit::Cover {
            Some(dimensions())
        } else {
            None
        };
        let slice = clip.map(|(height, _)| (area.width, height));
        let key = Key {
            id,
            remote,
            generation: self.generation,
            size,
            modified,
            shape,
            slice,
        };
        if !self.cache.contains(&key) && self.active.load(Ordering::Relaxed) < 2 {
            // Only this UI-owned cache admits work; background tasks only decrement.
            self.active.fetch_add(1, Ordering::Relaxed);
            let cancel = Arc::new(AtomicBool::new(false));
            self.cache.put(key.clone(), Entry::Pending(cancel.clone()));
            let storage = app.storage.clone();
            let tx = self.tx.clone();
            let active = self.active.clone();
            let key = key.clone();
            app.notes_worker_pool.spawn(move || {
                let image = (|| {
                    if cancel.load(Ordering::Relaxed) {
                        bail!("cover cancelled");
                    }
                    let bytes = if key.remote {
                        crate::image_render::remote::load_remote_image(&key.id, &cancel)?
                    } else {
                        storage.load_public_attachment(&key.id)?
                    };
                    if cancel.load(Ordering::Relaxed) {
                        bail!("cover cancelled");
                    }
                    let image = crate::image_render::worker::decode_public_image(bytes, 256)?;
                    Ok(if let Some((width, height)) = key.shape {
                        image.resize_to_fill(width, height, image::imageops::FilterType::Triangle)
                    } else {
                        image
                    })
                })();
                let mut response = Response { key, image };
                while !cancel.load(Ordering::Relaxed) {
                    match tx.try_send(response) {
                        Ok(()) => break,
                        Err(TrySendError::Full(pending)) => {
                            response = pending;
                            std::thread::sleep(std::time::Duration::from_millis(1));
                        }
                        Err(TrySendError::Disconnected(_)) => break,
                    }
                }
                active.fetch_sub(1, Ordering::Relaxed);
            });
        }
        match self.cache.get_mut(&key) {
            Some(Entry::Ready(protocol)) => frame.render_stateful_widget(
                StatefulImage::default().resize(Resize::Fit(None)),
                area,
                &mut **protocol,
            ),
            Some(Entry::Sliced(protocol)) => frame.render_widget(
                SlicedImage::new(
                    protocol,
                    SignedPosition::from((0, -(clip.expect("sliced cover has viewport").1 as i16))),
                ),
                area,
            ),
            Some(Entry::Error(error)) => diagnostic(frame, area, &format!("Cover: {error}"), app),
            _ => diagnostic(frame, area, "Loading cover…", app),
        }
    }
}
impl super::state::BasesState {
    pub(crate) fn poll_media(&mut self, app: &App) -> bool {
        app.image_picker
            .as_ref()
            .is_some_and(|picker| self.media.poll(picker))
    }
}
fn diagnostic(frame: &mut Frame, area: Rect, text: &str, app: &App) {
    frame.render_widget(
        Paragraph::new(crate::fsutil::sanitize_for_terminal(text))
            .style(Style::default().fg(app.app_theme.muted))
            .wrap(Wrap { trim: false }),
        area,
    );
}
fn hex_color(text: &str) -> Option<Color> {
    let digits = text.strip_prefix('#')?;
    let rgb = match digits.len() {
        3 => {
            let n = u16::from_str_radix(digits, 16).ok()?;
            (
                ((n >> 8) & 15) as u8 * 17,
                ((n >> 4) & 15) as u8 * 17,
                (n & 15) as u8 * 17,
            )
        }
        6 => {
            let n = u32::from_str_radix(digits, 16).ok()?;
            ((n >> 16) as u8, (n >> 8) as u8, n as u8)
        }
        _ => return None,
    };
    Some(Color::Rgb(rgb.0, rgb.1, rgb.2))
}
pub(super) enum CoverSource {
    Public(String),
    Remote(String),
}
pub(super) fn cover_source(
    value: &Value,
    host: &str,
    metadata: &VaultMetadata,
) -> Result<CoverSource> {
    let host = if let Value::Link(link) = value {
        &link.source
    } else {
        host
    };
    let target = match value {
        Value::Image(source) => source.display(),
        Value::Link(link) => link.target.display(),
        Value::File(id) => id.clone(),
        Value::List(values) => {
            return values.first().map_or_else(
                || bail!("empty image list"),
                |value| cover_source(value, host, metadata),
            );
        }
        _ => value.to_text().display(),
    };
    if target.to_ascii_lowercase().starts_with("http://")
        || target.to_ascii_lowercase().starts_with("https://")
    {
        return Ok(CoverSource::Remote(
            crate::image_render::remote::validate_url(&target)?.to_string(),
        ));
    }
    let target = target
        .strip_prefix("![[")
        .or_else(|| target.strip_prefix("[["))
        .and_then(|target| target.strip_suffix("]]"))
        .unwrap_or(&target);
    let id = metadata
        .resolve_link(host, target)
        .ok_or_else(|| anyhow::anyhow!("image unresolved in public dataset"))?
        .id
        .clone();
    if !metadata.file(&id).is_some_and(|file| {
        matches!(
            file.extension.as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "webp"
        )
    }) {
        bail!("native preview unavailable for this attachment type");
    }
    Ok(CoverSource::Public(id))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refreshed_cover_generations_never_install_queued_stale_images() {
        let picker = Picker::halfblocks();
        let mut images = PublicImages::default();
        let old = Key {
            id: "public.png".into(),
            remote: false,
            generation: 0,
            size: 1,
            modified: None,
            shape: None,
            slice: None,
        };
        let cancelled = Arc::new(AtomicBool::new(false));
        images
            .cache
            .put(old.clone(), Entry::Pending(cancelled.clone()));
        assert!(
            images
                .tx
                .try_send(Response {
                    key: old.clone(),
                    image: Ok(image::DynamicImage::new_rgb8(1, 1))
                })
                .is_ok()
        );
        images.invalidate();
        assert!(cancelled.load(Ordering::Relaxed));
        let current = Key {
            generation: 1,
            ..old
        };
        images.cache.put(
            current.clone(),
            Entry::Pending(Arc::new(AtomicBool::new(false))),
        );
        images.poll(&picker);
        assert!(matches!(
            images.cache.get(&current),
            Some(Entry::Pending(_))
        ));
    }
    #[test]
    fn color_covers_parse_without_io_and_private_targets_stay_unresolved() {
        assert_eq!(hex_color("#a2f"), Some(Color::Rgb(170, 34, 255)));
        assert_eq!(hex_color("#1234ef"), Some(Color::Rgb(18, 52, 239)));
        assert_eq!(hex_color("#bad-input"), None);
        let (_dir, storage) = crate::bases::tests::vault();
        let metadata = storage
            .load_vault_metadata(&AtomicBool::new(false))
            .unwrap();
        for target in [
            "secret.clin",
            ".clin/secret.png",
            "http://127.0.0.1/image.png",
        ] {
            assert!(cover_source(&Value::String(target.into()), "public.md", &metadata).is_err());
        }
    }
}
