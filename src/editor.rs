use crate::markdown::MarkdownRenderer;
use ratatui_textarea::TextArea;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditFocus {
    Title,
    Body,
}

pub struct NoteEditor {
    pub editing_id: Option<String>,
    pub initial_word_count: usize,
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
    pub last_preview_pane_height: u16,
    pub preview_content_width: Option<u16>,
}

impl Default for NoteEditor {
    fn default() -> Self {
        Self {
            editing_id: None,
            initial_word_count: 0,
            template_edit_path: None,
            title_editor: TextArea::default(),
            editor: TextArea::default(),
            external_editor_enabled: false,
            external_editor: None,
            editor_preview_enabled: false,
            md_preview_renderer: None,
            show_line_numbers: false,
            pending_editor_preview_update: false,
            last_editor_change: None,
            last_preview_pane_width: 0,
            last_preview_pane_height: 36,
            preview_content_width: None,
        }
    }
}

impl NoteEditor {
    pub fn new() -> Self {
        Self::default()
    }
}
