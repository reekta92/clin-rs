use anyhow::Result;
use image::DynamicImage;
use ratatui_image::{Resize, ResizeEncodeRender, protocol::StatefulProtocol};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender},
};

pub enum ImageWork {
    Decode(u32),
    Encode {
        protocol: Box<StatefulProtocol>,
        resize: Resize,
        size: ratatui::layout::Size,
    },
}

pub struct ImageJob {
    pub key: PathBuf,
    pub request: u64,
    pub cancel: Arc<AtomicBool>,
    pub work: ImageWork,
}

pub enum ImageOutput {
    Decoded(DynamicImage),
    Encoded(Box<StatefulProtocol>),
}

pub struct DecodedImage {
    pub key: PathBuf,
    pub request: u64,
    pub result: Result<ImageOutput>,
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
pub fn spawn() -> (SyncSender<ImageJob>, Receiver<DecodedImage>) {
    let (tx, rx) = mpsc::sync_channel(16);
    let (result_tx, result_rx) = mpsc::sync_channel(2);

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
pub(crate) fn process_job(job: ImageJob, result_tx: &SyncSender<DecodedImage>) {
    if job.cancel.load(Ordering::Relaxed) {
        return;
    }
    let result = match job.work {
        ImageWork::Decode(max_dim) => decode_image(&job.key, max_dim).map(ImageOutput::Decoded),
        ImageWork::Encode {
            mut protocol,
            resize,
            size,
        } => {
            protocol.resize_encode(&resize, size);
            protocol
                .last_encoding_result()
                .unwrap_or(Ok(()))
                .map(|()| ImageOutput::Encoded(protocol))
                .map_err(Into::into)
        }
    };
    let mut response = DecodedImage {
        key: job.key,
        request: job.request,
        result,
    };
    loop {
        if job.cancel.load(Ordering::Relaxed) {
            return;
        }
        match result_tx.try_send(response) {
            Ok(()) | Err(mpsc::TrySendError::Disconnected(_)) => return,
            Err(mpsc::TrySendError::Full(pending)) => {
                response = pending;
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }
    }
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
        let bytes: &[u8] = match (width, height) {
            (1024, 512) => include_bytes!("../../tests/fixtures/image-limits/1024x512.png"),
            (8193, 1) => include_bytes!("../../tests/fixtures/image-limits/8193x1.png"),
            (4096, 2048) => include_bytes!("../../tests/fixtures/image-limits/4096x2048.png"),
            _ => panic!("missing image fixture"),
        };
        bytes.to_vec()
    }
    #[test]
    fn existing_image_decoders_and_png_output_stay_enabled() {
        use image::ImageFormat;
        for format in [
            ImageFormat::Png,
            ImageFormat::Jpeg,
            ImageFormat::Gif,
            ImageFormat::WebP,
            ImageFormat::Tiff,
            ImageFormat::Tga,
            ImageFormat::Bmp,
            ImageFormat::Ico,
            ImageFormat::Hdr,
            ImageFormat::OpenExr,
            ImageFormat::Pnm,
            ImageFormat::Farbfeld,
            ImageFormat::Qoi,
        ] {
            assert!(format.reading_enabled(), "{format:?} decoder disabled");
        }
        assert!(ImageFormat::Png.writing_enabled());
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
