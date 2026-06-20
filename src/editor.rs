use crate::markdown::MarkdownRenderer;
use ratatui_textarea::TextArea;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditFocus {
    Title,
    Body,
}

#[derive(Default)]
pub struct NoteEditor {
    pub editing_id: Option<String>,
    pub template_edit_path: Option<PathBuf>,
    pub title_editor: TextArea<'static>,
    pub editor: TextArea<'static>,
    pub external_editor_enabled: bool,
    pub external_editor: Option<String>,
    pub editor_preview_enabled: bool,
    pub md_preview_renderer: Option<MarkdownRenderer>,
    pub show_line_numbers: bool,
    pub pending_editor_preview_update: bool,
    pub last_editor_change: Option<Instant>,
    pub last_preview_pane_width: u16,
    pub preview_content_width: Option<u16>,
}

impl NoteEditor {
    pub fn new() -> Self {
        Self::default()
    }
}
