pub mod messages;

pub(crate) mod catalog;
mod edit_panes;
pub(crate) mod folder_preview;
mod folders;
mod import_ops;
mod loading;
mod notes;
mod popups;
mod search;
pub(crate) mod search_worker;
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
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

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
    Outline,
    Setup,
}

impl ViewMode {
    /// Help tab related to this view, or `None` when F1 should not open help
    /// from this view (Help: F1 toggles close via `HelpAction::Close`;
    /// Setup: intentionally no help path).
    #[must_use]
    pub fn help_tab(self) -> Option<HelpTab> {
        match self {
            ViewMode::List => Some(HelpTab::Notes),
            ViewMode::Edit => Some(HelpTab::Editor),
            ViewMode::Graph => Some(HelpTab::Graph),
            ViewMode::Draw => Some(HelpTab::Draw),
            ViewMode::Canvas => Some(HelpTab::Canvas),
            ViewMode::Backup => Some(HelpTab::Backup),
            ViewMode::Outline => Some(HelpTab::Notes), // no Outline tab; matches existing OutlineAction::Help target
            ViewMode::Help => None,
            ViewMode::Setup => None,
        }
    }
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
pub struct WatchedFsEvent {
    pub observed_at: Instant,
    pub event: notify::Event,
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
    pub quick_keybinds_open: bool,
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
    pub host: Box<dyn crate::host::HostHooks>,
    pub app_theme: crate::app_theme::AppThemeColors,
    pub graph_state: Option<crate::graf::app::GrafAppState>,
    pub draw_state: Option<crate::draw::app::DrawAppState>,
    pub backup_state: Option<crate::backup::state::BackupState>,
    pub outline_state: Option<crate::outline::state::OutlineState>,
    pub setup_state: Option<crate::setup::SetupState>,
    pub(crate) setup_rebootstrap: Option<crate::setup::SetupRebootstrapRequest>,
    pub config_errors: Vec<String>,
    pub canvas_state: Option<crate::pinstar::state::PinstarState>,
    pub config: crate::config::ClinConfig,
    pub catalog_cmd_tx: std::sync::mpsc::SyncSender<crate::app::catalog::CatalogCommand>,
    pub catalog_event_rx: std::sync::mpsc::Receiver<crate::app::catalog::CatalogEvent>,
    pub catalog_generation: Arc<AtomicU64>,
    pub catalog_folders: Vec<String>,
    pub catalog_status: Option<String>,
    pub(crate) search_worker: crate::app::search_worker::SearchWorker,
    pub search_debounce_deadline: Option<Instant>,
    pub search_query_generation: Arc<AtomicU64>,
    pub(crate) unsent_search_request: Option<crate::app::search_worker::SearchRequest>,
    pub search_status: Option<String>,
    pub note_index: Option<crate::note_index::NoteIndex>,
    pub folder_preview_service: crate::app::folder_preview::FolderPreviewService,
    pub folder_preview_catalog: Option<Arc<crate::app::folder_preview::FolderPreviewCatalog>>,
    #[allow(dead_code)]
    pub(crate) folder_preview_model: Option<Arc<crate::app::folder_preview::FolderGraphModel>>,
    pub notes_revision: u64,
    pub note_stamps: HashMap<String, crate::storage::FileStamp>,
    pub subnotes_view_cache: Vec<(String, Vec<crate::storage::SubNote>)>,
    pub subnotes_view_cache_sig: usize,
    pub notes_with_subnotes: std::collections::HashSet<String>,
    pub fs_event_rx: Option<std::sync::mpsc::Receiver<WatchedFsEvent>>,
    pub fs_overflow: Arc<AtomicBool>,
    pub watcher_window_start: Option<Instant>,
    pub initial_load_done: bool,
    pub is_first_cache_build: bool,
    pub load_spinner_tick: usize,
    pub backup_tx: Option<std::sync::mpsc::Sender<crate::backup::worker::BackupJob>>,
    pub git_lock: Arc<Mutex<()>>,
    pub backup_status: Arc<Mutex<Option<String>>>,
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
    pub notes_worker_pool: Arc<rayon::ThreadPool>,
    pub fps: f64,
    pub last_frame_time: std::time::Instant,
    pub messages: crate::app::messages::MessageOverlay,
    pub message_tx: std::sync::mpsc::Sender<crate::app::messages::OverlayMessage>,
    pub message_rx: std::sync::mpsc::Receiver<crate::app::messages::OverlayMessage>,
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
    pub fn rebuild_note_index(&mut self) {
        let now = crate::ui::now_unix_secs();
        let index = crate::note_index::NoteIndex::build(
            self.notes_revision,
            &self.notes,
            &self.catalog_folders,
            &self.config.list.custom_smart_folders,
            now,
        );
        let preview_cat = crate::app::folder_preview::FolderPreviewCatalog::build(
            self.notes_revision,
            &self.notes,
            &self.catalog_folders,
            &index,
        );
        self.note_index = Some(index);
        self.folder_preview_catalog = Some(preview_cat);
    }
    pub fn desired_list_preview_height(&self) -> u16 {
        self.list.last_preview_pane_height
    }

    pub fn desired_editor_preview_height(&self) -> u16 {
        self.editor.last_preview_pane_height
    }

    fn build_notes_worker_pool() -> anyhow::Result<Arc<rayon::ThreadPool>> {
        let threads = std::thread::available_parallelism()
            .map(|n| n.get().saturating_sub(1).max(1))
            .unwrap_or(1);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .thread_name(|i| format!("notes-worker-{i}"))
            .build()?;
        Ok(Arc::new(pool))
    }

    pub fn new(storage: Storage) -> Result<Self> {
        let bootstrap_config = crate::config::ClinConfig::load().0.unwrap_or_default();
        let notes_worker_pool = Self::build_notes_worker_pool()?;
        let config_errors = bootstrap_config.validate();
        let (keybinds, keybind_warnings) =
            storage.load_keybinds_with_preset(bootstrap_config.core.keybind_preset);
        let mut theme_warnings = Vec::new();
        let app_theme = crate::app_theme::AppThemeColors::from_config(
            &bootstrap_config.ui,
            &mut theme_warnings,
        );

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
        list.week_start = bootstrap_config.list.week_start;
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

        let (cmd_tx, cmd_rx) = std::sync::mpsc::sync_channel(32);
        let (evt_tx, evt_rx) = std::sync::mpsc::sync_channel(4);
        let catalog_generation = Arc::new(AtomicU64::new(1));

        let load = crate::app::catalog::load_notes_blocking(
            &storage,
            &notes_worker_pool,
            bootstrap_config.list.show_hidden_files,
            bootstrap_config.list.show_all_files,
        )?;

        let vault_id = crate::local_state::vault_identity_path(&storage.data_dir)?;
        let digest = crate::paths::vault_cache_digest(&vault_id);
        let app_paths = crate::paths::AppPaths::discover(
            crate::config::ClinConfig::config_path().unwrap_or_default(),
        )?;
        let scoped_cache_path = app_paths.scoped_summary_cache_path(&digest);
        let legacy_cache_path = app_paths.summary_cache_path();

        let notes = load.summaries;
        let initial_complete = load.complete;
        let catalog_folders = load.folders;
        let note_stamps: HashMap<String, crate::storage::FileStamp> =
            load.map.iter().map(|(k, (s, _))| (k.clone(), *s)).collect();

        crate::app::catalog::spawn_catalog_worker(
            storage.clone(),
            notes_worker_pool.clone(),
            scoped_cache_path,
            legacy_cache_path,
            digest,
            bootstrap_config.list.show_hidden_files,
            bootstrap_config.list.show_all_files,
            load.map,
            catalog_folders.clone(),
            initial_complete,
            catalog_generation.clone(),
            cmd_rx,
            evt_tx,
        );

        if !initial_complete {
            let _ = cmd_tx.try_send(crate::app::catalog::CatalogCommand::Reconcile {
                generation: 1,
                show_hidden: bootstrap_config.list.show_hidden_files,
                show_all: bootstrap_config.list.show_all_files,
            });
        }

        let (message_tx, message_rx) = std::sync::mpsc::channel();

        let mut app = Self {
            storage: storage.clone(),
            notes_worker_pool: notes_worker_pool.clone(),
            keybinds,
            seq_matcher: crate::keybinds::KeyMatcher::new(),
            notes,
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
            quick_keybinds_open: false,
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
            outline_state: None,
            setup_state: None,
            setup_rebootstrap: None,
            pinned_on_top: bootstrap_config.list.pinned_on_top,
            default_folder: bootstrap_config.core.default_folder.clone(),
            return_mode: None,
            host: Box::new(crate::host::TuiHost),
            app_theme,
            canvas_state: None,
            config: bootstrap_config,
            catalog_cmd_tx: cmd_tx,
            catalog_event_rx: evt_rx,
            catalog_generation,
            catalog_folders,
            catalog_status: None,
            search_status: None,
            search_worker: crate::app::search_worker::SearchWorker::spawn(
                storage.clone(),
                notes_worker_pool.clone(),
            ),
            search_debounce_deadline: None,
            search_query_generation: Arc::new(AtomicU64::new(1)),
            unsent_search_request: None,
            note_index: None,
            folder_preview_service: crate::app::folder_preview::FolderPreviewService::spawn(
                notes_worker_pool.clone(),
            ),
            folder_preview_catalog: None,
            folder_preview_model: None,
            notes_revision: 0,
            note_stamps,
            notes_with_subnotes: std::collections::HashSet::new(),
            subnotes_view_cache: Vec::new(),
            subnotes_view_cache_sig: 0,
            initial_load_done: initial_complete,
            is_first_cache_build: false,
            load_spinner_tick: 0,
            backup_tx: None,
            git_lock: Arc::new(Mutex::new(())),
            backup_status: Arc::new(Mutex::new(None)),
            fs_event_rx: None,
            fs_overflow: Arc::new(AtomicBool::new(false)),
            watcher_window_start: None,
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
            fps: 0.0,
            last_frame_time: std::time::Instant::now(),
            messages: crate::app::messages::MessageOverlay::default(),
            message_tx,
            message_rx,
        };
        for w in keybind_warnings {
            app.messages
                .push(w, crate::app::messages::MessageSeverity::Warning);
        }
        for w in theme_warnings {
            app.messages
                .push(w, crate::app::messages::MessageSeverity::Warning);
        }
        app.goals_progress = app.load_goals_progress();
        app.list.folder_expanded.insert(String::new());

        if let Ok(vault_id) = crate::local_state::vault_identity_path(&app.storage.data_dir) {
            let vault_key = vault_id.to_string_lossy().into_owned();
            if let Ok(paths) = crate::paths::AppPaths::discover(
                crate::config::ClinConfig::config_path().unwrap_or_default(),
            ) {
                if let Ok(state) = crate::local_state::LocalState::load(&paths.state_path()) {
                    if let Some(vault_state) = state.vaults.get(&vault_key) {
                        if !vault_state.expanded_folders.is_empty() {
                            for folder in &vault_state.expanded_folders {
                                app.list.folder_expanded.insert(folder.clone());
                            }
                        } else if let Some(d) = app.config.list.default_expand_depth {
                            app.expand_folders_to_depth(d);
                        }
                    } else if let Some(d) = app.config.list.default_expand_depth {
                        app.expand_folders_to_depth(d);
                    }
                } else if let Some(d) = app.config.list.default_expand_depth {
                    app.expand_folders_to_depth(d);
                }
            } else if let Some(d) = app.config.list.default_expand_depth {
                app.expand_folders_to_depth(d);
            }
        } else if let Some(d) = app.config.list.default_expand_depth {
            app.expand_folders_to_depth(d);
        }

        app.list.pending_preview_update = true;
        app.sort_notes();
        app.refresh_visual_list();
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
        let bootstrap_config = crate::config::ClinConfig::load().0.unwrap_or_default();
        let config_errors = bootstrap_config.validate();
        let notes_worker_pool = Self::build_notes_worker_pool().unwrap_or_else(|_| {
            Arc::new(
                rayon::ThreadPoolBuilder::new()
                    .num_threads(1)
                    .build()
                    .expect("single thread pool"),
            )
        });
        let (keybinds, keybind_warnings) =
            storage.load_keybinds_with_preset(bootstrap_config.core.keybind_preset);
        let mut theme_warnings = Vec::new();
        let app_theme = crate::app_theme::AppThemeColors::from_config(
            &bootstrap_config.ui,
            &mut theme_warnings,
        );

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
        list.week_start = bootstrap_config.list.week_start;
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

        let (cmd_tx, cmd_rx) = std::sync::mpsc::sync_channel(32);
        let (evt_tx, evt_rx) = std::sync::mpsc::sync_channel(4);
        let catalog_generation = Arc::new(AtomicU64::new(1));

        let vault_id = crate::local_state::vault_identity_path(&storage.data_dir)
            .unwrap_or_else(|_| storage.data_dir.join("vault_id"));
        let digest = crate::paths::vault_cache_digest(&vault_id);
        let app_paths = crate::paths::AppPaths::discover(
            crate::config::ClinConfig::config_path().unwrap_or_default(),
        );
        let scoped_cache_path = app_paths
            .as_ref()
            .map(|p| p.scoped_summary_cache_path(&digest))
            .unwrap_or_default();
        let legacy_cache_path = app_paths
            .as_ref()
            .map(|p| p.summary_cache_path())
            .unwrap_or_default();

        let (cached_summaries, cached_map, cached_folders) =
            crate::app::catalog::load_persisted_note_cache(
                &storage,
                &scoped_cache_path,
                &digest,
                bootstrap_config.list.show_hidden_files,
                bootstrap_config.list.show_all_files,
            );

        let initial_complete = false;
        let notes = cached_summaries;
        let catalog_folders = cached_folders;
        let note_stamps: HashMap<String, crate::storage::FileStamp> = cached_map
            .iter()
            .map(|(k, (s, _))| (k.clone(), *s))
            .collect();

        crate::app::catalog::spawn_catalog_worker(
            storage.clone(),
            notes_worker_pool.clone(),
            scoped_cache_path,
            legacy_cache_path,
            digest,
            bootstrap_config.list.show_hidden_files,
            bootstrap_config.list.show_all_files,
            cached_map,
            catalog_folders.clone(),
            initial_complete,
            catalog_generation.clone(),
            cmd_rx,
            evt_tx,
        );

        let _ = cmd_tx.try_send(crate::app::catalog::CatalogCommand::Reconcile {
            generation: 1,
            show_hidden: bootstrap_config.list.show_hidden_files,
            show_all: bootstrap_config.list.show_all_files,
        });

        let (message_tx, message_rx) = std::sync::mpsc::channel();
        let mut app = Self {
            storage: storage.clone(),
            keybinds,
            seq_matcher: crate::keybinds::KeyMatcher::new(),
            notes,
            editor,
            list,
            notes_worker_pool: notes_worker_pool.clone(),
            mode: ViewMode::List,
            status: Cow::Borrowed("Validating notes…"),
            status_until: None,
            help_page: 0,
            help_tab: HelpTab::Notes,
            help_page_size: 20,
            help_tab_page: HashMap::new(),
            help_search: HelpSearchState::default(),
            help_info_active: 0,
            help_suggestions: Vec::new(),
            command_palette: None,
            quick_keybinds_open: false,
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
            outline_state: None,
            setup_state: None,
            setup_rebootstrap: None,
            pinned_on_top: bootstrap_config.list.pinned_on_top,
            default_folder: bootstrap_config.core.default_folder.clone(),
            return_mode: None,
            host: Box::new(crate::host::TuiHost),
            app_theme,
            canvas_state: None,
            config: bootstrap_config,
            catalog_cmd_tx: cmd_tx,
            catalog_event_rx: evt_rx,
            catalog_generation,
            catalog_folders,
            catalog_status: Some("Validating notes…".to_string()),
            search_status: None,
            search_worker: crate::app::search_worker::SearchWorker::spawn(
                storage.clone(),
                notes_worker_pool.clone(),
            ),
            search_debounce_deadline: None,
            search_query_generation: Arc::new(AtomicU64::new(1)),
            unsent_search_request: None,
            note_index: None,
            folder_preview_service: crate::app::folder_preview::FolderPreviewService::spawn(
                notes_worker_pool.clone(),
            ),
            folder_preview_catalog: None,
            folder_preview_model: None,
            notes_revision: 0,
            note_stamps,
            notes_with_subnotes: std::collections::HashSet::new(),
            subnotes_view_cache: Vec::new(),
            subnotes_view_cache_sig: 0,
            initial_load_done: false,
            is_first_cache_build: false,
            load_spinner_tick: 0,
            backup_tx: None,
            git_lock: Arc::new(Mutex::new(())),
            backup_status: Arc::new(Mutex::new(None)),
            fs_event_rx: None,
            fs_overflow: Arc::new(AtomicBool::new(false)),
            watcher_window_start: None,
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
            fps: 0.0,
            last_frame_time: std::time::Instant::now(),
            messages: crate::app::messages::MessageOverlay::default(),
            message_tx,
            message_rx,
        };
        for w in keybind_warnings {
            app.messages
                .push(w, crate::app::messages::MessageSeverity::Warning);
        }
        for w in theme_warnings {
            app.messages
                .push(w, crate::app::messages::MessageSeverity::Warning);
        }
        app.goals_progress = app.load_goals_progress();
        app.list.folder_expanded.insert(String::new());

        if let Ok(vault_id) = crate::local_state::vault_identity_path(&app.storage.data_dir) {
            let vault_key = vault_id.to_string_lossy().into_owned();
            if let Ok(paths) = crate::paths::AppPaths::discover(
                crate::config::ClinConfig::config_path().unwrap_or_default(),
            ) {
                if let Ok(state) = crate::local_state::LocalState::load(&paths.state_path()) {
                    if let Some(vault_state) = state.vaults.get(&vault_key) {
                        if !vault_state.expanded_folders.is_empty() {
                            for folder in &vault_state.expanded_folders {
                                app.list.folder_expanded.insert(folder.clone());
                            }
                        } else if let Some(d) = app.config.list.default_expand_depth {
                            app.expand_folders_to_depth(d);
                        }
                    } else if let Some(d) = app.config.list.default_expand_depth {
                        app.expand_folders_to_depth(d);
                    }
                } else if let Some(d) = app.config.list.default_expand_depth {
                    app.expand_folders_to_depth(d);
                }
            } else if let Some(d) = app.config.list.default_expand_depth {
                app.expand_folders_to_depth(d);
            }
        } else if let Some(d) = app.config.list.default_expand_depth {
            app.expand_folders_to_depth(d);
        }
        if app.config.accent_hint_migrated {
            app.set_temporary_status(
                "Hint bar style \u{2018}Accent\u{2019} was removed; using Classic.",
            );
            app.config.accent_hint_migrated = false;
            if let Err(e) = app.config.save() {
                app.messages.push(
                    format!("Failed to save config: {e}"),
                    crate::app::messages::MessageSeverity::Warning,
                );
            }
        }
        app.sort_notes();
        app.refresh_visual_list();
        Ok(app)
    }
    pub fn reload_config(&mut self) {
        let (config_res, load_warnings) = crate::config::ClinConfig::load();
        self.config = match config_res {
            Ok(c) => c,
            Err(e) => {
                self.messages.push(
                    format!("Config reload error: {e}"),
                    crate::app::messages::MessageSeverity::Warning,
                );
                self.config.clone()
            }
        };
        for w in load_warnings {
            self.messages
                .push(w, crate::app::messages::MessageSeverity::Warning);
        }
        self.preview_wrap = self.config.core.preview_wrap;
        let mut theme_warnings = Vec::new();
        self.app_theme =
            crate::app_theme::AppThemeColors::from_config(&self.config.ui, &mut theme_warnings);
        for w in theme_warnings {
            self.messages
                .push(w, crate::app::messages::MessageSeverity::Warning);
        }
        self.list.pinned_folders = self.config.list.pinned_folders.iter().cloned().collect();
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

    pub fn format_visual_item(&self, vi: usize) -> ListItem<'static> {
        let Some(item) = self.list.visual_list.get(vi) else {
            return ListItem::new(Vec::<Line<'static>>::new());
        };
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
                            crate::ui::get_icon("\u{f078}", "\u{25bc}", self.config.ui.icon_mode),
                            crate::ui::get_icon("\u{f08d}", "\u{1f4cc}", self.config.ui.icon_mode)
                        )
                    } else {
                        format!(
                            "{} {}",
                            crate::ui::get_icon("\u{f054}", "\u{25b6}", self.config.ui.icon_mode),
                            crate::ui::get_icon("\u{f08d}", "\u{1f4cc}", self.config.ui.icon_mode)
                        )
                    }
                } else {
                    let folder_glyph = if *path == crate::app::VIRTUAL_SUBNOTES_PATH {
                        crate::ui::get_icon("\u{f02c}", "\u{1f3f7}", self.config.ui.icon_mode)
                    } else if path.starts_with("subnotes:") {
                        crate::ui::get_icon("\u{f15b}", "\u{1f4c3}", self.config.ui.icon_mode)
                    } else {
                        crate::ui::get_icon("\u{f07b}", "\u{1f4c1}", self.config.ui.icon_mode)
                    };
                    if *is_expanded {
                        format!(
                            "{} {}",
                            crate::ui::get_icon("\u{f078}", "\u{25bc}", self.config.ui.icon_mode),
                            folder_glyph
                        )
                    } else {
                        format!(
                            "{} {}",
                            crate::ui::get_icon("\u{f054}", "\u{25b6}", self.config.ui.icon_mode),
                            folder_glyph
                        )
                    }
                };
                let color = if *is_pinned || *path == crate::app::VIRTUAL_PINNED_PATH {
                    self.app_theme.pinned
                } else if *path == crate::app::VIRTUAL_SMART_PATH {
                    self.app_theme.smart
                } else if *path == crate::app::VIRTUAL_SUBNOTES_PATH
                    || path.starts_with("subnotes:")
                {
                    self.app_theme.subnote
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
                ListItem::new(lines)
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
                                .fg(self.app_theme.pinned)
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
                if !summary.pinned && !*is_clin && !*is_draw && !*is_canvas {
                    let ext = std::path::Path::new(&summary.id)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    let is_unknown =
                        ext != "md" && ext != "txt" && !crate::storage::is_image_ext(ext);
                    let (nerd, unicode) = if is_unknown {
                        ("\u{3f}", "\u{3f}")
                    } else {
                        ("\u{f15c}", "\u{1f4c4}")
                    };
                    let icon = crate::ui::get_icon(nerd, unicode, self.config.ui.icon_mode);
                    if !icon.is_empty() {
                        spans.push(Span::styled(
                            format!("{icon} "),
                            Style::default()
                                .fg(self.app_theme.text)
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

                    let secs =
                        std::time::UNIX_EPOCH + std::time::Duration::from_secs(summary.updated_at);
                    let dt: chrono::DateTime<chrono::Local> = secs.into();
                    let formatted = dt.format(&self.date_format).to_string();
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        format!("({formatted})"),
                        Style::default().fg(self.app_theme.muted),
                    ));
                }

                let mut lines = vec![Line::from(spans)];
                if self.list.list_density == crate::config::ListDensity::Comfortable {
                    lines.push(Line::from(""));
                }
                ListItem::new(lines)
            }
            VisualItem::CreateNew { depth, .. } => {
                let indent = "  ".repeat(*depth);
                let icon = crate::ui::get_icon("\u{f067}", "\u{2795}", self.config.ui.icon_mode);
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
                ListItem::new(lines)
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
                    SmartFolderKind::Tagged => ("\u{f0e7}", "\u{26a1}"),
                };

                let arrow = if *is_expanded {
                    crate::ui::get_icon("\u{f078}", "\u{25bc}", icon_mode)
                } else {
                    crate::ui::get_icon("\u{f054}", "\u{25b6}", icon_mode)
                };

                let folder_icon = crate::ui::get_icon(nerd, unicode, icon_mode);
                let icon = format!("{arrow} {folder_icon}");
                let color = self.app_theme.smart;
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
                ListItem::new(lines)
            }
            VisualItem::Subnote {
                parent_id,
                subnote_idx,
                depth,
            } => {
                let indent = "  ".repeat(*depth);
                let icon = crate::ui::get_icon("\u{f02c}", "\u{1f3f7}", self.config.ui.icon_mode);
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
                let style = Style::default().fg(self.app_theme.subnote);
                let mut lines = vec![Line::from(vec![Span::styled(text, style)])];
                if self.list.list_density == crate::config::ListDensity::Comfortable {
                    lines.push(Line::from(""));
                }
                ListItem::new(lines)
            }
        }
    }

    /// Suspend the TUI, run `command` (split on whitespace) with `extra_args`
    /// appended, wait for exit, then resume the TUI. Returns the command's exit
    /// status (or launch error) and the resolved program string for diagnostics.
    fn run_external_command(
        &mut self,
        command: &str,
        extra_args: &[String],
    ) -> (std::io::Result<std::process::ExitStatus>, String) {
        self.host.suspend_for_external();

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

        self.host.resume_from_external();
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
            self.editor.body.lines().join("\n")
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
        let clin_temp = std::env::temp_dir().join("clin");
        let _ = std::fs::create_dir_all(&clin_temp);
        let temp_file_path = clin_temp.join(format!("clin_preview_{}.md", uuid::Uuid::new_v4()));
        if let Err(e) = crate::fsutil::atomic_write_str(&temp_file_path, &content) {
            self.set_temporary_status(&format!("Failed to write temp file: {e}"));
            self.messages.push(
                format!("Failed to write temp file: {e}"),
                crate::app::messages::MessageSeverity::Warning,
            );
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
        let content = self.editor.body.lines().join("\n");

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
                        self.messages.push(
                            format!("Failed to rename template: {e}"),
                            crate::app::messages::MessageSeverity::Warning,
                        );
                    } else {
                        path_to_write = new_path;
                        self.editor.template_edit_path = Some(path_to_write.clone());
                    }
                }
            }

            if let Err(e) = crate::fsutil::atomic_write_str(&path_to_write, &content) {
                self.set_temporary_status(&format!("Template save failed: {e}"));
                self.messages.push(
                    format!("Template save failed: {e}"),
                    crate::app::messages::MessageSeverity::Warning,
                );
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
        match self.storage.save_note(&id, &note) {
            Ok(saved_id) => {
                self.editor.editing_id = Some(saved_id.clone());
                self.enqueue_backup(format!("auto: {}", note.title));

                let current_words = crate::goals::count_words(&note.content);
                let mut diff = 0;
                if current_words > self.editor.initial_word_count {
                    diff = current_words - self.editor.initial_word_count;
                }
                self.editor.initial_word_count = current_words;

                let vault_identity =
                    crate::local_state::vault_identity_path(&self.storage.data_dir)
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|_| self.storage.data_dir.to_string_lossy().into_owned());
                let progress = {
                    let progress = self.get_current_goals_progress();
                    progress.words_written += diff;
                    progress.notes_modified.insert(crate::goals::TrackedNote {
                        vault: vault_identity,
                        note_id: saved_id,
                    });
                    progress.clone()
                };
                if let Err(error) = self.save_goals_progress(&progress) {
                    self.set_temporary_status(&format!("Failed to save local state: {error}"));
                }
            }
            Err(e) => {
                let text = format!("Autosave failed for '{id}': {e}");
                self.set_temporary_status(&text);
                self.messages
                    .push(text, crate::app::messages::MessageSeverity::Warning);
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
                &self.storage,
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
        let (config_res, load_warnings) = crate::config::ClinConfig::load();
        let config = config_res.unwrap_or_default();
        for w in load_warnings {
            self.messages
                .push(w, crate::app::messages::MessageSeverity::Warning);
        }
        let mut theme_warnings = Vec::new();
        self.app_theme =
            crate::app_theme::AppThemeColors::from_config(&config.ui, &mut theme_warnings);
        for w in theme_warnings {
            self.messages
                .push(w, crate::app::messages::MessageSeverity::Warning);
        }
        if self.mode == ViewMode::Help {
            self.list.help_text_cache = None;
        }
    }

    /// Re-derive `app_theme` from the in-memory `self.config` (no disk read).
    /// Used for live preview where config was mutated but not yet saved.
    pub fn refresh_theme_from_config(&mut self) {
        let mut theme_warnings = Vec::new();
        self.app_theme =
            crate::app_theme::AppThemeColors::from_config(&self.config.ui, &mut theme_warnings);
        for w in theme_warnings {
            self.messages
                .push(w, crate::app::messages::MessageSeverity::Warning);
        }
        if self.mode == ViewMode::Help {
            self.list.help_text_cache = None;
        }
        // Force SourceHighlighter to reinitialize with new theme.
        // md_highlight_lines = 0 so stale → true, triggering cache rebuild.
        self.editor.source_highlighter = None;
        self.editor.md_highlight_memo.clear();
        self.editor.md_highlight_lines = 0;
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
            skip_dir_patterns: Vec::new(),
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
            skip_dir_patterns: Vec::new(),
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
            skip_dir_patterns: Vec::new(),
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
            skip_dir_patterns: Vec::new(),
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
        app.editor.body = crate::editor_document::EditorDocument::from_text(body_content);

        // Call autosave
        app.autosave();

        // Verify words_written is 10 and note ID is in notes_modified
        assert_eq!(app.goals_progress.words_written, 10);
        assert_eq!(app.goals_progress.notes_modified.len(), 1);

        // Edit note again: delete 3 words, and add 5 words (net new +2 words)
        let body_content_2 = "one two three four five six seven eight nine ten eleven twelve";
        app.editor.body = crate::editor_document::EditorDocument::from_text(body_content_2);
        app.autosave();

        // 10 + 2 = 12 words total
        assert_eq!(app.goals_progress.words_written, 12);

        // Edit note again: remove words (e.g. to 3 words)
        let body_content_3 = "one two three";
        app.editor.body = crate::editor_document::EditorDocument::from_text(body_content_3);
        app.autosave();

        // Should not decrease words_written (should remain 12)
        assert_eq!(app.goals_progress.words_written, 12);

        // Now create a second note
        app.start_blank_note_with_title(String::new(), "Second Note".to_string());
        assert_eq!(app.editor.initial_word_count, 0);
        app.editor.body = crate::editor_document::EditorDocument::from_text("hello world");
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
            skip_dir_patterns: Vec::new(),
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
        app.editor.body = crate::editor_document::EditorDocument::from_text(body_content);

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
            skip_dir_patterns: Vec::new(),
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
            skip_dir_patterns: Vec::new(),
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
            skip_dir_patterns: Vec::new(),
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
            skip_dir_patterns: Vec::new(),
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
            skip_dir_patterns: Vec::new(),
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
            skip_dir_patterns: Vec::new(),
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
            skip_dir_patterns: Vec::new(),
        };
        let mut app = App::new(storage).expect("value is present");

        // Mock folder cache
        app.catalog_folders = vec![
            "a".to_string(),
            "a/b".to_string(),
            "a/b/c".to_string(),
            "other".to_string(),
        ];

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
            data_dir: data_dir.clone(),
            config_dir: config_dir.clone(),
            notes_dir,
            templates_dir,
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
        };
        // Write default config
        std::fs::write(&config_path, crate::config::merge::default_config_content())
            .expect("value is present");
        set_config_path_override(config_path.clone());

        // Write expanded_folders to state.json directly
        let vault_id = crate::local_state::vault_identity_path(&data_dir)
            .expect("value is present")
            .to_string_lossy()
            .into_owned();
        // Use AppPaths to find where state.json would be for this config
        let state_path = crate::paths::AppPaths::discover(
            crate::config::ClinConfig::config_path().expect("value is present"),
        )
        .expect("value is present")
        .state_path();
        let mut state =
            crate::local_state::LocalState::load(&state_path).expect("value is present");
        {
            let vault = state.vaults.entry(vault_id.clone()).or_default();
            vault.expanded_folders = ["a", "a/b"].into_iter().map(Into::into).collect();
        }
        state.save(&state_path).expect("value is present");

        // Create App, should load folders from state.json
        let app = App::new(storage.clone()).expect("value is present");
        assert!(app.list.folder_expanded.contains("a"));
        assert!(app.list.folder_expanded.contains("a/b"));
        assert!(!app.list.folder_expanded.contains("other"));

        // Write config with default_expand_depth = 3
        let config_content = crate::config::merge::default_config_content().replace(
            "preview_enabled = true",
            "preview_enabled = true\ndefault_expand_depth = 3",
        );
        std::fs::write(&config_path, config_content).expect("value is present");

        // Clear state.json so no expanded_folders are remembered
        let mut state2 =
            crate::local_state::LocalState::load(&state_path).expect("value is present");
        state2.vaults.clear();
        state2.save(&state_path).expect("value is present");

        // Re-create App, should expand up to depth 2 (since no remembered expanded_folders)
        let mut app2 = App::new(storage).expect("value is present");
        // Mock folder cache
        app2.catalog_folders = vec!["a".to_string(), "a/b".to_string(), "a/b/c".to_string()];
        app2.list.folder_expanded.clear();
        app2.expand_folders_to_depth(3);
        assert!(app2.list.folder_expanded.contains("a"));
        assert!(app2.list.folder_expanded.contains("a/b"));
        assert!(!app2.list.folder_expanded.contains("a/b/c"));
    }
}
