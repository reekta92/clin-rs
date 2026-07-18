mod edit_panes;
mod folders;
mod import_ops;
mod loading;
mod notes;
mod popups;
mod search;
mod settings_ops;
mod status;
mod tags;
mod trash;
mod views;

pub use crate::editor::*;
use crate::events::get_title_text;
use crate::events::make_title_editor;
pub use crate::list_view::*;
use crate::markdown::MarkdownRenderer;
pub use crate::popups::*;
use crate::ui::text_area_from_content;
use crate::ui::{now_unix_secs, open_in_file_manager};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;
use std::borrow::Cow;
use std::time::Instant;

use crate::keybinds::Keybinds;
use crate::storage::{Note, NoteSummary, Storage};
use crate::templates::Template;
use anyhow::Result;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

pub const VIRTUAL_PINNED_PATH: &str = "__clin_virtual__/pinned";
pub const VIRTUAL_PINNED_LABEL: &str = "Pinned";
pub const VIRTUAL_SMART_PATH: &str = "__clin_virtual__/smart";
pub const VIRTUAL_SMART_LABEL: &str = "Smart";
pub const VIRTUAL_SUBNOTES_PATH: &str = "__clin_virtual__/subnotes";
pub const VIRTUAL_SUBNOTES_LABEL: &str = "Subnotes";

#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    pub text: String,
    pub folder_filter: Option<String>,
    pub pinned_only: bool,
    pub tag_filter: Option<Vec<String>>,
    pub grep_mode: bool,
    pub grep_text: String,
}

#[derive(Debug, Clone, Default)]
pub struct HelpSearchState {
    pub popup: Option<crate::ui::quick_search::QuickSearch<(usize, String)>>,
    pub highlight_row: Option<usize>,
}

#[derive(Debug)]
pub enum LoadBatch {
    Started(usize),
    Items(Vec<(String, NoteSummary, u64)>),
    Done(usize),
}

fn find_filter_tokens(s: &str) -> Vec<(usize, &'static str)> {
    let spaced = [" f:", " g:", " p:", " t:"];
    let bare = ["f:", "g:", "p:", "t:"];
    let mut tokens: Vec<(usize, &'static str)> = Vec::new();

    let is_escaped = |s: &str, pos: usize, _prefix_len: usize| -> bool {
        if pos < 3 {
            return false;
        }
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
        if s.starts_with(prefix)
            && !tokens.iter().any(|&(p, _)| p == 0)
            && !is_escaped(s, 0, prefix.len())
        {
            tokens.push((0, prefix));
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
                let is_filter = next
                    .zip(after)
                    .is_some_and(|(ch, colon)| filter_chars.contains(&ch) && colon == ':');
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
    ContentTree,
    Setup,
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
    #[must_use]
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

    #[must_use]
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

    pub fn index(self) -> usize {
        match self {
            HelpTab::Notes => 0,
            HelpTab::Editor => 1,
            HelpTab::Graph => 2,
            HelpTab::Draw => 3,
            HelpTab::Canvas => 4,
            HelpTab::Backup => 5,
            HelpTab::Templates => 6,
            HelpTab::About => 7,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutDrag {
    VDivider,
    HDivider,
    PreviewSwap,
    CalendarSwap,
}

pub struct App {
    pub popups: crate::popups::PopupManager,
    pub storage: Storage,
    pub keybinds: Keybinds,
    pub seq_matcher: crate::keybinds::KeyMatcher,
    pub notes: Vec<NoteSummary>,
    pub editor: NoteEditor,
    pub list: ListView,
    pub mode: ViewMode,
    pub status: Cow<'static, str>,
    pub status_until: Option<Instant>,
    pub help_page: u16,
    pub help_page_size: u16,
    pub help_tab: HelpTab,
    pub help_tab_page: HashMap<HelpTab, u16>,
    pub help_search: HelpSearchState,
    pub help_info_active: usize,
    pub help_suggestions: Vec<crate::ui::HelpSuggestion>,
    pub command_palette: Option<crate::palette::CommandPalette>,
    pub needs_full_redraw: bool,
    pub confirm_on_delete: bool,
    pub confirm_on_quit: bool,
    pub should_quit: bool,
    pub preview_encryption: bool,
    pub mouse_pos: Option<(u16, u16)>,
    pub preview_position: crate::config::PreviewPosition,
    pub calendar_position: crate::config::CalendarPosition,
    pub pinned_on_top: bool,
    pub default_folder: Option<String>,
    pub mouse_enabled: bool,
    pub date_format: String,
    pub last_auto_backup: Option<std::time::Instant>,
    pub return_mode: Option<ViewMode>,
    pub app_theme: crate::app_theme::AppThemeColors,
    pub graph_state: Option<crate::graf::app::GrafAppState>,
    pub draw_state: Option<crate::draw::app::DrawAppState>,
    pub backup_state: Option<crate::backup::state::BackupState>,
    pub content_tree_state: Option<crate::content_tree::state::ContentTreeState>,
    pub setup_state: Option<crate::setup::SetupState>,
    pub config_errors: Vec<String>,
    pub canvas_state: Option<crate::pinstar::state::PinstarState>,
    pub config: crate::config::ClinConfig,
    pub summary_cache: HashMap<String, NoteSummary>,
    /// (parent_id, Vec<SubNote>) cache for the Subnotes view; rebuilt on refresh.
    pub subnotes_view_cache: Vec<(String, Vec<crate::storage::SubNote>)>,
    /// Signature (notes.len() + subnotes hash) to invalidate subnotes_view_cache.
    pub subnotes_view_cache_sig: usize,
    pub summary_mtime: HashMap<String, u64>,
    pub notes_with_subnotes: std::collections::HashSet<String>,
    pub initial_load_done: bool,
    pub load_cancel: Arc<AtomicBool>,
    pub loading_total: usize,
    pub backup_tx: Option<mpsc::Sender<crate::backup::worker::BackupJob>>,
    pub git_lock: Arc<Mutex<()>>,
    pub backup_status: Arc<Mutex<Option<String>>>,
    pub fs_event_rx: Option<mpsc::Receiver<()>>,
    pub config_mtime: Option<std::time::SystemTime>,
    pub goals_progress: crate::goals::DailyProgress,
    pub draw_preview: Option<(String, crate::draw::state::DrawData)>,
    pub graph_preview: Option<crate::graf::graph::GraphState>,
    pub graph_preview_sig: usize,
    pub graph_preview_steps: usize,
    pub preview_wrap: bool,
    pub preview_fullscreen: bool,
    pub layout_edit: bool,
    pub layout_drag: Option<LayoutDrag>,
    pub image_picker: Option<ratatui_image::picker::Picker>,
    pub image_decode_tx: Option<std::sync::mpsc::Sender<crate::image_render::worker::ImageJob>>,
    pub image_decode_rx: Option<
        std::sync::mpsc::Receiver<anyhow::Result<crate::image_render::worker::DecodedImage>>,
    >,
}

const PREVIEW_INNER_PAD: u16 = 4;
const PREVIEW_NO_WRAP_WIDTH: u16 = 1000;

fn preview_render_cols(pane_width: u16, wrap: bool) -> u16 {
    if !wrap {
        return PREVIEW_NO_WRAP_WIDTH;
    }
    if pane_width == 0 {
        return 78;
    }
    pane_width.saturating_sub(PREVIEW_INNER_PAD).max(20)
}

impl App {
    pub fn desired_list_preview_width(&self) -> u16 {
        preview_render_cols(self.list.last_preview_pane_width, self.preview_wrap)
    }

    pub fn desired_editor_preview_width(&self) -> u16 {
        preview_render_cols(self.editor.last_preview_pane_width, self.preview_wrap)
    }
    pub fn desired_list_preview_height(&self) -> u16 {
        self.list.last_preview_pane_height
    }

    pub fn desired_editor_preview_height(&self) -> u16 {
        self.editor.last_preview_pane_height
    }

    pub fn new(storage: Storage) -> Result<Self> {
        let bootstrap_config = crate::config::ClinConfig::load().unwrap_or_default();
        let config_errors = bootstrap_config.validate();
        let keybinds = storage.load_keybinds_with_preset(bootstrap_config.core.keybind_preset);
        let app_theme = crate::app_theme::AppThemeColors::from_config(&bootstrap_config.ui);

        let mut editor = NoteEditor::new();
        editor.external_editor_enabled = bootstrap_config.editor.external_enabled;
        editor.external_editor = bootstrap_config.editor.external_command.clone();
        editor.editor_preview_enabled = bootstrap_config.editor.preview_enabled;
        editor.show_line_numbers = bootstrap_config.editor.show_line_numbers;
        editor.title_editor = make_title_editor("", Color::Black, Color::Cyan);

        let mut list = ListView::new();
        list.sort_field = bootstrap_config
            .list
            .default_sort_field
            .unwrap_or(SortField::Title);
        list.sort_order = bootstrap_config
            .list
            .default_sort_order
            .unwrap_or(SortOrder::Ascending);
        list.preview_enabled = bootstrap_config.list.preview_enabled;
        list.page_size = 10;
        list.notes_layout = bootstrap_config.list.default_view.clone();
        list.list_density = bootstrap_config.list.density.clone();
        list.inline_info = bootstrap_config.list.inline_info;
        list.show_file_size = bootstrap_config.list.show_file_size;
        list.folders_first = bootstrap_config.list.folders_first;
        list.show_hidden_files = bootstrap_config.list.show_hidden_files;
        list.show_all_files = bootstrap_config.list.show_all_files;
        list.calendar_enabled = bootstrap_config.list.calendar_enabled;
        list.preview_width_ratio = bootstrap_config.list.preview_width_ratio;
        list.calendar_height = bootstrap_config.list.calendar_height;
        list.calendar_position = bootstrap_config.list.calendar_position;
        list.sections = bootstrap_config.list.sections.clone();
        list.pinned_folders = bootstrap_config
            .list
            .pinned_folders
            .iter()
            .cloned()
            .collect();
        let preview_wrap = bootstrap_config.core.preview_wrap;
        let config_path = crate::config::ClinConfig::config_path().ok();
        let config_mtime =
            config_path.and_then(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());

        let mut app = Self {
            storage,
            keybinds,
            seq_matcher: crate::keybinds::KeyMatcher::new(),
            notes: Vec::new(),
            editor,
            list,
            mode: ViewMode::List,
            status: Cow::Borrowed(""),
            status_until: None,
            help_page: 0,
            help_tab: HelpTab::Notes,
            help_page_size: 20,
            help_tab_page: HashMap::new(),
            help_search: HelpSearchState::default(),
            help_info_active: 0,
            help_suggestions: Vec::new(),
            command_palette: None,
            popups: crate::popups::PopupManager::default(),
            needs_full_redraw: false,
            confirm_on_delete: bootstrap_config.core.confirm_on_delete,
            confirm_on_quit: bootstrap_config.core.confirm_on_quit,
            should_quit: false,
            preview_encryption: bootstrap_config.list.preview_encryption,
            mouse_pos: None,
            mouse_enabled: bootstrap_config.core.mouse_enabled,
            date_format: bootstrap_config.list.date_format.clone(),
            last_auto_backup: None,
            preview_position: bootstrap_config.list.preview_position,
            calendar_position: bootstrap_config.list.calendar_position,
            config_errors,
            graph_state: None,
            draw_state: None,
            backup_state: None,
            content_tree_state: None,
            setup_state: None,
            pinned_on_top: bootstrap_config.list.pinned_on_top,
            default_folder: bootstrap_config.core.default_folder.clone(),
            return_mode: None,
            app_theme,
            canvas_state: None,
            config: bootstrap_config,
            summary_cache: HashMap::new(),
            summary_mtime: HashMap::new(),
            notes_with_subnotes: std::collections::HashSet::new(),
            subnotes_view_cache: Vec::new(),
            subnotes_view_cache_sig: 0,
            initial_load_done: true,
            load_cancel: Arc::new(AtomicBool::new(false)),
            loading_total: 0,
            backup_tx: None,
            git_lock: Arc::new(Mutex::new(())),
            backup_status: Arc::new(Mutex::new(None)),
            fs_event_rx: None,
            config_mtime,
            goals_progress: crate::goals::DailyProgress::default(),
            draw_preview: None,
            graph_preview: None,
            graph_preview_sig: 0,
            graph_preview_steps: 0,
            preview_wrap,
            preview_fullscreen: false,
            layout_edit: false,
            layout_drag: None,
            image_picker: None,
            image_decode_tx: None,
            image_decode_rx: None,
        };
        app.goals_progress = app.load_goals_progress();
        app.list.folder_expanded.insert(String::new());
        if !app.config.list.expanded_folders.is_empty() {
            for folder in &app.config.list.expanded_folders {
                app.list.folder_expanded.insert(folder.clone());
            }
        } else if let Some(d) = app.config.list.default_expand_depth {
            app.expand_folders_to_depth(d);
        }
        app.refresh_notes()?;
        if app
            .list
            .sections
            .contains(&crate::config::NotesSection::Graf)
        {
            app.ensure_graph_preview();
        }
        Ok(app)
    }

    pub fn new_deferred(storage: Storage) -> Result<Self> {
        let bootstrap_config = crate::config::ClinConfig::load().unwrap_or_default();
        let config_errors = bootstrap_config.validate();
        let keybinds = storage.load_keybinds_with_preset(bootstrap_config.core.keybind_preset);
        let app_theme = crate::app_theme::AppThemeColors::from_config(&bootstrap_config.ui);

        let mut editor = NoteEditor::new();
        editor.external_editor_enabled = bootstrap_config.editor.external_enabled;
        editor.external_editor = bootstrap_config.editor.external_command.clone();
        editor.editor_preview_enabled = bootstrap_config.editor.preview_enabled;
        editor.show_line_numbers = bootstrap_config.editor.show_line_numbers;
        editor.title_editor = make_title_editor("", Color::Black, Color::Cyan);

        let mut list = ListView::new();
        list.sort_field = bootstrap_config
            .list
            .default_sort_field
            .unwrap_or(SortField::Title);
        list.sort_order = bootstrap_config
            .list
            .default_sort_order
            .unwrap_or(SortOrder::Ascending);
        list.preview_enabled = bootstrap_config.list.preview_enabled;
        list.notes_layout = bootstrap_config.list.default_view.clone();
        list.list_density = bootstrap_config.list.density.clone();
        list.inline_info = bootstrap_config.list.inline_info;
        list.show_file_size = bootstrap_config.list.show_file_size;
        list.folders_first = bootstrap_config.list.folders_first;
        list.show_all_files = bootstrap_config.list.show_all_files;
        list.show_hidden_files = bootstrap_config.list.show_hidden_files;
        list.calendar_enabled = bootstrap_config.list.calendar_enabled;
        list.preview_width_ratio = bootstrap_config.list.preview_width_ratio;
        list.calendar_height = bootstrap_config.list.calendar_height;
        list.calendar_position = bootstrap_config.list.calendar_position;
        list.sections = bootstrap_config.list.sections.clone();
        list.pinned_folders = bootstrap_config
            .list
            .pinned_folders
            .iter()
            .cloned()
            .collect();
        let preview_wrap = bootstrap_config.core.preview_wrap;
        let config_path = crate::config::ClinConfig::config_path().ok();
        let config_mtime =
            config_path.and_then(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());

        let mut app = Self {
            storage,
            keybinds,
            seq_matcher: crate::keybinds::KeyMatcher::new(),
            notes: Vec::new(),
            editor,
            list,
            mode: ViewMode::List,
            status: Cow::Borrowed(""),
            status_until: None,
            help_page: 0,
            help_tab: HelpTab::Notes,
            help_page_size: 20,
            help_tab_page: HashMap::new(),
            help_search: HelpSearchState::default(),
            help_info_active: 0,
            help_suggestions: Vec::new(),
            command_palette: None,
            popups: crate::popups::PopupManager::default(),
            needs_full_redraw: false,
            confirm_on_delete: bootstrap_config.core.confirm_on_delete,
            confirm_on_quit: bootstrap_config.core.confirm_on_quit,
            should_quit: false,
            preview_encryption: bootstrap_config.list.preview_encryption,
            mouse_pos: None,
            mouse_enabled: bootstrap_config.core.mouse_enabled,
            date_format: bootstrap_config.list.date_format.clone(),
            last_auto_backup: None,
            preview_position: bootstrap_config.list.preview_position,
            calendar_position: bootstrap_config.list.calendar_position,
            config_errors,
            graph_state: None,
            draw_state: None,
            backup_state: None,
            content_tree_state: None,
            setup_state: None,
            pinned_on_top: bootstrap_config.list.pinned_on_top,
            default_folder: bootstrap_config.core.default_folder.clone(),
            return_mode: None,
            app_theme,
            canvas_state: None,
            config: bootstrap_config,
            summary_cache: HashMap::new(),
            summary_mtime: HashMap::new(),
            notes_with_subnotes: std::collections::HashSet::new(),
            subnotes_view_cache: Vec::new(),
            subnotes_view_cache_sig: 0,
            initial_load_done: false,
            load_cancel: Arc::new(AtomicBool::new(false)),
            loading_total: 0,
            backup_tx: None,
            git_lock: Arc::new(Mutex::new(())),
            backup_status: Arc::new(Mutex::new(None)),
            fs_event_rx: None,
            config_mtime,
            goals_progress: crate::goals::DailyProgress::default(),
            draw_preview: None,
            graph_preview: None,
            graph_preview_steps: 0,
            graph_preview_sig: 0,
            preview_wrap,
            preview_fullscreen: false,
            layout_edit: false,
            layout_drag: None,
            image_picker: None,
            image_decode_tx: None,
            image_decode_rx: None,
        };
        app.goals_progress = app.load_goals_progress();
        app.list.folder_expanded.insert(String::new());
        if !app.config.list.expanded_folders.is_empty() {
            for folder in &app.config.list.expanded_folders {
                app.list.folder_expanded.insert(folder.clone());
            }
        } else if let Some(d) = app.config.list.default_expand_depth {
            app.expand_folders_to_depth(d);
        }
        if app.config.accent_hint_migrated {
            app.set_temporary_status(
                "Hint bar style \u{2018}Accent\u{2019} was removed; using Classic.",
            );
            app.config.accent_hint_migrated = false;
            let _ = app.config.save();
        }
        Ok(app)
    }
    pub fn reload_config(&mut self) {
        self.config = match crate::config::ClinConfig::load() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("RELOAD ERROR: {:?}", e);
                self.config.clone()
            }
        };
        self.preview_wrap = self.config.core.preview_wrap;
        self.app_theme = crate::app_theme::AppThemeColors::from_config(&self.config.ui);
        self.list.pinned_folders = self.config.list.pinned_folders.iter().cloned().collect();
        self.build_display_lines();
    }

    pub fn check_and_reload_config(&mut self) {
        if let Ok(config_path) = crate::config::ClinConfig::config_path()
            && let Ok(metadata) = std::fs::metadata(&config_path)
            && let Ok(mtime) = metadata.modified()
            && (self.config_mtime.is_none() || self.config_mtime.expect("value is present") < mtime)
        {
            self.config_mtime = Some(mtime);
            self.reload_config();
        }
    }

    pub(crate) fn is_virtual_pinned_path(path: &str) -> bool {
        path == VIRTUAL_PINNED_PATH
    }

    pub(crate) fn is_virtual_subnotes_path(path: &str) -> bool {
        path == VIRTUAL_SUBNOTES_PATH
    }

    pub(crate) fn is_subnotes_parent_grid_path(path: &str) -> bool {
        path.starts_with("subnotes:")
    }

    pub(crate) fn subnotes_parent_id_from_grid_path(path: &str) -> &str {
        path.strip_prefix("subnotes:").unwrap_or(path)
    }

    pub(crate) fn is_virtual_path(path: &str) -> bool {
        Self::is_virtual_pinned_path(path)
            || Self::is_virtual_subnotes_path(path)
            || Self::is_subnotes_parent_grid_path(path)
    }

    /// Rebuild cached display lines from the current visual_list.
    /// Mirrors the formatting logic from draw_list_view so per-frame work is O(1).
    fn build_display_lines(&mut self) {
        let mut items = Vec::with_capacity(self.list.visual_list.len());
        let visual = &self.list.visual_list.clone();
        for (vi, item) in visual.iter().enumerate() {
            match item {
                VisualItem::Folder {
                    path,
                    name,
                    depth,
                    is_expanded,
                    note_count,
                    recursive_count,
                    stale,
                    is_pinned,
                    ..
                } => {
                    let indent = "  ".repeat(*depth);
                    let is_virtual_pinned = name == crate::app::VIRTUAL_PINNED_LABEL;
                    let icon = if self.config.ui.icon_mode == crate::config::IconMode::None {
                        String::new()
                    } else if is_virtual_pinned {
                        if *is_expanded {
                            format!(
                                "{} {}",
                                crate::ui::get_icon(
                                    "\u{f078}",
                                    "\u{25bc}",
                                    self.config.ui.icon_mode
                                ),
                                crate::ui::get_icon(
                                    "\u{f08d}",
                                    "\u{1f4cc}",
                                    self.config.ui.icon_mode
                                )
                            )
                        } else {
                            format!(
                                "{} {}",
                                crate::ui::get_icon(
                                    "\u{f054}",
                                    "\u{25b6}",
                                    self.config.ui.icon_mode
                                ),
                                crate::ui::get_icon(
                                    "\u{f08d}",
                                    "\u{1f4cc}",
                                    self.config.ui.icon_mode
                                )
                            )
                        }
                    } else {
                        let folder_glyph = if *path == crate::app::VIRTUAL_SUBNOTES_PATH {
                            crate::ui::get_icon("\u{f02c}", "\u{1f3f7}", self.config.ui.icon_mode)
                        } else {
                            crate::ui::get_icon("\u{f114}", "\u{1f4c2}", self.config.ui.icon_mode)
                        };
                        if *is_expanded {
                            format!(
                                "{} {}",
                                crate::ui::get_icon(
                                    "\u{f078}",
                                    "\u{25bc}",
                                    self.config.ui.icon_mode
                                ),
                                folder_glyph
                            )
                        } else {
                            format!(
                                "{} {}",
                                crate::ui::get_icon(
                                    "\u{f054}",
                                    "\u{25b6}",
                                    self.config.ui.icon_mode
                                ),
                                folder_glyph
                            )
                        }
                    };
                    let color = if *is_pinned {
                        self.app_theme.heading
                    } else if *stale {
                        self.app_theme.muted
                    } else {
                        self.app_theme.folder
                    };
                    let count_str = if *recursive_count > *note_count {
                        format!("{} + {}", note_count, recursive_count - note_count)
                    } else {
                        format!("{}", note_count)
                    };
                    let count_suffix = if self.list.inline_info {
                        format!(" ({count_str})")
                    } else {
                        String::new()
                    };
                    let sanitized_name = crate::sanitize::sanitize_for_terminal(name);
                    let mut display_name = sanitized_name.into_owned();
                    if *is_pinned {
                        let pin_icon =
                            crate::ui::get_icon("\u{f08d}", "\u{1f4cc}", self.config.ui.icon_mode);
                        if !pin_icon.is_empty() {
                            display_name = format!("{pin_icon} {display_name}");
                        }
                    }
                    let text = if icon.is_empty() {
                        format!("{indent}{display_name}{count_suffix}")
                    } else {
                        format!("{indent}{icon} {display_name}{count_suffix}")
                    };
                    let mut style = Style::default().add_modifier(Modifier::BOLD).fg(color);
                    if *stale && !*is_pinned {
                        style = style.add_modifier(Modifier::DIM);
                    }
                    if self.list.drag_hover == Some(vi) {
                        style = style.bg(self.app_theme.highlight_bg);
                    }
                    let mut lines = vec![Line::from(vec![Span::styled(text, style)])];
                    if self.list.list_density == crate::config::ListDensity::Comfortable {
                        lines.push(Line::from(""));
                    }
                    items.push(ListItem::new(lines));
                }
                VisualItem::Note {
                    summary_idx,
                    depth,
                    is_clin,
                    is_draw,
                    is_canvas,
                    in_virtual_pinned_folder,
                    ..
                } => {
                    let summary = &self.notes[*summary_idx];
                    let indent = "  ".repeat(*depth);

                    let when = crate::ui::format_relative_time(summary.updated_at);
                    let mut text_style = Style::default();

                    let mut spans = Vec::new();
                    spans.push(Span::raw(indent));

                    spans.push(Span::raw("  "));
                    if summary.pinned {
                        let icon =
                            crate::ui::get_icon("\u{f4cc}", "\u{1f4cc}", self.config.ui.icon_mode);
                        if !icon.is_empty() {
                            spans.push(Span::styled(
                                format!("{icon} "),
                                Style::default()
                                    .fg(self.app_theme.heading)
                                    .add_modifier(Modifier::BOLD),
                            ));
                        }
                    }

                    if *is_clin {
                        text_style = text_style.fg(self.app_theme.muted);
                        let icon =
                            crate::ui::get_icon("\u{f023}", "\u{1f512}", self.config.ui.icon_mode);
                        if !icon.is_empty() {
                            spans.push(Span::styled(
                                format!("{icon} "),
                                Style::default()
                                    .fg(self.app_theme.destructive)
                                    .add_modifier(Modifier::BOLD),
                            ));
                        }
                    }

                    if *is_draw {
                        let icon =
                            crate::ui::get_icon("\u{f1fc}", "\u{270f}", self.config.ui.icon_mode);
                        if !icon.is_empty() {
                            spans.push(Span::styled(
                                format!("{icon} "),
                                Style::default()
                                    .fg(self.app_theme.success)
                                    .add_modifier(Modifier::BOLD),
                            ));
                        }
                    }

                    if *is_canvas {
                        let icon =
                            crate::ui::get_icon("\u{f005}", "\u{2b50}", self.config.ui.icon_mode);
                        if !icon.is_empty() {
                            spans.push(Span::styled(
                                format!("{icon} "),
                                Style::default()
                                    .fg(self.app_theme.accent)
                                    .add_modifier(Modifier::BOLD),
                            ));
                        }
                    }

                    let sanitized_title =
                        crate::sanitize::sanitize_for_terminal(summary.title.as_str()).into_owned();
                    spans.push(Span::styled(sanitized_title, text_style));
                    if self.list.inline_info {
                        if self.notes_with_subnotes.contains(&summary.id) {
                            let sub_icon = match self.config.ui.icon_mode {
                                crate::config::IconMode::Nerd => " ⧉",
                                crate::config::IconMode::Unicode => " ⧉",
                                crate::config::IconMode::None => " +",
                            };
                            spans.push(Span::styled(
                                sub_icon.to_string(),
                                Style::default().fg(self.app_theme.accent),
                            ));
                        }

                        for tag in &summary.tags {
                            spans.push(Span::raw(" "));
                            let sanitized_tag = crate::sanitize::sanitize_for_terminal(tag);
                            spans.push(Span::styled(
                                format!("[{sanitized_tag}]"),
                                Style::default().fg(self.app_theme.tag),
                            ));
                        }

                        if *in_virtual_pinned_folder {
                            let source = if summary.folder.is_empty() {
                                "Vault".to_string()
                            } else {
                                summary.folder.clone()
                            };
                            spans.push(Span::styled(
                                format!(
                                    "  (from {})",
                                    crate::sanitize::sanitize_for_terminal(&source)
                                ),
                                Style::default().fg(self.app_theme.muted),
                            ));
                        }
                        if self.list.show_file_size {
                            let size = crate::ui::format_size(summary.size_bytes);
                            spans.push(Span::raw(" "));
                            spans.push(Span::styled(
                                format!("[{size}]"),
                                Style::default().fg(self.app_theme.muted),
                            ));
                        }

                        let secs = std::time::UNIX_EPOCH
                            + std::time::Duration::from_secs(summary.updated_at);
                        let dt: chrono::DateTime<chrono::Local> = secs.into();
                        let formatted = dt.format(&self.date_format).to_string();
                        spans.push(Span::raw(" "));
                        spans.push(Span::styled(
                            format!("({formatted})"),
                            Style::default().fg(self.app_theme.muted),
                        ));
                    }

                    if vi == self.list.visual_index && !self.list.inline_info {
                        spans.push(Span::styled(
                            format!("  ({when})"),
                            Style::default().fg(self.app_theme.muted),
                        ));
                    }
                    let mut lines = vec![Line::from(spans)];
                    if self.list.list_density == crate::config::ListDensity::Comfortable {
                        lines.push(Line::from(""));
                    }
                    items.push(ListItem::new(lines));
                }
                VisualItem::CreateNew { depth, .. } => {
                    let indent = "  ".repeat(*depth);
                    let icon =
                        crate::ui::get_icon("\u{f067}", "\u{2795}", self.config.ui.icon_mode);
                    let text = if icon.is_empty() {
                        format!("{indent}Create new...")
                    } else {
                        format!("{indent} {icon} Create new...")
                    };
                    let mut lines = vec![Line::from(vec![Span::styled(
                        text,
                        Style::default().fg(self.app_theme.success),
                    )])];
                    if self.list.list_density == crate::config::ListDensity::Comfortable {
                        lines.push(Line::from(""));
                    }
                    items.push(ListItem::new(lines));
                }
                VisualItem::SmartFolder {
                    kind,
                    label,
                    depth,
                    is_expanded,
                    note_count,
                } => {
                    let indent = "  ".repeat(*depth);
                    let icon_mode = self.config.ui.icon_mode;
                    let (nerd, unicode) = match kind {
                        SmartFolderKind::Today => ("\u{f133}", "\u{1f4c5}"),
                        SmartFolderKind::ThisWeek => ("\u{f073}", "\u{1f5d3}"),
                        SmartFolderKind::Untagged => ("\u{f187}", "\u{1f4e5}"),
                        SmartFolderKind::Tag(_) => ("\u{f02c}", "\u{1f3f7}"),
                        SmartFolderKind::Custom(_) => ("\u{f0e7}", "\u{26a1}"),
                    };

                    let arrow = if *is_expanded {
                        crate::ui::get_icon("\u{f078}", "\u{25bc}", icon_mode)
                    } else {
                        crate::ui::get_icon("\u{f054}", "\u{25b6}", icon_mode)
                    };

                    let folder_icon = crate::ui::get_icon(nerd, unicode, icon_mode);
                    let icon = format!("{arrow} {folder_icon}");
                    let color = self.app_theme.tag;
                    let count_str = format!("{}", note_count);
                    let count_suffix = if self.list.inline_info {
                        format!(" ({count_str})")
                    } else {
                        String::new()
                    };
                    let sanitized_name = crate::sanitize::sanitize_for_terminal(label);

                    let text = if icon.is_empty() {
                        format!("{indent}{sanitized_name}{count_suffix}")
                    } else {
                        format!("{indent}{icon} {sanitized_name}{count_suffix}")
                    };

                    let style = Style::default().add_modifier(Modifier::BOLD).fg(color);
                    let mut lines = vec![Line::from(vec![Span::styled(text, style)])];
                    if self.list.list_density == crate::config::ListDensity::Comfortable {
                        lines.push(Line::from(""));
                    }
                    items.push(ListItem::new(lines));
                }
                VisualItem::Subnote {
                    parent_id,
                    subnote_idx,
                    depth,
                } => {
                    let indent = "  ".repeat(*depth);
                    let icon =
                        crate::ui::get_icon("\u{f02c}", "\u{1f3f7}", self.config.ui.icon_mode);
                    // Look up the subnote title from the cache.
                    let title = self
                        .subnotes_view_cache
                        .iter()
                        .find_map(|(p, subs)| {
                            if p == parent_id {
                                subs.get(*subnote_idx).map(|s| s.title.clone())
                            } else {
                                None
                            }
                        })
                        .unwrap_or_else(|| format!("subnote {}", subnote_idx + 1));
                    let sanitized = crate::sanitize::sanitize_for_terminal(&title);
                    let text = if icon.is_empty() {
                        format!("{indent}{}", sanitized.into_owned())
                    } else {
                        format!("{indent}{icon} {}", sanitized.into_owned())
                    };
                    let style = Style::default().fg(self.app_theme.tag);
                    let mut lines = vec![Line::from(vec![Span::styled(text, style)])];
                    if self.list.list_density == crate::config::ListDensity::Comfortable {
                        lines.push(Line::from(""));
                    }
                    items.push(ListItem::new(lines));
                }
            }
        }
        self.list.display_items = items;
    }

    /// Suspend the TUI, run `command` (split on whitespace) with `extra_args`
    /// appended, wait for exit, then resume the TUI. Returns the command's exit
    /// status (or launch error) and the resolved program string for diagnostics.
    fn run_external_command(
        &mut self,
        command: &str,
        extra_args: &[String],
    ) -> (std::io::Result<std::process::ExitStatus>, String) {
        if let Err(e) = disable_raw_mode() {
            eprintln!("Failed to disable raw mode: {e}");
        }
        if let Err(e) = crossterm::execute!(
            std::io::stdout(),
            LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture,
            crossterm::event::DisableBracketedPaste
        ) {
            eprintln!("Failed to reset terminal: {e}");
        }

        let parts: Vec<&str> = command.split_whitespace().collect();
        let (program, cmd_args) = parts
            .split_first()
            .map(|(p, a)| (*p, a.to_vec()))
            .unwrap_or(("vi", vec![]));
        let mut command = std::process::Command::new(program);
        for arg in cmd_args {
            command.arg(arg);
        }
        for arg in extra_args {
            command.arg(arg);
        }
        let result = command.status();

        if let Err(e) = enable_raw_mode() {
            eprintln!("Failed to enable raw mode: {e}");
        }
        if let Err(e) = crossterm::execute!(
            std::io::stdout(),
            EnterAlternateScreen,
            crossterm::event::EnableMouseCapture,
            crossterm::event::EnableBracketedPaste,
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
        ) {
            eprintln!("Failed to restore terminal: {e}");
        }
        self.needs_full_redraw = true;
        (result, program.to_string())
    }
    /// Resolve the configured external editor and delegate to run_external_command.
    fn run_in_external_editor(
        &mut self,
        extra_args: &[String],
    ) -> (std::io::Result<std::process::ExitStatus>, String) {
        let editor_prog = self
            .editor
            .external_editor
            .clone()
            .or_else(|| std::env::var("VISUAL").ok())
            .or_else(|| std::env::var("EDITOR").ok())
            .unwrap_or_else(|| "vi".to_string());
        self.run_external_command(&editor_prog, extra_args)
    }

    pub fn open_path_in_external_editor(&mut self, path: &std::path::Path) {
        let (result, editor_prog) =
            self.run_in_external_editor(&[path.to_string_lossy().into_owned()]);

        match result {
            Ok(status) if status.success() => {
                self.set_temporary_status_static("External editor closed");
            }
            Ok(status) => {
                self.set_temporary_status(&format!(
                    "Editor '{editor_prog}' exited with status: {status}"
                ));
            }
            Err(e) => {
                self.set_temporary_status(&format!("Failed to launch editor '{editor_prog}': {e}"));
            }
        }
    }

    /// Launch an external preview command for the currently-selected note (Edit mode) or
    /// currently-visible note (list/graph modes). The TUI suspends, the preview command
    /// renders the note's content (or live editor buffer), and resumes on exit.
    fn open_external_preview(&mut self) {
        let content = if self.mode == ViewMode::Edit {
            // In edit mode, preview the live editor buffer (unsaved changes).
            if self.editor.editing_id.is_none() {
                self.set_temporary_status_static("No note open to preview");
                return;
            }
            self.editor.editor.lines().join("\n")
        } else {
            // In list/graph mode, preview the selected note.
            let item = match self.list.visual_list.get(self.list.visual_index) {
                Some(item) => item,
                None => {
                    self.set_temporary_status_static("No note open to preview");
                    return;
                }
            };

            match item {
                crate::list_view::VisualItem::Note { summary_idx, .. } => {
                    let note = match self.storage.load_note(&self.notes[*summary_idx].id) {
                        Ok(note) => note,
                        Err(e) => {
                            self.set_temporary_status(&format!("Failed to load note: {e}"));
                            return;
                        }
                    };
                    note.content.clone()
                }
                crate::list_view::VisualItem::Folder { .. }
                | crate::list_view::VisualItem::CreateNew { .. }
                | crate::list_view::VisualItem::SmartFolder { .. }
                | crate::list_view::VisualItem::Subnote { .. } => {
                    self.set_temporary_status_static(
                        "External preview only supports markdown notes",
                    );
                    return;
                }
            }
        };

        // Write content to a temp file with 0o600 permissions (secret).
        let temp_dir = std::env::temp_dir();
        let temp_file_path = temp_dir.join(format!("clin_preview_{}.md", uuid::Uuid::new_v4()));
        if let Err(e) = crate::fsutil::atomic_write_str(&temp_file_path, &content) {
            self.set_temporary_status(&format!("Failed to write temp file: {e}"));
            return;
        }

        // Resolve preview command: config -> $PAGER -> "less"
        let preview_prog = self
            .config
            .core
            .preview_command
            .clone()
            .or_else(|| std::env::var("PAGER").ok())
            .unwrap_or_else(|| "less".to_string());

        // Launch the external command.
        let (result, prog) = self.run_external_command(
            &preview_prog,
            &[temp_file_path.to_string_lossy().into_owned()],
        );

        // Report status based on command result.
        match result {
            Ok(status) if status.success() => {
                self.set_temporary_status_static("External preview closed");
            }
            Ok(status) => {
                self.set_temporary_status(&format!(
                    "Preview command '{prog}' exited with status: {status}"
                ));
            }
            Err(e) => {
                self.set_temporary_status(&format!(
                    "Failed to launch preview command '{prog}': {e}"
                ));
            }
        }
    }
    pub fn autosave(&mut self) {
        let content = self.editor.editor.lines().join("\n");

        if let Some(path) = &self.editor.template_edit_path
            && self.editor.editing_id.is_none()
        {
            let mut path_to_write = path.clone();
            if let Ok(template) = toml::from_str::<Template>(&content) {
                let new_path = self
                    .storage
                    .template_manager()
                    .template_path(&template.name);
                if new_path != *path && !new_path.exists() {
                    if let Err(e) = std::fs::rename(path, &new_path) {
                        self.set_temporary_status(&format!("Failed to rename template: {e}"));
                    } else {
                        path_to_write = new_path;
                        self.editor.template_edit_path = Some(path_to_write.clone());
                    }
                }
            }

            if let Err(e) = crate::fsutil::atomic_write_str(&path_to_write, &content) {
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
            self.editor.editing_id = Some(saved_id.clone());
            self.enqueue_backup(format!("auto: {}", note.title));

            let current_words = crate::goals::count_words(&note.content);
            let mut diff = 0;
            if current_words > self.editor.initial_word_count {
                diff = current_words - self.editor.initial_word_count;
            }
            self.editor.initial_word_count = current_words;

            let progress = self.get_current_goals_progress();
            progress.words_written += diff;
            progress.notes_modified.insert(saved_id);
            let progress_clone = progress.clone();
            self.save_goals_progress(&progress_clone);
        }
    }

    pub fn handle_menu_action(
        &mut self,
        action: usize,
        focus: &mut EditFocus,
        items: &[&'static str],
    ) {
        if self.editor.edit_mode == crate::editor::EditMode::Read {
            if let Some(label) = items.get(action) {
                match *label {
                    " Copy " => {
                        let text = crate::events::read_selection_text(self);
                        if !text.is_empty() {
                            crate::text_edit::write_system_clipboard(&text);
                        }
                    }
                    " Select All " => {
                        let last = self.editor.read_grid.len().saturating_sub(1);
                        let last_col = self
                            .editor
                            .read_grid
                            .last()
                            .map(|r| r.len().saturating_sub(1))
                            .unwrap_or(0);
                        self.editor.read_sel_anchor = Some((0, 0));
                        self.editor.read_sel_end = Some((last, last_col));
                        self.editor.read_selecting = false;
                    }
                    _ => {}
                }
            }
            return;
        }
        let textarea = match focus {
            EditFocus::Title => &mut self.editor.title_editor,
            EditFocus::Body => &mut self.editor.editor,
            EditFocus::Sidebar => return,
        };
        if let Some(label) = items.get(action) {
            match *label {
                " Copy " => {
                    textarea.copy();
                    crate::text_edit::write_system_clipboard(&textarea.yank_text());
                }
                " Cut " => {
                    if textarea.cut() {
                        crate::text_edit::write_system_clipboard(&textarea.yank_text());
                    }
                }
                " Paste " => {
                    if let Some(t) = crate::text_edit::read_system_clipboard() {
                        textarea.set_yank_text(&t);
                        textarea.paste();
                    }
                }
                " Select All " => {
                    textarea.select_all();
                }
                _ => {}
            }
        }
    }
    pub fn get_help_rows(&mut self) -> Vec<crate::ui::HelpRow> {
        if self.list.help_text_cache.is_none() {
            let rows = crate::ui::help_text_for_tab(
                self.help_tab,
                &self.keybinds,
                &self.app_theme,
                &self.config,
            );
            self.list.help_text_cache = Some(rows);
        }
        self.list.help_text_cache.clone().unwrap_or_default()
    }

    pub fn update_help_search(&mut self) {
        let query = match &self.help_search.popup {
            Some(popup) => popup.query().to_lowercase(),
            None => return,
        };
        let rows = self.get_help_rows();
        let popup = match &mut self.help_search.popup {
            Some(popup) => popup,
            None => return,
        };
        if query.is_empty() {
            popup.results.clear();
        } else {
            let results: Vec<_> = rows
                .iter()
                .enumerate()
                .filter(|(_, hr)| hr.search_text.to_lowercase().contains(&query))
                .map(|(i, hr)| (i, hr.display.clone()))
                .collect();
            popup.results = results;
        }
        if popup.selected >= popup.results.len() {
            popup.selected = popup.results.len().saturating_sub(1);
        }
        popup.scroll_to_selected(10);
    }

    pub fn initiate_quit(&mut self) {
        if self.confirm_on_quit {
            self.show_confirm(ConfirmAction::QuitApp);
        } else {
            self.should_quit = true;
        }
    }

    pub fn reload_theme(&mut self) {
        let config = crate::config::ClinConfig::load().unwrap_or_default();
        self.app_theme = crate::app_theme::AppThemeColors::from_config(&config.ui);
        self.build_display_lines();
        if self.mode == ViewMode::Help {
            self.list.help_text_cache = None;
        }
    }

    /// Re-derive `app_theme` from the in-memory `self.config` (no disk read).
    /// Used for live preview where config was mutated but not yet saved.
    pub fn refresh_theme_from_config(&mut self) {
        self.app_theme = crate::app_theme::AppThemeColors::from_config(&self.config.ui);
        self.build_display_lines();
        if self.mode == ViewMode::Help {
            self.list.help_text_cache = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::set_config_path_override;
    use crate::storage::Storage;
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::Rect;
    use ratatui_textarea::TextArea;
    use tempfile::tempdir;

    #[test]
    fn test_preview_render_cols() {
        assert_eq!(preview_render_cols(80, true), 76);
        assert_eq!(preview_render_cols(22, true), 20);
        assert_eq!(preview_render_cols(0, true), 78);
        assert_eq!(preview_render_cols(80, false), 1000);
        assert_eq!(preview_render_cols(0, false), 1000);
    }
    #[test]
    fn test_refresh_visual_list_requests_preview_update() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let temp_dir = tempdir().expect("value is present");
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        std::fs::create_dir_all(&data_dir).expect("value is present");
        std::fs::create_dir_all(&config_dir).expect("value is present");
        std::fs::create_dir_all(&notes_dir).expect("value is present");
        std::fs::create_dir_all(&templates_dir).expect("value is present");

        let storage = Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0u8; 32],
        };
        let mut app = App::new(storage).expect("value is present");
        app.list.preview_enabled = true;

        // Test Grid layout (visual list is empty -> preview_content_index is None)
        app.list.notes_layout = crate::config::NotesLayout::Grid;
        app.list.preview_content_index = Some(999);
        app.refresh_visual_list();
        assert!(!app.list.pending_preview_update);
        assert_eq!(app.list.preview_content_index, None);

        // Test Tree layout (visual list contains folders -> preview_content_index is Some(0))
        app.list.notes_layout = crate::config::NotesLayout::Tree;
        app.list.preview_content_index = Some(999);
        app.refresh_visual_list();
        assert!(!app.list.pending_preview_update);
        assert_eq!(app.list.preview_content_index, Some(0));
    }

    #[test]
    fn test_y_inserts_in_create_note_popup() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let temp_dir = tempdir().expect("value is present");
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        std::fs::create_dir_all(&data_dir).expect("value is present");
        std::fs::create_dir_all(&config_dir).expect("value is present");
        std::fs::create_dir_all(&notes_dir).expect("value is present");
        std::fs::create_dir_all(&templates_dir).expect("value is present");

        let storage = Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0u8; 32],
        };
        let mut app = App::new(storage).expect("value is present");

        // Open the create-note popup
        app.begin_create_note_in_folder(String::new());
        assert!(
            matches!(
                app.popups.active,
                Some(crate::popups::ActivePopup::CreateNote(..))
            ),
            "create_note popup should be open"
        );

        // Dispatch 'y' key — must insert, not confirm
        crate::events::handle_global_popups_and_palette(
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)),
            Rect::default(),
        );

        // Popup must still be open
        assert!(
            matches!(
                app.popups.active,
                Some(crate::popups::ActivePopup::CreateNote(..))
            ),
            "popup should remain open after y"
        );

        // Input must contain "y"
        let (popup, _) =
            if let Some(crate::popups::ActivePopup::CreateNote(p, f)) = &app.popups.active {
                (p, f)
            } else {
                panic!("create_note popup should be open")
            };
        let text: String = popup.input.lines().join("");
        assert_eq!(text, "y", "input should contain y, got: {text}");
    }

    #[test]
    fn test_external_editor_uses_saved_id() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let temp_dir = tempdir().expect("value is present");
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        std::fs::create_dir_all(&data_dir).expect("value is present");
        std::fs::create_dir_all(&config_dir).expect("value is present");
        std::fs::create_dir_all(&notes_dir).expect("value is present");
        std::fs::create_dir_all(&templates_dir).expect("value is present");

        let storage = Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0u8; 32],
        };
        let mut app = App::new(storage).expect("value is present");

        // Enable external editor with `false` (exits non-zero — proves load succeeded)
        app.editor.external_editor_enabled = true;
        app.editor.external_editor = Some("false".into());

        app.start_blank_note_with_title(String::new(), "Yellow".into());

        let status = app.status.to_string();
        assert!(
            !status.contains("Failed to load note"),
            "status should not say 'Failed to load note': {status}"
        );
    }

    #[test]
    fn test_goals_progress_tracking_autosave() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let temp_dir = tempdir().expect("value is present");
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        std::fs::create_dir_all(&data_dir).expect("value is present");
        std::fs::create_dir_all(&config_dir).expect("value is present");
        std::fs::create_dir_all(&notes_dir).expect("value is present");
        std::fs::create_dir_all(&templates_dir).expect("value is present");

        let storage = Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0u8; 32],
        };
        let mut app = App::new(storage).expect("value is present");
        app.editor.external_editor_enabled = false;
        // Initially no words written and no notes modified
        assert_eq!(app.goals_progress.words_written, 0);
        assert!(app.goals_progress.notes_modified.is_empty());

        // Create a new blank note and edit it
        app.start_blank_note_with_title(String::new(), "Test Note".to_string());

        // Editor starts with 0 words
        assert_eq!(app.editor.initial_word_count, 0);

        // Edit body: type 10 words
        let body_content = "one two three four five six seven eight nine ten";
        app.editor.editor = TextArea::from(body_content.lines());

        // Call autosave
        app.autosave();

        // Verify words_written is 10 and note ID is in notes_modified
        assert_eq!(app.goals_progress.words_written, 10);
        assert_eq!(app.goals_progress.notes_modified.len(), 1);

        // Edit note again: delete 3 words, and add 5 words (net new +2 words)
        let body_content_2 = "one two three four five six seven eight nine ten eleven twelve";
        app.editor.editor = TextArea::from(body_content_2.lines());
        app.autosave();

        // 10 + 2 = 12 words total
        assert_eq!(app.goals_progress.words_written, 12);

        // Edit note again: remove words (e.g. to 3 words)
        let body_content_3 = "one two three";
        app.editor.editor = TextArea::from(body_content_3.lines());
        app.autosave();

        // Should not decrease words_written (should remain 12)
        assert_eq!(app.goals_progress.words_written, 12);

        // Now create a second note
        app.start_blank_note_with_title(String::new(), "Second Note".to_string());
        assert_eq!(app.editor.initial_word_count, 0);
        app.editor.editor = TextArea::from(vec!["hello world"].into_iter().map(String::from));
        app.autosave();

        // words_written: 12 + 2 = 14
        assert_eq!(app.goals_progress.words_written, 14);
        // notes_modified: 2 unique notes
        assert_eq!(app.goals_progress.notes_modified.len(), 2);
    }

    #[test]
    fn test_incremental_refresh_on_back_to_list() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let temp_dir = tempdir().expect("value is present");
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        std::fs::create_dir_all(&data_dir).expect("value is present");
        std::fs::create_dir_all(&config_dir).expect("value is present");
        std::fs::create_dir_all(&notes_dir).expect("value is present");
        std::fs::create_dir_all(&templates_dir).expect("value is present");

        let storage = Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0u8; 32],
        };
        let mut app = App::new(storage).expect("value is present");

        // Create 3 notes, using the incremental refresh path for each
        for title in ["Note A", "Note B", "Note C"] {
            app.start_blank_note_with_title(String::new(), title.to_string());
            let prev = app.editor.editing_id.clone();
            app.autosave();
            let new = app.editor.editing_id.clone();
            app.back_to_list(prev.as_deref(), new.as_deref());
        }

        assert_eq!(app.notes.len(), 3, "should have 3 notes after setup");

        // Capture note B's id (use title to find it since ids are title-based)
        let b_id = app
            .notes
            .iter()
            .find(|n| n.title == "Note B")
            .map(|n| n.id.clone())
            .expect("Note B should exist");

        // Open note B, edit body, simulate back-to-list flow with incremental refresh
        app.load_and_open_note(&b_id, None);
        let body_content = "edited body content for note b";
        app.editor.editor = TextArea::from(body_content.lines());

        let prev_id = app.editor.editing_id.clone();
        app.autosave();
        let new_id = app.editor.editing_id.clone();
        app.back_to_list(prev_id.as_deref(), new_id.as_deref());

        // All 3 notes should still be present (incremental refresh preserved others)
        assert_eq!(
            app.notes.len(),
            3,
            "other notes preserved after incremental body edit"
        );

        // Note B should still exist with same id (body edit doesn't rename)
        let b_summary = app
            .notes
            .iter()
            .find(|n| n.id == b_id)
            .expect("Note B should still exist after body edit");
        assert!(
            b_summary.size_bytes > 30,
            "note summary should reflect larger body after edit (size_bytes={})",
            b_summary.size_bytes
        );

        // Rename case: change title, autosave renames the file
        let old_id = b_id.clone();
        app.load_and_open_note(&old_id, None);
        app.editor.title_editor = TextArea::from(vec!["Note B Renamed".to_string()].into_iter());

        let prev_id = app.editor.editing_id.clone();
        app.autosave();
        let new_id = app.editor.editing_id.clone();
        let renamed_id = new_id
            .clone()
            .expect("autosave should produce an id after rename");
        app.back_to_list(prev_id.as_deref(), new_id.as_deref());

        // Old id should be gone, new id present, still 3 notes
        assert!(
            !app.notes.iter().any(|n| n.id == old_id),
            "old note id should be removed after rename"
        );
        assert!(
            app.notes.iter().any(|n| n.id == renamed_id),
            "new note id should appear after rename"
        );
        assert_eq!(app.notes.len(), 3, "should still have 3 notes after rename");
    }

    #[test]
    fn test_theme_reload_updates_cached_display_items() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let config_path = crate::config::ClinConfig::config_path().expect("value is present");
        if let Some(parent) = config_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::remove_file(&config_path);

        let temp_dir = tempdir().expect("value is present");
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        std::fs::create_dir_all(&data_dir).expect("value is present");
        std::fs::create_dir_all(&config_dir).expect("value is present");
        std::fs::create_dir_all(&notes_dir).expect("value is present");
        std::fs::create_dir_all(&templates_dir).expect("value is present");

        let storage = Storage {
            data_dir,
            config_dir: config_dir.clone(),
            notes_dir,
            templates_dir,
            key: [0u8; 32],
        };
        let mut app = App::new(storage).expect("value is present");

        // Write a clean base config first so it has tokyo_night
        let config_content = crate::config::merge::default_config_content()
            .replace("theme = \"default\"", "theme = \"tokyo_night\"");
        std::fs::write(&config_path, config_content).expect("value is present");
        app.reload_theme();
        // Verify the theme colors changed
        assert_ne!(app.app_theme.accent, ratatui::style::Color::Cyan);
    }

    #[test]
    fn test_set_goals_actions() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let config_path = crate::config::ClinConfig::config_path().expect("value is present");
        if let Some(parent) = config_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::remove_file(&config_path);

        let temp_dir = tempdir().expect("value is present");
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        std::fs::create_dir_all(&data_dir).expect("value is present");
        std::fs::create_dir_all(&config_dir).expect("value is present");
        std::fs::create_dir_all(&notes_dir).expect("value is present");
        std::fs::create_dir_all(&templates_dir).expect("value is present");

        let storage = Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0u8; 32],
        };
        let mut app = App::new(storage).expect("value is present");

        // Initially defaults are 500 and 3
        assert_eq!(app.config.goals.word_goal, 500);
        assert_eq!(app.config.goals.note_goal, 3);

        // Execute set word goal action
        crate::actions::execute_action("settings.word_goal", &mut app, None)
            .expect("value is present");
        assert!(matches!(
            app.popups.active,
            Some(crate::popups::ActivePopup::Goals(_))
        ));

        let mut popup = if let Some(crate::popups::ActivePopup::Goals(p)) = app.popups.active.take()
        {
            p
        } else {
            panic!()
        };
        assert!(matches!(
            popup.mode,
            crate::popups::GoalsPopupMode::WordGoal
        ));

        // Enter new word goal: 750
        popup.input = TextArea::from(vec!["750".to_string()]);
        app.popups.active = Some(crate::popups::ActivePopup::Goals(popup));
        app.confirm_goals_popup();

        assert_eq!(app.config.goals.word_goal, 750);

        // Execute set note goal action
        crate::actions::execute_action("settings.note_goal", &mut app, None)
            .expect("value is present");
        assert!(matches!(
            app.popups.active,
            Some(crate::popups::ActivePopup::Goals(_))
        ));

        let mut popup2 =
            if let Some(crate::popups::ActivePopup::Goals(p)) = app.popups.active.take() {
                p
            } else {
                panic!()
            };
        assert!(matches!(
            popup2.mode,
            crate::popups::GoalsPopupMode::NoteGoal
        ));

        // Enter new note goal: 5
        popup2.input = TextArea::from(vec!["5".to_string()]);
        app.popups.active = Some(crate::popups::ActivePopup::Goals(popup2));
        app.confirm_goals_popup();

        assert_eq!(app.config.goals.note_goal, 5);
    }

    #[test]
    fn test_auto_reload_config_on_disk_change() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let config_path = crate::config::ClinConfig::config_path().expect("value is present");
        if let Some(parent) = config_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::remove_file(&config_path);

        let temp_dir = tempdir().expect("value is present");
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        std::fs::create_dir_all(&data_dir).expect("value is present");
        std::fs::create_dir_all(&config_dir).expect("value is present");
        std::fs::create_dir_all(&notes_dir).expect("value is present");
        std::fs::create_dir_all(&templates_dir).expect("value is present");

        let storage = Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0u8; 32],
        };
        let mut app = App::new(storage).expect("value is present");

        // Initially defaults to 500
        assert_eq!(app.config.goals.word_goal, 500);

        // Edit config on disk
        let config_content = r#"[goals]
word_goal = 1200
"#;
        std::fs::write(&config_path, config_content).expect("value is present");

        // Force a reload by clearing the cached mtime
        // Force a reload by clearing the cached mtime
        app.config_mtime = None;
        app.get_current_goals_progress();

        // Verify the config has been reloaded and word_goal is now 1200
        assert_eq!(app.config.goals.word_goal, 1200);
    }

    #[test]
    fn adjust_preview_width_to_clamps_to_max() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let temp_dir = tempfile::tempdir().expect("value is present");
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        std::fs::create_dir_all(&data_dir).expect("value is present");
        std::fs::create_dir_all(&config_dir).expect("value is present");
        std::fs::create_dir_all(&notes_dir).expect("value is present");
        std::fs::create_dir_all(&templates_dir).expect("value is present");

        let storage = crate::storage::Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0u8; 32],
        };
        let mut app = App::new(storage).expect("value is present");

        app.adjust_preview_width_to(5.0);
        assert!(
            (app.list.preview_width_ratio - 0.8).abs() < f32::EPSILON,
            "expected 0.8, got {}",
            app.list.preview_width_ratio
        );

        app.adjust_preview_width_to(-1.0);
        assert!(
            (app.list.preview_width_ratio - 0.2).abs() < f32::EPSILON,
            "expected 0.2, got {}",
            app.list.preview_width_ratio
        );
    }

    #[test]
    fn adjust_calendar_height_clamps() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let temp_dir = tempfile::tempdir().expect("value is present");
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        std::fs::create_dir_all(&data_dir).expect("value is present");
        std::fs::create_dir_all(&config_dir).expect("value is present");
        std::fs::create_dir_all(&notes_dir).expect("value is present");
        std::fs::create_dir_all(&templates_dir).expect("value is present");

        let storage = crate::storage::Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0u8; 32],
        };
        let mut app = App::new(storage).expect("value is present");

        app.adjust_calendar_height(-20);
        assert_eq!(app.list.calendar_height, 9);

        app.adjust_calendar_height(50);
        assert_eq!(app.list.calendar_height, 20);
    }

    #[test]
    fn test_view_mode_transitions_prevent_zombie_state() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let temp_dir = tempdir().expect("value is present");
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        std::fs::create_dir_all(&data_dir).expect("value is present");
        std::fs::create_dir_all(&config_dir).expect("value is present");
        std::fs::create_dir_all(&notes_dir).expect("value is present");
        std::fs::create_dir_all(&templates_dir).expect("value is present");

        let storage = Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0u8; 32],
        };
        let mut app = App::new(storage).expect("value is present");

        // 1. Initially mode is List, return_mode is None
        assert_eq!(app.mode, ViewMode::List);
        assert_eq!(app.return_mode, None);

        // 2. Open Backup view first time
        app.open_backup_view();
        assert_eq!(app.mode, ViewMode::Backup);
        assert_eq!(app.return_mode, Some(ViewMode::List));

        // 3. Open Backup view a second time (e.g. from command palette while in Backup)
        app.open_backup_view();
        assert_eq!(app.mode, ViewMode::Backup);
        assert_eq!(app.return_mode, Some(ViewMode::List)); // Should STILL be List, NOT Backup!

        // 4. Simulate exit back
        let prev_mode = app.return_mode.take().unwrap_or(ViewMode::List);
        app.mode = prev_mode;
        assert_eq!(app.mode, ViewMode::List);
    }

    #[test]
    fn test_folder_expand_and_collapse_operations() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let temp_dir = tempdir().expect("value is present");
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        std::fs::create_dir_all(&data_dir).expect("value is present");
        std::fs::create_dir_all(&config_dir).expect("value is present");
        std::fs::create_dir_all(&notes_dir).expect("value is present");
        std::fs::create_dir_all(&templates_dir).expect("value is present");

        let storage = Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0u8; 32],
        };
        let mut app = App::new(storage).expect("value is present");

        // Mock folder cache
        app.list.folder_cache = Some(vec![
            "a".to_string(),
            "a/b".to_string(),
            "a/b/c".to_string(),
            "other".to_string(),
        ]);

        // 1. Test expand_all_folders
        app.expand_all_folders();
        assert!(app.list.folder_expanded.contains(""));
        assert!(app.list.folder_expanded.contains(VIRTUAL_PINNED_PATH));
        assert!(app.list.folder_expanded.contains("a"));
        assert!(app.list.folder_expanded.contains("a/b"));
        assert!(app.list.folder_expanded.contains("a/b/c"));
        assert!(app.list.folder_expanded.contains("other"));

        // 2. Test collapse_all_folders
        app.list.visual_index = 4;
        app.collapse_all_folders();
        assert_eq!(app.list.visual_index, 0);
        assert!(app.list.folder_expanded.contains(""));
        assert!(!app.list.folder_expanded.contains("a"));
        assert!(!app.list.folder_expanded.contains("a/b"));

        // 3. Test expand_to_level
        app.expand_to_level(2); // Should expand depth < 2 (root = 0, "a" = 1, "other" = 1)
        assert!(app.list.folder_expanded.contains(""));
        assert!(app.list.folder_expanded.contains(VIRTUAL_PINNED_PATH));
        assert!(app.list.folder_expanded.contains("a"));
        assert!(app.list.folder_expanded.contains("other"));
        assert!(!app.list.folder_expanded.contains("a/b")); // depth = 2 is not < 2

        app.expand_to_level(3); // Should expand depth < 3 (includes "a/b" depth 2)
        assert!(app.list.folder_expanded.contains("a/b"));
        assert!(!app.list.folder_expanded.contains("a/b/c")); // depth = 3 is not < 3
    }

    #[test]
    fn test_startup_folder_expansion_config_and_default_depth() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let config_path = crate::config::ClinConfig::config_path().expect("value is present");
        if let Some(parent) = config_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::remove_file(&config_path);

        let temp_dir = tempdir().expect("value is present");
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        std::fs::create_dir_all(&data_dir).expect("value is present");
        std::fs::create_dir_all(&config_dir).expect("value is present");
        std::fs::create_dir_all(&notes_dir).expect("value is present");
        std::fs::create_dir_all(&templates_dir).expect("value is present");

        let storage = Storage {
            data_dir,
            config_dir: config_dir.clone(),
            notes_dir,
            templates_dir,
            key: [0u8; 32],
        };
        let config_content = crate::config::merge::default_config_content().replace(
            "preview_enabled = true",
            "preview_enabled = true\nexpanded_folders = [\"a\", \"a/b\"]",
        );
        std::fs::write(&config_path, config_content).expect("value is present");
        set_config_path_override(config_path.clone());

        // Create App, should load folders
        let app = App::new(storage.clone()).expect("value is present");
        assert!(app.list.folder_expanded.contains("a"));
        assert!(app.list.folder_expanded.contains("a/b"));
        assert!(!app.list.folder_expanded.contains("other"));

        // Write config with default_expand_depth = 2
        let config_content = crate::config::merge::default_config_content().replace(
            "preview_enabled = true",
            "preview_enabled = true\ndefault_expand_depth = 2",
        );
        std::fs::write(&config_path, config_content).expect("value is present");

        // Re-create App, should expand up to depth 2 (since expanded_folders is empty now)
        let mut app2 = App::new(storage).expect("value is present");
        // Mock folder cache
        app2.list.folder_cache = Some(vec![
            "a".to_string(),
            "a/b".to_string(),
            "a/b/c".to_string(),
            "other".to_string(),
        ]);
        // Trigger expansion to depth
        app2.expand_folders_to_depth(2);
        assert!(app2.list.folder_expanded.contains("a"));
        assert!(app2.list.folder_expanded.contains("other"));
        assert!(!app2.list.folder_expanded.contains("a/b"));

        let _ = std::fs::remove_file(&config_path);
    }
}
