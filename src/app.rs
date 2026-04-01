use crate::constants::*;
use crate::events::get_title_text;
use crate::events::make_title_editor;
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
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use std::collections::{HashMap, HashSet};
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
    ExternalEditorToggle,
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

pub struct TagPopup {
    pub note_id: String,
    pub input: TextArea<'static>,
}

pub enum FolderPopupMode {
    Create { parent_path: String },
    Rename { old_path: String },
}

pub struct FolderPopup {
    pub mode: FolderPopupMode,
    pub input: TextArea<'static>,
}

pub struct FolderPicker {
    pub note_id: String,
    pub folders: Vec<String>,
    pub selected: usize,
}

#[derive(Debug, Clone)]
pub enum VisualItem {
    Folder {
        path: String,
        name: String,
        depth: usize,
        is_expanded: bool,
        note_count: usize,
    },
    Note {
        id: String,
        summary_idx: usize,
        depth: usize,
        is_clin: bool,
    },
    CreateNew {
        path: String,
        depth: usize,
    },
}

pub struct App {
    pub storage: Storage,
    pub keybinds: Keybinds,
    pub notes: Vec<NoteSummary>,
    pub visual_list: Vec<VisualItem>,
    pub visual_index: usize,
    pub list_focus: ListFocus,
    pub mode: ViewMode,
    pub editing_id: Option<String>,
    pub title_editor: TextArea<'static>,
    pub editor: TextArea<'static>,
    pub encryption_enabled: bool,
    pub external_editor_enabled: bool,
    pub external_editor: Option<String>,
    pub status: Cow<'static, str>,
    pub status_until: Option<Instant>,
    pub pending_delete_note_id: Option<String>,
    pub pending_encrypt_note_id: Option<String>,
    pub help_scroll: u16,
    pub context_menu: Option<ContextMenu>,
    pub template_popup: Option<TemplatePopup>,
    pub tag_popup: Option<TagPopup>,
    pub folder_popup: Option<FolderPopup>,
    pub folder_picker: Option<FolderPicker>,
    pub folder_expanded: HashSet<String>,
    pub filter_tags: Vec<String>,
    pub filter_popup: Option<TextArea<'static>>,
    /// Cached help page text (rebuilt when keybinds change)
    pub help_text_cache: Option<Text<'static>>,
    pub list_state: ListState,
    pub needs_full_redraw: bool,
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
    MigrateStorage,
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
        let bootstrap_config = crate::config::BootstrapConfig::load().unwrap_or_default();

        let mut app = Self {
            storage,
            keybinds,
            notes: Vec::new(),
            visual_list: Vec::new(),
            visual_index: 0,
            list_focus: ListFocus::Notes,
            mode: ViewMode::List,
            editing_id: None,
            title_editor: make_title_editor(""),
            editor: TextArea::default(),
            encryption_enabled: settings.encryption_enabled,
            external_editor_enabled: bootstrap_config.external_editor_enabled,
            external_editor: bootstrap_config.external_editor,
            status: Cow::Borrowed(LIST_HELP_HINTS),
            status_until: None,
            pending_delete_note_id: None,
            pending_encrypt_note_id: None,
            help_scroll: 0,
            context_menu: None,
            template_popup: None,
            tag_popup: None,
            folder_popup: None,
            folder_picker: None,
            folder_expanded: HashSet::new(),
            filter_tags: Vec::new(),
            filter_popup: None,
            help_text_cache: None,
            list_state: ListState::default(),
            needs_full_redraw: false,
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
                // Apply tag filter
                if !self.filter_tags.is_empty() {
                    let mut matches = false;
                    for tag in &self.filter_tags {
                        if summary.tags.contains(tag) {
                            matches = true;
                            break;
                        }
                    }
                    if !matches {
                        continue;
                    }
                }
                summaries.push(summary);
            }
        }
        summaries.sort_by(|a, b| {
            let a_clin = a.id.ends_with(".clin");
            let b_clin = b.id.ends_with(".clin");
            b_clin.cmp(&a_clin).then(b.updated_at.cmp(&a.updated_at))
        });
        self.notes = summaries;

        self.refresh_visual_list();
        Ok(())
    }

    pub fn refresh_visual_list(&mut self) {
        let mut visual = Vec::new();

        // Notes are currently flattened. Let's group them by folder.
        // We'll construct a simple tree.
        let mut by_folder: HashMap<String, Vec<(usize, &NoteSummary)>> = HashMap::new();
        for (i, note) in self.notes.iter().enumerate() {
            by_folder
                .entry(note.folder.clone())
                .or_default()
                .push((i, note));
        }

        // Always show root folder "Vault"
        visual.push(VisualItem::Folder {
            path: "".to_string(),
            name: "Vault".to_string(),
            depth: 0,
            is_expanded: self.folder_expanded.contains(""),
            note_count: by_folder
                .get("")
                .map_or(0, |v: &Vec<(usize, &NoteSummary)>| v.len()),
        });

        if self.folder_expanded.contains("") {
            if let Some(notes) = by_folder.get("") {
                for (idx, note) in notes {
                    visual.push(VisualItem::Note {
                        id: note.id.clone(),
                        summary_idx: *idx,
                        depth: 1,
                        is_clin: note.id.ends_with(".clin"),
                    });
                }
            }
            visual.push(VisualItem::CreateNew {
                path: "".to_string(),
                depth: 1,
            });
        }

        // Get all other folders sorted
        let mut subfolders: Vec<String> = by_folder
            .keys()
            .filter(|k: &&String| !k.is_empty())
            .cloned()
            .collect();
        subfolders.sort();

        // Wait, what if a parent folder has no notes but has subfolders?
        // We should really build a proper tree from `storage.list_folders()`.
        if let Ok(all_folders) = self.storage.list_folders() {
            for folder in all_folders {
                let parts: Vec<&str> = folder.split('/').collect();
                let depth = parts.len();
                let name = parts.last().unwrap_or(&"").to_string();

                // Only show if parent is expanded
                let parent_path = if parts.len() > 1 {
                    parts[..parts.len() - 1].join("/")
                } else {
                    "".to_string()
                };

                // Fast check if parent is expanded
                let mut is_visible = true;
                let mut current_parent = parent_path.clone();
                while !current_parent.is_empty() {
                    if !self.folder_expanded.contains(&current_parent) {
                        is_visible = false;
                        break;
                    }
                    if let Some(slash) = current_parent.rfind('/') {
                        current_parent = current_parent[..slash].to_string();
                    } else {
                        current_parent = "".to_string();
                    }
                }

                // Finally check root
                if !self.folder_expanded.contains("") {
                    is_visible = false;
                }

                if is_visible {
                    let is_expanded = self.folder_expanded.contains(&folder);
                    visual.push(VisualItem::Folder {
                        path: folder.clone(),
                        name,
                        depth,
                        is_expanded,
                        note_count: by_folder
                            .get(&folder)
                            .map_or(0, |v: &Vec<(usize, &NoteSummary)>| v.len()),
                    });

                    if is_expanded {
                        if let Some(notes) = by_folder.get(&folder) {
                            for (idx, note) in notes {
                                visual.push(VisualItem::Note {
                                    id: note.id.clone(),
                                    summary_idx: *idx,
                                    depth: depth + 1,
                                    is_clin: note.id.ends_with(".clin"),
                                });
                            }
                        }
                        visual.push(VisualItem::CreateNew {
                            path: folder.clone(),
                            depth: depth + 1,
                        });
                    }
                }
            }
        }

        self.visual_list = visual;
    }

    /// Get the folder context based on current selection.
    /// If a folder is selected, returns that folder's path.
    /// If a note is selected, returns the folder containing that note.
    /// If a "Create New" item is selected, returns its target folder.
    pub fn get_current_folder_context(&self) -> String {
        match self.visual_list.get(self.visual_index) {
            Some(VisualItem::Folder { path, .. }) => path.clone(),
            Some(VisualItem::Note { summary_idx, .. }) => self
                .notes
                .get(*summary_idx)
                .map(|n| n.folder.clone())
                .unwrap_or_default(),
            Some(VisualItem::CreateNew { path, .. }) => path.clone(),
            None => String::new(),
        }
    }

    pub fn open_selected(&mut self) {
        if self.visual_list.is_empty() {
            return;
        }

        // Clamp index
        if self.visual_index >= self.visual_list.len() {
            self.visual_index = self.visual_list.len().saturating_sub(1);
        }

        match &self.visual_list[self.visual_index] {
            VisualItem::CreateNew { path, .. } => {
                self.start_new_note(path.clone());
            }
            VisualItem::Folder { path, .. } => {
                // Toggle expand/collapse
                let p = path.clone();
                if self.folder_expanded.contains(&p) {
                    self.folder_expanded.remove(&p);
                } else {
                    self.folder_expanded.insert(p);
                }
                self.refresh_visual_list();
            }
            VisualItem::Note { summary_idx, .. } => {
                let note_id = if let Some(summary) = self.notes.get(*summary_idx) {
                    let is_clin = summary.id.ends_with(".clin");
                    if !self.encryption_enabled && is_clin {
                        self.status = Cow::Borrowed(
                            "Cannot open encrypted notes while encryption is disabled.",
                        );
                        return;
                    }

                    if self.encryption_enabled && !is_clin {
                        self.pending_encrypt_note_id = Some(summary.id.clone());
                        self.status_until = None;
                        self.status = Cow::Borrowed(
                            "WARNING: This action will encrypt the file! y confirm, n cancel",
                        );
                        return;
                    }
                    Some(summary.id.clone())
                } else {
                    None
                };

                if let Some(id) = note_id {
                    if self.external_editor_enabled {
                        self.open_note_in_external_editor(&id);
                    } else {
                        self.load_and_open_note(&id);
                    }
                }
            }
        }
    }

    pub fn load_and_open_note(&mut self, note_id: &str) {
        if let Ok(note) = self.storage.load_note(note_id) {
            self.editing_id = Some(note_id.to_string());
            self.title_editor = make_title_editor(&note.title);
            self.editor = text_area_from_content(&note.content);
            self.mode = ViewMode::Edit;
            self.status = Cow::Borrowed(EDIT_HELP_HINTS);
        } else {
            self.status = Cow::Borrowed("Failed to load note!");
        }
    }

    pub fn open_note_in_external_editor(&mut self, note_id: &str) {
        if let Ok(note) = self.storage.load_note(note_id) {
            let temp_dir = std::env::temp_dir();
            let temp_id = uuid::Uuid::new_v4().to_string();
            let temp_file_path = temp_dir.join(format!("clin_{}.md", temp_id));

            if let Err(e) = std::fs::write(&temp_file_path, &note.content) {
                self.set_temporary_status(&format!("Failed to write temp file: {}", e));
                return;
            }

            // Suspend TUI
            let _ = disable_raw_mode();
            let _ = crossterm::execute!(
                std::io::stdout(),
                LeaveAlternateScreen,
                crossterm::event::DisableMouseCapture,
                crossterm::event::DisableBracketedPaste
            );

            let editor = self.external_editor.clone()
                .or_else(|| std::env::var("VISUAL").ok())
                .or_else(|| std::env::var("EDITOR").ok())
                .unwrap_or_else(|| "vi".to_string());

            let mut command = if cfg!(target_os = "windows") {
                let mut c = std::process::Command::new("cmd");
                c.arg("/C").arg(format!("{} \"{}\"", editor, temp_file_path.display()));
                c
            } else {
                let mut c = std::process::Command::new("sh");
                c.arg("-c").arg(format!("{} \"{}\"", editor, temp_file_path.display()));
                c
            };
            let result = command.status();

            // Resume TUI
            let _ = enable_raw_mode();
            let _ = crossterm::execute!(
                std::io::stdout(),
                EnterAlternateScreen,
                crossterm::event::EnableMouseCapture,
                crossterm::event::EnableBracketedPaste,
                crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
            );
            self.needs_full_redraw = true;

            match result {
                Ok(status) if status.success() => {
                    if let Ok(new_content) = std::fs::read_to_string(&temp_file_path) {
                        if new_content != note.content {
                            let mut updated_note = note.clone();
                            updated_note.content = new_content;
                            updated_note.updated_at = now_unix_secs();
                            if let Err(e) = self.storage.save_note(note_id, &updated_note, self.encryption_enabled) {
                                self.set_temporary_status(&format!("Failed to save note: {}", e));
                            } else {
                                self.set_temporary_status("Note saved from external editor.");
                                let _ = self.refresh_notes();
                            }
                        } else {
                            self.set_temporary_status("No changes made in external editor.");
                        }
                    } else {
                        self.set_temporary_status("Failed to read from temp file.");
                    }
                }
                Ok(status) => {
                    self.set_temporary_status(&format!("Editor '{}' exited with status: {}", editor, status));
                }
                Err(e) => {
                    self.set_temporary_status(&format!("Failed to launch editor '{}': {}", editor, e));
                }
            }

            let _ = std::fs::remove_file(&temp_file_path);
        } else {
            self.set_temporary_status("Failed to load note for external editor!");
        }
    }

    pub fn confirm_encrypt_warning(&mut self) {
        if let Some(id) = self.pending_encrypt_note_id.take() {
            if self.external_editor_enabled {
                self.open_note_in_external_editor(&id);
            } else {
                self.load_and_open_note(&id);
            }
        }
    }

    pub fn cancel_encrypt_warning(&mut self) {
        self.pending_encrypt_note_id = None;
        self.set_default_status();
    }

    pub fn collapse_selected_folder(&mut self) {
        if self.visual_list.is_empty() {
            return;
        }

        if self.visual_index >= self.visual_list.len() {
            self.visual_index = self.visual_list.len().saturating_sub(1);
        }

        match &self.visual_list[self.visual_index] {
            VisualItem::Folder {
                path, is_expanded, ..
            } => {
                if *is_expanded {
                    self.folder_expanded.remove(path);
                    self.refresh_visual_list();
                } else {
                    // Navigate to parent folder
                    if !path.is_empty() {
                        let parent_path = if let Some(slash) = path.rfind('/') {
                            &path[..slash]
                        } else {
                            "" // root
                        };

                        if let Some(idx) = self.visual_list.iter().position(|v| {
                            if let VisualItem::Folder { path: p, .. } = v {
                                p == parent_path
                            } else {
                                false
                            }
                        }) {
                            self.visual_index = idx;
                        }
                    }
                }
            }
            VisualItem::Note { .. } | VisualItem::CreateNew { .. } => {
                // Determine folder path and navigate to parent folder
                let item_path = match &self.visual_list[self.visual_index] {
                    VisualItem::Note { summary_idx, .. } => &self.notes[*summary_idx].folder,
                    VisualItem::CreateNew { path, .. } => path,
                    _ => unreachable!(),
                };

                if let Some(idx) = self.visual_list.iter().position(|v| {
                    if let VisualItem::Folder { path: p, .. } = v {
                        p == item_path
                    } else {
                        false
                    }
                }) {
                    self.visual_index = idx;
                }
            }
        }
    }

    pub fn expand_selected_folder(&mut self) {
        if self.visual_list.is_empty() {
            return;
        }

        if self.visual_index >= self.visual_list.len() {
            self.visual_index = self.visual_list.len().saturating_sub(1);
        }

        match &self.visual_list[self.visual_index] {
            VisualItem::Folder {
                path, is_expanded, ..
            } => {
                if !is_expanded {
                    self.folder_expanded.insert(path.clone());
                    self.refresh_visual_list();
                } else {
                    // Navigate to first child
                    if self.visual_index + 1 < self.visual_list.len() {
                        self.visual_index += 1;
                    }
                }
            }
            VisualItem::Note { .. } | VisualItem::CreateNew { .. } => {
                self.open_selected();
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
            // Now we need to find its visual index...
            if let Some(v_idx) = self.visual_list.iter().position(|v| match v {
                VisualItem::Note { summary_idx, .. } => *summary_idx == index,
                _ => false,
            }) {
                self.visual_index = v_idx;
                self.open_selected();
                return true;
            }
        }

        false
    }

    pub fn start_new_note(&mut self, folder: String) {
        // Check if default template exists and use it
        let template_manager = self.storage.template_manager();
        if let Some(default_template) = template_manager.load_default() {
            self.start_note_from_template(&default_template, folder);
        } else {
            self.start_blank_note(folder);
        }
    }

    pub fn start_blank_note(&mut self, folder: String) {
        let mut new_id = self.storage.new_note_id();
        if !folder.is_empty() {
            new_id = format!("{}/{}", folder, new_id);
        }
        
        if self.external_editor_enabled {
            let new_note = Note {
                title: String::from("Untitled note"),
                content: String::new(),
                updated_at: now_unix_secs(),
                tags: Vec::new(),
            };
            if let Ok(_) = self.storage.save_note(&new_id, &new_note, self.encryption_enabled) {
                let _ = self.refresh_notes();
                self.open_note_in_external_editor(&new_id);
            }
            return;
        }

        self.mode = ViewMode::Edit;
        self.editing_id = Some(new_id);
        self.title_editor = make_title_editor("");
        self.editor = TextArea::default();
        self.editor
            .set_cursor_style(Style::default().fg(Color::Black).bg(Color::Cyan));
        self.editor
            .set_cursor_line_style(Style::default().bg(Color::Rgb(32, 36, 44)));
        self.set_default_status();
    }

    pub fn start_note_from_template(&mut self, template: &Template, folder: String) {
        let rendered = template.render();

        let mut new_id = self.storage.new_note_id();
        if !folder.is_empty() {
            new_id = format!("{}/{}", folder, new_id);
        }

        if self.external_editor_enabled {
            let new_note = Note {
                title: rendered.title.clone().unwrap_or_else(|| String::from("Untitled note")),
                content: rendered.content.clone(),
                updated_at: now_unix_secs(),
                tags: Vec::new(),
            };
            if let Ok(_) = self.storage.save_note(&new_id, &new_note, self.encryption_enabled) {
                let _ = self.refresh_notes();
                self.open_note_in_external_editor(&new_id);
            }
            return;
        }

        self.mode = ViewMode::Edit;
        self.editing_id = Some(new_id);

        self.title_editor = make_title_editor(rendered.title.as_deref().unwrap_or(""));
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
        let folder = self.get_current_folder_context();
        if let Some(popup) = self.template_popup.take()
            && let Some(summary) = popup.templates.get(popup.selected)
        {
            let template_manager = self.storage.template_manager();
            if let Ok(template) = template_manager.load(&summary.filename) {
                self.start_note_from_template(&template, folder);
                return;
            } else {
                self.set_temporary_status("Failed to load selected template");
            }
        }
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
            title: title.clone(),
            content,
            updated_at: self
                .storage
                .load_note(&id)
                .map(|n| n.updated_at)
                .unwrap_or_else(|_| now_unix_secs()),
            tags: self
                .storage
                .load_note(&id)
                .map(|n| n.tags)
                .unwrap_or_default(),
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
        self.pending_encrypt_note_id = None;
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
        if self.visual_index >= self.visual_list.len() {
            self.set_temporary_status("No item selected to delete");
            return;
        }

        match &self.visual_list[self.visual_index].clone() {
            VisualItem::Note { summary_idx, .. } => {
                if let Some(note) = self.notes.get(*summary_idx) {
                    self.pending_delete_note_id = Some(note.id.clone());
                    self.status_until = None;
                    self.status =
                        Cow::Owned(format!("Delete \"{}\"? y confirm, n cancel", note.title));
                }
            }
            VisualItem::Folder { path, .. } => {
                if path.is_empty() {
                    self.set_temporary_status("Cannot delete Vault root");
                    return;
                }
                if let Err(e) = self.storage.delete_folder(path) {
                    self.set_temporary_status(&format!("Failed to delete folder: {e}"));
                } else {
                    let _ = self.refresh_notes();
                    self.set_temporary_status("Folder deleted");
                }
            }
            _ => {
                self.set_temporary_status("Cannot delete this item");
            }
        }
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
                if self.visual_index >= self.visual_list.len() && !self.visual_list.is_empty() {
                    self.visual_index = self.visual_list.len() - 1;
                }
                self.set_temporary_status("Note deleted");
            }
            Err(err) => {
                self.pending_delete_note_id = None;
                self.set_temporary_status(&format!("Delete failed: {err:#}"));
            }
        }
    }

    pub fn begin_create_folder(&mut self) {
        let parent_path = self.get_current_folder_context();
        let mut input = TextArea::default();
        let title = if parent_path.is_empty() {
            "Create Folder - Esc to cancel, Enter to save".to_string()
        } else {
            format!(
                "Create Folder in '{}' - Esc to cancel, Enter to save",
                parent_path
            )
        };
        input.set_block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .title(title),
        );
        self.folder_popup = Some(FolderPopup {
            mode: FolderPopupMode::Create { parent_path },
            input,
        });
    }

    pub fn begin_rename_folder(&mut self) {
        if let Some(VisualItem::Folder { path, .. }) = self.visual_list.get(self.visual_index) {
            if path.is_empty() {
                self.set_temporary_status("Cannot rename Vault root");
                return;
            }
            let mut input = TextArea::default();
            input.insert_str(path);
            input.set_block(
                ratatui::widgets::Block::default()
                    .borders(ratatui::widgets::Borders::ALL)
                    .title("Rename Folder - Esc to cancel, Enter to save"),
            );
            self.folder_popup = Some(FolderPopup {
                mode: FolderPopupMode::Rename {
                    old_path: path.clone(),
                },
                input,
            });
        } else {
            self.set_temporary_status("Select a folder to rename");
        }
    }

    pub fn confirm_folder_popup(&mut self) {
        if let Some(popup) = self.folder_popup.take() {
            let text = popup.input.lines().join("");
            let text = text.trim();
            if text.is_empty() {
                self.set_temporary_status("Folder name cannot be empty");
                return;
            }

            match &popup.mode {
                FolderPopupMode::Create { parent_path } => {
                    // Combine parent path with the new folder name
                    let full_path = if parent_path.is_empty() {
                        text.to_string()
                    } else {
                        format!("{}/{}", parent_path, text)
                    };
                    if let Err(e) = self.storage.create_folder(&full_path) {
                        self.set_temporary_status(&format!("Failed to create folder: {e}"));
                    } else {
                        let _ = self.refresh_notes();
                        self.set_temporary_status("Folder created");
                    }
                }
                FolderPopupMode::Rename { old_path } => {
                    if let Err(e) = self.storage.rename_folder(old_path, text) {
                        self.set_temporary_status(&format!("Failed to rename folder: {e}"));
                    } else {
                        let _ = self.refresh_notes();
                        self.set_temporary_status("Folder renamed");
                    }
                }
            }
        }
    }

    pub fn begin_move_note(&mut self) {
        if let Some(VisualItem::Note { summary_idx, .. }) = self.visual_list.get(self.visual_index)
        {
            let note = &self.notes[*summary_idx];
            if let Ok(folders) = self.storage.list_folders() {
                let mut all_folders = vec!["".to_string()]; // Root folder
                all_folders.extend(folders);
                self.folder_picker = Some(FolderPicker {
                    note_id: note.id.clone(),
                    folders: all_folders,
                    selected: 0,
                });
            } else {
                self.set_temporary_status("Failed to list folders");
            }
        } else {
            self.set_temporary_status("Select a note to move");
        }
    }

    pub fn confirm_move_note(&mut self) {
        if let Some(picker) = self.folder_picker.take()
            && let Some(target_folder) = picker.folders.get(picker.selected)
        {
            if let Err(e) = self.storage.move_note(&picker.note_id, target_folder) {
                self.set_temporary_status(&format!("Failed to move note: {e}"));
            } else {
                let _ = self.refresh_notes();
                self.set_temporary_status("Note moved");
            }
        }
    }

    pub fn begin_manage_tags(&mut self) {
        if let Some(VisualItem::Note { summary_idx, .. }) = self.visual_list.get(self.visual_index)
        {
            let note = &self.notes[*summary_idx];
            let current_tags = note.tags.clone();

            let mut input = TextArea::default();
            input.insert_str(current_tags.join(", "));
            input.set_block(
                ratatui::widgets::Block::default()
                    .borders(ratatui::widgets::Borders::ALL)
                    .title("Manage Tags (comma separated) - Esc to cancel, Enter to save"),
            );

            self.tag_popup = Some(TagPopup {
                note_id: note.id.clone(),
                input,
            });
        } else {
            self.set_temporary_status("Select a note to manage tags");
        }
    }

    pub fn confirm_manage_tags(&mut self) {
        if let Some(popup) = self.tag_popup.take() {
            let text = popup.input.lines().join("");
            let tags: Vec<String> = text
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            if let Ok(mut note) = self.storage.load_note(&popup.note_id) {
                note.tags = tags.clone();
                let is_clin = popup.note_id.ends_with(".clin");
                if let Err(e) = self.storage.save_note(&popup.note_id, &note, is_clin) {
                    self.set_temporary_status(&format!("Failed to save tags: {e}"));
                } else {
                    let mut all_tags = self.storage.load_tag_cache();
                    let mut changed = false;
                    for t in &tags {
                        if !all_tags.contains(t) {
                            all_tags.push(t.clone());
                            changed = true;
                        }
                    }
                    if changed {
                        all_tags.sort();
                        let _ = self.storage.save_tag_cache(&all_tags);
                    }
                    let _ = self.refresh_notes();
                    self.set_temporary_status("Tags updated");
                }
            } else {
                self.set_temporary_status("Failed to load note to update tags");
            }
        }
    }

    pub fn begin_filter_tags(&mut self) {
        let mut input = TextArea::default();
        input.insert_str(self.filter_tags.join(", "));
        input.set_block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .title("Filter Tags (comma separated OR logic) - Esc to clear, Enter to apply"),
        );
        self.filter_popup = Some(input);
    }

    pub fn confirm_filter_tags(&mut self) {
        if let Some(input) = self.filter_popup.take() {
            let text = input.lines().join("");
            let tags: Vec<String> = text
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            self.filter_tags = tags;
            let _ = self.refresh_notes();
            self.visual_index = 0;
        }
    }

    pub fn cancel_filter_tags(&mut self) {
        self.filter_popup = None;
    }

    pub fn open_selected_note_location(&mut self) {
        if self.visual_index >= self.visual_list.len() {
            self.set_temporary_status("No note selected for location");
            return;
        }

        let summary_idx = match &self.visual_list[self.visual_index] {
            VisualItem::Note { summary_idx, .. } => *summary_idx,
            _ => {
                self.set_temporary_status("Selected item is not a note");
                return;
            }
        };

        let Some(note) = self.notes.get(summary_idx) else {
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
        self.storage.save_settings(&crate::storage::AppSettings {
            encryption_enabled: self.encryption_enabled,
        });
    }

    pub fn toggle_external_editor_mode(&mut self) {
        self.external_editor_enabled = !self.external_editor_enabled;
        self.set_default_status();
        if let Ok(mut config) = crate::config::BootstrapConfig::load() {
            config.external_editor_enabled = self.external_editor_enabled;
            let _ = config.save();
        }
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
    ExternalEditorToggle,
}
