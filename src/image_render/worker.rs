use std::sync::mpsc::{self, Receiver, Sender};

use anyhow::Result;
use image::DynamicImage;

use crate::image_render::ImageKey;

pub struct ImageJob {
    pub key: ImageKey,
    pub max_dim: u32,
}

pub struct DecodedImage {
    pub key: ImageKey,
    pub image: DynamicImage,
}

/// Spawn the background image decode worker.
/// Returns (job sender, result receiver).
///
/// The worker loop:
/// 1. Block on `recv` for the next job.
/// 2. Open + decode + downscale the image.
/// 3. Send the result back (or an error).
/// 4. Drain any additional immediately-available jobs for throughput.
/// 5. Loop.
pub fn spawn() -> (Sender<ImageJob>, Receiver<Result<DecodedImage>>) {
    let (tx, rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();

    std::thread::Builder::new()
        .name("clin-image-decode-worker".into())
        .spawn(move || {
            loop {
                let first = match rx.recv() {
                    Ok(job) => job,
                    Err(_) => {
                        // All senders dropped — shut down.
                        return;
                    }
                };

                process_job(first, &result_tx);

                // Drain any additional jobs that arrived while we were processing.
                while let Ok(job) = rx.try_recv() {
                    process_job(job, &result_tx);
                }
            }
        })
        .expect("failed to spawn image decode worker");

    (tx, result_rx)
}
fn process_job(job: ImageJob, result_tx: &Sender<Result<DecodedImage>>) {
    let result = decode_image(&job.key, job.max_dim);
    let _ = result_tx.send(result.map(|img| DecodedImage {
        key: job.key,
        image: img,
    }));
}

fn decode_image(key: &ImageKey, max_dim: u32) -> Result<DynamicImage> {
    let img = image::ImageReader::open(&key.path)?
        .decode()
        .map_err(|e| anyhow::anyhow!("Failed to decode image {}: {e}", key.path.display()))?;

    if max_dim > 0 {
        let w = img.width();
        let h = img.height();
        let max_current = w.max(h);
        if max_current > max_dim {
            let ratio = max_dim as f64 / max_current as f64;
            let new_w = (w as f64 * ratio) as u32;
            let new_h = (h as f64 * ratio) as u32;
            return Ok(img.resize_exact(
                new_w.max(1),
                new_h.max(1),
                image::imageops::FilterType::Lanczos3,
            ));
        }
    }

    Ok(img)
}
