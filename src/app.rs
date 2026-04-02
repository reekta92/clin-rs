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
    pub all_tags: Vec<String>,
    pub suggestions: Vec<String>,
    pub suggestion_index: usize,
    pub pending_delete_tag: Option<String>,
}

pub struct FilterTagPopup {
    pub input: TextArea<'static>,
    pub all_tags: Vec<String>,
    pub suggestions: Vec<String>,
    pub suggestion_index: usize,
}

pub enum FolderPopupMode {
    Create { parent_path: String },
    Rename { old_path: String },
}

pub struct FolderPopup {
    pub mode: FolderPopupMode,
    pub input: TextArea<'static>,
}

pub enum FolderPickerMode {
    MoveNote { note_id: String },
    MoveFolder { folder_path: String },
}

pub struct FolderPicker {
    pub mode: FolderPickerMode,
    pub folders: Vec<String>,
    pub selected: usize,
}

/// Note rename popup state
pub struct NoteRenamePopup {
    pub note_id: String,
    pub input: TextArea<'static>,
}

/// Note create popup state
pub struct NoteCreatePopup {
    pub folder: String,
    pub input: TextArea<'static>,
}

/// Search popup state for filtering notes
pub struct SearchPopup {
    pub input: TextArea<'static>,
    pub original_index: usize,
}

/// Sort field options
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortField {
    Title,
    Modified,
}

/// Sort order options
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortOrder {
    Ascending,
    Descending,
}

/// Trash view state
pub struct TrashView {
    pub notes: Vec<NoteSummary>,
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
    pub filter_popup: Option<FilterTagPopup>,
    pub command_palette: Option<crate::palette::CommandPalette>,
    /// Cached help page text (rebuilt when keybinds change)
    pub help_text_cache: Option<Text<'static>>,
    pub folder_cache: Option<Vec<String>>,
    pub list_state: ListState,
    pub needs_full_redraw: bool,
    // QoL features
    pub note_rename_popup: Option<NoteRenamePopup>,
    pub note_create_popup: Option<NoteCreatePopup>,
    pub search_popup: Option<SearchPopup>,
    pub sort_field: SortField,
    pub sort_order: SortOrder,
    pub trash_view: Option<TrashView>,
    pub preview_enabled: bool,
    pub preview_content: Option<String>,
    /// For vim-style 'gg' command - tracks last 'g' press time
    pub last_g_press: Option<Instant>,
    /// Page size for Ctrl+u/d navigation
    pub page_size: usize,
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
            encryption_enabled: bootstrap_config.encryption_enabled,
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
            command_palette: None,
            help_text_cache: None,
            folder_cache: None,
            list_state: ListState::default(),
            needs_full_redraw: false,
            // QoL features
            note_rename_popup: None,
            note_create_popup: None,
            search_popup: None,
            sort_field: SortField::Modified,
            sort_order: SortOrder::Descending,
            trash_view: None,
            preview_enabled: bootstrap_config.preview_enabled,
            preview_content: None,
            last_g_press: None,
            page_size: 10,
        };
        app.context_menu = None;
        app.template_popup = None;
        app.folder_expanded.insert(String::new());
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
        
        // Sort based on current sort options
        // Pinned notes always come first, then apply user's sort preference
        summaries.sort_by(|a, b| {
            // Pinned notes first
            let pin_cmp = b.pinned.cmp(&a.pinned);
            if pin_cmp != std::cmp::Ordering::Equal {
                return pin_cmp;
            }
            
            // Then encrypted notes (for backwards compat)
            let a_clin = a.id.ends_with(".clin");
            let b_clin = b.id.ends_with(".clin");
            let clin_cmp = b_clin.cmp(&a_clin);
            if clin_cmp != std::cmp::Ordering::Equal {
                return clin_cmp;
            }
            
            // Then apply user's sort preference
            match self.sort_field {
                SortField::Modified => {
                    match self.sort_order {
                        SortOrder::Descending => b.updated_at.cmp(&a.updated_at),
                        SortOrder::Ascending => a.updated_at.cmp(&b.updated_at),
                    }
                }
                SortField::Title => {
                    match self.sort_order {
                        SortOrder::Ascending => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
                        SortOrder::Descending => b.title.to_lowercase().cmp(&a.title.to_lowercase()),
                    }
                }
            }
        });

        if !self.filter_tags.is_empty() {
            for summary in &summaries {
                if !summary.folder.is_empty() {
                    let mut path = String::new();
                    for part in summary.folder.split('/') {
                        if !path.is_empty() {
                            path.push('/');
                        }
                        path.push_str(part);
                        self.folder_expanded.insert(path.clone());
                    }
                }
            }
            self.folder_expanded.insert(String::new());
        }

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
            path: String::new(),
            name: String::from("Vault"),
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
                path: String::new(),
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
        let all_folders = if let Some(ref cached) = self.folder_cache {
            cached.clone()
        } else {
            let folders = self.storage.list_folders().unwrap_or_default();
            self.folder_cache = Some(folders.clone());
            folders
        };

        for folder in all_folders {
            let parts: Vec<&str> = folder.split('/').collect();
                let depth = parts.len();
                let name = parts.last().unwrap_or(&"").to_string();

                // Only show if parent is expanded
                let parent_path = if parts.len() > 1 {
                    parts[..parts.len() - 1].join("/")
                } else {
                    String::new()
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
                        current_parent = String::new();
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
                self.begin_create_note_in_folder(path.clone());
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

            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                let file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&temp_file_path);

                match file {
                    Ok(mut f) => {
                        use std::io::Write;
                        if let Err(e) = f.write_all(note.content.as_bytes()) {
                            self.set_temporary_status(&format!("Failed to write temp file: {}", e));
                            return;
                        }
                    }
                    Err(e) => {
                        self.set_temporary_status(&format!("Failed to create temp file: {}", e));
                        return;
                    }
                }
            }

            #[cfg(not(unix))]
            {
                if let Err(e) = std::fs::write(&temp_file_path, &note.content) {
                    self.set_temporary_status(&format!("Failed to write temp file: {}", e));
                    return;
                }
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

            let parts: Vec<&str> = editor.split_whitespace().collect();
            let (program, editor_args) = parts.split_first()
                .map(|(p, a)| (*p, a.to_vec()))
                .unwrap_or(("vi", vec![]));

            let mut command = std::process::Command::new(program);
            for arg in editor_args {
                command.arg(arg);
            }
            command.arg(&temp_file_path);
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
                            let updated_note = Note {
                                title: note.title,
                                content: new_content,
                                updated_at: now_unix_secs(),
                                tags: note.tags,
                            };
                            if let Err(e) = self.storage.save_note(note_id, &updated_note, self.encryption_enabled) {
                                self.set_temporary_status(&format!("Failed to save note: {}", e));
                            } else {
                                self.set_temporary_status_static("Note saved from external editor.");
                                let _ = self.refresh_notes();
                            }
                        } else {
                            self.set_temporary_status_static("No changes made in external editor.");
                        }
                    } else {
                        self.set_temporary_status_static("Failed to read from temp file.");
                    }
                }
                Ok(status) => {
                    self.set_temporary_status(&format!("Editor '{}' exited with status: {}", editor, status));
                }
                Err(e) => {
                    self.set_temporary_status(&format!("Failed to launch editor '{}': {}", editor, e));
                }
            }

            // Secure: Overwrite file contents before deletion
            if let Ok(len) = std::fs::metadata(&temp_file_path).map(|m| m.len()) {
                let _ = std::fs::write(&temp_file_path, vec![0u8; len as usize]);
            }
            let _ = std::fs::remove_file(&temp_file_path);
        } else {
            self.set_temporary_status_static("Failed to load note for external editor!");
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

    pub fn start_new_note_with_title(&mut self, folder: String, title: String) {
        // Check if default template exists and use it
        let template_manager = self.storage.template_manager();
        if let Some(default_template) = template_manager.load_default() {
            self.start_note_from_template_with_title(&default_template, folder, title);
        } else {
            self.start_blank_note_with_title(folder, title);
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

    pub fn start_blank_note_with_title(&mut self, folder: String, title: String) {
        let mut new_id = self.storage.new_note_id();
        if !folder.is_empty() {
            new_id = format!("{}/{}", folder, new_id);
        }
        
        if self.external_editor_enabled {
            let new_note = Note {
                title: title,
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
        self.title_editor = make_title_editor(&title);
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

    pub fn start_note_from_template_with_title(&mut self, template: &Template, folder: String, title: String) {
        let rendered = template.render();

        let mut new_id = self.storage.new_note_id();
        if !folder.is_empty() {
            new_id = format!("{}/{}", folder, new_id);
        }

        if self.external_editor_enabled {
            let new_note = Note {
                title: title,
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

        self.title_editor = make_title_editor(&title);
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
                self.set_temporary_status_static("Failed to load templates");
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
                self.set_temporary_status_static("Failed to load selected template");
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
        
        let (updated_at, tags) = self
            .storage
            .load_note(&id)
            .map(|n| (n.updated_at, n.tags))
            .unwrap_or_else(|_| (now_unix_secs(), Vec::new()));
            
        let note = Note {
            title,
            content,
            updated_at,
            tags,
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
            self.set_temporary_status_static("No item selected to delete");
            return;
        }

        match &self.visual_list[self.visual_index] {
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
                    self.set_temporary_status_static("Cannot delete Vault root");
                    return;
                }
                if let Err(e) = self.storage.delete_folder(path) {
                    self.set_temporary_status(&format!("Failed to delete folder: {e}"));
                } else {
                    self.folder_cache = None;
                    let _ = self.refresh_notes();
                    self.set_temporary_status_static("Folder deleted");
                }
            }
            _ => {
                self.set_temporary_status_static("Cannot delete this item");
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
                self.set_temporary_status_static("Note deleted");
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
                self.set_temporary_status_static("Cannot rename Vault root");
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
            self.set_temporary_status_static("Select a folder to rename");
        }
    }

    pub fn confirm_folder_popup(&mut self) {
        if let Some(popup) = self.folder_popup.take() {
            let text = popup.input.lines().join("");
            let text = text.trim();
            if text.is_empty() {
                self.set_temporary_status_static("Folder name cannot be empty");
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
                        self.folder_cache = None;
                        let _ = self.refresh_notes();
                        self.set_temporary_status_static("Folder created");
                    }
                }
                FolderPopupMode::Rename { old_path } => {
                    if let Err(e) = self.storage.rename_folder(old_path, text) {
                        self.set_temporary_status(&format!("Failed to rename folder: {e}"));
                    } else {
                        self.folder_cache = None;
                        let _ = self.refresh_notes();
                        self.set_temporary_status_static("Folder renamed");
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
                    mode: FolderPickerMode::MoveNote { note_id: note.id.clone() },
                    folders: all_folders,
                    selected: 0,
                });
            } else {
                self.set_temporary_status_static("Failed to list folders");
            }
        } else {
            self.set_temporary_status_static("Select a note to move");
        }
    }

    pub fn begin_move_folder(&mut self) {
        if let Some(VisualItem::Folder { path, .. }) = self.visual_list.get(self.visual_index) {
            let folder_path = path.clone();
            if let Ok(folders) = self.storage.list_folders() {
                let mut all_folders = vec!["".to_string()]; // Root folder
                all_folders.extend(
                    folders.into_iter()
                        .filter(|f| f != &folder_path && !f.starts_with(&format!("{}/", folder_path)))
                );
                
                self.folder_picker = Some(FolderPicker {
                    mode: FolderPickerMode::MoveFolder { folder_path },
                    folders: all_folders,
                    selected: 0,
                });
            } else {
                self.set_temporary_status_static("Failed to list folders");
            }
        } else {
            self.set_temporary_status_static("Select a folder to move");
        }
    }

    /// Context-sensitive move - works for both notes and folders
    pub fn begin_move(&mut self) {
        match self.visual_list.get(self.visual_index) {
            Some(VisualItem::Note { .. }) => self.begin_move_note(),
            Some(VisualItem::Folder { .. }) => self.begin_move_folder(),
            _ => self.set_temporary_status_static("Nothing selected"),
        }
    }

    pub fn confirm_move(&mut self) {
        if let Some(picker) = self.folder_picker.take()
            && let Some(target_folder) = picker.folders.get(picker.selected)
        {
            match picker.mode {
                FolderPickerMode::MoveNote { note_id } => {
                    if let Err(e) = self.storage.move_note(&note_id, target_folder) {
                        self.set_temporary_status(&format!("Failed to move note: {e}"));
                    } else {
                        self.folder_cache = None;
                        let _ = self.refresh_notes();
                        self.set_temporary_status_static("Note moved");
                    }
                }
                FolderPickerMode::MoveFolder { folder_path } => {
                    let folder_name = folder_path.rsplit('/').next().unwrap_or(&folder_path);
                    let new_path = if target_folder.is_empty() {
                        folder_name.to_string()
                    } else {
                        format!("{}/{}", target_folder, folder_name)
                    };
                    
                    if folder_path == new_path {
                        self.set_temporary_status_static("Folder is already in this location");
                        return;
                    }
                    
                    if let Err(e) = self.storage.rename_folder(&folder_path, &new_path) {
                        self.set_temporary_status(&format!("Failed to move folder: {e}"));
                    } else {
                        if self.folder_expanded.remove(&folder_path) {
                            self.folder_expanded.insert(new_path);
                        }
                        self.folder_cache = None;
                        let _ = self.refresh_notes();
                        self.set_temporary_status_static("Folder moved");
                    }
                }
            }
        }
    }

    pub fn collect_live_tags(&self) -> Vec<String> {
        let mut tags: HashSet<String> = HashSet::new();
        for note in &self.notes {
            for tag in &note.tags {
                tags.insert(tag.clone());
            }
        }
        let mut result: Vec<String> = tags.into_iter().collect();
        result.sort();
        result
    }

    pub fn begin_manage_tags(&mut self) {
        if let Some(VisualItem::Note { summary_idx, .. }) = self.visual_list.get(self.visual_index)
        {
            let note = &self.notes[*summary_idx];
            let current_tags = note.tags.clone();
            let all_tags = self.collect_live_tags();

            let mut input = TextArea::default();
            input.insert_str(current_tags.join(", "));

            self.tag_popup = Some(TagPopup {
                note_id: note.id.clone(),
                input,
                all_tags,
                suggestions: Vec::new(),
                suggestion_index: 0,
                pending_delete_tag: None,
            });
            self.update_tag_suggestions();
        } else {
            self.set_temporary_status_static("Select a note to manage tags");
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
                let is_clin = popup.note_id.ends_with(".clin");
                note.tags = tags;
                if let Err(e) = self.storage.save_note(&popup.note_id, &note, is_clin) {
                    self.set_temporary_status(&format!("Failed to save tags: {e}"));
                } else {
                    let _ = self.refresh_notes();
                    self.set_temporary_status_static("Tags updated");
                }
            } else {
                self.set_temporary_status_static("Failed to load note to update tags");
            }
        }
    }

    /// Extract the current word being typed (after last comma)
    fn get_current_tag_word(input: &str) -> &str {
        input.rsplit(',').next().map(|s| s.trim()).unwrap_or("")
    }

    /// Update tag suggestions based on current input
    pub fn update_tag_suggestions(&mut self) {
        if let Some(popup) = &mut self.tag_popup {
            let text = popup.input.lines().join("");
            let current_word = Self::get_current_tag_word(&text).to_lowercase();
            
            // Get already entered tags
            let entered_tags: Vec<String> = text
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            
            if current_word.is_empty() {
                popup.suggestions.clear();
            } else {
                // Filter suggestions: match prefix, exclude already entered
                popup.suggestions = popup.all_tags
                    .iter()
                    .filter(|tag| {
                        let tag_lower = tag.to_lowercase();
                        tag_lower.starts_with(&current_word) 
                            && !entered_tags.contains(&tag_lower)
                    })
                    .cloned()
                    .collect();
            }
            popup.suggestion_index = 0;
        }
    }

    /// Cycle to next suggestion
    pub fn cycle_tag_suggestion(&mut self) {
        if let Some(popup) = &mut self.tag_popup {
            if !popup.suggestions.is_empty() {
                popup.suggestion_index = (popup.suggestion_index + 1) % popup.suggestions.len();
            }
        }
    }

    /// Accept current suggestion (replace current word)
    pub fn accept_tag_suggestion(&mut self) {
        if let Some(popup) = &mut self.tag_popup {
            if let Some(suggestion) = popup.suggestions.get(popup.suggestion_index).cloned() {
                let text = popup.input.lines().join("");
                
                // Find position of last comma
                if let Some(last_comma) = text.rfind(',') {
                    // Replace everything after last comma with suggestion
                    let prefix = &text[..=last_comma];
                    let new_text = format!("{} {}, ", prefix, suggestion);
                    
                    // Clear and re-insert
                    popup.input.select_all();
                    popup.input.cut();
                    popup.input.insert_str(&new_text);
                } else {
                    // No comma, replace entire text
                    popup.input.select_all();
                    popup.input.cut();
                    popup.input.insert_str(&format!("{}, ", suggestion));
                }
                
                popup.suggestions.clear();
                popup.suggestion_index = 0;
            }
        }
    }

    pub fn begin_delete_tag(&mut self) {
        if let Some(popup) = &mut self.tag_popup {
            if let Some(tag) = popup.suggestions.get(popup.suggestion_index).cloned() {
                popup.pending_delete_tag = Some(tag);
            }
        }
    }

    pub fn cancel_delete_tag(&mut self) {
        if let Some(popup) = &mut self.tag_popup {
            popup.pending_delete_tag = None;
        }
    }

    pub fn confirm_delete_tag(&mut self) {
        let tag_to_delete = if let Some(popup) = &self.tag_popup {
            popup.pending_delete_tag.clone()
        } else {
            None
        };

        if let Some(tag) = tag_to_delete {
            let mut count = 0;
            if let Ok(note_ids) = self.storage.list_note_ids() {
                for note_id in note_ids {
                    if let Ok(mut note) = self.storage.load_note(&note_id) {
                        if note.tags.contains(&tag) {
                            note.tags.retain(|t| t != &tag);
                            let is_clin = note_id.ends_with(".clin");
                            let _ = self.storage.save_note(&note_id, &note, is_clin);
                            count += 1;
                        }
                    }
                }
            }

            self.set_temporary_status(&format!("Deleted '{}' from {} note(s)", tag, count));
            let _ = self.refresh_notes();
            let live_tags = self.collect_live_tags();

            if let Some(popup) = &mut self.tag_popup {
                popup.pending_delete_tag = None;
                popup.all_tags = live_tags;
                let text = popup.input.lines().join("");
                
                // If the deleted tag was typed in the input, remove it
                let entered_tags: Vec<String> = text
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty() && s != &tag)
                    .collect();
                
                let new_text = if entered_tags.is_empty() {
                    String::new()
                } else {
                    format!("{}, ", entered_tags.join(", "))
                };
                
                popup.input.select_all();
                popup.input.cut();
                popup.input.insert_str(&new_text);
            }
            self.update_tag_suggestions();
        }
    }

    pub fn begin_filter_tags(&mut self) {
        let all_tags = self.collect_live_tags();
        let mut input = TextArea::default();
        input.insert_str(self.filter_tags.join(", "));
        
        self.filter_popup = Some(FilterTagPopup {
            input,
            all_tags,
            suggestions: Vec::new(),
            suggestion_index: 0,
        });
        self.update_filter_suggestions();
    }

    pub fn confirm_filter_tags(&mut self) {
        if let Some(popup) = self.filter_popup.take() {
            let text = popup.input.lines().join("");
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

    pub fn update_filter_suggestions(&mut self) {
        if let Some(popup) = &mut self.filter_popup {
            let text = popup.input.lines().join("");
            let current_word = Self::get_current_tag_word(&text).to_lowercase();
            
            let entered_tags: Vec<String> = text
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            
            if current_word.is_empty() {
                popup.suggestions.clear();
            } else {
                popup.suggestions = popup.all_tags
                    .iter()
                    .filter(|tag| {
                        let tag_lower = tag.to_lowercase();
                        tag_lower.starts_with(&current_word) 
                            && !entered_tags.contains(&tag_lower)
                    })
                    .cloned()
                    .collect();
            }
            popup.suggestion_index = 0;
        }
    }

    pub fn cycle_filter_suggestion(&mut self) {
        if let Some(popup) = &mut self.filter_popup {
            if !popup.suggestions.is_empty() {
                popup.suggestion_index = (popup.suggestion_index + 1) % popup.suggestions.len();
            }
        }
    }

    pub fn accept_filter_suggestion(&mut self) {
        if let Some(popup) = &mut self.filter_popup {
            if let Some(suggestion) = popup.suggestions.get(popup.suggestion_index).cloned() {
                let text = popup.input.lines().join("");
                
                if let Some(last_comma) = text.rfind(',') {
                    let prefix = &text[..=last_comma];
                    let new_text = format!("{} {}, ", prefix, suggestion);
                    
                    popup.input.select_all();
                    popup.input.cut();
                    popup.input.insert_str(&new_text);
                } else {
                    popup.input.select_all();
                    popup.input.cut();
                    popup.input.insert_str(&format!("{}, ", suggestion));
                }
                
                popup.suggestions.clear();
                popup.suggestion_index = 0;
            }
        }
    }

    pub fn open_selected_note_location(&mut self) {
        if self.visual_index >= self.visual_list.len() {
            self.set_temporary_status_static("No note selected for location");
            return;
        }

        let summary_idx = match &self.visual_list[self.visual_index] {
            VisualItem::Note { summary_idx, .. } => *summary_idx,
            _ => {
                self.set_temporary_status_static("Selected item is not a note");
                return;
            }
        };

        let Some(note) = self.notes.get(summary_idx) else {
            self.set_temporary_status_static("No note selected for location");
            return;
        };

        let note_path = self.storage.note_path(&note.id);
        let Some(parent) = note_path.parent() else {
            self.set_temporary_status_static("Could not determine note directory");
            return;
        };

        match open_in_file_manager(parent) {
            Ok(()) => self.set_temporary_status_static("Opened note file location"),
            Err(err) => self.set_temporary_status(&format!("Open location failed: {err:#}")),
        }
    }

    pub fn toggle_encryption_mode(&mut self) {
        self.encryption_enabled = !self.encryption_enabled;
        self.set_default_status();
        if let Ok(mut config) = crate::config::BootstrapConfig::load() {
            config.encryption_enabled = self.encryption_enabled;
            let _ = config.save();
        }
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

    pub fn set_temporary_status_static(&mut self, message: &'static str) {
        self.status = Cow::Borrowed(message);
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
    pub fn get_help_text(&mut self) -> &Text<'static> {
        if self.help_text_cache.is_none() {
            self.help_text_cache = Some(help_page_text(&self.keybinds));
        }
        self.help_text_cache.as_ref().unwrap()
    }

    // ===== QoL Feature Methods =====

    /// Begin creating a new note with a name prompt
    pub fn begin_create_note(&mut self) {
        let folder = self.get_current_folder_context();
        self.begin_create_note_in_folder(folder);
    }

    /// Begin creating a new note in a specific folder
    pub fn begin_create_note_in_folder(&mut self, folder: String) {
        let mut input = TextArea::default();
        input.set_block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .title("New Note Name - Esc to cancel, Enter to create"),
        );
        self.note_create_popup = Some(NoteCreatePopup {
            folder,
            input,
        });
    }

    /// Confirm and create the note with the prompted name
    pub fn confirm_create_note(&mut self) {
        if let Some(popup) = self.note_create_popup.take() {
            let mut title = popup.input.lines().join("");
            title = title.trim().to_string();
            if title.is_empty() {
                title = String::from("Untitled note");
            }
            self.start_new_note_with_title(popup.folder, title);
        }
    }

    /// Begin renaming a note (context-sensitive with folder rename)
    pub fn begin_rename_note(&mut self) {
        if let Some(VisualItem::Note { summary_idx, id, .. }) = self.visual_list.get(self.visual_index).cloned() {
            let note = &self.notes[summary_idx];
            let mut input = TextArea::default();
            input.insert_str(&note.title);
            input.set_block(
                ratatui::widgets::Block::default()
                    .borders(ratatui::widgets::Borders::ALL)
                    .title("Rename Note - Esc to cancel, Enter to save"),
            );
            self.note_rename_popup = Some(NoteRenamePopup {
                note_id: id,
                input,
            });
        } else {
            self.set_temporary_status_static("Select a note to rename");
        }
    }

    /// Confirm and apply note rename
    pub fn confirm_rename_note(&mut self) {
        if let Some(popup) = self.note_rename_popup.take() {
            let new_title = popup.input.lines().join("");
            let new_title = new_title.trim();
            if new_title.is_empty() {
                self.set_temporary_status_static("Title cannot be empty");
                return;
            }
            match self.storage.rename_note(&popup.note_id, new_title) {
                Ok(_) => {
                    let _ = self.refresh_notes();
                    self.set_temporary_status_static("Note renamed");
                }
                Err(e) => {
                    self.set_temporary_status(&format!("Failed to rename: {e}"));
                }
            }
        }
    }

    /// Duplicate the selected note
    pub fn duplicate_note(&mut self) {
        if let Some(VisualItem::Note { id, .. }) = self.visual_list.get(self.visual_index).cloned() {
            match self.storage.duplicate_note(&id) {
                Ok(_) => {
                    let _ = self.refresh_notes();
                    self.set_temporary_status_static("Note duplicated");
                }
                Err(e) => {
                    self.set_temporary_status(&format!("Failed to duplicate: {e}"));
                }
            }
        } else {
            self.set_temporary_status_static("Select a note to duplicate");
        }
    }

    /// Toggle pin status of selected note
    pub fn toggle_pin(&mut self) {
        if let Some(VisualItem::Note { id, .. }) = self.visual_list.get(self.visual_index).cloned() {
            match self.storage.toggle_pin(&id) {
                Ok(pinned) => {
                    let _ = self.refresh_notes();
                    if pinned {
                        self.set_temporary_status_static("Note pinned");
                    } else {
                        self.set_temporary_status_static("Note unpinned");
                    }
                }
                Err(e) => {
                    self.set_temporary_status(&format!("Failed to toggle pin: {e}"));
                }
            }
        } else {
            self.set_temporary_status_static("Select a note to pin/unpin");
        }
    }

    /// Cycle through sort options
    pub fn cycle_sort(&mut self) {
        match (self.sort_field, self.sort_order) {
            (SortField::Modified, SortOrder::Descending) => {
                self.sort_field = SortField::Modified;
                self.sort_order = SortOrder::Ascending;
            }
            (SortField::Modified, SortOrder::Ascending) => {
                self.sort_field = SortField::Title;
                self.sort_order = SortOrder::Ascending;
            }
            (SortField::Title, SortOrder::Ascending) => {
                self.sort_field = SortField::Title;
                self.sort_order = SortOrder::Descending;
            }
            (SortField::Title, SortOrder::Descending) => {
                self.sort_field = SortField::Modified;
                self.sort_order = SortOrder::Descending;
            }
        }
        let _ = self.refresh_notes();
        let sort_desc = match (self.sort_field, self.sort_order) {
            (SortField::Modified, SortOrder::Descending) => "Sort: Modified (newest)",
            (SortField::Modified, SortOrder::Ascending) => "Sort: Modified (oldest)",
            (SortField::Title, SortOrder::Ascending) => "Sort: Title (A-Z)",
            (SortField::Title, SortOrder::Descending) => "Sort: Title (Z-A)",
        };
        self.set_temporary_status_static(sort_desc);
    }

    /// Begin search/filter mode
    pub fn begin_search(&mut self) {
        let mut input = TextArea::default();
        input.set_block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .title("Search Notes - Esc to cancel, Enter to confirm"),
        );
        input.set_cursor_line_style(Style::default());
        self.search_popup = Some(SearchPopup {
            input,
            original_index: self.visual_index,
        });
    }

    /// Update search results as user types
    pub fn update_search(&mut self) {
        if let Some(popup) = &self.search_popup {
            let query = popup.input.lines().join("").to_lowercase();
            if query.is_empty() {
                return;
            }

            // Find first matching note
            for (note_idx, note) in self.notes.iter().enumerate() {
                if note.title.to_lowercase().contains(&query) {
                    // Expand the folder containing this note
                    if !note.folder.is_empty() {
                        let mut path = String::new();
                        for part in note.folder.split('/') {
                            if !path.is_empty() {
                                path.push('/');
                            }
                            path.push_str(part);
                            self.folder_expanded.insert(path.clone());
                        }
                    }
                    
                    // Rebuild visual list with newly expanded folders
                    let _ = self.refresh_visual_list();
                    
                    // Find the note in the updated visual_list
                    for (idx, item) in self.visual_list.iter().enumerate() {
                        if let VisualItem::Note { summary_idx, .. } = item {
                            if *summary_idx == note_idx {
                                self.visual_index = idx;
                                self.update_preview();
                                return;
                            }
                        }
                    }
                    return;
                }
            }
        }
    }

    /// Confirm search and stay at current position
    pub fn confirm_search(&mut self) {
        self.search_popup = None;
    }

    /// Cancel search and return to original position
    pub fn cancel_search(&mut self) {
        if let Some(popup) = self.search_popup.take() {
            self.visual_index = popup.original_index;
            self.update_preview();
        }
    }

    // ===== Vim-style Navigation =====

    /// Jump to the top of the list
    pub fn jump_to_top(&mut self) {
        self.visual_index = 0;
        self.update_preview();
    }

    /// Jump to the bottom of the list
    pub fn jump_to_bottom(&mut self) {
        self.visual_index = self.visual_list.len().saturating_sub(1);
        self.update_preview();
    }

    /// Page up (half page)
    pub fn page_up(&mut self) {
        self.visual_index = self.visual_index.saturating_sub(self.page_size);
        self.update_preview();
    }

    /// Page down (half page)
    pub fn page_down(&mut self) {
        let max_index = self.visual_list.len().saturating_sub(1);
        self.visual_index = (self.visual_index + self.page_size).min(max_index);
        self.update_preview();
    }

    /// Handle 'g' key press for vim-style gg
    pub fn handle_g_press(&mut self) -> bool {
        let now = Instant::now();
        if let Some(last) = self.last_g_press {
            if now.duration_since(last) < Duration::from_millis(500) {
                self.last_g_press = None;
                self.jump_to_top();
                return true;
            }
        }
        self.last_g_press = Some(now);
        false
    }

    // ===== Trash Functions =====

    /// Move note to trash instead of permanent delete
    pub fn trash_selected_note(&mut self) {
        if let Some(VisualItem::Note { id, .. }) = self.visual_list.get(self.visual_index).cloned() {
            match self.storage.trash_note(&id) {
                Ok(()) => {
                    let _ = self.refresh_notes();
                    self.set_temporary_status_static("Note moved to trash");
                }
                Err(e) => {
                    self.set_temporary_status(&format!("Failed to trash: {e}"));
                }
            }
        } else {
            self.set_temporary_status_static("Select a note to delete");
        }
    }

    /// Open trash view
    pub fn open_trash_view(&mut self) {
        match self.storage.list_trash() {
            Ok(notes) => {
                if notes.is_empty() {
                    self.set_temporary_status_static("Trash is empty");
                    return;
                }
                self.trash_view = Some(TrashView { notes, selected: 0 });
            }
            Err(e) => {
                self.set_temporary_status(&format!("Failed to open trash: {e}"));
            }
        }
    }

    /// Close trash view
    pub fn close_trash_view(&mut self) {
        self.trash_view = None;
    }

    /// Restore selected note from trash
    pub fn restore_from_trash(&mut self) {
        if let Some(ref mut trash) = self.trash_view {
            if let Some(note) = trash.notes.get(trash.selected) {
                let note_id = note.id.clone();
                match self.storage.restore_note(&note_id) {
                    Ok(_) => {
                        // Refresh trash view
                        if let Ok(notes) = self.storage.list_trash() {
                            if notes.is_empty() {
                                self.trash_view = None;
                                self.set_temporary_status_static("Note restored, trash is now empty");
                            } else {
                                trash.notes = notes;
                                trash.selected = trash.selected.min(trash.notes.len().saturating_sub(1));
                                self.set_temporary_status_static("Note restored");
                            }
                        }
                        let _ = self.refresh_notes();
                    }
                    Err(e) => {
                        self.set_temporary_status(&format!("Failed to restore: {e}"));
                    }
                }
            }
        }
    }

    /// Permanently delete selected note from trash
    pub fn delete_from_trash(&mut self) {
        if let Some(ref mut trash) = self.trash_view {
            if let Some(note) = trash.notes.get(trash.selected) {
                let note_id = note.id.clone();
                match self.storage.delete_from_trash(&note_id) {
                    Ok(()) => {
                        if let Ok(notes) = self.storage.list_trash() {
                            if notes.is_empty() {
                                self.trash_view = None;
                                self.set_temporary_status_static("Note deleted, trash is now empty");
                            } else {
                                trash.notes = notes;
                                trash.selected = trash.selected.min(trash.notes.len().saturating_sub(1));
                                self.set_temporary_status_static("Note permanently deleted");
                            }
                        }
                    }
                    Err(e) => {
                        self.set_temporary_status(&format!("Failed to delete: {e}"));
                    }
                }
            }
        }
    }

    /// Empty the entire trash
    pub fn empty_trash(&mut self) {
        match self.storage.empty_trash() {
            Ok(count) => {
                self.trash_view = None;
                self.set_temporary_status(&format!("Deleted {} notes from trash", count));
            }
            Err(e) => {
                self.set_temporary_status(&format!("Failed to empty trash: {e}"));
            }
        }
    }

    // ===== Preview Pane =====

    /// Toggle preview pane
    pub fn toggle_preview(&mut self) {
        self.preview_enabled = !self.preview_enabled;
        if self.preview_enabled {
            self.update_preview();
            self.set_temporary_status_static("Preview enabled");
        } else {
            self.preview_content = None;
            self.set_temporary_status_static("Preview disabled");
        }
    }

    /// Update preview content for currently selected note
    pub fn update_preview(&mut self) {
        if !self.preview_enabled {
            return;
        }

        if let Some(VisualItem::Note { id, .. }) = self.visual_list.get(self.visual_index).cloned() {
            if let Ok(note) = self.storage.load_note(&id) {
                // Truncate to first 50 lines for preview
                let preview: String = note.content.lines().take(50).collect::<Vec<_>>().join("\n");
                self.preview_content = Some(preview);
            } else {
                self.preview_content = None;
            }
        } else {
            self.preview_content = None;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditFocus {
    Title,
    Body,
    EncryptionToggle,
    ExternalEditorToggle,
}
