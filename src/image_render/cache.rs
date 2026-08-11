use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::time::Instant;

use image::DynamicImage;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;

use crate::image_render::ImageKey;
use crate::image_render::worker::{DecodedImage, ImageJob};

/// LRU-evicting cache of decoded images and their protocol renderers for one view.
pub struct ImageCache {
    map: HashMap<ImageKey, ImageEntry>,
    limit: usize,
}

struct ImageEntry {
    /// Set immediately on first request; `Some` once decode worker returns.
    decoded: Option<DynamicImage>,
    /// Created lazily from decoded + picker; `None` while decode pending.
    proto: Option<StatefulProtocol>,
    last_used: Instant,
}

impl ImageCache {
    pub fn new(limit: usize) -> Self {
        Self {
            map: HashMap::new(),
            limit: limit.max(1),
        }
    }

    /// Request an image for display. If the key is absent (or mtime changed),
    /// sends a decode job to the worker. Call `install_decoded` when the
    /// result arrives.
    pub fn request(
        &mut self,
        key: ImageKey,
        max_dim: u32,
        tx: &Sender<ImageJob>,
        _picker: &Picker,
    ) {
        let entry = self.map.entry(key);
        use std::collections::hash_map::Entry;
        if let Entry::Vacant(e) = entry {
            let _ = tx.send(ImageJob::Decode {
                key: e.key().clone(),
                max_dim,
            });
            e.insert(ImageEntry {
                decoded: None,
                proto: None,
                last_used: Instant::now(),
            });
        } else {
            // Touch existing entry
            if let Entry::Occupied(mut e) = entry {
                e.get_mut().last_used = Instant::now();
            }
        }
    }

    /// Install a completed decode result and build the protocol renderer.
    pub fn install_decoded(&mut self, img: DecodedImage, picker: &Picker) {
        if let Some(entry) = self.map.get_mut(&img.key) {
            let proto = picker.new_resize_protocol(img.image.clone());
            entry.decoded = Some(img.image);
            entry.proto = Some(proto);
            entry.last_used = Instant::now();
        }
    }

    /// Get a mutable reference to the protocol for rendering, if ready.
    pub fn get_proto(&mut self, key: &ImageKey) -> Option<&mut StatefulProtocol> {
        if let Some(entry) = self.map.get_mut(key) {
            entry.last_used = Instant::now();
            entry.proto.as_mut()
        } else {
            None
        }
    }

    /// Evict least-recently-used entries when over capacity.
    pub fn evict_stale(&mut self) {
        while self.map.len() > self.limit {
            let oldest_key = self
                .map
                .iter()
                .min_by_key(|(_, e)| e.last_used)
                .map(|(k, _)| k.clone());

            if let Some(k) = oldest_key {
                self.map.remove(&k);
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eviction() {
        let mut cache = ImageCache::new(2);

        let dir = std::env::temp_dir().join("clin_test_img_cache");
        let _ = std::fs::create_dir_all(&dir);
        let path_a = dir.join("a.png");
        let path_b = dir.join("b.png");
        let path_c = dir.join("c.png");

        let k1 = ImageKey {
            path: path_a,
            mtime: 1,
        };
        let k2 = ImageKey {
            path: path_b,
            mtime: 2,
        };
        let k3 = ImageKey {
            path: path_c,
            mtime: 3,
        };

        // Insert two
        let (tx, _) = std::sync::mpsc::channel();
        let picker = Picker::halfblocks();
        cache.request(k1.clone(), 100, &tx, &picker);
        cache.request(k2.clone(), 100, &tx, &picker);
        assert_eq!(cache.map.len(), 2);

        // Touch k1 so it's more recent
        cache.get_proto(&k1);

        // Insert third — should evict k2 (oldest)
        cache.request(k3.clone(), 100, &tx, &picker);
        cache.evict_stale();
        assert_eq!(cache.map.len(), 2);
        assert!(cache.map.contains_key(&k1), "k1 should survive");
        assert!(
            !cache.map.contains_key(&k2),
            "k2 should be evicted as oldest"
        );
        assert!(cache.map.contains_key(&k3), "k3 should survive");

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }
}
