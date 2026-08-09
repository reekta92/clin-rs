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
    if which::which("wl-paste").is_err() {
        anyhow::bail!("wl-paste is not installed. Please install wl-clipboard.");
    }

    let mut child = Command::new("wl-paste")
        .arg("--type")
        .arg("image/png")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn wl-paste")?;

    let mut stdout = child.stdout.take().context("Failed to capture stdout")?;
    let mut image_data = Vec::new();
    stdout
        .read_to_end(&mut image_data)
        .context("Failed to read image data")?;

    let status = child.wait().context("Failed to wait on wl-paste")?;

    if !status.success() {
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
    fn category(&self) -> super::ActionCategory {
        super::ActionCategory::Append
    }

    fn glyph(&self) -> (&'static str, &'static str) {
        ("\u{f03e}", "\u{1f5b9}")
    }

    fn execute(&self, app: &mut App, context_note_id: Option<&str>) -> Result<()> {
        let note_id = context_note_id.context("No note selected for OCR Paste")?;

        let dynamic_image = if is_wayland() {
            get_clipboard_image_wayland().or_else(|e| {
                app.messages.push(
                    format!("Wayland clipboard failed: {e}. Falling back to arboard."),
                    crate::app::messages::MessageSeverity::Warning,
                );
                get_clipboard_image_arboard()
            })?
        } else {
            get_clipboard_image_arboard()?
        };

        let temp_file = NamedTempFile::new().context("Failed to create temporary image file")?;
        let temp_path = temp_file.path().to_owned();

        dynamic_image
            .save_with_format(&temp_path, image::ImageFormat::Png)
            .context("Failed to save clipboard image to temp file")?;

        let output = Command::new("tesseract")
            .arg(temp_path)
            .arg("-")
            .arg("-l")
            .arg("eng")
            .output()
            .context("Failed to execute tesseract. Make sure it is installed and in your PATH.")?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Tesseract failed: {err}");
        }

        let extracted_text =
            crate::sanitize::sanitize_for_terminal(String::from_utf8_lossy(&output.stdout).trim())
                .into_owned();

        if extracted_text.is_empty() {
            anyhow::bail!("OCR extracted no text.");
        }

        let mut note = app.storage.load_note(note_id)?;
        note.content.push_str("\n\n---\n**OCR Extract:**\n");
        note.content.push_str(&extracted_text);
        note.updated_at = crate::ui::now_unix_secs();

        app.storage.save_note(note_id, &note)?;
        app.refresh_note_single(None, note_id);
        app.set_temporary_status("OCR text appended successfully");

        Ok(())
    }
}

/// Paste an image from the clipboard into the active view.
/// Saves the image as a PNG attachment and inserts the appropriate reference.
pub struct PasteImageAction;

impl Action for PasteImageAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("paste_image")
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Paste Image")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Paste an image from the clipboard into the active view as an attachment")
    }

    fn category(&self) -> super::ActionCategory {
        super::ActionCategory::General
    }

    fn glyph(&self) -> (&'static str, &'static str) {
        ("\u{f03e}", "\u{1f5bc}")
    }

    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        // Get image from clipboard
        let dynamic_image = if is_wayland() {
            get_clipboard_image_wayland().or_else(|e| {
                app.messages.push(
                    format!("Wayland clipboard failed: {e}. Falling back to arboard."),
                    crate::app::messages::MessageSeverity::Warning,
                );
                get_clipboard_image_arboard()
            })?
        } else {
            get_clipboard_image_arboard()?
        };

        // Save to temp file then import
        let temp_file = NamedTempFile::new().context("Failed to create temporary file")?;
        let temp_path = temp_file.path().to_owned();
        dynamic_image
            .save_with_format(&temp_path, image::ImageFormat::Png)
            .context("Failed to save clipboard image to temp PNG")?;

        let rel_path = app
            .storage
            .import_attachment(&temp_path, &app.config.image.attachments_subdir)?;
        insert_image_reference(app, &rel_path);
        Ok(())
    }
}

/// Insert an image from a file picker dialog into the active view.
pub struct InsertImageFromFileAction;

impl Action for InsertImageFromFileAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("insert_image_from_file")
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Insert Image from File")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Pick an image file and insert it into the active view as an attachment")
    }

    fn category(&self) -> super::ActionCategory {
        super::ActionCategory::General
    }

    fn glyph(&self) -> (&'static str, &'static str) {
        ("\u{f15c}", "\u{1f4c1}")
    }

    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        let picked = crate::ui::pick_file("Image", "png;jpg;jpeg;gif;webp;bmp")?;
        if let Some(path_str) = picked {
            let src = std::path::Path::new(&path_str);
            let rel_path = app
                .storage
                .import_attachment(src, &app.config.image.attachments_subdir)?;
            insert_image_reference(app, &rel_path);
        }
        Ok(())
    }
}

/// Shared helper: insert an attachment reference into the currently active view.
fn insert_image_reference(app: &mut App, rel_path: &str) {
    match app.mode {
        crate::app::ViewMode::Edit => {
            // Insert `![](path)` at cursor in the editor body
            let ref_text = format!("![]({rel_path})");
            app.editor.body.insert_str(&ref_text);
            app.request_editor_preview_update();
        }
        crate::app::ViewMode::Canvas => {
            if let Some(state) = &mut app.canvas_state {
                let (cx, cy) = (state.viewport_x, state.viewport_y);
                let id = format!(
                    "img_{}",
                    uuid::Uuid::new_v4()
                        .to_string()
                        .split('-')
                        .next()
                        .unwrap_or("0")
                );
                state
                    .data
                    .nodes
                    .push(crate::pinstar::data::CanvasNode::File(
                        crate::pinstar::data::FileNode {
                            id,
                            x: cx,
                            y: cy,
                            width: 200.0,
                            height: 150.0,
                            file: rel_path.to_string(),
                            subpath: None,
                            title: None,
                            color: None,
                        },
                    ));
                let _ = state.save();
            }
        }
        crate::app::ViewMode::Draw => {
            app.set_temporary_status("Image pasting not supported in this view");
        }
        _ => {
            app.set_temporary_status("Image pasting not supported in this view");
        }
    }
}
