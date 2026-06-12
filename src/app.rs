use crate::config::ClinConfig;
use std::path::PathBuf;

pub use crate::cli::CliCommand;
use crate::constants::*;
pub use crate::editor::*;
use crate::events::get_title_text;
use crate::events::make_title_editor;
pub use crate::list_view::*;
use crate::markdown::MarkdownRenderer;
pub use crate::popups::*;
use crate::ui::text_area_from_content;
use crate::ui::{now_unix_secs, open_in_file_manager};
use ratatui::style::{Color, Style};
use ratatui::text::Text;
use std::borrow::Cow;
use std::time::Duration;
use std::time::Instant;

use crate::keybinds::Keybinds;
use crate::storage::{Note, NoteSummary, Storage};
use crate::templates::Template;
use anyhow::Result;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui_textarea::TextArea;
use std::collections::{HashMap, HashSet};

const VIRTUAL_PINNED_PATH: &str = "__clin_virtual__/pinned";
pub const VIRTUAL_PINNED_LABEL: &str = "Pinned";





#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    pub text: String,
    pub folder_filter: Option<String>,
    pub pinned_only: bool,
    pub tag_filter: Option<Vec<String>>,
    pub grep_mode: bool,
    pub grep_text: String,
}





fn find_filter_tokens(s: &str) -> Vec<(usize, &'static str)> {
    let spaced = [" f:", " g:", " p:", " t:"];
    let bare = ["f:", "g:", "p:", "t:"];
    let mut tokens: Vec<(usize, &'static str)> = Vec::new();

    
    let is_escaped = |s: &str, pos: usize, _prefix_len: usize| -> bool {
        if pos < 3 { return false; }
        &s[pos - 3..pos] == "\\e\\"
    };

    
    for &prefix in &spaced {
        let mut start = 0;
        while let Some(pos) = s[start..].find(prefix) {
            let abs_pos = start + pos;
            if !is_escaped(s, abs_pos, prefix.len()) {
                tokens.push((abs_pos, prefix));
            }
            start = abs_pos + prefix.len();
        }
    }
    
    for &prefix in &bare {
        if s.starts_with(prefix) && !tokens.iter().any(|&(p, _)| p == 0) {
            
            if !is_escaped(s, 0, prefix.len()) {
                tokens.push((0, prefix));
            }
        }
    }
    tokens.sort_by_key(|&(pos, _)| pos);
    tokens
}


fn strip_escape_filter(s: &str) -> String {
    if !s.contains("\\e\\") {
        return s.to_string();
    }
    let filter_chars = ['f', 'g', 'p', 't'];
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().collect::<Vec<_>>().into_iter().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'e') {
            chars.next(); 
            if chars.peek() == Some(&'\\') {
                chars.next(); 
                
                let next = chars.peek().copied();
                let after = {
                    let mut it = chars.clone();
                    it.next(); 
                    it.next() 
                };
                let is_filter = next.zip(after).map_or(false, |(ch, colon)| {
                    filter_chars.contains(&ch) && colon == ':'
                });
                if is_filter {
                    
                    continue;
                }
                
                out.push('\\');
                out.push('e');
                out.push('\\');
            } else {
                out.push('\\');
                out.push('e');
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn parse_search_query(query: &str) -> SearchQuery {
    let text = query.to_string();
    let mut folder_filter = None;
    let mut pinned_only = false;
    let mut grep_mode = false;
    let mut grep_text = String::new();
    let mut tag_filter = None;

    let tokens = find_filter_tokens(&text);
    if tokens.is_empty() {
        return SearchQuery {
            text,
            folder_filter,
            pinned_only,
            grep_mode,
            grep_text,
            tag_filter,
        };
    }

    
    let mut ranges: Vec<(usize, usize)> = Vec::with_capacity(tokens.len());

    for i in 0..tokens.len() {
        let (pos, prefix) = tokens[i];
        let val_start = pos + prefix.len();
        let val_end = tokens.get(i + 1).map_or(text.len(), |&(next, _)| next);
        let value = text[val_start..val_end].trim().to_string();
        ranges.push((pos, val_end));

        match prefix {
            " f:" | "f:" => {
                folder_filter = Some(if value.is_empty() {
                    String::new()
                } else {
                    strip_escape_filter(&value)
                });
            }
            " p:" | "p:" => {
                pinned_only = true;
            }
            " g:" | "g:" => {
                grep_mode = true;
                grep_text = strip_escape_filter(&value);
            }
            " t:" | "t:" => {
                let stripped = strip_escape_filter(&value);
                let tags: Vec<String> = stripped
                    .split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect();
                tag_filter = Some(tags);
            }
            _ => {}
        }
    }

    
    let mut clean = text.clone();
    for (start, end) in ranges.into_iter().rev() {
        clean.replace_range(start..end, "");
    }
    clean = strip_escape_filter(&clean);
    clean = clean.trim().to_string();

    SearchQuery {
        text: clean,
        folder_filter,
        pinned_only,
        grep_mode,
        grep_text,
        tag_filter,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    List,
    Edit,
    Help,
    Graph,
    Draw,
    Canvas,
    Backup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HelpTab {
    Notes,
    Editor,
    Graph,
    Draw,
    Canvas,
    Backup,
    Templates,
    About,
}

impl HelpTab {
    pub fn prev(self) -> Self {
        match self {
            HelpTab::Notes => HelpTab::About,
            HelpTab::Editor => HelpTab::Notes,
            HelpTab::Graph => HelpTab::Editor,
            HelpTab::Draw => HelpTab::Graph,
            HelpTab::Canvas => HelpTab::Draw,
            HelpTab::Backup => HelpTab::Canvas,
            HelpTab::Templates => HelpTab::Backup,
            HelpTab::About => HelpTab::Templates,
        }
    }

    pub fn next(self) -> Self {
        match self {
            HelpTab::Notes => HelpTab::Editor,
            HelpTab::Editor => HelpTab::Graph,
            HelpTab::Graph => HelpTab::Draw,
            HelpTab::Draw => HelpTab::Canvas,
            HelpTab::Canvas => HelpTab::Backup,
            HelpTab::Backup => HelpTab::Templates,
            HelpTab::Templates => HelpTab::About,
            HelpTab::About => HelpTab::Notes,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            HelpTab::Notes => "Notes",
            HelpTab::Editor => "Editor",
            HelpTab::Graph => "Graph",
            HelpTab::Draw => "Draw",
            HelpTab::Canvas => "Pinstar",
            HelpTab::Backup => "Backup",
            HelpTab::Templates => "Templates",
            HelpTab::About => "About",
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i {
            0 => HelpTab::Notes,
            1 => HelpTab::Editor,
            2 => HelpTab::Graph,
            3 => HelpTab::Draw,
            4 => HelpTab::Canvas,
            5 => HelpTab::Backup,
            6 => HelpTab::Templates,
            _ => HelpTab::About,
        }
    }

    pub fn count() -> usize {
        8
    }
}

pub struct App {
    pub popups: crate::popups::PopupManager,
    pub storage: Storage,
    pub keybinds: Keybinds,
    pub notes: Vec<NoteSummary>,
    pub editor: NoteEditor,
    pub list: ListView,
    pub mode: ViewMode,
    pub status: Cow<'static, str>,
    pub status_until: Option<Instant>,
    pub help_scroll: u16,
    pub help_tab: HelpTab,
    pub help_tab_scroll: HashMap<HelpTab, u16>,
    pub command_palette: Option<crate::palette::CommandPalette>,
    pub needs_full_redraw: bool,
    pub confirm_on_delete: bool,
    pub confirm_on_quit: bool,
    pub should_quit: bool,
    pub preview_encryption: bool,
    pub preview_position: crate::config::PreviewPosition,
    pub pinned_on_top: bool,
    pub default_folder: Option<String>,
    pub last_g_press: Option<Instant>,
    pub last_esc_press: Option<Instant>,
    pub return_mode: Option<ViewMode>,
    pub app_theme: crate::app_theme::AppThemeColors,
    pub canvas_state: Option<crate::pinstar::state::PinstarState>,
}

impl App {
    pub fn new(storage: Storage) -> Result<Self> {
        let keybinds = storage.load_keybinds();
        let bootstrap_config = crate::config::ClinConfig::load().unwrap_or_default();
        let app_theme = crate::app_theme::AppThemeColors::from_config(&bootstrap_config.theme);

        let mut editor = NoteEditor::new();
        editor.external_editor_enabled = bootstrap_config.external_editor_enabled;
        editor.external_editor = bootstrap_config.external_editor;
        editor.editor_preview_enabled = bootstrap_config.editor_preview_enabled;
        editor.show_line_numbers = bootstrap_config.show_line_numbers;
        editor.title_editor = make_title_editor("", Color::Black, Color::Cyan);

        let mut list = ListView::new();
        list.sort_field = bootstrap_config
            .default_sort_field
            .unwrap_or(SortField::Title);
        list.sort_order = bootstrap_config
            .default_sort_order
            .unwrap_or(SortOrder::Ascending);
        list.preview_enabled = bootstrap_config.preview_enabled;
        list.page_size = 10;

        let mut app = Self {
            storage,
            keybinds,
            notes: Vec::new(),
            editor,
            list,
            mode: ViewMode::List,
            status: Cow::Borrowed(LIST_HELP_HINTS),
            status_until: None,
            help_scroll: 0,
            help_tab: HelpTab::Notes,
            help_tab_scroll: HashMap::new(),
            command_palette: None,
            popups: crate::popups::PopupManager::default(),
            needs_full_redraw: false,
            confirm_on_delete: bootstrap_config.confirm_on_delete,
            confirm_on_quit: bootstrap_config.confirm_on_quit,
            should_quit: false,
            preview_encryption: bootstrap_config.preview_encryption,
            preview_position: bootstrap_config.preview_position,
            pinned_on_top: bootstrap_config.pinned_on_top,
            default_folder: bootstrap_config.default_folder.clone(),
            last_g_press: None,
            last_esc_press: None,
            return_mode: None,
            app_theme,
            canvas_state: None,
        };
        app.list.folder_expanded.insert(String::new());
        app.refresh_notes()?;
        Ok(app)
    }

    fn is_virtual_pinned_path(path: &str) -> bool {
        path == VIRTUAL_PINNED_PATH
    }

    pub fn refresh_notes(&mut self) -> Result<()> {
        let mut summaries = Vec::new();
        for id in self.storage.list_note_ids()? {
            if let Ok(summary) = self.storage.load_note_summary(&id) {
                summaries.push(summary);
            }
        }

        summaries.sort_by(|a, b| {
            
            if self.pinned_on_top {
                let pin_cmp = b.pinned.cmp(&a.pinned);
                if pin_cmp != std::cmp::Ordering::Equal {
                    return pin_cmp;
                }
            }

            let a_clin = a.id.ends_with(".clin");
            let b_clin = b.id.ends_with(".clin");
            let clin_cmp = b_clin.cmp(&a_clin);
            if clin_cmp != std::cmp::Ordering::Equal {
                return clin_cmp;
            }

            match self.list.sort_field {
                SortField::Modified => match self.list.sort_order {
                    SortOrder::Descending => b.updated_at.cmp(&a.updated_at),
                    SortOrder::Ascending => a.updated_at.cmp(&b.updated_at),
                },
                SortField::Title => match self.list.sort_order {
                    SortOrder::Ascending => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
                    SortOrder::Descending => b.title.to_lowercase().cmp(&a.title.to_lowercase()),
                },
            }
        });

        self.notes = summaries;

        self.refresh_visual_list();
        Ok(())
    }

    pub fn refresh_visual_list(&mut self) {
        let mut visual = Vec::new();

        let mut by_folder: HashMap<&str, Vec<(usize, &NoteSummary)>> = HashMap::new();
        let mut pinned_notes: Vec<(usize, &NoteSummary)> = Vec::new();
        for (i, note) in self.notes.iter().enumerate() {
            by_folder
                .entry(note.folder.as_str())
                .or_default()
                .push((i, note));
            if note.pinned {
                pinned_notes.push((i, note));
            }
        }

        
        visual.push(VisualItem::Folder {
            path: VIRTUAL_PINNED_PATH.to_string(),
            name: VIRTUAL_PINNED_LABEL.to_string(),
            depth: 0,
            is_expanded: self.list.folder_expanded.contains(VIRTUAL_PINNED_PATH),
            note_count: pinned_notes.len(),
        });

        if self.list.folder_expanded.contains(VIRTUAL_PINNED_PATH) {
            for (idx, note) in pinned_notes {
                visual.push(VisualItem::Note {
                    id: note.id.clone(),
                    summary_idx: idx,
                    depth: 1,
                    is_clin: note.id.ends_with(".clin"),
                    is_draw: note.id.ends_with(".draw"),
                    is_canvas: note.id.ends_with(".canvas"),
                    in_virtual_pinned_folder: true,
                });
            }
        }

        
        visual.push(VisualItem::Folder {
            path: String::new(),
            name: String::from("Vault"),
            depth: 0,
            is_expanded: self.list.folder_expanded.contains(""),
            note_count: by_folder.get("").map_or(0, |v| v.len()),
        });

        if self.list.folder_expanded.contains("") {
            if let Some(notes) = by_folder.get("") {
                for (idx, note) in notes {
                    visual.push(VisualItem::Note {
                        id: note.id.clone(),
                        summary_idx: *idx,
                        depth: 1,
                        is_clin: note.id.ends_with(".clin"),
                        is_draw: note.id.ends_with(".draw"),
                        is_canvas: note.id.ends_with(".canvas"),
                        in_virtual_pinned_folder: false,
                    });
                }
            }
            visual.push(VisualItem::CreateNew {
                path: String::new(),
                depth: 1,
            });
        }

        let all_folders = if let Some(ref cached) = self.list.folder_cache {
            cached.clone()
        } else {
            let folders = self.storage.list_folders().unwrap_or_default();
            self.list.folder_cache = Some(folders.clone());
            folders
        };

        for folder in all_folders {
            let parts: Vec<&str> = folder.split('/').collect();
            let depth = parts.len();
            let name = parts.last().unwrap_or(&"").to_string();

            let parent_path = if parts.len() > 1 {
                parts[..parts.len() - 1].join("/")
            } else {
                String::new()
            };

            let mut is_visible = true;
            let mut current_parent = parent_path.clone();
            while !current_parent.is_empty() {
                if !self.list.folder_expanded.contains(&current_parent) {
                    is_visible = false;
                    break;
                }
                if let Some(slash) = current_parent.rfind('/') {
                    current_parent = current_parent[..slash].to_string();
                } else {
                    current_parent = String::new();
                }
            }

            if !self.list.folder_expanded.contains("") {
                is_visible = false;
            }

            if is_visible {
                let is_expanded = self.list.folder_expanded.contains(&folder);
                visual.push(VisualItem::Folder {
                    path: folder.clone(),
                    name,
                    depth,
                    is_expanded,
                    note_count: by_folder.get(folder.as_str()).map_or(0, |v| v.len()),
                });

                if is_expanded {
                    if let Some(notes) = by_folder.get(folder.as_str()) {
                        for (idx, note) in notes {
                            visual.push(VisualItem::Note {
                                id: note.id.clone(),
                                summary_idx: *idx,
                                depth: depth + 1,
                                is_clin: note.id.ends_with(".clin"),
                                is_draw: note.id.ends_with(".draw"),
                                is_canvas: note.id.ends_with(".canvas"),
                                in_virtual_pinned_folder: false,
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

        self.list.visual_list = visual;
    }

    pub fn get_current_folder_context(&self) -> String {
        let current = match self.list.visual_list.get(self.list.visual_index) {
            Some(VisualItem::Folder { path, .. }) => path.clone(),
            Some(VisualItem::Note { summary_idx, .. }) => self
                .notes
                .get(*summary_idx)
                .map(|n| n.folder.clone())
                .unwrap_or_default(),
            Some(VisualItem::CreateNew { path, .. }) => path.clone(),
            None => String::new(),
        };

        if current.is_empty() {
            self.default_folder.clone().unwrap_or_default()
        } else {
            current
        }
    }

    pub fn open_note_at_line(&mut self, note_id: &str, line_number: Option<usize>) {
        if note_id.ends_with(".draw") {
            self.open_draw_view();
            return;
        }
        if note_id.ends_with(".canvas") {
            self.open_canvas_view();
            return;
        }
        if note_id.ends_with(".clin") {
            self.status = Cow::Borrowed(
                "Note is encrypted. Use command palette (Ctrl+P) to decrypt.",
            );
            return;
        }
        if self.editor.external_editor_enabled {
            self.open_note_in_external_editor(note_id, line_number);
        } else {
            self.load_and_open_note(note_id, line_number);
        }
    }

    pub fn open_selected(&mut self) {
        if self.list.visual_list.is_empty() {
            return;
        }

        if self.list.visual_index >= self.list.visual_list.len() {
            self.list.visual_index = self.list.visual_list.len().saturating_sub(1);
        }

        match &self.list.visual_list[self.list.visual_index] {
            VisualItem::CreateNew { path, .. } => {
                self.begin_create_note_in_folder(path.clone());
            }
            VisualItem::Folder { path, .. } => {
                let p = path.clone();
                if self.list.folder_expanded.contains(&p) {
                    self.list.folder_expanded.remove(&p);
                } else {
                    self.list.folder_expanded.insert(p);
                }
                self.refresh_visual_list();
            }
            VisualItem::Note {
                summary_idx,
                ..
            } => {
                let note_id = self.notes.get(*summary_idx).map(|s| s.id.clone());
                if let Some(id) = note_id {
                    self.open_note_at_line(&id, None);
                }
            }
        }
    }

    pub fn load_and_open_note(&mut self, note_id: &str, line_number: Option<usize>) {
        if let Ok(note) = self.storage.load_note(note_id) {
            self.editor.editing_id = Some(note_id.to_string());
            self.editor.title_editor = make_title_editor(
                &note.title,
                self.app_theme.highlight_fg,
                self.app_theme.highlight_bg,
            );
            let mut editor = text_area_from_content(&note.content);
            if let Some(l) = line_number {
                editor.move_cursor(ratatui_textarea::CursorMove::Jump(
                    l.saturating_sub(1) as u16,
                    0,
                ));
            }
            self.editor.editor = editor;
            self.mode = ViewMode::Edit;
            if self.editor.editor_preview_enabled {
                self.update_editor_markdown_preview();
            } else {
                self.editor.md_preview_renderer = None;
            }
            self.status = Cow::Borrowed(EDIT_HELP_HINTS);
        } else {
            self.status = Cow::Borrowed("Failed to load note!");
        }
    }

    pub fn open_note_in_external_editor(&mut self, note_id: &str, line_number: Option<usize>) {
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

            let _ = disable_raw_mode();
            let _ = crossterm::execute!(
                std::io::stdout(),
                LeaveAlternateScreen,
                crossterm::event::DisableMouseCapture,
                crossterm::event::DisableBracketedPaste
            );

            let editor_prog = self
                .editor
                .external_editor
                .clone()
                .or_else(|| std::env::var("VISUAL").ok())
                .or_else(|| std::env::var("EDITOR").ok())
                .unwrap_or_else(|| "vi".to_string());

            let parts: Vec<&str> = editor_prog.split_whitespace().collect();
            let (program, editor_args) = parts
                .split_first()
                .map(|(p, a)| (*p, a.to_vec()))
                .unwrap_or(("vi", vec![]));

            let mut command = std::process::Command::new(program);
            for arg in editor_args {
                command.arg(arg);
            }
            if let Some(l) = line_number {
                command.arg(format!("+{}", l));
            }
            command.arg(&temp_file_path);
            let result = command.status();

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
                            if let Err(e) = self.storage.save_note(note_id, &updated_note) {
                                self.set_temporary_status(&format!("Failed to save note: {}", e));
                            } else {
                                self.try_auto_backup(&updated_note.title);
                                self.set_temporary_status_static("Note saved");
                                self.list.folder_cache = None;
                                if let Err(e) = self.refresh_notes() {
                                    self.set_temporary_status(&format!("Refresh failed: {e}"));
                                }
                            }
                        } else {
                            self.set_temporary_status_static("No changes made in external editor.");
                        }
                    } else {
                        self.set_temporary_status_static("Failed to read from temp file.");
                    }
                }
                Ok(status) => {
                    self.set_temporary_status(&format!(
                        "Editor '{}' exited with status: {}",
                        editor_prog, status
                    ));
                }
                Err(e) => {
                    self.set_temporary_status(&format!(
                        "Failed to launch editor '{}': {}",
                        editor_prog, e
                    ));
                }
            }

            if let Ok(len) = std::fs::metadata(&temp_file_path).map(|m| m.len()) {
                let _ = std::fs::write(&temp_file_path, vec![0u8; len as usize]);
            }
            let _ = std::fs::remove_file(&temp_file_path);
        } else {
            self.set_temporary_status_static("Failed to load note for external editor!");
        }
    }

    pub fn collapse_selected_folder(&mut self) {
        if self.list.visual_list.is_empty() {
            return;
        }

        if self.list.visual_index >= self.list.visual_list.len() {
            self.list.visual_index = self.list.visual_list.len().saturating_sub(1);
        }

        match &self.list.visual_list[self.list.visual_index] {
            VisualItem::Folder {
                path, is_expanded, ..
            } => {
                if *is_expanded {
                    self.list.folder_expanded.remove(path);
                    self.refresh_visual_list();
                } else {
                    if !path.is_empty() {
                        let parent_path = if let Some(slash) = path.rfind('/') {
                            &path[..slash]
                        } else {
                            ""
                        };

                        if let Some(idx) = self.list.visual_list.iter().position(|v| {
                            if let VisualItem::Folder { path: p, .. } = v {
                                p == parent_path
                            } else {
                                false
                            }
                        }) {
                            self.list.visual_index = idx;
                        }
                    }
                }
            }
            VisualItem::Note { .. } | VisualItem::CreateNew { .. } => {
                let item_path = match &self.list.visual_list[self.list.visual_index] {
                    VisualItem::Note { summary_idx, .. } => &self.notes[*summary_idx].folder,
                    VisualItem::CreateNew { path, .. } => path,
                    _ => unreachable!(),
                };

                if let Some(idx) = self.list.visual_list.iter().position(|v| {
                    if let VisualItem::Folder { path: p, .. } = v {
                        p == item_path
                    } else {
                        false
                    }
                }) {
                    self.list.visual_index = idx;
                }
            }
        }
    }

    pub fn expand_selected_folder(&mut self) {
        if self.list.visual_list.is_empty() {
            return;
        }

        if self.list.visual_index >= self.list.visual_list.len() {
            self.list.visual_index = self.list.visual_list.len().saturating_sub(1);
        }

        match &self.list.visual_list[self.list.visual_index] {
            VisualItem::Folder {
                path, is_expanded, ..
            } => {
                if !is_expanded {
                    self.list.folder_expanded.insert(path.clone());
                    self.refresh_visual_list();
                } else {
                    if self.list.visual_index + 1 < self.list.visual_list.len() {
                        self.list.visual_index += 1;
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
            && let Some(v_idx) = self.list.visual_list.iter().position(|v| match v {
                VisualItem::Note { summary_idx, .. } => *summary_idx == index,
                _ => false,
            })
        {
            self.list.visual_index = v_idx;
            self.open_selected();
            return true;
        }

        false
    }

    pub fn start_new_note(&mut self, folder: String) {
        let template_manager = self.storage.template_manager();
        if let Some(default_template) = template_manager.load_default() {
            self.start_note_from_template(&default_template, folder);
        } else {
            self.start_blank_note(folder);
        }
    }

    pub fn start_new_note_with_title(&mut self, folder: String, title: String) {
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

        if self.editor.external_editor_enabled {
            let new_note = Note {
                title: String::from("Untitled note"),
                content: String::new(),
                updated_at: now_unix_secs(),
                tags: Vec::new(),
            };
            if self.storage.save_note(&new_id, &new_note).is_ok() {
                self.try_auto_backup(&new_note.title);
                if let Err(e) = self.refresh_notes() {
                    self.set_temporary_status(&format!("Refresh failed: {e}"));
                }
                self.open_note_in_external_editor(&new_id, None);
            }
            return;
        }

        self.mode = ViewMode::Edit;
        self.editor.editing_id = Some(new_id);
        self.editor.title_editor =
            make_title_editor("", self.app_theme.highlight_fg, self.app_theme.highlight_bg);
        self.editor.editor = TextArea::default();
        self.editor.editor.set_cursor_style(
            Style::default()
                .fg(self.app_theme.highlight_fg)
                .bg(self.app_theme.highlight_bg),
        );
        self.editor.editor.set_cursor_line_style(Style::default());
        self.set_default_status();
    }

    pub fn start_blank_note_with_title(&mut self, folder: String, title: String) {
        let mut new_id = self.storage.new_note_id();
        if !folder.is_empty() {
            new_id = format!("{}/{}", folder, new_id);
        }

        if self.editor.external_editor_enabled {
            let new_note = Note {
                title,
                content: String::new(),
                updated_at: now_unix_secs(),
                tags: Vec::new(),
            };
            if self.storage.save_note(&new_id, &new_note).is_ok() {
                self.try_auto_backup(&new_note.title);
                if let Err(e) = self.refresh_notes() {
                    self.set_temporary_status(&format!("Refresh failed: {e}"));
                }
                self.open_note_in_external_editor(&new_id, None);
            }
            return;
        }

        self.mode = ViewMode::Edit;
        self.editor.editing_id = Some(new_id);
        self.editor.title_editor = make_title_editor(
            &title,
            self.app_theme.highlight_fg,
            self.app_theme.highlight_bg,
        );
        self.editor.editor = TextArea::default();
        self.editor.editor.set_cursor_style(
            Style::default()
                .fg(self.app_theme.highlight_fg)
                .bg(self.app_theme.highlight_bg),
        );
        self.editor.editor.set_cursor_line_style(Style::default());
        self.set_default_status();
    }

    pub fn start_note_from_template(&mut self, template: &Template, folder: String) {
        let rendered = template.render();

        let mut new_id = self.storage.new_note_id();
        if !folder.is_empty() {
            new_id = format!("{}/{}", folder, new_id);
        }

        if self.editor.external_editor_enabled {
            let new_note = Note {
                title: rendered
                    .title
                    .clone()
                    .unwrap_or_else(|| String::from("Untitled note")),
                content: rendered.content.clone(),
                updated_at: now_unix_secs(),
                tags: Vec::new(),
            };
            if self.storage.save_note(&new_id, &new_note).is_ok() {
                self.try_auto_backup(&new_note.title);
                if let Err(e) = self.refresh_notes() {
                    self.set_temporary_status(&format!("Refresh failed: {e}"));
                }
                self.open_note_in_external_editor(&new_id, None);
            }
            return;
        }

        self.mode = ViewMode::Edit;
        self.editor.editing_id = Some(new_id);

        self.editor.title_editor = make_title_editor(
            rendered.title.as_deref().unwrap_or(""),
            self.app_theme.highlight_fg,
            self.app_theme.highlight_bg,
        );
        self.editor.editor = text_area_from_content(&rendered.content);

        self.editor.editor.set_cursor_style(
            Style::default()
                .fg(self.app_theme.highlight_fg)
                .bg(self.app_theme.highlight_bg),
        );
        self.editor.editor.set_cursor_line_style(Style::default());

        self.set_default_status();
    }

    pub fn start_note_from_template_with_title(
        &mut self,
        template: &Template,
        folder: String,
        title: String,
    ) {
        let rendered = template.render();

        let mut new_id = self.storage.new_note_id();
        if !folder.is_empty() {
            new_id = format!("{}/{}", folder, new_id);
        }

        if self.editor.external_editor_enabled {
            let new_note = Note {
                title,
                content: rendered.content.clone(),
                updated_at: now_unix_secs(),
                tags: Vec::new(),
            };
            if self.storage.save_note(&new_id, &new_note).is_ok() {
                self.try_auto_backup(&new_note.title);
                if let Err(e) = self.refresh_notes() {
                    self.set_temporary_status(&format!("Refresh failed: {e}"));
                }
                self.open_note_in_external_editor(&new_id, None);
            }
            return;
        }

        self.mode = ViewMode::Edit;
        self.editor.editing_id = Some(new_id);

        self.editor.title_editor = make_title_editor(
            &title,
            self.app_theme.highlight_fg,
            self.app_theme.highlight_bg,
        );
        self.editor.editor = text_area_from_content(&rendered.content);

        self.editor.editor.set_cursor_style(
            Style::default()
                .fg(self.app_theme.highlight_fg)
                .bg(self.app_theme.highlight_bg),
        );
        self.editor.editor.set_cursor_line_style(Style::default());

        self.set_default_status();
    }

    pub fn open_template_popup(&mut self) {
        let template_manager = self.storage.template_manager();
        match template_manager.list() {
            Ok(templates) => {
                let mut input = TextArea::default();
                input.set_style(self.app_theme.bg_style());
                input.set_cursor_line_style(Style::default());
                input.set_placeholder_text("Search templates...");
                self.popups.template = Some(TemplatePopup {
                    all_templates: templates.clone(),
                    filtered_templates: templates,
                    input,
                    selected: 0,
                    focus: crate::popups::TemplatePopupFocus::Search,
                });
            }
            Err(_) => {
                self.set_temporary_status_static("Failed to load templates");
            }
        }
    }

    pub fn close_template_popup(&mut self) {
        self.popups.template = None;
    }

    pub fn select_template(&mut self) {
        let folder = self.get_current_folder_context();
        if let Some(popup) = self.popups.template.take()
            && let Some(summary) = popup.filtered_templates.get(popup.selected)
        {
            let template_manager = self.storage.template_manager();
            if let Ok(template) = template_manager.load(&summary.filename) {
                self.start_note_from_template(&template, folder);
            } else {
                self.set_temporary_status_static("Failed to load selected template");
            }
        }
    }

    pub fn edit_selected_template_from_popup(&mut self) {
        let path = if let Some(popup) = self.popups.template.as_ref()
            && let Some(summary) = popup.filtered_templates.get(popup.selected)
        {
            self.storage
                .template_manager()
                .template_path(&summary.filename)
        } else {
            self.set_temporary_status_static("No template selected");
            return;
        };

        self.popups.template = None;
        self.open_template_path_in_editor(&path);
    }

    fn open_template_path_in_editor(&mut self, path: &std::path::Path) {
        if self.editor.external_editor_enabled {
            self.open_path_in_external_editor(path);
            return;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) => {
                self.set_temporary_status(&format!("Failed to load template: {e}"));
                return;
            }
        };

        self.mode = ViewMode::Edit;
        self.editor.editing_id = None;
        self.editor.template_edit_path = Some(path.to_path_buf());
        self.editor.title_editor = make_title_editor(
            &format!(
                "Template: {}",
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("template")
            ),
            self.app_theme.highlight_fg,
            self.app_theme.highlight_bg,
        );
        self.editor.editor = text_area_from_content(&content);
        self.editor.editor.set_cursor_style(
            Style::default()
                .fg(self.app_theme.highlight_fg)
                .bg(self.app_theme.highlight_bg),
        );
        self.editor.editor.set_cursor_line_style(Style::default());
        self.set_temporary_status_static("Editing template (Esc to save and return)");
    }

    pub fn update_template_popup_filter(&mut self) {
        if let Some(popup) = &mut self.popups.template {
            let query = popup.input.lines()[0].trim().to_lowercase();
            if query.is_empty() {
                popup.filtered_templates = popup.all_templates.clone();
            } else {
                popup.filtered_templates = popup
                    .all_templates
                    .iter()
                    .filter(|t| {
                        t.name.to_lowercase().contains(&query)
                            || t.filename.to_lowercase().contains(&query)
                    })
                    .cloned()
                    .collect();
            }
            if popup.selected >= popup.filtered_templates.len() {
                popup.selected = popup.filtered_templates.len().saturating_sub(1);
            }
        }
    }

    pub fn begin_delete_selected_template_from_popup(&mut self) {
        let (filename, name) = if let Some(popup) = self.popups.template.as_ref()
            && let Some(summary) = popup.filtered_templates.get(popup.selected)
        {
            (summary.filename.clone(), summary.name.clone())
        } else {
            self.set_temporary_status_static("No template selected");
            return;
        };

        self.show_confirm(ConfirmAction::DeleteTemplate { filename, name });
    }

    pub fn confirm_delete_template(&mut self, filename: String) {
        let template_manager = self.storage.template_manager();
        match template_manager.delete(&filename) {
            Ok(()) => {
                if let Some(popup) = &mut self.popups.template {
                    let selected = popup.selected;
                    let focus = popup.focus;
                    match template_manager.list() {
                        Ok(all_templates) => {
                            popup.all_templates = all_templates;
                            popup.focus = focus;
                            self.update_template_popup_filter();
                            if let Some(popup) = &mut self.popups.template {
                                if popup.filtered_templates.is_empty() {
                                    popup.selected = 0;
                                } else {
                                    popup.selected =
                                        selected.min(popup.filtered_templates.len() - 1);
                                }
                            }
                            self.set_temporary_status_static("Template deleted");
                        }
                        Err(e) => {
                            self.set_temporary_status(&format!(
                                "Template deleted but refresh failed: {e}"
                            ));
                        }
                    }
                }
            }
            Err(e) => {
                self.set_temporary_status(&format!("Failed to delete template: {e}"));
            }
        }
    }

    pub fn create_template_from_popup(&mut self) {
        let template_manager = self.storage.template_manager();
        if let Err(e) = template_manager.ensure_dir() {
            self.set_temporary_status(&format!("Failed to prepare templates dir: {e}"));
            return;
        }

        let mut idx = 1usize;
        let filename = loop {
            let candidate = if idx == 1 {
                "new_template".to_string()
            } else {
                format!("new_template_{idx}")
            };
            let path = template_manager.template_path(&candidate);
            if !path.exists() {
                break candidate;
            }
            idx += 1;
        };

        let path = template_manager.template_path(&filename);
        let skeleton = r#"name = "New Template"

[title]
template = "Note - {date}"

[content]
template = """
"""
"#;

        if let Err(e) = std::fs::write(&path, skeleton) {
            self.set_temporary_status(&format!("Failed to create template: {e}"));
            return;
        }

        self.popups.template = None;
        self.open_template_path_in_editor(&path);
    }

    fn open_path_in_external_editor(&mut self, path: &std::path::Path) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture,
            crossterm::event::DisableBracketedPaste
        );

        let editor_prog = self
            .editor
            .external_editor
            .clone()
            .or_else(|| std::env::var("VISUAL").ok())
            .or_else(|| std::env::var("EDITOR").ok())
            .unwrap_or_else(|| "vi".to_string());

        let parts: Vec<&str> = editor_prog.split_whitespace().collect();
        let (program, editor_args) = parts
            .split_first()
            .map(|(p, a)| (*p, a.to_vec()))
            .unwrap_or(("vi", vec![]));

        let mut command = std::process::Command::new(program);
        for arg in editor_args {
            command.arg(arg);
        }
        command.arg(path);
        let result = command.status();

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
                self.set_temporary_status_static("External editor closed");
            }
            Ok(status) => {
                self.set_temporary_status(&format!(
                    "Editor '{}' exited with status: {}",
                    editor_prog, status
                ));
            }
            Err(e) => {
                self.set_temporary_status(&format!(
                    "Failed to launch editor '{}': {}",
                    editor_prog, e
                ));
            }
        }
    }

    pub fn try_auto_backup(&self, note_title: &str) {
        if let Ok(config) = ClinConfig::load() {
            if config.backup.enabled && config.backup.backup_on_save {
                let vault_path = config.effective_storage_path().unwrap_or_else(|_| PathBuf::from("."));
                if let Ok(git_ops) = crate::backup::git_ops::GitOps::init(&vault_path) {
                    if git_ops.has_changes().unwrap_or(false) {
                        let msg = format!("auto: {}", note_title);
                        let _ = git_ops.add_all().and_then(|_| git_ops.commit(&msg));
                        if config.backup.auto_push {
                            if let Some(remote) = &config.backup.remote_name {
                                let _ = git_ops.push(remote);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn try_auto_backup_on_quit(&self) {
        if let Ok(config) = ClinConfig::load() {
            if config.backup.enabled && config.backup.backup_on_quit {
                let vault_path = config.effective_storage_path().unwrap_or_else(|_| PathBuf::from("."));
                if let Ok(git_ops) = crate::backup::git_ops::GitOps::init(&vault_path) {
                    if git_ops.has_changes().unwrap_or(false) {
                        let msg = "auto: backup on quit";
                        let _ = git_ops.add_all().and_then(|_| git_ops.commit(msg));
                        if config.backup.auto_push {
                            if let Some(remote) = &config.backup.remote_name {
                                let _ = git_ops.push(remote);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn autosave(&mut self) {
        let content = self.editor.editor.lines().join("\n");

        if let Some(path) = &self.editor.template_edit_path
            && self.editor.editing_id.is_none()
        {
            if let Err(e) = std::fs::write(path, content) {
                self.set_temporary_status(&format!("Template save failed: {e}"));
            }
            return;
        }

        let mut title = get_title_text(&self.editor.title_editor).trim().to_string();
        if title.is_empty() {
            title = String::from("Untitled note");
        }
        let id = match &self.editor.editing_id {
            Some(id) => id.clone(),
            None => return,
        };

        if id.ends_with(".clin") {
            return;
        }

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
        if let Ok(saved_id) = self.storage.save_note(&id, &note) {
            self.editor.editing_id = Some(saved_id);
            self.try_auto_backup(&note.title);
        }
    }

    pub fn back_to_list(&mut self) {
        if let Some(return_to) = self.return_mode.take() {
            self.editor.editing_id = None;
            self.editor.template_edit_path = None;
            self.editor.title_editor =
                make_title_editor("", self.app_theme.highlight_fg, self.app_theme.highlight_bg);
            self.editor.editor = TextArea::default();
            self.popups.confirm = None;
            self.editor.md_preview_renderer = None;
            self.mode = return_to;
            self.set_default_status();
            return;
        }
        self.mode = ViewMode::List;
        self.editor.editing_id = None;
        self.editor.template_edit_path = None;
        self.list.list_focus = ListFocus::Notes;
        self.editor.title_editor =
            make_title_editor("", self.app_theme.highlight_fg, self.app_theme.highlight_bg);
        self.editor.editor = TextArea::default();
        self.popups.confirm = None;
        self.editor.md_preview_renderer = None;
        if let Err(e) = self.refresh_notes() {
            self.set_temporary_status(&format!("Refresh failed: {e}"));
        }
        self.set_default_status();
    }

    pub fn handle_menu_action(&mut self, action: usize, focus: &mut EditFocus) {
        match action {
            0 => match focus {
                EditFocus::Title => {
                    self.editor.title_editor.copy();
                }
                EditFocus::Body => {
                    self.editor.editor.copy();
                }
                _ => {}
            },
            1 => match focus {
                EditFocus::Title => {
                    self.editor.title_editor.cut();
                }
                EditFocus::Body => {
                    self.editor.editor.cut();
                }
                _ => {}
            },
            2 => match focus {
                EditFocus::Title => {
                    self.editor.title_editor.paste();
                }
                EditFocus::Body => {
                    self.editor.editor.paste();
                }
                _ => {}
            },
            3 => match focus {
                EditFocus::Title => {
                    self.editor.title_editor.select_all();
                }
                EditFocus::Body => {
                    self.editor.editor.select_all();
                }
                _ => {}
            },
            _ => {}
        }
    }

    pub fn begin_delete_selected(&mut self) {
        if !self.list.selected_indices.is_empty() {
            let mut note_ids = Vec::new();
            for &idx in &self.list.selected_indices {
                if let Some(VisualItem::Note { id, .. }) = self.list.visual_list.get(idx) {
                    note_ids.push(id.clone());
                }
            }

            if !note_ids.is_empty() {
                if self.confirm_on_delete {
                    self.show_confirm(ConfirmAction::BulkDeleteNotes { note_ids });
                } else {
                    self.confirm_bulk_delete(note_ids);
                }
                return;
            }
        }

        if self.list.visual_index >= self.list.visual_list.len() {
            self.set_temporary_status_static("No item selected to delete");
            return;
        }

        match &self.list.visual_list[self.list.visual_index] {
            VisualItem::Note { summary_idx, .. } => {
                if let Some(note) = self.notes.get(*summary_idx) {
                    let note_id = note.id.clone();
                    let title = note.title.clone();
                    if self.confirm_on_delete {
                        self.show_confirm(ConfirmAction::DeleteNote { note_id, title });
                    } else {
                        self.confirm_delete_selected(note_id);
                    }
                }
            }
            VisualItem::Folder { path, .. } => {
                if path.is_empty() {
                    self.set_temporary_status_static("Cannot delete Vault root");
                    return;
                }
                if Self::is_virtual_pinned_path(path) {
                    self.set_temporary_status_static("Cannot delete virtual Pinned folder");
                    return;
                }
                let path = path.clone();
                if self.confirm_on_delete {
                    self.show_confirm(ConfirmAction::DeleteFolder { path });
                } else {
                    self.confirm_delete_folder(path);
                }
            }
            _ => {
                self.set_temporary_status_static("Cannot delete this item");
            }
        }
    }

    pub fn show_confirm(&mut self, action: ConfirmAction) {
        let (title, message, detail, confirm_label, is_destructive) = match &action {
            ConfirmAction::DeleteNote { title, .. } => (
                "Move to Trash".into(),
                format!("Move \"{}\" to trash?", title),
                Some("Use Shift+T to view/restore trashed notes.".into()),
                "Trash".into(),
                false,
            ),
            ConfirmAction::DeleteFolder { path } => (
                "Move Folder to Trash".into(),
                format!("Move folder \"{}\" and all contents to trash?", path),
                Some("Use Shift+T to view/restore trashed notes.".into()),
                "Trash".into(),
                false,
            ),
            ConfirmAction::DeleteTag { tag } => (
                "Confirm Delete Tag".into(),
                format!("Delete tag \"{}\"?", tag),
                Some("This will remove the tag from all notes.".into()),
                "Delete".into(),
                true,
            ),
            ConfirmAction::DeleteTemplate { name, .. } => (
                "Delete Template".into(),
                format!("Delete template \"{}\"?", name),
                Some("This removes template file permanently.".into()),
                "Delete".into(),
                true,
            ),
            ConfirmAction::DeleteFromTrash { item } => (
                "Confirm Permanent Delete".into(),
                format!("Permanently delete \"{}\"?", item.name.to_string_lossy()),
                Some("This action cannot be undone.".into()),
                "Delete Forever".into(),
                true,
            ),
            ConfirmAction::EmptyTrash { items } => (
                "Confirm Empty Trash".into(),
                format!("Permanently delete {} note(s)?", items.len()),
                Some("This action cannot be undone.".into()),
                "Empty Trash".into(),
                true,
            ),
            ConfirmAction::BulkDeleteNotes { note_ids } => (
                "Move to Trash".into(),
                format!("Move {} selected note(s) to trash?", note_ids.len()),
                Some("Use Shift+T to view/restore trashed notes.".into()),
                "Trash".into(),
                false,
            ),
            ConfirmAction::QuitApp => (
                "Exit Application".into(),
                "Are you sure you want to quit?".into(),
                None,
                "Quit".into(),
                false,
            ),
        };

        self.popups.confirm = Some(ConfirmPopup {
            action,
            title,
            message,
            detail,
            confirm_label,
            is_destructive,
            selected_button: 1,
        });
    }

    pub fn confirm_action(&mut self) {
        if let Some(popup) = self.popups.confirm.take() {
            match popup.action {
                ConfirmAction::DeleteNote { note_id, .. } => {
                    self.confirm_delete_selected(note_id);
                }
                ConfirmAction::DeleteFolder { path } => {
                    self.confirm_delete_folder(path);
                }
                ConfirmAction::DeleteTag { tag } => {
                    self.confirm_delete_tag(tag);
                }
                ConfirmAction::DeleteTemplate { filename, .. } => {
                    self.confirm_delete_template(filename);
                }
                ConfirmAction::DeleteFromTrash { item } => {
                    self.confirm_delete_from_trash(item);
                }
                ConfirmAction::EmptyTrash { items } => {
                    self.confirm_empty_trash(items);
                }
                ConfirmAction::BulkDeleteNotes { note_ids } => {
                    self.confirm_bulk_delete(note_ids);
                }
                ConfirmAction::QuitApp => {
                    self.should_quit = true;
                }
            }
        }
    }

    pub fn cancel_confirm(&mut self) {
        self.popups.confirm = None;
    }

    pub fn confirm_popup_select_confirm(&mut self) {
        if let Some(popup) = &mut self.popups.confirm {
            popup.selected_button = 0;
        }
    }

    pub fn confirm_popup_select_cancel(&mut self) {
        if let Some(popup) = &mut self.popups.confirm {
            popup.selected_button = 1;
        }
    }

    pub fn confirm_popup_toggle_button(&mut self) {
        if let Some(popup) = &mut self.popups.confirm {
            popup.selected_button = (popup.selected_button + 1) % 2;
        }
    }

    pub fn confirm_popup_activate(&mut self) {
        let is_confirm = self
            .popups
            .confirm
            .as_ref()
            .map(|p| p.selected_button == 0)
            .unwrap_or(false);
        if is_confirm {
            self.confirm_action();
        } else {
            self.cancel_confirm();
        }
    }

    pub fn confirm_delete_selected(&mut self, id: String) {
        match self.storage.trash_note(&id) {
            Ok(()) => {
                if let Err(e) = self.refresh_notes() {
                    self.set_temporary_status(&format!("Refresh failed: {e}"));
                }
                if self.list.visual_index >= self.list.visual_list.len()
                    && !self.list.visual_list.is_empty()
                {
                    self.list.visual_index = self.list.visual_list.len() - 1;
                }
                self.set_temporary_status_static("Note moved to trash");
            }
            Err(err) => {
                self.set_temporary_status(&format!("Move to trash failed: {err:#}"));
            }
        }
    }

    pub fn confirm_delete_folder(&mut self, path: String) {
        match self.storage.trash_folder(&path) {
            Ok(()) => {
                self.list.folder_cache = None;
                if let Err(e) = self.refresh_notes() {
                    self.set_temporary_status(&format!("Refresh failed: {e}"));
                }
                if self.list.visual_index >= self.list.visual_list.len()
                    && !self.list.visual_list.is_empty()
                {
                    self.list.visual_index = self.list.visual_list.len() - 1;
                }
                self.set_temporary_status_static("Folder moved to trash");
            }
            Err(e) => {
                self.set_temporary_status(&format!("Failed to trash folder: {e}"));
            }
        }
    }

    pub fn begin_create_folder(&mut self) {
        let parent_path = self.get_current_folder_context();
        if Self::is_virtual_pinned_path(&parent_path) {
            self.set_temporary_status_static("Cannot create folder inside virtual Pinned");
            return;
        }
        let mut input = TextArea::default();
        input.set_cursor_line_style(ratatui::style::Style::default());
        let title = if parent_path.is_empty() {
            "Create Folder - Esc to cancel, Enter to save".to_string()
        } else {
            format!(
                "Create Folder in '{}' - Esc to cancel, Enter to save",
                parent_path
            )
        };
        input.set_style(self.app_theme.bg_style());
        input.set_block(
            ratatui::widgets::Block::default()
                .style(self.app_theme.bg_style())
                .borders(ratatui::widgets::Borders::ALL)
                .title(title),
        );
        self.popups.folder = Some(FolderPopup {
            mode: FolderPopupMode::Create { parent_path },
            input,
        });
    }

    pub fn begin_rename_folder(&mut self) {
        if let Some(VisualItem::Folder { path, .. }) =
            self.list.visual_list.get(self.list.visual_index)
        {
            if path.is_empty() {
                self.set_temporary_status_static("Cannot rename Vault root");
                return;
            }
            if Self::is_virtual_pinned_path(path) {
                self.set_temporary_status_static("Cannot rename virtual Pinned folder");
                return;
            }
            let mut input = TextArea::default();
            input.set_cursor_line_style(ratatui::style::Style::default());
            input.insert_str(path);
            input.set_style(self.app_theme.bg_style());
            input.set_block(
                ratatui::widgets::Block::default()
                    .style(self.app_theme.bg_style())
                    .borders(ratatui::widgets::Borders::ALL)
                    .title("Rename Folder - Esc to cancel, Enter to save"),
            );
            self.popups.folder = Some(FolderPopup {
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
        if let Some(popup) = self.popups.folder.take() {
            let text = popup.input.lines().join("");
            let text = text.trim();
            if text.is_empty() {
                self.set_temporary_status_static("Folder name cannot be empty");
                return;
            }

            match &popup.mode {
                FolderPopupMode::Create { parent_path } => {
                    if Self::is_virtual_pinned_path(parent_path) {
                        self.set_temporary_status_static(
                            "Cannot create folder inside virtual Pinned",
                        );
                        return;
                    }
                    let full_path = if parent_path.is_empty() {
                        text.to_string()
                    } else {
                        format!("{}/{}", parent_path, text)
                    };
                    if let Err(e) = self.storage.create_folder(&full_path) {
                        self.set_temporary_status(&format!("Failed to create folder: {e}"));
                    } else {
                        self.list.folder_cache = None;
                        if let Err(e) = self.refresh_notes() {
                            self.set_temporary_status(&format!("Refresh failed: {e}"));
                        }
                        self.set_temporary_status_static("Folder created");
                    }
                }
                FolderPopupMode::Rename { old_path } => {
                    if Self::is_virtual_pinned_path(old_path) {
                        self.set_temporary_status_static("Cannot rename virtual Pinned folder");
                        return;
                    }
                    if let Err(e) = self.storage.rename_folder(old_path, text) {
                        self.set_temporary_status(&format!("Failed to rename folder: {e}"));
                    } else {
                        self.list.folder_cache = None;
                        if let Err(e) = self.refresh_notes() {
                            self.set_temporary_status(&format!("Refresh failed: {e}"));
                        }
                        self.set_temporary_status_static("Folder renamed");
                    }
                }
            }
        }
    }

    pub fn confirm_bulk_delete(&mut self, note_ids: Vec<String>) {
        let mut failed = 0;
        for id in note_ids {
            if self.storage.trash_note(&id).is_err() {
                failed += 1;
            }
        }

        if let Err(e) = self.refresh_notes() {
            self.set_temporary_status(&format!("Refresh failed: {e}"));
        }

        self.list.selected_indices.clear();
        self.list.list_mode = ListMode::Normal;

        if failed > 0 {
            self.set_temporary_status(&format!("Failed to trash {} note(s)", failed));
        } else {
            self.set_temporary_status_static("Selected notes moved to trash");
        }
    }

    pub fn begin_move_note(&mut self) {
        if let Some(VisualItem::Note { summary_idx, .. }) =
            self.list.visual_list.get(self.list.visual_index)
        {
            let note = &self.notes[*summary_idx];
            if let Ok(folders) = self.storage.list_folders() {
                let mut all_folders = vec!["".to_string()];
                all_folders.extend(folders);
                let mut input = TextArea::default();
                input.set_cursor_line_style(ratatui::style::Style::default());
                input.set_placeholder_text("Search folders...");
                self.popups.folder_picker = Some(FolderPicker {
                    mode: FolderPickerMode::MoveNote {
                        note_id: note.id.clone(),
                    },
                    filtered_folders: all_folders.clone(),
                    all_folders,
                    selected: 0,
                    input,
                    focus: FolderPickerFocus::Search,
                });
            } else {
                self.set_temporary_status_static("Failed to list folders");
            }
        } else {
            self.set_temporary_status_static("Select a note to move");
        }
    }

    pub fn begin_move_folder(&mut self) {
        if let Some(VisualItem::Folder { path, .. }) =
            self.list.visual_list.get(self.list.visual_index)
        {
            if Self::is_virtual_pinned_path(path) {
                self.set_temporary_status_static("Cannot move virtual Pinned folder");
                return;
            }
            let folder_path = path.clone();
            if let Ok(folders) = self.storage.list_folders() {
                let mut all_folders = vec!["".to_string()];
                all_folders.extend(
                    folders.into_iter().filter(|f| {
                        f != &folder_path && !f.starts_with(&format!("{}/", folder_path))
                    }),
                );

                let mut input = TextArea::default();
                input.set_cursor_line_style(ratatui::style::Style::default());
                input.set_placeholder_text("Search folders...");
                self.popups.folder_picker = Some(FolderPicker {
                    mode: FolderPickerMode::MoveFolder { folder_path },
                    filtered_folders: all_folders.clone(),
                    all_folders,
                    selected: 0,
                    input,
                    focus: FolderPickerFocus::Search,
                });
            } else {
                self.set_temporary_status_static("Failed to list folders");
            }
        } else {
            self.set_temporary_status_static("Select a folder to move");
        }
    }

    pub fn begin_move(&mut self) {
        if !self.list.selected_indices.is_empty() {
            let mut note_ids = Vec::new();
            for &idx in &self.list.selected_indices {
                if let Some(VisualItem::Note { id, .. }) = self.list.visual_list.get(idx) {
                    note_ids.push(id.clone());
                }
            }

            if !note_ids.is_empty() {
                if let Ok(folders) = self.storage.list_folders() {
                    let mut all_folders = vec!["".to_string()];
                    all_folders.extend(folders);
                    let mut input = TextArea::default();
                    input.set_cursor_line_style(ratatui::style::Style::default());
                    input.set_placeholder_text("Search folders...");
                    self.popups.folder_picker = Some(FolderPicker {
                        mode: FolderPickerMode::BulkMoveNotes { note_ids },
                        filtered_folders: all_folders.clone(),
                        all_folders,
                        selected: 0,
                        input,
                        focus: FolderPickerFocus::Search,
                    });
                    return;
                }
            }
        }

        match self.list.visual_list.get(self.list.visual_index) {
            Some(VisualItem::Note { .. }) => self.begin_move_note(),
            Some(VisualItem::Folder { .. }) => self.begin_move_folder(),
            _ => self.set_temporary_status_static("Nothing selected"),
        }
    }

    pub fn confirm_move(&mut self) {
        if let Some(picker) = self.popups.folder_picker.take()
            && let Some(target_folder) = picker.filtered_folders.get(picker.selected)
        {
            match picker.mode {
                FolderPickerMode::MoveNote { note_id } => {
                    if let Err(e) = self.storage.move_note(&note_id, target_folder) {
                        self.set_temporary_status(&format!("Failed to move note: {e}"));
                    } else {
                        self.list.folder_cache = None;
                        if let Err(e) = self.refresh_notes() {
                            self.set_temporary_status(&format!("Refresh failed: {e}"));
                        }
                        self.set_temporary_status_static("Note moved");
                    }
                }
                FolderPickerMode::CopyNote { note_id } => {
                    if let Err(e) = self.storage.duplicate_note(&note_id, target_folder) {
                        self.set_temporary_status(&format!("Failed to copy note: {e}"));
                    } else {
                        self.list.folder_cache = None;
                        if let Err(e) = self.refresh_notes() {
                            self.set_temporary_status(&format!("Refresh failed: {e}"));
                        }
                        self.set_temporary_status_static("Note copied");
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
                        if self.list.folder_expanded.remove(&folder_path) {
                            self.list.folder_expanded.insert(new_path);
                        }
                        self.list.folder_cache = None;
                        if let Err(e) = self.refresh_notes() {
                            self.set_temporary_status(&format!("Refresh failed: {e}"));
                        }
                        self.set_temporary_status_static("Folder moved");
                    }
                }
                FolderPickerMode::BulkMoveNotes { note_ids } => {
                    let mut failed = 0;
                    for id in note_ids {
                        if self.storage.move_note(&id, target_folder).is_err() {
                            failed += 1;
                        }
                    }

                    self.list.folder_cache = None;
                    if let Err(e) = self.refresh_notes() {
                        self.set_temporary_status(&format!("Refresh failed: {e}"));
                    }
                    self.list.selected_indices.clear();
                    self.list.list_mode = ListMode::Normal;

                    if failed > 0 {
                        self.set_temporary_status(&format!("Failed to move {} note(s)", failed));
                    } else {
                        self.set_temporary_status_static("Selected notes moved");
                    }
                }
            }
        }
    }

    pub fn update_folder_picker_filter(&mut self) {
        if let Some(picker) = &mut self.popups.folder_picker {
            let query = picker.input.lines().join("").trim().to_lowercase();
            if query.is_empty() {
                picker.filtered_folders = picker.all_folders.clone();
            } else {
                picker.filtered_folders = picker
                    .all_folders
                    .iter()
                    .filter(|folder| folder.to_lowercase().contains(&query))
                    .cloned()
                    .collect();
            }
            if picker.selected >= picker.filtered_folders.len() {
                picker.selected = picker.filtered_folders.len().saturating_sub(1);
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
        if let Some(VisualItem::Note { summary_idx, .. }) =
            self.list.visual_list.get(self.list.visual_index)
        {
            let note = &self.notes[*summary_idx];
            let current_tags = note.tags.clone();
            let all_tags = self.collect_live_tags();

            let mut input = TextArea::default();
            input.set_cursor_line_style(ratatui::style::Style::default());
            input.set_style(self.app_theme.bg_style());
            input.set_placeholder_text("Add tags...");
            input.insert_str(current_tags.join(", "));

            self.popups.tag = Some(TagPopup {
                note_id: note.id.clone(),
                input,
                all_tags,
                suggestions: Vec::new(),
                suggestion_index: 0,
                focus: crate::popups::TagPopupFocus::Input,
                all_tags_selected: 0,
            });
            self.update_tag_suggestions();
        } else {
            self.set_temporary_status_static("Select a note to manage tags");
        }
    }

    pub fn confirm_manage_tags(&mut self) {
        if let Some(popup) = self.popups.tag.take() {
            let text = popup.input.lines().join("");
            let tags: Vec<String> = text
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            if let Ok(mut note) = self.storage.load_note(&popup.note_id) {
                note.tags = tags;
                if let Err(e) = self.storage.save_note(&popup.note_id, &note) {
                    self.set_temporary_status(&format!("Failed to save tags: {e}"));
                } else {
                    self.try_auto_backup(&note.title);
                    if let Err(e) = self.refresh_notes() {
                        self.set_temporary_status(&format!("Refresh failed: {e}"));
                    }
                    self.set_temporary_status_static("Tags updated");
                }
            } else {
                self.set_temporary_status_static("Failed to load note to update tags");
            }
        }
    }

    fn get_current_tag_word(input: &str) -> &str {
        input.rsplit(',').next().map(|s| s.trim()).unwrap_or("")
    }

    pub fn update_tag_suggestions(&mut self) {
        if let Some(popup) = &mut self.popups.tag {
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
                popup.suggestions = popup
                    .all_tags
                    .iter()
                    .filter(|tag| {
                        let tag_lower = tag.to_lowercase();
                        tag_lower.starts_with(&current_word) && !entered_tags.contains(&tag_lower)
                    })
                    .cloned()
                    .collect();
            }
            popup.suggestion_index = 0;
        }
    }

    pub fn cycle_tag_suggestion(&mut self) {
        if let Some(popup) = &mut self.popups.tag
            && !popup.suggestions.is_empty()
        {
            popup.suggestion_index = (popup.suggestion_index + 1) % popup.suggestions.len();
        }
    }

    pub fn accept_tag_suggestion(&mut self) {
        if let Some(popup) = &mut self.popups.tag
            && let Some(suggestion) = popup.suggestions.get(popup.suggestion_index).cloned()
        {
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
                popup.input.insert_str(format!("{}, ", suggestion));
            }

            popup.suggestions.clear();
            popup.suggestion_index = 0;
        }
    }

    pub fn begin_delete_tag_with_name(&mut self, tag: String) {
        
        let count = self
            .storage
            .list_note_ids()
            .ok()
            .map(|ids| {
                ids.iter()
                    .filter(|id| {
                        self.storage
                            .load_note(id)
                            .ok()
                            .map_or(false, |n| n.tags.contains(&tag))
                    })
                    .count()
            })
            .unwrap_or(0);

        let detail = if count == 1 {
            "Will remove tag from 1 note.".to_string()
        } else {
            format!("Will remove tag from {} notes.", count)
        };

        self.popups.confirm = Some(ConfirmPopup {
            action: ConfirmAction::DeleteTag { tag: tag.clone() },
            title: "Confirm Delete Tag".into(),
            message: format!("Delete tag \"{}\"?", tag),
            detail: Some(detail),
            confirm_label: "Delete".into(),
            is_destructive: true,
            selected_button: 1,
        });
    }

    pub fn confirm_delete_tag(&mut self, tag: String) {
        let mut count = 0;
        if let Ok(note_ids) = self.storage.list_note_ids() {
            for note_id in note_ids {
                if let Ok(mut note) = self.storage.load_note(&note_id)
                    && note.tags.contains(&tag)
                {
                    note.tags.retain(|t| t != &tag);
                    if self.storage.save_note(&note_id, &note).is_ok() {
                        self.try_auto_backup(&note.title);
                    }
                    count += 1;
                }
            }
        }

        self.set_temporary_status(&format!("Deleted '{}' from {} note(s)", tag, count));
        if let Err(e) = self.refresh_notes() {
            self.set_temporary_status(&format!("Refresh failed: {e}"));
        }
        let live_tags = self.collect_live_tags();

        if let Some(popup) = &mut self.popups.tag {
            popup.all_tags = live_tags;
            let text = popup.input.lines().join("");

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

    pub fn apply_tag_to_selected(&mut self, tag: String) {
        let mut count = 0;
        let indices: Vec<usize> = self.list.selected_indices.iter().copied().collect();

        for &idx in &indices {
            if let crate::list_view::VisualItem::Note { summary_idx, .. } = self
                .list
                .visual_list
                .get(idx)
                .unwrap_or(&crate::list_view::VisualItem::CreateNew {
                    path: String::new(),
                    depth: 0,
                })
            {
                let note = &self.notes[*summary_idx];
                let note_id = note.id.clone();
                if let Ok(mut loaded) = self.storage.load_note(&note_id) {
                    if !loaded.tags.contains(&tag) {
                        loaded.tags.push(tag.clone());
                    }
                    if self.storage.save_note(&note_id, &loaded).is_ok() {
                        self.try_auto_backup(&loaded.title);
                        count += 1;
                    }
                }
            }
        }

        self.list.selected_indices.clear();
        self.list.list_mode = crate::list_view::ListMode::Normal;

        if let Err(e) = self.refresh_notes() {
            self.set_temporary_status(&format!("Refresh failed: {e}"));
            return;
        }

        self.set_temporary_status(&format!("Tag '{}' applied to {} note(s)", tag, count));
    }

    pub fn open_selected_note_location(&mut self) {
        if self.list.visual_index >= self.list.visual_list.len() {
            self.set_temporary_status_static("No note selected for location");
            return;
        }

        let summary_idx = match &self.list.visual_list[self.list.visual_index] {
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

    pub fn toggle_external_editor_mode(&mut self) {
        self.editor.external_editor_enabled = !self.editor.external_editor_enabled;
        self.set_default_status();
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.external_editor_enabled = self.editor.external_editor_enabled;
            let _ = config.save();
        }
    }

    pub fn open_help_page(&mut self) {
        self.open_help_page_with_tab(HelpTab::Notes);
    }

    pub fn open_help_page_with_tab(&mut self, tab: HelpTab) {
        self.mode = ViewMode::Help;
        self.help_tab = tab;
        self.help_scroll = 0;
        self.help_tab_scroll.insert(tab, 0);
        self.status = Cow::Borrowed(HELP_PAGE_HINTS);
        self.status_until = None;
        self.list.help_text_cache = None;
    }

    pub fn close_help_page(&mut self) {
        self.mode = self.return_mode.take().unwrap_or(ViewMode::List);
        self.help_scroll = 0;
        self.help_tab = HelpTab::Notes;
        self.help_tab_scroll.clear();
        self.list.help_text_cache = None;
        self.set_default_status();
    }

    pub fn open_graph_view(&mut self) {
        self.return_mode = Some(self.mode);
        self.mode = ViewMode::Graph;
    }

    pub fn open_backup_view(&mut self) {
        self.return_mode = Some(self.mode);
        self.mode = ViewMode::Backup;
    }

    pub fn open_draw_view(&mut self) {
        self.return_mode = Some(self.mode);
        self.mode = ViewMode::Draw;
    }

    pub fn close_draw_view(&mut self) {
        self.editor.editing_id = None;
        self.mode = self.return_mode.take().unwrap_or(ViewMode::List);
        if let Err(e) = self.refresh_notes() {
            self.set_temporary_status(&format!("Refresh failed: {e}"));
        }
        self.set_default_status();
    }

    pub fn open_canvas_view(&mut self) {
        if let Some(VisualItem::Note { id, .. }) = self.list.visual_list.get(self.list.visual_index)
        {
            let path = self.storage.note_path(id);
            if let Ok(state) = crate::pinstar::state::PinstarState::load(&path) {
                self.canvas_state = Some(state);
                self.return_mode = Some(self.mode);
                self.mode = ViewMode::Canvas;
                self.editor.editing_id = Some(id.clone());
                self.set_default_status();
            } else {
                self.set_temporary_status_static("Failed to load .canvas file!");
            }
        }
    }

    pub fn close_canvas_view(&mut self) {
        self.editor.editing_id = None;
        self.canvas_state = None;
        self.mode = self.return_mode.take().unwrap_or(ViewMode::List);
        if let Err(e) = self.refresh_notes() {
            self.set_temporary_status(&format!("Refresh failed: {e}"));
        }
        self.set_default_status();
    }

    pub fn get_selected_note_id(&self) -> Option<String> {
        if let Some(id) = &self.editor.editing_id {
            return Some(id.clone());
        }
        if let Some(VisualItem::Note { id, .. }) = self.list.visual_list.get(self.list.visual_index)
        {
            Some(id.clone())
        } else {
            None
        }
    }

    pub fn open_note_from_graph(&mut self, note_id: &str) {
        if note_id.ends_with(".clin") {
            self.set_temporary_status_static("Cannot open encrypted notes. Decrypt first.");
            return;
        }
        self.return_mode = Some(ViewMode::Graph);
        if self.editor.external_editor_enabled {
            self.open_note_in_external_editor(note_id, None);
            self.mode = self.return_mode.take().unwrap_or(ViewMode::List);
        } else {
            self.load_and_open_note(note_id, None);
        }
    }

    pub fn default_status_text(&self) -> &'static str {
        match self.mode {
            ViewMode::List => LIST_HELP_HINTS,
            ViewMode::Edit => EDIT_HELP_HINTS,
            ViewMode::Help => HELP_PAGE_HINTS,
            ViewMode::Graph => "Graph View · Esc: back · +/-: zoom · L: labels · a: fit",
            ViewMode::Draw => {
                "Draw View · Esc: back · d: draw · s: shape · t: text · e: erase · Ctrl+S: save"
            }
            ViewMode::Canvas => {
                "Canvas View · Esc: back · Arrows: pan · i/Enter: edit · Ctrl+S: save"
            }
            ViewMode::Backup => "Backup · Esc: back · s: commit · p: push · r: refresh · /: settings",
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

    pub fn switch_help_tab(&mut self, tab: HelpTab) {
        if tab == self.help_tab {
            return;
        }
        let current_scroll = self.help_scroll;
        self.help_tab_scroll.insert(self.help_tab, current_scroll);
        self.help_tab = tab;
        self.help_scroll = self.help_tab_scroll.get(&tab).copied().unwrap_or(0);
        self.list.help_text_cache = None;
    }

    pub fn get_help_text(&mut self) -> &Text<'static> {
        let tab = self.help_tab;
        if self.list.help_text_cache.is_none() {
            self.list.help_text_cache = Some(crate::ui::help_text_for_tab(
                tab,
                &self.keybinds,
                &self.app_theme,
            ));
        }
        self.list.help_text_cache.as_ref().unwrap()
    }

    pub fn begin_create_note(&mut self) {
        let folder = self.get_current_folder_context();
        self.begin_create_note_in_folder(folder);
    }

    pub fn begin_create_note_in_folder(&mut self, folder: String) {
        if Self::is_virtual_pinned_path(&folder) {
            self.set_temporary_status_static("Cannot create note inside virtual Pinned");
            return;
        }
        let mut input = TextArea::default();
        input.set_cursor_line_style(ratatui::style::Style::default());
        input.set_style(self.app_theme.bg_style());
        input.set_block(
            ratatui::widgets::Block::default()
                .style(self.app_theme.bg_style())
                .borders(ratatui::widgets::Borders::ALL)
                .title("New Note Name - Esc to cancel, Enter to create"),
        );
        self.popups.note_create = Some(NoteCreatePopup { folder, input });
    }

    pub fn confirm_create_note(&mut self) {
        if let Some(popup) = self.popups.note_create.take() {
            let mut title = popup.input.lines().join("");
            title = title.trim().to_string();
            if title.is_empty() {
                title = String::from("Untitled note");
            }
            self.start_new_note_with_title(popup.folder, title);
        }
    }

    pub fn begin_create_draw(&mut self) {
        let folder = self.get_current_folder_context();
        if Self::is_virtual_pinned_path(&folder) {
            self.set_temporary_status_static("Cannot create drawing inside virtual Pinned");
            return;
        }
        let mut input = TextArea::default();
        input.set_cursor_line_style(ratatui::style::Style::default());
        input.set_style(self.app_theme.bg_style());
        input.set_block(
            ratatui::widgets::Block::default()
                .style(self.app_theme.bg_style())
                .borders(ratatui::widgets::Borders::ALL)
                .title("New Drawing Name - Esc to cancel, Enter to create"),
        );
        self.popups.draw_create = Some(NoteCreatePopup { folder, input });
    }

    pub fn confirm_create_draw(&mut self) {
        if let Some(popup) = self.popups.draw_create.take() {
            let mut title = popup.input.lines().join("");
            title = title.trim().to_string();
            if title.is_empty() {
                title = String::from("Untitled drawing");
            }

            let canvas_id = if popup.folder.is_empty() {
                format!("{}.draw", title)
            } else {
                format!("{}/{}.draw", popup.folder, title)
            };

            self.return_mode = Some(self.mode);
            self.mode = ViewMode::Draw;

            self.editor.editing_id = Some(canvas_id);
        }
    }

    pub fn begin_create_canvas(&mut self) {
        let folder = self.get_current_folder_context();
        if Self::is_virtual_pinned_path(&folder) {
            self.set_temporary_status_static("Cannot create canvas inside virtual Pinned");
            return;
        }
        let mut input = TextArea::default();
        input.set_cursor_line_style(ratatui::style::Style::default());
        input.set_style(self.app_theme.bg_style());
        input.set_block(
            ratatui::widgets::Block::default()
                .style(self.app_theme.bg_style())
                .borders(ratatui::widgets::Borders::ALL)
                .title("New Canvas Name - Esc to cancel, Enter to create"),
        );
        self.popups.canvas_create = Some(NoteCreatePopup { folder, input });
    }

    pub fn confirm_create_canvas(&mut self) {
        if let Some(popup) = self.popups.canvas_create.take() {
            let mut title = popup.input.lines().join("");
            title = title.trim().to_string();
            if title.is_empty() {
                title = String::from("Untitled canvas");
            }

            let canvas_id = if popup.folder.is_empty() {
                format!("{}.canvas", title)
            } else {
                format!("{}/{}.canvas", popup.folder, title)
            };

            let path = self.storage.note_path(&canvas_id);
            if !path.exists() {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let data = crate::pinstar::data::CanvasData {
                    nodes: vec![],
                    edges: vec![],
                };
                if let Ok(content) = serde_json::to_string_pretty(&data)
                    && let Err(e) = std::fs::write(&path, content)
                {
                    self.set_temporary_status(&format!("Failed to write canvas file: {}", e));
                    return;
                }
            }

            self.return_mode = Some(self.mode);
            self.mode = ViewMode::Canvas;
            self.editor.editing_id = Some(canvas_id);
            if let Ok(state) = crate::pinstar::state::PinstarState::load(&path) {
                self.canvas_state = Some(state);
            }
            self.set_default_status();
        }
    }

    pub fn begin_rename_note(&mut self) {
        if let Some(VisualItem::Note {
            summary_idx, id, ..
        }) = self.list.visual_list.get(self.list.visual_index).cloned()
        {
            let note = &self.notes[summary_idx];
            let mut input = TextArea::default();
            input.set_cursor_line_style(ratatui::style::Style::default());
            input.insert_str(&note.title);
            input.set_style(self.app_theme.bg_style());
            input.set_block(
                ratatui::widgets::Block::default()
                    .style(self.app_theme.bg_style())
                    .borders(ratatui::widgets::Borders::ALL)
                    .title("Rename Note - Esc to cancel, Enter to save"),
            );
            self.popups.note_rename = Some(NoteRenamePopup { note_id: id, input });
        } else {
            self.set_temporary_status_static("Select a note to rename");
        }
    }

    pub fn confirm_rename_note(&mut self) {
        if let Some(popup) = self.popups.note_rename.take() {
            let new_title = popup.input.lines().join("");
            let new_title = new_title.trim();
            if new_title.is_empty() {
                self.set_temporary_status_static("Title cannot be empty");
                return;
            }
            match self.storage.rename_note(&popup.note_id, new_title) {
                Ok(_) => {
                    if let Err(e) = self.refresh_notes() {
                        self.set_temporary_status(&format!("Refresh failed: {e}"));
                    }
                    self.set_temporary_status_static("Note renamed");
                }
                Err(e) => {
                    self.set_temporary_status(&format!("Failed to rename: {e}"));
                }
            }
        }
    }

    pub fn duplicate_note(&mut self) {
        if let Some(VisualItem::Note { id, .. }) =
            self.list.visual_list.get(self.list.visual_index).cloned()
        {
            if let Ok(folders) = self.storage.list_folders() {
                let mut all_folders = vec!["".to_string()];
                all_folders.extend(folders);
                let mut input = ratatui_textarea::TextArea::default();
                input.set_cursor_line_style(ratatui::style::Style::default());
                input.set_placeholder_text("Search folders...");
                self.popups.folder_picker = Some(crate::popups::FolderPicker {
                    mode: crate::popups::FolderPickerMode::CopyNote { note_id: id },
                    filtered_folders: all_folders.clone(),
                    all_folders,
                    selected: 0,
                    input,
                    focus: crate::popups::FolderPickerFocus::Search,
                });
            } else {
                self.set_temporary_status_static("Failed to list folders");
            }
        } else {
            self.set_temporary_status_static("Select a note to duplicate");
        }
    }

    pub fn toggle_pin(&mut self) {
        if let Some(VisualItem::Note { id, .. }) =
            self.list.visual_list.get(self.list.visual_index).cloned()
        {
            match self.storage.toggle_pin(&id) {
                Ok(pinned) => {
                    if let Err(e) = self.refresh_notes() {
                        self.set_temporary_status(&format!("Refresh failed: {e}"));
                    }
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

    pub fn cycle_sort(&mut self) {
        match (self.list.sort_field, self.list.sort_order) {
            (SortField::Modified, SortOrder::Descending) => {
                self.list.sort_field = SortField::Modified;
                self.list.sort_order = SortOrder::Ascending;
            }
            (SortField::Modified, SortOrder::Ascending) => {
                self.list.sort_field = SortField::Title;
                self.list.sort_order = SortOrder::Ascending;
            }
            (SortField::Title, SortOrder::Ascending) => {
                self.list.sort_field = SortField::Title;
                self.list.sort_order = SortOrder::Descending;
            }
            (SortField::Title, SortOrder::Descending) => {
                self.list.sort_field = SortField::Modified;
                self.list.sort_order = SortOrder::Descending;
            }
        }
        if let Err(e) = self.refresh_notes() {
            self.set_temporary_status(&format!("Refresh failed: {e}"));
        }
        let sort_desc = match (self.list.sort_field, self.list.sort_order) {
            (SortField::Modified, SortOrder::Descending) => "Sort: Modified (newest)",
            (SortField::Modified, SortOrder::Ascending) => "Sort: Modified (oldest)",
            (SortField::Title, SortOrder::Ascending) => "Sort: Title (A-Z)",
            (SortField::Title, SortOrder::Descending) => "Sort: Title (Z-A)",
        };
        self.set_temporary_status_static(sort_desc);
    }

    pub fn begin_search(&mut self) {
        let mut input = TextArea::default();
        input.set_style(self.app_theme.bg_style());
        input.set_cursor_line_style(Style::default());
        input.set_placeholder_text("Search notes...");

        self.popups.search = Some(SearchPopup {
            input,
            focus: crate::popups::SearchFocus::Input,
            title_results: Vec::new(),
            title_result_indices: Vec::new(),
            title_selected: 0,
            grep_results: Vec::new(),
            grep_result_indices: Vec::new(),
            grep_is_header: Vec::new(),
            grep_expanded: std::collections::HashSet::new(),
            grep_selected: 0,
            original_index: self.list.visual_index,
            original_folder_expanded: self.list.folder_expanded.clone(),
        });
    }

    fn jump_to_note_index(&mut self, note_idx: usize) {
        if let Some(note) = self.notes.get(note_idx)
            && !note.folder.is_empty()
        {
            let mut path = String::new();
            for part in note.folder.split('/') {
                if !path.is_empty() {
                    path.push('/');
                }
                path.push_str(part);
                self.list.folder_expanded.insert(path.clone());
            }
        }

        self.refresh_visual_list();

        for (idx, item) in self.list.visual_list.iter().enumerate() {
            if let VisualItem::Note { summary_idx, .. } = item
                && *summary_idx == note_idx
            {
                self.list.visual_index = idx;
                self.request_preview_update();
                return;
            }
        }
    }

    pub fn update_search(&mut self) {
        let Some(popup) = self.popups.search.as_ref() else {
            return;
        };
        let query_text = popup.input.lines().join("");
        let parsed = parse_search_query(&query_text);
        let title_query = parsed.text.trim().to_lowercase();
        let grep_query = parsed.grep_text.trim().to_lowercase();

        let no_filters = title_query.is_empty()
            && grep_query.is_empty()
            && parsed.folder_filter.is_none()
            && !parsed.pinned_only
            && parsed.tag_filter.is_none();
        if no_filters {
            if let Some(popup) = &mut self.popups.search {
                popup.title_results.clear();
                popup.title_result_indices.clear();
                popup.title_selected = 0;
                popup.grep_results.clear();
                popup.grep_result_indices.clear();
                popup.grep_is_header.clear();
                popup.grep_expanded.clear();
                popup.grep_selected = 0;
            }
            return;
        }

        let mut title_results = Vec::new();
        let mut title_result_indices = Vec::new();
        let mut grep_results = Vec::new();
        let mut grep_result_indices = Vec::new();
        let mut grep_is_header = Vec::new();

        for (note_idx, note) in self.notes.iter().enumerate() {
            
            if parsed.pinned_only && !note.pinned {
                continue;
            }
            
            if let Some(ref folder) = parsed.folder_filter {
                let matches_folder = if folder.is_empty() {
                    note.folder.is_empty()
                } else {
                    note.folder == *folder
                        || note.folder.starts_with(&format!("{folder}/"))
                };
                if !matches_folder {
                    continue;
                }
            }
            
            if let Some(ref tags) = parsed.tag_filter
                && !tags.is_empty()
            {
                let note_tags: Vec<String> =
                    note.tags.iter().map(|t| t.to_lowercase()).collect();
                let matches_tag = tags.iter().any(|t| note_tags.contains(t));
                if !matches_tag {
                    continue;
                }
            }

            
            let matched_title = title_query.is_empty()
                || note.title.to_lowercase().contains(&title_query)
                || note.id.to_lowercase().contains(&title_query);

            
            let content_opt = if !grep_query.is_empty() {
                self.storage.load_note(&note.id).ok()
            } else {
                None
            };
            let matched_grep = grep_query.is_empty()
                || content_opt
                    .as_ref()
                    .is_some_and(|n| n.content.to_lowercase().contains(&grep_query));

            let label = if note.folder.is_empty() {
                note.title.clone()
            } else {
                format!("{}/{}", note.folder, note.title)
            };
            let lock_prefix = if note.id.ends_with(".clin") { "\u{f023} " } else { "" };
            let tags_str = if note.tags.is_empty() {
                String::new()
            } else {
                format!(
                    " [{}]",
                    note.tags
                        .iter()
                        .map(|t| t.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            };

            
            if !title_query.is_empty() && matched_title {
                title_results.push(format!("{}{}{}", lock_prefix, label, tags_str));
                title_result_indices.push(note_idx);
            }

            
            if !grep_query.is_empty() && matched_grep && matched_title {
                if let Some(note_data) = content_opt
                    .filter(|n| n.content.to_lowercase().contains(&grep_query))
                {
                    
                    let match_count = note_data
                        .content
                        .lines()
                        .filter(|l| l.to_lowercase().contains(&grep_query))
                        .count();
                    grep_results.push(format!(" {}{}{} ({})", lock_prefix, label, tags_str, match_count));
                    grep_result_indices.push(note_idx);
                    grep_is_header.push(true);

                    
                    for (line_no, line) in note_data
                        .content
                        .lines()
                        .enumerate()
                        .filter(|(_, line)| line.to_lowercase().contains(&grep_query))
                    {
                        let trimmed = line.trim();
                        let snippet: String =
                            if trimmed.chars().count() > 56 {
                                trimmed.chars().take(56).collect::<String>() + "…"
                            } else {
                                trimmed.to_string()
                            };
                        grep_results.push(format!("  L{}: {}", line_no + 1, snippet));
                        grep_result_indices.push(note_idx);
                        grep_is_header.push(false);
                    }
                }
            }

            
            if title_query.is_empty()
                && grep_query.is_empty()
                && (parsed.folder_filter.is_some()
                    || parsed.pinned_only
                    || parsed.tag_filter.is_some())
            {
                let tags_str = if note.tags.is_empty() {
                    String::new()
                } else {
                    format!(
                        "  [{}]",
                        note.tags
                            .iter()
                            .map(|t| t.as_str())
                            .collect::<Vec<_>>()
                            .join(" ")
                    )
                };
                title_results.push(format!("{}{}{}", lock_prefix, label, tags_str));
                title_result_indices.push(note_idx);
            }
        }

        if let Some(popup) = &mut self.popups.search {
            popup.title_results = title_results;
            popup.title_result_indices = title_result_indices;
            if popup.title_selected >= popup.title_results.len() {
                popup.title_selected = popup.title_results.len().saturating_sub(1);
            }
            popup.grep_results = grep_results;
            popup.grep_result_indices = grep_result_indices;
            popup.grep_is_header = grep_is_header;
            if popup.grep_selected >= popup.grep_results.len() {
                popup.grep_selected = popup.grep_results.len().saturating_sub(1);
            }
        }
    }

    pub fn confirm_search(&mut self) {
        self.popups.search = None;
    }


    pub fn jump_to_selected_result(&mut self) {
        if let Some(popup) = &self.popups.search {
            let mut target_line = None;
            let note_idx = match popup.focus {
                crate::popups::SearchFocus::Results => {
                    let has_grep = !popup.grep_results.is_empty();
                    if has_grep {
                        let is_header = popup.grep_is_header.get(popup.grep_selected).copied().unwrap_or(false);
                        if !is_header {
                            if let Some(line_str) = popup.grep_results.get(popup.grep_selected) {
                                if let Some(l_pos) = line_str.find('L') {
                                    if let Some(colon_pos) = line_str.find(':') {
                                        if colon_pos > l_pos + 1 {
                                            if let Ok(num) = line_str[l_pos + 1..colon_pos].trim().parse::<usize>() {
                                                target_line = Some(num);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        popup.grep_result_indices.get(popup.grep_selected).copied()
                    } else {
                        popup.title_result_indices.get(popup.title_selected).copied()
                    }
                }
                crate::popups::SearchFocus::Input => None,
            };
            if let Some(idx) = note_idx {
                self.jump_to_note_index(idx);
                if let Some(note) = self.notes.get(idx) {
                    let id = note.id.clone();
                    self.open_note_at_line(&id, target_line);
                }
            }
        }
    }

    pub fn cancel_search(&mut self) {
        if let Some(popup) = self.popups.search.take() {
            self.list.visual_index = popup.original_index;
            self.list.folder_expanded = popup.original_folder_expanded;
            self.refresh_visual_list();
            if !self.list.visual_list.is_empty() {
                self.list.visual_index = self
                    .list
                    .visual_index
                    .min(self.list.visual_list.len().saturating_sub(1));
            }
            self.request_preview_update();
        }
    }

    pub fn jump_to_top(&mut self) {
        self.list.visual_index = 0;
        self.request_preview_update();
    }

    pub fn jump_to_bottom(&mut self) {
        self.list.visual_index = self.list.visual_list.len().saturating_sub(1);
        self.request_preview_update();
    }

    pub fn page_up(&mut self) {
        self.list.visual_index = self.list.visual_index.saturating_sub(self.list.page_size);
        self.request_preview_update();
    }

    pub fn page_down(&mut self) {
        let max_index = self.list.visual_list.len().saturating_sub(1);
        self.list.visual_index = (self.list.visual_index + self.list.page_size).min(max_index);
        self.request_preview_update();
    }

    pub fn handle_g_press(&mut self) -> bool {
        let now = Instant::now();
        if let Some(last) = self.last_g_press
            && now.duration_since(last) < Duration::from_millis(500)
        {
            self.last_g_press = None;
            self.jump_to_top();
            return true;
        }
        self.last_g_press = Some(now);
        false
    }

    pub fn initiate_quit(&mut self) {
        if self.confirm_on_quit {
            self.show_confirm(ConfirmAction::QuitApp);
        } else {
            self.should_quit = true;
        }
    }

    pub fn handle_esc_press(&mut self) {
        let now = Instant::now();
        if let Some(last) = self.last_esc_press
            && now.duration_since(last) < Duration::from_millis(500)
        {
            self.last_esc_press = None;
            self.collapse_all_folders();
            return;
        }
        self.last_esc_press = Some(now);
    }

    pub fn collapse_all_folders(&mut self) {
        self.list.folder_expanded.clear();
        self.list.folder_expanded.insert(String::new());
        self.refresh_visual_list();
        self.request_preview_update();
    }

    pub fn open_trash_view(&mut self) {
        match self.storage.list_trash() {
            Ok(items) => {
                if items.is_empty() {
                    self.set_temporary_status_static("Trash is empty");
                    return;
                }
                self.popups.trash_view = Some(TrashView { items, selected: 0 });
            }
            Err(e) => {
                self.set_temporary_status(&format!("Failed to open trash: {e}"));
            }
        }
    }

    pub fn close_trash_view(&mut self) {
        self.popups.trash_view = None;
    }

    pub fn restore_from_trash(&mut self) {
        let item = self
            .popups
            .trash_view
            .as_ref()
            .and_then(|t| t.items.get(t.selected).cloned());

        let Some(item) = item else { return };

        match self.storage.restore_trash_items(vec![item]) {
            Ok(_) => {
                if let Ok(items) = self.storage.list_trash() {
                    if items.is_empty() {
                        self.popups.trash_view = None;
                        self.set_temporary_status_static("Note restored, trash is now empty");
                    } else if let Some(ref mut trash) = self.popups.trash_view {
                        trash.items = items;
                        trash.selected = trash.selected.min(trash.items.len().saturating_sub(1));
                        self.set_temporary_status_static("Note restored");
                    }
                }
                self.list.folder_cache = None;
                if let Err(e) = self.refresh_notes() {
                    self.set_temporary_status(&format!("Refresh failed: {e}"));
                }
            }
            Err(e) => {
                self.set_temporary_status(&format!("Failed to restore: {e}"));
            }
        }
    }

    pub fn begin_delete_from_trash(&mut self) {
        if let Some(trash) = &self.popups.trash_view
            && let Some(item) = trash.items.get(trash.selected).cloned()
        {
            self.show_confirm(ConfirmAction::DeleteFromTrash { item });
        }
    }

    pub fn confirm_delete_from_trash(&mut self, item: trash::TrashItem) {
        match self.storage.purge_trash_items(vec![item]) {
            Ok(()) => {
                if let Some(ref mut trash) = self.popups.trash_view
                    && let Ok(items) = self.storage.list_trash()
                {
                    if items.is_empty() {
                        self.popups.trash_view = None;
                        self.set_temporary_status_static("Note deleted, trash is now empty");
                    } else {
                        trash.items = items;
                        trash.selected = trash.selected.min(trash.items.len().saturating_sub(1));
                        self.set_temporary_status_static("Note permanently deleted");
                    }
                }
            }
            Err(e) => {
                self.set_temporary_status(&format!("Failed to delete: {e}"));
            }
        }
    }

    pub fn begin_empty_trash(&mut self) {
        if let Some(trash) = &self.popups.trash_view {
            if trash.items.is_empty() {
                self.set_temporary_status_static("Trash is already empty");
            } else {
                self.show_confirm(ConfirmAction::EmptyTrash {
                    items: trash.items.clone(),
                });
            }
        }
    }

    pub fn confirm_empty_trash(&mut self, items: Vec<trash::TrashItem>) {
        let count = items.len();
        match self.storage.purge_trash_items(items) {
            Ok(()) => {
                self.popups.trash_view = None;
                self.set_temporary_status(&format!("Deleted {} notes from trash", count));
            }
            Err(e) => {
                self.set_temporary_status(&format!("Failed to empty trash: {e}"));
            }
        }
    }

    pub fn toggle_preview(&mut self) {
        self.list.preview_enabled = !self.list.preview_enabled;
        if self.list.preview_enabled {
            self.update_preview();
            self.set_temporary_status_static("Preview enabled");
        } else {
            self.list.preview_content = None;
            self.set_temporary_status_static("Preview disabled");
        }
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.preview_enabled = self.list.preview_enabled;
            let _ = config.save();
        }
    }

    pub fn poll_renderers(&mut self) -> bool {
        let mut updated = false;

        if let Some(last) = self.list.last_selection_change
            && last.elapsed() > Duration::from_millis(150)
            && self.list.pending_preview_update
        {
            self.update_preview();
            self.list.pending_preview_update = false;
            self.list.last_selection_change = None;
            updated = true;
        }

        if let Some(last) = self.editor.last_editor_change
            && last.elapsed() > Duration::from_millis(150)
            && self.editor.pending_editor_preview_update
        {
            self.update_editor_markdown_preview();
            self.editor.pending_editor_preview_update = false;
            self.editor.last_editor_change = None;
            updated = true;
        }

        if let Some(PreviewContent::Markdown(renderer)) = &mut self.list.preview_content
            && renderer.poll()
        {
            if !renderer.pages_built() {
                let visible = 34u16;
                renderer.build_pages(visible, self.app_theme.preview_bg());
            }
            updated = true;
        }
        if let Some(renderer) = &mut self.editor.md_preview_renderer
            && renderer.poll()
        {
            if !renderer.pages_built() {
                let visible = 36u16;
                renderer.build_pages(visible, self.app_theme.preview_bg());
            }
            updated = true;
        }
        updated
    }

    pub fn request_preview_update(&mut self) {
        if !self.list.preview_enabled {
            return;
        }
        
        self.list.preview_content_index = None;
        self.list.last_selection_change = Some(Instant::now());
        self.list.pending_preview_update = true;
    }

    pub fn request_editor_preview_update(&mut self) {
        if !self.editor.editor_preview_enabled {
            return;
        }
        self.editor.last_editor_change = Some(Instant::now());
        self.editor.pending_editor_preview_update = true;
    }

    pub fn update_preview(&mut self) {
        if !self.list.preview_enabled {
            return;
        }

        let Some(VisualItem::Note {
            id,
            is_draw,
            is_canvas,
            is_clin,
            ..
        }) = self.list.visual_list.get(self.list.visual_index).cloned()
        else {
            self.list.preview_content = None;
            self.list.preview_content_index = None;
            return;
        };

        
        
        if self.preview_encryption && is_clin {
            self.list.preview_content = None;
            self.list.preview_content_index = Some(self.list.visual_index);
            return;
        }

        if is_draw {
            let path = self.storage.note_path(&id);
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str::<crate::draw::state::DrawData>(&content)
                {
                    Ok(data) => {
                        let grid = crate::snapshot::render_draw_snapshot(&data, &self.app_theme);
                        self.list.preview_content = Some(PreviewContent::DrawGrid(grid));
                    }
                    Err(e) => {
                        self.list.preview_content = None;
                        self.status = Cow::Owned(format!("Failed to parse draw: {e}"));
                    }
                },
                Err(_) => {
                    self.list.preview_content = None;
                }
            }
            self.list.preview_content_index = Some(self.list.visual_index);
            return;
        }

        if is_canvas {
            let path = self.storage.note_path(&id);
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    match serde_json::from_str::<crate::pinstar::data::CanvasData>(&content) {
                        Ok(data) => {
                            let grid =
                                crate::snapshot::render_canvas_snapshot(&data, &self.app_theme);
                            self.list.preview_content = Some(PreviewContent::CanvasGrid(grid));
                        }
                        Err(e) => {
                            self.list.preview_content = None;
                            self.status = Cow::Owned(format!("Failed to parse canvas: {e}"));
                        }
                    }
                }
                Err(_) => {
                    self.list.preview_content = None;
                }
            }
            self.list.preview_content_index = Some(self.list.visual_index);
            return;
        }

        if let Ok(note) = self.storage.load_note(&id) {
            let width = 80u16.saturating_sub(2).max(40);
            let mut renderer = MarkdownRenderer::new(width);
            renderer.render(&note.content, width);
            self.list.preview_content = Some(PreviewContent::Markdown(Box::new(renderer)));
        } else {
            self.list.preview_content = None;
        }
        self.list.preview_content_index = Some(self.list.visual_index);
    }

    pub fn update_editor_markdown_preview(&mut self) {
        if !self.editor.editor_preview_enabled {
            return;
        }

        let content = self.editor.editor.lines().join("\n");
        let width = 80u16.saturating_sub(2).max(40);
        let mut renderer = MarkdownRenderer::new(width);
        renderer.render(&content, width);
        self.editor.md_preview_renderer = Some(renderer);
    }

    pub fn toggle_markdown_preview(&mut self) {
        self.editor.editor_preview_enabled = !self.editor.editor_preview_enabled;
        if self.editor.editor_preview_enabled {
            self.update_editor_markdown_preview();
            self.set_temporary_status_static("Markdown preview enabled");
        } else {
            self.editor.md_preview_renderer = None;
            self.set_temporary_status_static("Markdown preview disabled");
        }
        if let Ok(mut config) = crate::config::ClinConfig::load() {
            config.editor_preview_enabled = self.editor.editor_preview_enabled;
            let _ = config.save();
        }
    }

    pub fn reload_theme(&mut self) {
        let config = crate::config::ClinConfig::load().unwrap_or_default();
        self.app_theme = crate::app_theme::AppThemeColors::from_config(&config.theme);
        if self.mode == ViewMode::Help {
            self.list.help_text_cache = None;
        }
    }

    pub fn begin_theme_selection(&mut self) {
        let themes = vec![
            "default".to_string(),
            "tokyo_night".to_string(),
            "catppuccin_mocha".to_string(),
            "onedark".to_string(),
            "gruvbox".to_string(),
            "dracula".to_string(),
            "nord".to_string(),
            "rose_pine".to_string(),
            "everforest".to_string(),
            "kanagawa".to_string(),
            "solarized".to_string(),
        ];

        let config = crate::config::ClinConfig::load().unwrap_or_default();
        let current = config.theme.theme.to_string();
        let selected = themes.iter().position(|t| t == &current).unwrap_or(0);
        let general_is_solid = matches!(config.theme.background, crate::config::Background::Solid);
        let graph_is_solid = matches!(
            config.visual.graph_background,
            crate::config::Background::Solid
        );

        self.popups.theme = Some(ThemePopup {
            themes,
            selected,
            focus: ThemePopupFocus::ThemeList,
            general_is_solid,
            graph_is_solid,
        });
    }

    pub fn select_theme(&mut self) {
        if let Some(mut popup) = self.popups.theme.take() {
            match popup.focus {
                ThemePopupFocus::ThemeList => {
                    let next_theme = popup.themes[popup.selected].clone();
                    let mut config = crate::config::ClinConfig::load().unwrap_or_default();
                    config.theme.theme = next_theme.parse().unwrap_or_default();
                    if let Err(e) = config.save() {
                        self.set_temporary_status(&format!("Failed to save theme: {}", e));
                        return;
                    }
                    self.reload_theme();
                    self.set_temporary_status(&format!("Theme set to: {}", next_theme));
                    self.popups.theme = Some(popup);
                }
                ThemePopupFocus::GeneralBg => {
                    popup.general_is_solid = !popup.general_is_solid;
                    let mut config = crate::config::ClinConfig::load().unwrap_or_default();
                    config.theme.background = if popup.general_is_solid {
                        crate::config::Background::Solid
                    } else {
                        crate::config::Background::Transparent
                    };
                    if let Err(e) = config.save() {
                        self.set_temporary_status(&format!("Failed to save bg: {}", e));
                    }
                    self.reload_theme();
                    self.popups.theme = Some(popup);
                }
                ThemePopupFocus::GraphBg => {
                    popup.graph_is_solid = !popup.graph_is_solid;
                    let mut config = crate::config::ClinConfig::load().unwrap_or_default();
                    config.visual.graph_background = if popup.graph_is_solid {
                        crate::config::Background::Solid
                    } else {
                        crate::config::Background::Transparent
                    };
                    if let Err(e) = config.save() {
                        self.set_temporary_status(&format!("Failed to save graph bg: {}", e));
                    }
                    self.popups.theme = Some(popup);
                }
            }
        }
    }

    pub fn close_theme_popup(&mut self) {
        self.popups.theme = None;
    }

    pub fn app_theme_name(&self) -> String {
        let config = crate::config::ClinConfig::load().unwrap_or_default();
        config.theme.theme.to_string()
    }
}
