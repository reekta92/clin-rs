use anyhow::Result;
use image::DynamicImage;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

pub struct ImageJob {
    pub key: PathBuf,
    pub max_dim: u32,
}

pub struct DecodedImage {
    pub key: PathBuf,
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

fn decode_image(key: &PathBuf, max_dim: u32) -> Result<DynamicImage> {
    let img = image::ImageReader::open(key)?
        .decode()
        .map_err(|e| anyhow::anyhow!("Failed to decode image {}: {e}", key.display()))?;

    Ok(resize_image(img, max_dim))
}

pub(crate) fn decode_public_image(bytes: Vec<u8>, max_dim: u32) -> Result<DynamicImage> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(16 * 1024 * 1024);
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|error| anyhow::anyhow!("public image decode rejected: {error}"))?;
    Ok(resize_image(image, max_dim))
}

fn resize_image(img: DynamicImage, max_dim: u32) -> DynamicImage {
    if max_dim > 0 {
        let w = img.width();
        let h = img.height();
        let max_current = w.max(h);
        if max_current > max_dim {
            let ratio = max_dim as f64 / max_current as f64;
            let new_w = (w as f64 * ratio) as u32;
            let new_h = (h as f64 * ratio) as u32;
            return img.resize_exact(
                new_w.max(1),
                new_h.max(1),
                image::imageops::FilterType::Lanczos3,
            );
        }
    }

    img
}

#[cfg(test)]
mod tests {
    use super::*;
    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut output = std::io::Cursor::new(Vec::new());
        DynamicImage::new_rgba8(width, height)
            .write_to(&mut output, image::ImageFormat::Png)
            .unwrap();
        output.into_inner()
    }
    #[test]
    fn public_image_decode_is_bounded_and_downscales() {
        let decoded = decode_public_image(png(1024, 512), 64).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (64, 32));
        assert!(decode_public_image(png(8193, 1), 512).is_err());
        assert!(decode_public_image(png(4096, 2048), 512).is_err());
        assert!(decode_public_image(b"not an image".to_vec(), 512).is_err());
    }
}
