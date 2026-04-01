use super::Action;
use crate::app::App;
use anyhow::{Context, Result};
use arboard::Clipboard;
use image::{DynamicImage, RgbaImage};
use std::borrow::Cow;
use std::io::Read;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

pub struct OcrPasteAction;

fn is_wayland() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY_NAME").is_ok()
}

fn get_clipboard_image_wayland() -> Result<DynamicImage> {
    // Check if wl-paste is available
    if which::which("wl-paste").is_err() {
        anyhow::bail!("wl-paste is not installed. Please install wl-clipboard.");
    }

    // Run wl-paste to get the raw image data (PNG usually)
    let mut child = Command::new("wl-paste")
        .arg("--type")
        .arg("image/png")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn wl-paste")?;

    let mut stdout = child.stdout.take().context("Failed to capture stdout")?;
    let mut image_data = Vec::new();
    stdout.read_to_end(&mut image_data).context("Failed to read image data")?;

    let status = child.wait().context("Failed to wait on wl-paste")?;

    if !status.success() {
        // Attempt to read stderr for better error message
        if let Some(mut stderr) = child.stderr {
            let mut err_msg = String::new();
            let _ = stderr.read_to_string(&mut err_msg);
            if !err_msg.is_empty() {
                anyhow::bail!("wl-paste failed: {}", err_msg.trim());
            }
        }
        anyhow::bail!("Clipboard does not contain an image or wl-paste failed.");
    }

    if image_data.is_empty() {
         anyhow::bail!("wl-paste returned empty data.");
    }

    // Decode the image
    let img = image::load_from_memory(&image_data)
        .context("Failed to decode clipboard image data (expected PNG)")?;

    Ok(img)
}

fn get_clipboard_image_arboard() -> Result<DynamicImage> {
    let mut clipboard = Clipboard::new().context("Failed to initialize clipboard")?;
    let image_data = clipboard
        .get_image()
        .context("No image found in clipboard")?;

    let img = RgbaImage::from_raw(
        image_data.width as u32,
        image_data.height as u32,
        image_data.bytes.into_owned(),
    )
    .context("Failed to construct image from clipboard data")?;

    Ok(DynamicImage::ImageRgba8(img))
}

impl Action for OcrPasteAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("ocr.paste")
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("OCR Paste")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Extract text from image in clipboard using Tesseract and append to note")
    }

    fn execute(&self, app: &mut App, context_note_id: Option<&str>) -> Result<()> {
        let note_id = context_note_id.context("No note selected for OCR Paste")?;

        let dynamic_image = if is_wayland() {
            get_clipboard_image_wayland()
                .or_else(|e| {
                    // Fallback to arboard if Wayland method fails for some reason
                    // e.g. user is on Wayland but has Xwayland primary clipboard synced
                    eprintln!("Wayland clipboard failed: {}. Falling back to arboard.", e);
                    get_clipboard_image_arboard()
                })?
        } else {
            get_clipboard_image_arboard()?
        };

        let temp_file = NamedTempFile::new().context("Failed to create temporary image file")?;
        let temp_path = temp_file.path().to_owned();

        // Save image to temp file in PNG format
        dynamic_image
            .save_with_format(&temp_path, image::ImageFormat::Png)
            .context("Failed to save clipboard image to temp file")?;

        // Run tesseract
        let output = Command::new("tesseract")
            .arg(temp_path)
            .arg("-") // stdout
            .arg("-l")
            .arg("eng") // default to english, could be configurable later
            .output()
            .context("Failed to execute tesseract. Make sure it is installed and in your PATH.")?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Tesseract failed: {}", err);
        }

        let extracted_text = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if extracted_text.is_empty() {
            anyhow::bail!("OCR extracted no text.");
        }

        // Append to note
        let mut note = app.storage.load_note(note_id)?;
        note.content.push_str("\n\n---\n**OCR Extract:**\n");
        note.content.push_str(&extracted_text);
        note.updated_at = crate::ui::now_unix_secs();

        let is_clin = note_id.ends_with(".clin");
        app.storage.save_note(note_id, &note, is_clin)?;
        app.refresh_notes()?;
        app.set_temporary_status("OCR text appended successfully");

        Ok(())
    }
}
