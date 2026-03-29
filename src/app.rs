use crate::constants::*;
use crate::events::get_title_text;
use crate::events::make_title_editor;
use crate::storage::AppSettings;
use crate::ui::help_page_text;
use crate::ui::text_area_from_content;
use crate::ui::{now_unix_secs, open_in_file_manager};
use ratatui::style::{Color, Style};
use ratatui::text::Text;
use ratatui::widgets::ListState;
use std::borrow::Cow;
use std::time::Duration;
use std::time::Instant;

use crate::keybinds::Keybinds;
use crate::storage::{Note, NoteSummary, Storage};
use crate::templates::{Template, TemplateSummary};
use anyhow::Result;
use std::path::PathBuf;
use tui_textarea::TextArea;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    List,
    Edit,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListFocus {
    Notes,
    EncryptionToggle,
}

pub struct ContextMenu {
    pub x: u16,
    pub y: u16,
    pub selected: usize,
}

/// Template selection popup state
pub struct TemplatePopup {
    pub templates: Vec<TemplateSummary>,
    pub selected: usize,
}

pub struct App {
    pub storage: Storage,
    pub keybinds: Keybinds,
    pub notes: Vec<NoteSummary>,
    pub selected: usize,
    pub list_focus: ListFocus,
    pub mode: ViewMode,
    pub editing_id: Option<String>,
    pub title_editor: TextArea<'static>,
    pub editor: TextArea<'static>,
    pub encryption_enabled: bool,
    pub status: Cow<'static, str>,
    pub status_until: Option<Instant>,
    pub pending_delete_note_id: Option<String>,
    pub help_scroll: u16,
    pub context_menu: Option<ContextMenu>,
    pub template_popup: Option<TemplatePopup>,
    /// Cached help page text (rebuilt when keybinds change)
    pub help_text_cache: Option<Text<'static>>,
    pub list_state: ListState,
}

pub enum CliCommand {
    Run {
        edit_title: Option<String>,
    },
    NewAndOpen {
        title: Option<String>,
        template: Option<String>,
    },
    QuickNote {
        content: String,
        title: Option<String>,
    },
    ListNoteTitles,
    Help,
    // Storage path commands
    ShowStoragePath,
    SetStoragePath {
        path: PathBuf,
    },
    ResetStoragePath,
    // Keybind commands
    ShowKeybinds,
    ExportKeybinds,
    ResetKeybinds,
    // Template commands
    ListTemplates,
    CreateExampleTemplates,
}

impl App {
    pub fn new(storage: Storage) -> Result<Self> {
        let settings = storage.load_settings();
        let keybinds = storage.load_keybinds();

        let mut app = Self {
            storage,
            keybinds,
            notes: Vec::new(),
            selected: 0,
            list_focus: ListFocus::Notes,
            mode: ViewMode::List,
            editing_id: None,
            title_editor: make_title_editor(""),
            editor: TextArea::default(),
            encryption_enabled: settings.encryption_enabled,
            status: Cow::Borrowed(LIST_HELP_HINTS),
            status_until: None,
            pending_delete_note_id: None,
            help_scroll: 0,
            context_menu: None,
            template_popup: None,
            help_text_cache: None,
            list_state: ListState::default(),
        };
        app.context_menu = None;
        app.template_popup = None;
        app.refresh_notes()?;
        Ok(app)
    }

    pub fn refresh_notes(&mut self) -> Result<()> {
        let mut summaries = Vec::new();
        for id in self.storage.list_note_ids()? {
            if let Ok(summary) = self.storage.load_note_summary(&id) {
                summaries.push(summary);
            }
        }
        summaries.sort_by(|a, b| {
            let a_clin = a.id.ends_with(".clin");
            let b_clin = b.id.ends_with(".clin");
            b_clin.cmp(&a_clin).then(b.updated_at.cmp(&a.updated_at))
        });
        self.notes = summaries;

        if self.selected > self.notes.len() {
            self.selected = self.notes.len();
        }
        Ok(())
    }

    pub fn open_selected(&mut self) {
        if self.selected == self.notes.len() {
            self.start_new_note();
            return;
        }

        if let Some(summary) = self.notes.get(self.selected) {
            let is_clin = summary.id.ends_with(".clin");
            if !self.encryption_enabled && is_clin {
                self.status =
                    Cow::Borrowed("Cannot open encrypted notes while encryption is disabled.");
                return;
            }

            match self.storage.load_note(&summary.id) {
                Ok(note) => {
                    self.mode = ViewMode::Edit;
                    if self.encryption_enabled && !is_clin {
                        let title_stem = self.storage.note_file_stem_from_title(&note.title);
                        let target_id = self.storage.unique_note_id(&title_stem, "clin", "");
                        self.editing_id = Some(target_id);
                    } else {
                        self.editing_id = Some(summary.id.clone());
                    }

                    self.title_editor = make_title_editor(&note.title);
                    self.editor = text_area_from_content(&note.content);
                    self.editor
                        .set_cursor_style(Style::default().fg(Color::Black).bg(Color::Cyan));
                    self.editor
                        .set_cursor_line_style(Style::default().bg(Color::Rgb(32, 36, 44)));
                    self.title_editor
                        .set_cursor_style(Style::default().fg(Color::Black).bg(Color::Cyan));
                    self.editor
                        .set_cursor_style(Style::default().fg(Color::Black).bg(Color::Cyan));
                    self.set_default_status();
                }
                Err(err) => {
                    self.status = Cow::Owned(format!("Could not open note: {err:#}"));
                }
            }
        }
    }

    pub fn open_note_by_title(&mut self, title: &str) -> bool {
        let query = title.trim();
        if query.is_empty() {
            return false;
        }

        if let Some(index) = self
            .notes
            .iter()
            .position(|note| note.title.eq_ignore_ascii_case(query))
        {
            self.selected = index;
            self.open_selected();
            return true;
        }

        false
    }

    pub fn start_new_note(&mut self) {
        // Check if default template exists and use it
        let template_manager = self.storage.template_manager();
        if let Some(default_template) = template_manager.load_default() {
            self.start_note_from_template(&default_template);
        } else {
            self.start_blank_note();
        }
    }

    pub fn start_blank_note(&mut self) {
        self.mode = ViewMode::Edit;
        self.editing_id = Some(self.storage.new_note_id());
        self.title_editor = make_title_editor("");
        self.editor = TextArea::default();
        self.editor
            .set_cursor_style(Style::default().fg(Color::Black).bg(Color::Cyan));
        self.editor
            .set_cursor_line_style(Style::default().bg(Color::Rgb(32, 36, 44)));
        self.set_default_status();
    }

    pub fn start_note_from_template(&mut self, template: &Template) {
        let rendered = template.render();

        self.mode = ViewMode::Edit;
        self.editing_id = Some(self.storage.new_note_id());

        let title = rendered.title.as_deref().unwrap_or("");
        self.title_editor = make_title_editor(title);
        self.editor = text_area_from_content(&rendered.content);
        self.editor
            .set_cursor_style(Style::default().fg(Color::Black).bg(Color::Cyan));
        self.editor
            .set_cursor_line_style(Style::default().bg(Color::Rgb(32, 36, 44)));
        self.set_default_status();
    }

    pub fn open_template_popup(&mut self) {
        let template_manager = self.storage.template_manager();
        match template_manager.list() {
            Ok(templates) => {
                if templates.is_empty() {
                    self.set_temporary_status(
                        "No templates found. Create templates in the templates directory.",
                    );
                } else {
                    self.template_popup = Some(TemplatePopup {
                        templates,
                        selected: 0,
                    });
                }
            }
            Err(_) => {
                self.set_temporary_status("Failed to load templates");
            }
        }
    }

    pub fn close_template_popup(&mut self) {
        self.template_popup = None;
    }

    pub fn select_template(&mut self) {
        if let Some(popup) = self.template_popup.take()
            && let Some(summary) = popup.templates.get(popup.selected)
        {
            let template_manager = self.storage.template_manager();
            if let Ok(template) = template_manager.load(&summary.filename) {
                self.start_note_from_template(&template);
                return;
            }
        }
        // Fallback to blank note if something went wrong
        self.start_blank_note();
    }

    pub fn autosave(&mut self) {
        let mut title = get_title_text(&self.title_editor).trim().to_string();
        if title.is_empty() {
            title = String::from("Untitled note");
        }
        let content = self.editor.lines().join("\n");
        let id = match &self.editing_id {
            Some(id) => id.clone(),
            None => return,
        };
        let note = Note {
            title,
            content,
            updated_at: now_unix_secs(),
        };
        if let Ok(saved_id) = self.storage.save_note(&id, &note, self.encryption_enabled) {
            self.editing_id = Some(saved_id);
        }
    }

    pub fn back_to_list(&mut self) {
        self.mode = ViewMode::List;
        self.editing_id = None;
        self.list_focus = ListFocus::Notes;
        self.title_editor = make_title_editor("");
        self.editor = TextArea::default();
        self.pending_delete_note_id = None;
        let _ = self.refresh_notes();
        self.set_default_status();
    }

    pub fn handle_menu_action(&mut self, action: usize, focus: &mut EditFocus) {
        match action {
            0 => match focus {
                EditFocus::Title => {
                    self.title_editor.copy();
                }
                EditFocus::Body => {
                    self.editor.copy();
                }
                _ => {}
            },
            1 => match focus {
                EditFocus::Title => {
                    self.title_editor.cut();
                }
                EditFocus::Body => {
                    self.editor.cut();
                }
                _ => {}
            },
            2 => match focus {
                EditFocus::Title => {
                    self.title_editor.paste();
                }
                EditFocus::Body => {
                    self.editor.paste();
                }
                _ => {}
            },
            3 => match focus {
                EditFocus::Title => {
                    self.title_editor.select_all();
                }
                EditFocus::Body => {
                    self.editor.select_all();
                }
                _ => {}
            },
            _ => {}
        }
    }

    pub fn begin_delete_selected(&mut self) {
        if self.selected >= self.notes.len() {
            self.set_temporary_status("No note selected to delete");
            return;
        }

        let Some(note) = self.notes.get(self.selected) else {
            self.set_temporary_status("No note selected to delete");
            return;
        };

        self.pending_delete_note_id = Some(note.id.clone());
        self.status_until = None;
        self.status = Cow::Owned(format!("Delete \"{}\"? y confirm, n cancel", note.title));
    }

    pub fn cancel_delete_prompt(&mut self) {
        self.pending_delete_note_id = None;
        self.set_default_status();
    }

    pub fn confirm_delete_selected(&mut self) {
        let id = match self.pending_delete_note_id.take() {
            Some(id) => id,
            None => return,
        };

        match self.storage.delete_note(&id) {
            Ok(()) => {
                let _ = self.refresh_notes();
                if self.selected > self.notes.len() {
                    self.selected = self.notes.len();
                }
                self.set_temporary_status("Note deleted");
            }
            Err(err) => {
                self.pending_delete_note_id = None;
                self.set_temporary_status(&format!("Delete failed: {err:#}"));
            }
        }
    }

    pub fn open_selected_note_location(&mut self) {
        if self.selected >= self.notes.len() {
            self.set_temporary_status("No note selected for location");
            return;
        }

        let Some(note) = self.notes.get(self.selected) else {
            self.set_temporary_status("No note selected for location");
            return;
        };

        let note_path = self.storage.note_path(&note.id);
        let Some(parent) = note_path.parent() else {
            self.set_temporary_status("Could not determine note directory");
            return;
        };

        match open_in_file_manager(parent) {
            Ok(()) => self.set_temporary_status("Opened note file location"),
            Err(err) => self.set_temporary_status(&format!("Open location failed: {err:#}")),
        }
    }

    pub fn toggle_encryption_mode(&mut self) {
        self.encryption_enabled = !self.encryption_enabled;
        self.set_default_status();
        self.storage.save_settings(&AppSettings {
            encryption_enabled: self.encryption_enabled,
        });
    }

    pub fn open_help_page(&mut self) {
        self.mode = ViewMode::Help;
        self.help_scroll = 0;
        self.status = Cow::Borrowed(HELP_PAGE_HINTS);
        self.status_until = None;
    }

    pub fn close_help_page(&mut self) {
        self.mode = ViewMode::List;
        self.help_scroll = 0;
        self.set_default_status();
    }

    pub fn default_status_text(&self) -> &'static str {
        match self.mode {
            ViewMode::List => LIST_HELP_HINTS,
            ViewMode::Edit => EDIT_HELP_HINTS,
            ViewMode::Help => HELP_PAGE_HINTS,
        }
    }

    pub fn set_default_status(&mut self) {
        self.status = Cow::Borrowed(self.default_status_text());
        self.status_until = None;
    }

    pub fn set_temporary_status(&mut self, message: &str) {
        self.status = Cow::Owned(message.to_string());
        self.status_until = Some(Instant::now() + Duration::from_secs(2));
    }

    pub fn tick_status(&mut self) {
        if let Some(until) = self.status_until
            && Instant::now() >= until
        {
            self.set_default_status();
        }
    }

    /// Get cached help text, building it if necessary
    pub fn get_help_text(&mut self) -> Text<'static> {
        if let Some(ref text) = self.help_text_cache {
            return text.clone();
        }
        let text = help_page_text(&self.keybinds);
        self.help_text_cache = Some(text.clone());
        text
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditFocus {
    Title,
    Body,
    EncryptionToggle,
}
