use std::fs;
use std::io::{self, Stdout};
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{env, process};

use anyhow::{Context, Result, anyhow};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use directories::ProjectDirs;
use rand::RngCore;
use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use serde::{Deserialize, Serialize};
use tui_textarea::{Input, TextArea};
use tui_textarea::CursorMove;
use uuid::Uuid;
use vim_line::{Key as VimKey, KeyCode as VimKeyCode, LineEditor, VimLineEditor};

const FILE_MAGIC: &[u8; 5] = b"CLIN1";
const NONCE_LEN: usize = 12;
const LIST_HELP_HINTS: &str = "Up/Down move   Enter open/new   Del/d delete   f location   ? help   Tab change focus   q quit";
const EDIT_HELP_HINTS: &str = "Esc autosave+back   Tab change focus   Ctrl+Q quit";
const HELP_PAGE_HINTS: &str = "Esc/q/?/F1 close help";
const _HELP_PAGE_TEXT: &str = "clin help\n\
\n\
Core features\n\
- Encrypted local notes stored as binary .clin files\n\
- Notes shown in a list, open/edit directly in terminal UI\n\
- Auto-save on Esc back from editor\n\
- Save+quit from editor with Ctrl+Q\n\
- Delete notes with confirmation (d/Delete then y or Enter)\n\
- Open note file location in file manager (f in notes view)\n\
\n\
Notes view shortcuts\n\
- Up/Down: move selection\n\
- Enter: open selected note or create new\n\
- d/Delete: request note delete\n\
- y or Enter: confirm delete\n\
- n or Esc: cancel delete\n\
- f: open selected note location\n\
- Tab: switch focus between note list and Vim toggle button\n\
- Enter/Space on Vim button: toggle Vim mode\n\
- ?: open help page\n\
- q: quit app\n\
\n\
Editor shortcuts (Vim OFF)\n\
- Tab: change focus (Title, Content, Vim toggle button)\n\
- Esc: auto-save and return to notes view\n\
- Ctrl+Q: save and quit app\n\
- Mouse selection in content area\n\
- Clipboard: Ctrl+C/Ctrl+X/Ctrl+V, Ctrl+Insert, Shift+Insert, Shift+Delete\n\
- Edit: Ctrl+A, Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z\n\
- Word edit: Ctrl+Backspace, Ctrl+Delete\n\
\n\
Editor shortcuts (Vim ON)\n\
- Powered by vim-line (library-backed Vim key handling)\n\
- Modes: NORMAL, INSERT, VISUAL, OPERATOR-PENDING, REPLACE\n\
- Movement: h j k l, w e b, 0/^, $, %\n\
- Visual line: V to enter linewise selection (j/k move, y/d/c/x/p apply)\n\
- Insert: i, I, a, A, o, O\n\
- Operators: d, y, c (including dd, yy, cc and dw/cw/yw)\n\
- Inner object ops: ciw/diw, ci(/di(, ci[/di[, ci{/di{, ci</di<, ci\"/di\", ci'/di', ci`/di`\n\
- Edits: x, D, C, r<char>, p, P\n\
\n\
Help page\n\
- Esc/q/?/F1: close help and return to notes\n";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AppSettings {
    vim_enabled: bool,
}

#[derive(Debug, Clone, Default)]
struct VimCompatState {
    awaiting_inner_object_op: Option<char>,
    visual_line_anchor_row: Option<u16>,
}

impl VimCompatState {
    fn reset(&mut self) {
        self.awaiting_inner_object_op = None;
        self.visual_line_anchor_row = None;
    }

    fn visual_line_active(&self) -> bool {
        self.visual_line_anchor_row.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Note {
    title: String,
    content: String,
    updated_at: u64,
}

#[derive(Debug, Clone)]
struct NoteSummary {
    id: String,
    title: String,
    updated_at: u64,
}

#[derive(Debug)]
struct Storage {
    data_dir: PathBuf,
    notes_dir: PathBuf,
    key: [u8; 32],
}

impl Storage {
    fn init() -> Result<Self> {
        let proj_dirs = ProjectDirs::from("com", "clin", "clin")
            .context("could not determine local data directory")?;
        let data_dir = proj_dirs.data_local_dir().to_path_buf();
        let notes_dir = data_dir.join("notes");
        fs::create_dir_all(&notes_dir).context("failed to create notes directory")?;

        let key_path = data_dir.join("key.bin");
        let key = if key_path.exists() {
            let raw = fs::read(&key_path).context("failed to read encryption key")?;
            if raw.len() != 32 {
                anyhow::bail!("invalid key file length")
            }
            let mut key = [0_u8; 32];
            key.copy_from_slice(&raw);
            key
        } else {
            let mut key = [0_u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut key);
            fs::create_dir_all(&data_dir).context("failed to create app data directory")?;
            fs::write(&key_path, key).context("failed to write encryption key")?;
            key
        };

        Ok(Self {
            data_dir,
            notes_dir,
            key,
        })
    }

    fn settings_path(&self) -> PathBuf {
        self.data_dir.join("settings.json")
    }

    fn load_settings(&self) -> AppSettings {
        let path = self.settings_path();
        if !path.exists() {
            return AppSettings::default();
        }

        fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<AppSettings>(&raw).ok())
            .unwrap_or_default()
    }

    fn save_settings(&self, settings: &AppSettings) {
        if let Ok(raw) = serde_json::to_string_pretty(settings) {
            let _ = fs::write(self.settings_path(), raw);
        }
    }

    fn note_path(&self, id: &str) -> PathBuf {
        self.notes_dir.join(format!("{id}.clin"))
    }

    fn list_note_ids(&self) -> Result<Vec<String>> {
        let mut ids = Vec::new();
        for entry in fs::read_dir(&self.notes_dir).context("failed reading notes directory")? {
            let entry = entry.context("failed to read note entry")?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("clin")
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            {
                ids.push(stem.to_string());
            }
        }
        Ok(ids)
    }

    fn load_note(&self, id: &str) -> Result<Note> {
        let encrypted = fs::read(self.note_path(id)).context("failed to read note")?;
        let plain = self.decrypt(&encrypted)?;
        let (note, _) = bincode::serde::decode_from_slice::<Note, _>(&plain, bincode::config::standard())
            .context("failed to decode note")?;
        Ok(note)
    }

    fn save_note(&self, id: &str, note: &Note) -> Result<String> {
        let preferred_id = self.note_file_stem_from_title(&note.title);
        let target_id = self.unique_note_id(&preferred_id, id);

        let bytes = bincode::serde::encode_to_vec(note, bincode::config::standard())
            .context("failed to encode note")?;
        let encrypted = self.encrypt(&bytes)?;
        fs::write(self.note_path(&target_id), encrypted).context("failed to write note")?;

        if id != target_id {
            let old_path = self.note_path(id);
            if old_path.exists() {
                fs::remove_file(&old_path).context("failed to rename note file")?;
            }
        }

        Ok(target_id)
    }

    fn delete_note(&self, id: &str) -> Result<()> {
        fs::remove_file(self.note_path(id)).context("failed to delete note")?;
        Ok(())
    }

    fn new_note_id(&self) -> String {
        Uuid::new_v4().to_string()
    }

    fn note_file_stem_from_title(&self, title: &str) -> String {
        let trimmed = title.trim();
        let source = if trimmed.is_empty() {
            "Untitled note"
        } else {
            trimmed
        };

        let mut out = String::new();
        for ch in source.chars() {
            let valid = ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.');
            out.push(if valid { ch } else { '_' });
        }

        let collapsed = out
            .split_whitespace()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        if collapsed.is_empty() {
            String::from("Untitled note")
        } else {
            collapsed
        }
    }

    fn unique_note_id(&self, preferred: &str, current_id: &str) -> String {
        let mut candidate = preferred.to_string();
        let mut counter = 2_u32;

        while candidate != current_id && self.note_path(&candidate).exists() {
            candidate = format!("{preferred} ({counter})");
            counter += 1;
        }

        candidate
    }

    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&self.key));
        let mut nonce = [0_u8; NONCE_LEN];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext)
            .map_err(|_| anyhow!("note encryption failed"))?;

        let mut output = Vec::with_capacity(FILE_MAGIC.len() + NONCE_LEN + ciphertext.len());
        output.extend_from_slice(FILE_MAGIC);
        output.extend_from_slice(&nonce);
        output.extend_from_slice(&ciphertext);
        Ok(output)
    }

    fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>> {
        if encrypted.len() <= FILE_MAGIC.len() + NONCE_LEN {
            anyhow::bail!("note file is too short")
        }
        if &encrypted[..FILE_MAGIC.len()] != FILE_MAGIC {
            anyhow::bail!("invalid note header")
        }
        let nonce_start = FILE_MAGIC.len();
        let nonce_end = nonce_start + NONCE_LEN;
        let nonce = &encrypted[nonce_start..nonce_end];
        let ciphertext = &encrypted[nonce_end..];

        let cipher = ChaCha20Poly1305::new(Key::from_slice(&self.key));
        let plain = cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| anyhow!("failed to decrypt note file"))?;
        Ok(plain)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    List,
    Edit,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListFocus {
    Notes,
    VimToggle,
}

struct App {
    storage: Storage,
    notes: Vec<NoteSummary>,
    selected: usize,
    list_focus: ListFocus,
    mode: ViewMode,
    editing_id: Option<String>,
    title_editor: TextArea<'static>,
    editor: TextArea<'static>,
    vim_enabled: bool,
    vim_title: VimLineEditor,
    vim_body: VimLineEditor,
    vim_title_compat: VimCompatState,
    vim_body_compat: VimCompatState,
    status: String,
    status_until: Option<Instant>,
    pending_delete_note_id: Option<String>,
    help_scroll: u16,
}

enum CliCommand {
    Run { edit_title: Option<String> },
    NewAndOpen { title: Option<String> },
    QuickNote { content: String, title: Option<String> },
    ShowNotesLocation,
    ListNoteTitles,
    Help,
}

impl App {
    fn new(storage: Storage) -> Result<Self> {
        let settings = storage.load_settings();

        let mut app = Self {
            storage,
            notes: Vec::new(),
            selected: 0,
            list_focus: ListFocus::Notes,
            mode: ViewMode::List,
            editing_id: None,
            title_editor: make_title_editor(""),
            editor: TextArea::default(),
            vim_enabled: settings.vim_enabled,
            vim_title: VimLineEditor::new(),
            vim_body: VimLineEditor::new(),
            vim_title_compat: VimCompatState::default(),
            vim_body_compat: VimCompatState::default(),
            status: String::from(LIST_HELP_HINTS),
            status_until: None,
            pending_delete_note_id: None,
            help_scroll: 0,
        };
        app.refresh_notes()?;
        Ok(app)
    }

    fn refresh_notes(&mut self) -> Result<()> {
        let mut summaries = Vec::new();
        for id in self.storage.list_note_ids()? {
            if let Ok(note) = self.storage.load_note(&id) {
                summaries.push(NoteSummary {
                    id,
                    title: note.title,
                    updated_at: note.updated_at,
                });
            }
        }
        summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        self.notes = summaries;

        if self.selected > self.notes.len() {
            self.selected = self.notes.len();
        }
        Ok(())
    }

    fn open_selected(&mut self) {
        if self.selected == self.notes.len() {
            self.start_new_note();
            return;
        }

        if let Some(summary) = self.notes.get(self.selected) {
            match self.storage.load_note(&summary.id) {
                Ok(note) => {
                    self.mode = ViewMode::Edit;
                    self.editing_id = Some(summary.id.clone());
                    self.title_editor = make_title_editor(&note.title);
                    self.editor = text_area_from_content(&note.content);
                    self.editor.set_cursor_style(Style::default().fg(Color::Black).bg(Color::Cyan));
                    self.editor
                        .set_cursor_line_style(Style::default().bg(Color::Rgb(32, 36, 44)));
                    self.reset_vim_states_for_edit();
                    self.set_default_status();
                }
                Err(err) => {
                    self.status = format!("Could not open note: {err:#}");
                }
            }
        }
    }

    fn open_note_by_title(&mut self, title: &str) -> bool {
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

    fn start_new_note(&mut self) {
        self.mode = ViewMode::Edit;
        self.editing_id = Some(self.storage.new_note_id());
        self.title_editor = make_title_editor("");
        self.editor = TextArea::default();
        self.editor.set_cursor_style(Style::default().fg(Color::Black).bg(Color::Cyan));
        self.editor
            .set_cursor_line_style(Style::default().bg(Color::Rgb(32, 36, 44)));
        self.reset_vim_states_for_edit();
        self.set_default_status();
    }

    fn save_current(&mut self) -> bool {
        let Some(id) = self.editing_id.clone() else {
            self.status = String::from("No active note to save");
            return false;
        };

        let mut title = get_title_text(&self.title_editor).trim().to_string();
        if title.is_empty() {
            title = String::from("Untitled note");
        }

        let note = Note {
            title,
            content: self.editor.lines().join("\n"),
            updated_at: now_unix_secs(),
        };

        match self.storage.save_note(&id, &note) {
            Ok(saved_id) => {
                self.editing_id = Some(saved_id.clone());
                self.status = String::from("Saved");
                let _ = self.refresh_notes();
                if let Some(index) = self.notes.iter().position(|n| n.id == saved_id) {
                    self.selected = index;
                }
                true
            }
            Err(err) => {
                self.status = format!("Save failed: {err:#}");
                false
            }
        }
    }

    fn back_to_list(&mut self) {
        self.mode = ViewMode::List;
        self.editing_id = None;
        self.list_focus = ListFocus::Notes;
        self.title_editor = make_title_editor("");
        self.editor = TextArea::default();
        self.pending_delete_note_id = None;
        let _ = self.refresh_notes();
        self.set_default_status();
    }

    fn begin_delete_selected(&mut self) {
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
        self.status = format!("Delete \"{}\"? y confirm, n cancel", note.title);
    }

    fn cancel_delete_prompt(&mut self) {
        self.pending_delete_note_id = None;
        self.set_default_status();
    }

    fn confirm_delete_selected(&mut self) {
        let Some(id) = self.pending_delete_note_id.clone() else {
            return;
        };

        match self.storage.delete_note(&id) {
            Ok(()) => {
                self.pending_delete_note_id = None;
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

    fn open_selected_note_location(&mut self) {
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

    fn reset_vim_states_for_edit(&mut self) {
        self.vim_title = VimLineEditor::new();
        self.vim_body = VimLineEditor::new();
        self.vim_title_compat.reset();
        self.vim_body_compat.reset();

        if self.vim_enabled {
            apply_vim_cursor_style(&mut self.title_editor, self.vim_title.status());
            apply_vim_cursor_style(&mut self.editor, self.vim_body.status());
        } else {
            apply_vim_cursor_style(&mut self.title_editor, "INSERT");
            apply_vim_cursor_style(&mut self.editor, "INSERT");
        }
    }

    fn toggle_vim_mode(&mut self) {
        self.vim_enabled = !self.vim_enabled;
        self.reset_vim_states_for_edit();

        if !self.vim_enabled {
            self.title_editor.cancel_selection();
            self.editor.cancel_selection();
        }

        self.set_default_status();
        self.storage.save_settings(&AppSettings {
            vim_enabled: self.vim_enabled,
        });
    }

    fn open_help_page(&mut self) {
        self.mode = ViewMode::Help;
        self.help_scroll = 0;
        self.status = String::from(HELP_PAGE_HINTS);
        self.status_until = None;
    }

    fn close_help_page(&mut self) {
        self.mode = ViewMode::List;
        self.help_scroll = 0;
        self.set_default_status();
    }

    fn default_status_text(&self) -> &'static str {
        match self.mode {
            ViewMode::List => LIST_HELP_HINTS,
            ViewMode::Edit => EDIT_HELP_HINTS,
            ViewMode::Help => HELP_PAGE_HINTS,
        }
    }

    fn set_default_status(&mut self) {
        self.status = self.default_status_text().to_string();
        self.status_until = None;
    }

    fn set_temporary_status(&mut self, message: &str) {
        self.status = message.to_string();
        self.status_until = Some(Instant::now() + Duration::from_secs(2));
    }

    fn tick_status(&mut self) {
        if let Some(until) = self.status_until
            && Instant::now() >= until
        {
            self.set_default_status();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditFocus {
    Title,
    Body,
    VimToggle,
}

fn main() -> Result<()> {
    let cli = parse_cli_command()?;

    match cli {
        CliCommand::Help => {
            print_cli_help();
            Ok(())
        }
        CliCommand::ShowNotesLocation => {
            let storage = Storage::init()?;
            println!("{}", storage.notes_dir.display());
            Ok(())
        }
        CliCommand::ListNoteTitles => {
            let storage = Storage::init()?;
            let mut app = App::new(storage)?;
            app.refresh_notes()?;
            for (index, note) in app.notes.iter().enumerate() {
                println!("{}. {}", index + 1, note.title);
            }
            Ok(())
        }
        CliCommand::QuickNote { content, title } => {
            let storage = Storage::init()?;
            let final_title = title
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| String::from("Untitled note"));

            let note = Note {
                title: final_title.clone(),
                content,
                updated_at: now_unix_secs(),
            };

            let id = storage.new_note_id();
            let saved_id = storage.save_note(&id, &note)?;
            println!("Saved quick note \"{}\" as {}.clin", final_title, saved_id);
            Ok(())
        }
        CliCommand::NewAndOpen { title } => {
            let storage = Storage::init()?;
            let final_title = title
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| String::from("Untitled note"));

            let note = Note {
                title: final_title.clone(),
                content: String::new(),
                updated_at: now_unix_secs(),
            };
            let id = storage.new_note_id();
            let saved_id = storage.save_note(&id, &note)?;

            let mut app = App::new(storage)?;
            if let Some(index) = app.notes.iter().position(|n| n.id == saved_id) {
                app.selected = index;
                app.open_selected();
            } else {
                app.open_note_by_title(&final_title);
            }

            run_tui_session(&mut app)
        }
        CliCommand::Run { edit_title } => {
            let storage = Storage::init()?;
            let mut app = App::new(storage)?;

            if let Some(title) = edit_title
                && !app.open_note_by_title(&title)
            {
                eprintln!("No note found with title: {title}");
                process::exit(1);
            }

            run_tui_session(&mut app)
        }
    }
}

fn parse_cli_command() -> Result<CliCommand> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        return Ok(CliCommand::Run { edit_title: None });
    }

    match args[0].as_str() {
        "-h" | "--help" => Ok(CliCommand::Help),
        "-f" => Ok(CliCommand::ShowNotesLocation),
        "-l" => Ok(CliCommand::ListNoteTitles),
        "-n" => {
            let title = if args.len() > 1 {
                Some(args[1..].join(" "))
            } else {
                None
            };
            Ok(CliCommand::NewAndOpen { title })
        }
        "-q" => {
            if args.len() < 2 {
                anyhow::bail!("-q requires note content. Try: clin -q \"content\" [title]");
            }
            let content = args[1].clone();
            let title = if args.len() > 2 {
                Some(args[2..].join(" "))
            } else {
                None
            };
            Ok(CliCommand::QuickNote { content, title })
        }
        "-e" => {
            if args.len() < 2 {
                anyhow::bail!("-e requires a note title. Try: clin -e \"My Note\"");
            }
            Ok(CliCommand::Run {
                edit_title: Some(args[1..].join(" ")),
            })
        }
        unknown => anyhow::bail!("unknown argument: {unknown}. Use clin -h for help."),
    }
}

fn print_cli_help() {
    println!(
        "clin arguments:\n\
  clin                Launch interactive app\n\
  clin -q <CONTENT> [TITLE]\n\
                     Create a quick note and exit\n\
    clin -n [TITLE]    Create a new note and open it in editor\n\
  clin -f            Print notes directory location\n\
    clin -l            List note titles\n\
  clin -e <TITLE>    Open a specific note title directly in editor\n\
  clin -h            Show this help\n"
    );
}

fn run_tui_session(app: &mut App) -> Result<()> {
        enable_raw_mode().context("failed to enable raw mode")?;
        let mut stdout = io::stdout();
        execute!(
                stdout,
                EnterAlternateScreen,
                EnableMouseCapture,
                EnableBracketedPaste
        )
        .context("failed to enter alternate screen")?;

        let backend = ratatui::backend::CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).context("failed to create terminal")?;

        let run_result = run_app(&mut terminal, app);

        disable_raw_mode().ok();
        execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture,
                DisableBracketedPaste
        )
        .ok();
        terminal.show_cursor().ok();

        run_result
}

fn run_app(terminal: &mut Terminal<ratatui::backend::CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
    let mut should_quit = false;
    let mut focus = EditFocus::Body;
    let mut mouse_selecting = false;

    while !should_quit {
        app.tick_status();
        terminal.draw(|frame| draw_ui(frame, app, focus))?;

        if event::poll(Duration::from_millis(200)).context("event poll failed")? {
            match event::read().context("failed to read event")? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match app.mode {
                    ViewMode::List => {
                        if handle_list_keys(app, key) {
                            should_quit = true;
                        }
                    }
                    ViewMode::Edit => {
                        if handle_edit_keys(app, key, &mut focus) {
                            should_quit = true;
                        }
                    }
                    ViewMode::Help => {
                        handle_help_keys(app, key);
                    }
                },
                Event::Mouse(mouse_event) if app.mode == ViewMode::Edit => {
                    let size = terminal.size().context("failed to get terminal size")?;
                    let area = Rect::new(0, 0, size.width, size.height);
                    handle_edit_mouse(app, mouse_event, area, &mut focus, &mut mouse_selecting);
                }
                Event::Paste(data) if app.mode == ViewMode::Edit => {
                    match focus {
                        EditFocus::Title => {
                            let normalized = data.replace(['\r', '\n'], " ");
                            app.title_editor.insert_str(normalized);
                            app.status = String::from("Pasted title text");
                        }
                        EditFocus::Body => {
                            app.editor.insert_str(data);
                            app.status = String::from("Pasted content");
                        }
                        EditFocus::VimToggle => {}
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn handle_list_keys(app: &mut App, key: KeyEvent) -> bool {
    if app.pending_delete_note_id.is_some() {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => app.confirm_delete_selected(),
            KeyCode::Char('n') | KeyCode::Esc => app.cancel_delete_prompt(),
            _ => {}
        }
        return false;
    }

    if key.code == KeyCode::Tab {
        app.list_focus = match app.list_focus {
            ListFocus::Notes => ListFocus::VimToggle,
            ListFocus::VimToggle => ListFocus::Notes,
        };
        return false;
    }

    if app.list_focus == ListFocus::VimToggle {
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                app.toggle_vim_mode();
            }
            KeyCode::Char('q') => return true,
            _ => {}
        }
        return false;
    }

    if app.vim_enabled {
        match key.code {
            KeyCode::Char('k') | KeyCode::Char('h') => {
                if app.selected > 0 {
                    app.selected -= 1;
                }
                return false;
            }
            KeyCode::Char('j') | KeyCode::Char('l') => {
                if app.selected < app.notes.len() {
                    app.selected += 1;
                }
                return false;
            }
            KeyCode::Char('d') => {
                app.begin_delete_selected();
                return false;
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Char('q') => return true,
        KeyCode::Char('?') | KeyCode::F(1) => app.open_help_page(),
        KeyCode::Char('f') => app.open_selected_note_location(),
        KeyCode::Char('d') | KeyCode::Delete => app.begin_delete_selected(),
        KeyCode::Down => {
            if app.selected < app.notes.len() {
                app.selected += 1;
            }
        }
        KeyCode::Up => {
            if app.selected > 0 {
                app.selected -= 1;
            }
        }
        KeyCode::Enter => app.open_selected(),
        _ => {}
    }
    false
}

fn handle_help_keys(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::F(1) => {
            app.close_help_page();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.help_scroll = app.help_scroll.saturating_add(1);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.help_scroll = app.help_scroll.saturating_sub(1);
        }
        _ => {}
    }
}

fn handle_edit_keys(app: &mut App, key: KeyEvent, focus: &mut EditFocus) -> bool {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
        return app.save_current();
    }

    if key.code == KeyCode::Esc {
        if app.vim_enabled
            && *focus != EditFocus::VimToggle
        {
            let in_insert = match *focus {
                EditFocus::Title => app.vim_title.status() == "INSERT",
                EditFocus::Body => app.vim_body.status() == "INSERT",
                EditFocus::VimToggle => false,
            };

            if in_insert {
                match *focus {
                    EditFocus::Title => {
                        let _ = handle_vim_line_key(
                            &mut app.title_editor,
                            &mut app.vim_title,
                            &mut app.vim_title_compat,
                            key,
                            true,
                        );
                    }
                    EditFocus::Body => {
                        let _ = handle_vim_line_key(
                            &mut app.editor,
                            &mut app.vim_body,
                            &mut app.vim_body_compat,
                            key,
                            false,
                        );
                    }
                    EditFocus::VimToggle => {}
                }
                return false;
            }
        }
        if app.save_current() {
            app.back_to_list();
            *focus = EditFocus::Body;
        }
        return false;
    }

    if key.code == KeyCode::Tab {
        *focus = match *focus {
            EditFocus::Title => EditFocus::Body,
            EditFocus::Body => EditFocus::VimToggle,
            EditFocus::VimToggle => EditFocus::Title,
        };
        return false;
    }

    match *focus {
        EditFocus::Title => {
            if key.code == KeyCode::Enter {
                *focus = EditFocus::Body;
                return false;
            }

            if app.vim_enabled
                && handle_vim_line_key(
                    &mut app.title_editor,
                    &mut app.vim_title,
                    &mut app.vim_title_compat,
                    key,
                    true,
                )
            {
                return false;
            }

            if handle_os_shortcuts(&mut app.title_editor, key) {
                return false;
            }

            if app.title_editor.input(Input::from(key)) {
                // Keep title as a single visual line; newline-producing input is flattened.
                if app.title_editor.lines().len() > 1 {
                    let normalized = get_title_text(&app.title_editor).replace(['\r', '\n'], " ");
                    app.title_editor = make_title_editor(&normalized);
                }
            }
        }
        EditFocus::Body => {
            if app.vim_enabled
                && handle_vim_line_key(
                    &mut app.editor,
                    &mut app.vim_body,
                    &mut app.vim_body_compat,
                    key,
                    false,
                )
            {
                return false;
            }

            if handle_os_shortcuts(&mut app.editor, key) {
                return false;
            }
            app.editor.input(Input::from(key));
        }
        EditFocus::VimToggle => {
            if key.code == KeyCode::Enter || key.code == KeyCode::Char(' ') {
                app.toggle_vim_mode();
            }
        }
    }

    false
}

fn handle_edit_mouse(
    app: &mut App,
    mouse_event: MouseEvent,
    terminal_area: Rect,
    focus: &mut EditFocus,
    mouse_selecting: &mut bool,
) {
    let (title_inner, body_inner) = edit_view_input_areas(terminal_area);

    if contains_cell(title_inner, mouse_event.column, mouse_event.row) {
        *focus = EditFocus::Title;
    }

    if contains_cell(title_inner, mouse_event.column, mouse_event.row) {
        app.title_editor.input(Input::from(mouse_event));
    }
    if contains_cell(body_inner, mouse_event.column, mouse_event.row) {
        app.editor.input(Input::from(mouse_event));
    }

    match mouse_event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            *mouse_selecting = false;
            if contains_cell(body_inner, mouse_event.column, mouse_event.row) {
                *focus = EditFocus::Body;
                move_editor_cursor_to_mouse(app, body_inner, mouse_event.column, mouse_event.row);
                app.editor.start_selection();
                *mouse_selecting = true;
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if *mouse_selecting {
                move_editor_cursor_to_mouse(app, body_inner, mouse_event.column, mouse_event.row);
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            *mouse_selecting = false;
        }
        _ => {}
    }
}

fn move_editor_cursor_to_mouse(app: &mut App, body_inner: Rect, mouse_col: u16, mouse_row: u16) {
    if app.editor.lines().is_empty() || body_inner.width == 0 || body_inner.height == 0 {
        return;
    }

    let row = mouse_row.saturating_sub(body_inner.y) as usize;
    let col = mouse_col.saturating_sub(body_inner.x) as usize;

    let max_row = app.editor.lines().len().saturating_sub(1);
    let target_row = row.min(max_row);
    let max_col = app.editor.lines()[target_row].chars().count();
    let target_col = col.min(max_col);

    app.editor
        .move_cursor(CursorMove::Jump(target_row as u16, target_col as u16));
}

fn edit_view_input_areas(area: Rect) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(area);

    let title_inner = chunks[1].inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let body_inner = chunks[2].inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    (title_inner, body_inner)
}

fn contains_cell(rect: Rect, col: u16, row: u16) -> bool {
    rect.width > 0
        && rect.height > 0
        && col >= rect.x
        && col < rect.x + rect.width
        && row >= rect.y
        && row < rect.y + rect.height
}

fn handle_os_shortcuts(textarea: &mut TextArea<'static>, key: KeyEvent) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    if ctrl {
        match key.code {
            KeyCode::Char('a') => {
                textarea.select_all();
                return true;
            }
            KeyCode::Char('c') => {
                textarea.copy();
                return true;
            }
            KeyCode::Char('x') => {
                let _ = textarea.cut();
                return true;
            }
            KeyCode::Char('v') => {
                let _ = textarea.paste();
                return true;
            }
            KeyCode::Char('z') if shift => {
                let _ = textarea.redo();
                return true;
            }
            KeyCode::Char('z') => {
                let _ = textarea.undo();
                return true;
            }
            KeyCode::Char('y') => {
                let _ = textarea.redo();
                return true;
            }
            KeyCode::Backspace => {
                let _ = textarea.delete_word();
                return true;
            }
            KeyCode::Delete => {
                let _ = textarea.delete_next_word();
                return true;
            }
            KeyCode::Home => {
                textarea.move_cursor(CursorMove::Top);
                return true;
            }
            KeyCode::End => {
                textarea.move_cursor(CursorMove::Bottom);
                return true;
            }
            KeyCode::Insert => {
                textarea.copy();
                return true;
            }
            _ => {}
        }
    }

    if shift {
        match key.code {
            KeyCode::Insert => {
                let _ = textarea.paste();
                return true;
            }
            KeyCode::Delete => {
                let _ = textarea.cut();
                return true;
            }
            _ => {}
        }
    }

    false
}

fn handle_vim_line_key(
    textarea: &mut TextArea<'static>,
    editor: &mut VimLineEditor,
    compat: &mut VimCompatState,
    key: KeyEvent,
    single_line: bool,
) -> bool {
    if key.code == KeyCode::Esc {
        compat.awaiting_inner_object_op = None;
    }

    if let Some(op) = compat.awaiting_inner_object_op {
        compat.awaiting_inner_object_op = None;
        if let KeyCode::Char(object) = key.code {
            return apply_inner_object_operator(textarea, editor, op, object, single_line);
        }
        apply_vim_line_event(textarea, editor, VimKey::code(VimKeyCode::Escape), single_line);
        return true;
    }

    if !single_line
        && key.code == KeyCode::Char('V')
        && editor.status() == "NORMAL"
    {
        compat.visual_line_anchor_row = Some(textarea.cursor().0 as u16);
        sync_visual_line_selection(textarea, textarea.cursor().0 as u16);
        apply_vim_cursor_style(textarea, "VISUAL LINE");
        return true;
    }

    if let Some(anchor_row) = compat.visual_line_anchor_row {
        match key.code {
            KeyCode::Esc | KeyCode::Char('v') | KeyCode::Char('V') => {
                compat.visual_line_anchor_row = None;
                textarea.cancel_selection();
                apply_vim_cursor_style(textarea, editor.status());
                return true;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                textarea.move_cursor(CursorMove::Down);
                sync_visual_line_selection(textarea, anchor_row);
                return true;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                textarea.move_cursor(CursorMove::Up);
                sync_visual_line_selection(textarea, anchor_row);
                return true;
            }
            KeyCode::Char('y') => {
                textarea.copy();
                compat.visual_line_anchor_row = None;
                textarea.cancel_selection();
                apply_vim_cursor_style(textarea, editor.status());
                return true;
            }
            KeyCode::Char('d') | KeyCode::Char('x') => {
                let _ = textarea.cut();
                compat.visual_line_anchor_row = None;
                textarea.cancel_selection();
                apply_vim_cursor_style(textarea, editor.status());
                return true;
            }
            KeyCode::Char('c') => {
                let _ = textarea.cut();
                compat.visual_line_anchor_row = None;
                textarea.cancel_selection();
                editor.reset();
                apply_vim_line_event(textarea, editor, VimKey::char('i'), single_line);
                return true;
            }
            KeyCode::Char('p') => {
                let _ = textarea.cut();
                let _ = textarea.paste();
                compat.visual_line_anchor_row = None;
                textarea.cancel_selection();
                apply_vim_cursor_style(textarea, editor.status());
                return true;
            }
            _ => {
                let Some(vim_key) = to_vim_key(key) else {
                    return true;
                };
                apply_vim_line_event(textarea, editor, vim_key, single_line);
                sync_visual_line_selection(textarea, anchor_row);
                return true;
            }
        }
    }

    if matches!(key.code, KeyCode::Char('i')) {
        let pending_op = match editor.status() {
            "c..." => Some('c'),
            "d..." => Some('d'),
            _ => None,
        };
        if let Some(op) = pending_op {
            compat.awaiting_inner_object_op = Some(op);
            return true;
        }
    }

    if editor.status() == "y..."
        && matches!(key.code, KeyCode::Char('i'))
    {
        compat.awaiting_inner_object_op = Some('y');
        return true;
    }

    let Some(vim_key) = to_vim_key(key) else {
        return true;
    };

    apply_vim_line_event(textarea, editor, vim_key, single_line);
    true
}

fn apply_vim_line_event(
    textarea: &mut TextArea<'static>,
    editor: &mut VimLineEditor,
    vim_key: VimKey,
    single_line: bool,
) {
    let mut text = textarea.lines().join("\n");
    let cursor = cursor_to_offset(&text, textarea.cursor().0, textarea.cursor().1);
    editor.set_cursor(cursor, &text);

    let result = editor.handle_key(vim_key, &text);
    for edit in result.edits.into_iter().rev() {
        edit.apply(&mut text);
    }

    if single_line {
        text = text.replace(['\r', '\n'], " ");
    }

    let target_offset = editor.cursor().min(text.len());
    replace_textarea_from_string(textarea, &text, single_line);

    let (row, col) = offset_to_cursor(textarea.lines(), target_offset);
    textarea.move_cursor(CursorMove::Jump(row as u16, col as u16));

    if let Some(selection) = editor.selection() {
        let start = selection.start.min(text.len());
        let end = selection.end.min(text.len());
        let (start_row, start_col) = offset_to_cursor(textarea.lines(), start);
        let (end_row, end_col) = offset_to_cursor(textarea.lines(), end);

        textarea.cancel_selection();
        textarea.move_cursor(CursorMove::Jump(start_row as u16, start_col as u16));
        textarea.start_selection();
        textarea.move_cursor(CursorMove::Jump(end_row as u16, end_col as u16));
    } else {
        textarea.cancel_selection();
        textarea.move_cursor(CursorMove::Jump(row as u16, col as u16));
    }

    apply_vim_cursor_style(textarea, editor.status());
}

fn apply_inner_object_operator(
    textarea: &mut TextArea<'static>,
    editor: &mut VimLineEditor,
    op: char,
    object: char,
    single_line: bool,
) -> bool {
    let mut text = textarea.lines().join("\n");
    let cursor = cursor_to_offset(&text, textarea.cursor().0, textarea.cursor().1);

    let range = match object {
        'w' => inner_word_range(&text, cursor),
        '(' | ')' | 'b' => inner_pair_range(&text, cursor, '(', ')'),
        '[' | ']' => inner_pair_range(&text, cursor, '[', ']'),
        '{' | '}' | 'B' => inner_pair_range(&text, cursor, '{', '}'),
        '<' | '>' => inner_pair_range(&text, cursor, '<', '>'),
        '"' => inner_quote_range(&text, cursor, '"'),
        '\'' => inner_quote_range(&text, cursor, '\''),
        '`' => inner_quote_range(&text, cursor, '`'),
        _ => None,
    };

    let Some((start, end)) = range else {
        apply_vim_line_event(textarea, editor, VimKey::code(VimKeyCode::Escape), single_line);
        return true;
    };

    if start >= end || end > text.len() {
        apply_vim_line_event(textarea, editor, VimKey::code(VimKeyCode::Escape), single_line);
        return true;
    }

    if op == 'y' {
        textarea.cancel_selection();
        let (start_row, start_col) = offset_to_cursor(textarea.lines(), start);
        let (end_row, end_col) = offset_to_cursor(textarea.lines(), end);
        textarea.move_cursor(CursorMove::Jump(start_row as u16, start_col as u16));
        textarea.start_selection();
        textarea.move_cursor(CursorMove::Jump(end_row as u16, end_col as u16));
        textarea.copy();
        textarea.cancel_selection();
        let (row, col) = offset_to_cursor(textarea.lines(), start.min(text.len()));
        textarea.move_cursor(CursorMove::Jump(row as u16, col as u16));
        editor.reset();
        apply_vim_line_event(textarea, editor, VimKey::code(VimKeyCode::Escape), single_line);
        return true;
    }

    text.replace_range(start..end, "");
    if single_line {
        text = text.replace(['\r', '\n'], " ");
    }

    replace_textarea_from_string(textarea, &text, single_line);
    let (row, col) = offset_to_cursor(textarea.lines(), start.min(text.len()));
    textarea.move_cursor(CursorMove::Jump(row as u16, col as u16));

    editor.reset();
    if op == 'c' {
        apply_vim_line_event(textarea, editor, VimKey::char('i'), single_line);
    } else {
        apply_vim_line_event(textarea, editor, VimKey::code(VimKeyCode::Escape), single_line);
    }
    true
}

fn inner_word_range(text: &str, cursor: usize) -> Option<(usize, usize)> {
    if text.is_empty() {
        return None;
    }

    let mut pos = cursor.min(text.len());
    let bytes = text.as_bytes();

    while pos < text.len() && bytes[pos].is_ascii_whitespace() {
        pos += 1;
    }
    if pos >= text.len() {
        return None;
    }

    let mut start = pos;
    while start > 0 && !bytes[start - 1].is_ascii_whitespace() {
        start -= 1;
    }

    let mut end = pos;
    while end < text.len() && !bytes[end].is_ascii_whitespace() {
        end += 1;
    }

    Some((start, end))
}

fn inner_pair_range(text: &str, cursor: usize, open: char, close: char) -> Option<(usize, usize)> {
    if text.is_empty() {
        return None;
    }

    let pos = cursor.min(text.len());
    let search_left = &text[..pos];
    let mut depth = 0usize;
    let mut open_idx = None;

    for (i, c) in search_left.char_indices().rev() {
        if c == close {
            depth += 1;
        } else if c == open {
            if depth == 0 {
                open_idx = Some(i);
                break;
            }
            depth -= 1;
        }
    }

    let open_idx = open_idx?;
    let mut depth = 0usize;
    let mut close_idx = None;

    for (i, c) in text[open_idx..].char_indices() {
        let abs = open_idx + i;
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                close_idx = Some(abs);
                break;
            }
        }
    }

    let close_idx = close_idx?;
    let start = open_idx + open.len_utf8();
    let end = close_idx;
    if start <= end {
        Some((start, end))
    } else {
        None
    }
}

fn inner_quote_range(text: &str, cursor: usize, quote: char) -> Option<(usize, usize)> {
    if text.is_empty() {
        return None;
    }

    let pos = cursor.min(text.len());
    let mut left = None;
    for (i, c) in text[..pos].char_indices().rev() {
        if c == quote {
            left = Some(i);
            break;
        }
    }

    let left = left?;
    let mut right = None;
    let start = left + quote.len_utf8();
    for (i, c) in text[start..].char_indices() {
        if c == quote {
            right = Some(start + i);
            break;
        }
    }

    right.map(|r| (start, r))
}

fn sync_visual_line_selection(textarea: &mut TextArea<'static>, anchor_row: u16) {
    let current_row = textarea.cursor().0 as u16;

    textarea.cancel_selection();

    if current_row > anchor_row {
        textarea.move_cursor(CursorMove::Jump(anchor_row, 0));
        textarea.start_selection();
        textarea.move_cursor(CursorMove::Jump(current_row, 0));
        textarea.move_cursor(CursorMove::End);
    } else {
        textarea.move_cursor(CursorMove::Jump(anchor_row, 0));
        textarea.move_cursor(CursorMove::End);
        textarea.start_selection();
        textarea.move_cursor(CursorMove::Jump(current_row, 0));
    }
}

fn to_vim_key(key: KeyEvent) -> Option<VimKey> {
    let mut out = match key.code {
        KeyCode::Char(c) => VimKey::char(c),
        KeyCode::Esc => VimKey::code(VimKeyCode::Escape),
        KeyCode::Backspace => VimKey::code(VimKeyCode::Backspace),
        KeyCode::Delete => VimKey::code(VimKeyCode::Delete),
        KeyCode::Left => VimKey::code(VimKeyCode::Left),
        KeyCode::Right => VimKey::code(VimKeyCode::Right),
        KeyCode::Up => VimKey::code(VimKeyCode::Up),
        KeyCode::Down => VimKey::code(VimKeyCode::Down),
        KeyCode::Home => VimKey::code(VimKeyCode::Home),
        KeyCode::End => VimKey::code(VimKeyCode::End),
        KeyCode::Tab => VimKey::code(VimKeyCode::Tab),
        KeyCode::Enter => VimKey::code(VimKeyCode::Enter),
        _ => return None,
    };

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        out = out.ctrl();
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        out = out.alt();
    }
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        out = out.shift();
    }

    Some(out)
}

fn apply_vim_cursor_style(textarea: &mut TextArea<'static>, status: &str) {
    let style = match status {
        "INSERT" => Style::default().fg(Color::Black).bg(Color::Cyan),
        "NORMAL" => Style::default().fg(Color::Black).bg(Color::Yellow),
        "VISUAL" => Style::default().fg(Color::Black).bg(Color::Green),
        _ => Style::default().fg(Color::Black).bg(Color::Magenta),
    };
    textarea.set_cursor_style(style);
}

fn cursor_to_offset(text: &str, row: usize, col: usize) -> usize {
    let mut offset = 0usize;
    let mut lines = text.split('\n').peekable();

    for current_row in 0..row {
        let Some(line) = lines.next() else {
            return text.len();
        };
        offset += line.len();
        if lines.peek().is_some() || current_row < row {
            offset = (offset + 1).min(text.len());
        }
    }

    let line = text.split('\n').nth(row).unwrap_or("");
    let char_count = line.chars().count();
    let target_chars = col.min(char_count);
    let byte_in_line = line
        .char_indices()
        .nth(target_chars)
        .map(|(i, _)| i)
        .unwrap_or(line.len());
    (offset + byte_in_line).min(text.len())
}

fn offset_to_cursor(lines: &[String], offset: usize) -> (usize, usize) {
    let mut remaining = offset;
    for (row, line) in lines.iter().enumerate() {
        let line_len = line.len();
        if remaining <= line_len {
            let col = line[..remaining].chars().count();
            return (row, col);
        }
        remaining = remaining.saturating_sub(line_len + 1);
    }

    let row = lines.len().saturating_sub(1);
    let col = lines
        .get(row)
        .map(|line| line.chars().count())
        .unwrap_or(0);
    (row, col)
}

fn replace_textarea_from_string(textarea: &mut TextArea<'static>, content: &str, single_line: bool) {
    let mut rebuilt = if single_line {
        make_title_editor(content)
    } else {
        text_area_from_content(content)
    };

    if !single_line {
        rebuilt.set_cursor_line_style(Style::default().bg(Color::Rgb(32, 36, 44)));
    }

    *textarea = rebuilt;
}

fn make_title_editor(initial: &str) -> TextArea<'static> {
    let mut title = if initial.is_empty() {
        TextArea::default()
    } else {
        TextArea::from([initial.to_string()])
    };
    title.set_cursor_style(Style::default().fg(Color::Black).bg(Color::Cyan));
    title
}

fn get_title_text(title_editor: &TextArea<'static>) -> String {
    title_editor
        .lines()
        .join(" ")
        .replace(['\r', '\n'], " ")
        .trim()
        .to_string()
}

fn draw_ui(frame: &mut Frame, app: &App, focus: EditFocus) {
    match app.mode {
        ViewMode::List => draw_list_view(frame, app),
        ViewMode::Edit => draw_edit_view(frame, app, focus),
        ViewMode::Help => draw_help_view(frame, app),
    }
}

fn draw_help_view(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(3)])
        .split(area);

    let help = Paragraph::new(help_page_text())
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .wrap(Wrap { trim: false })
        .scroll((app.help_scroll, 0));
    frame.render_widget(help, chunks[0]);

    let footer = Paragraph::new(HELP_PAGE_HINTS)
        .block(Block::default().borders(Borders::ALL).title("Navigation"));
    frame.render_widget(footer, chunks[1]);
}

fn help_page_text() -> Text<'static> {
    Text::from(vec![
        Line::from(vec![
            Span::styled("clin", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(" Help", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        help_heading("Core Features"),
        help_item("Encrypted local note files (.clin)", None),
        help_item("In-terminal note list, open/edit, and auto-save on Esc", None),
        help_item("Open note file location from notes view", Some("f")),
        help_item("Delete notes with confirmation prompt", Some("d/Delete")),
        Line::from(""),
        help_heading("Notes View"),
        help_item("Move selection", Some("Up/Down")),
        help_item("Open selected note or create new", Some("Enter")),
        help_item("Request delete", Some("d/Delete")),
        help_item("Confirm / cancel delete", Some("y or Enter / n or Esc")),
        help_item("Open selected note file location", Some("f")),
        help_item("Change focus (notes list <-> Vim button)", Some("Tab")),
        help_item("Toggle Vim from focused button", Some("Enter/Space")),
        help_item("Open help", Some("? or F1")),
        help_item("Quit app", Some("q")),
        Line::from(""),
        help_heading("Editor (Vim OFF)"),
        help_item("Change focus (Title, Content, Vim button)", Some("Tab")),
        help_item("Auto-save and return to notes", Some("Esc")),
        help_item("Save and quit", Some("Ctrl+Q")),
        help_item("Copy / Cut / Paste", Some("Ctrl+C / Ctrl+X / Ctrl+V")),
        help_item("Alt clipboard keys", Some("Ctrl+Insert / Shift+Insert / Shift+Delete")),
        help_item("Select all / Undo / Redo", Some("Ctrl+A / Ctrl+Z / Ctrl+Y")),
        help_item("Redo alternate", Some("Ctrl+Shift+Z")),
        help_item("Delete prev/next word", Some("Ctrl+Backspace / Ctrl+Delete")),
        Line::from(""),
        help_heading("Editor (Vim ON)"),
        help_item("Modes", Some("NORMAL, INSERT, VISUAL, OPERATOR, REPLACE")),
        help_item("Movement", Some("h j k l, w e b, 0/^, $, %")),
        help_item("Visual line", Some("V then j/k, apply y/d/c/x/p")),
        help_item("Enter insert mode", Some("i I a A o O")),
        help_item("Visual select", Some("v (char), Esc to leave")),
        help_item("Operators", Some("d y c, plus dd yy cc and dw/cw/yw")),
        help_item("Inner object ops", Some("ciw/diw, ci(/di(, ci[/di[, ci{/di{, ci</di<")),
        help_item("Actions", Some("x D C r<char> p P")),
        Line::from(""),
        help_heading("Help Page"),
        help_item("Close help", Some("Esc / q / ? / F1")),
        help_item("Scroll", Some("Up/Down or j/k")),
    ])
}

fn help_heading(title: &'static str) -> Line<'static> {
    Line::from(vec![Span::styled(
        title,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )])
}

fn help_item(text: &'static str, key: Option<&'static str>) -> Line<'static> {
    match key {
        Some(key) => Line::from(vec![
            Span::styled("  - ", Style::default().fg(Color::DarkGray)),
            Span::styled(key, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::raw(text),
        ]),
        None => Line::from(vec![
            Span::styled("  - ", Style::default().fg(Color::DarkGray)),
            Span::raw(text),
        ]),
    }
}

fn draw_list_view(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    let header = Paragraph::new(Line::from(vec![
        Span::styled("clin", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("  encrypted terminal notes"),
    ]))
    .block(Block::default().borders(Borders::ALL).title("Notes"));
    frame.render_widget(header, chunks[0]);

    let mut items = Vec::new();
    for summary in &app.notes {
        let when = format_relative_time(summary.updated_at);
        items.push(ListItem::new(Line::from(vec![
            Span::styled(&summary.title, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("  ({when})")),
        ])));
    }
    items.push(ListItem::new(Line::from(vec![
        Span::styled("+ Create a new note", Style::default().fg(Color::Green)),
    ])));

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Select"))
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("  > ");
    let mut list_state = ListState::default();
    list_state.select(Some(app.selected));
    frame.render_stateful_widget(list, chunks[1], &mut list_state);

    let vim_button_label = if app.vim_enabled {
        "[ Vim: ON ]"
    } else {
        "[ Vim: OFF ]"
    };
    let vim_button_style = if app.list_focus == ListFocus::VimToggle {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if app.vim_enabled {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    };
<<<<<<< HEAD
    let footer_block = Block::default().borders(Borders::ALL).title("Help");
    frame.render_widget(footer_block, chunks[2]);
    let footer_inner = chunks[2].inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let footer_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(12), Constraint::Min(0)])
        .split(footer_inner);

    let vim_button = Paragraph::new(Line::from(Span::styled(
        vim_button_label,
        vim_button_style,
    )));
    frame.render_widget(vim_button, footer_chunks[0]);

    let status = Paragraph::new(app.status.as_str());
    frame.render_widget(status, footer_chunks[1]);
=======
    let footer_line = Line::from(vec![
        Span::raw(app.status.as_str()),
        Span::raw("   "),
        Span::styled(vim_button_label, vim_button_style),
    ]);

    let footer = Paragraph::new(footer_line)
        .block(Block::default().borders(Borders::ALL).title("Help"));
    frame.render_widget(footer, chunks[2]);
>>>>>>> 41ee4543292daab38c988cb9aa124ea81a8c9273
}

fn draw_edit_view(frame: &mut Frame, app: &App, focus: EditFocus) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(area);

    let top_text = if app.vim_enabled {
        let active_mode = match focus {
            EditFocus::Title if app.vim_title_compat.visual_line_active() => "VISUAL LINE",
            EditFocus::Body if app.vim_body_compat.visual_line_active() => "VISUAL LINE",
            EditFocus::Title => app.vim_title.status(),
            EditFocus::Body => app.vim_body.status(),
            EditFocus::VimToggle if app.vim_body_compat.visual_line_active() => "VISUAL LINE",
            EditFocus::VimToggle => app.vim_body.status(),
        };
        format!(
            "Editing {}   [VIM: {}]",
            app.editing_id.as_deref().unwrap_or("<unsaved>"),
            active_mode
        )
    } else {
        format!(
            "Editing {}",
            app.editing_id.as_deref().unwrap_or("<unsaved>")
        )
    };

    let top = Paragraph::new(top_text)
    .block(Block::default().borders(Borders::ALL).title("clin"));
    frame.render_widget(top, chunks[0]);

    let title_border = if focus == EditFocus::Title {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let mut title_editor = app.title_editor.clone();
    title_editor.set_block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(title_border)
            .title("Title"),
    );
    frame.render_widget(&title_editor, chunks[1]);

    if get_title_text(&app.title_editor).is_empty() {
        let title_inner = chunks[1].inner(Margin {
            vertical: 1,
            horizontal: 1,
        });
        let placeholder = Paragraph::new(Line::from(Span::styled(
            "Untitled note",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(placeholder, title_inner);
    }

    let mut editor = app.editor.clone();
    let body_border = if focus == EditFocus::Body {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    editor.set_block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(body_border)
            .title("Content"),
    );
    frame.render_widget(&editor, chunks[2]);

    let vim_button_label = if app.vim_enabled {
        "[ Vim: ON ]"
    } else {
        "[ Vim: OFF ]"
    };
    let vim_button_style = if focus == EditFocus::VimToggle {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if app.vim_enabled {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    };
<<<<<<< HEAD
    let status_block = Block::default().borders(Borders::ALL).title("Help");
    frame.render_widget(status_block, chunks[3]);
    let status_inner = chunks[3].inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let status_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(12), Constraint::Min(0)])
        .split(status_inner);

    let vim_button = Paragraph::new(Line::from(Span::styled(
        vim_button_label,
        vim_button_style,
    )));
    frame.render_widget(vim_button, status_chunks[0]);

    let status = Paragraph::new(app.status.as_str());
    frame.render_widget(status, status_chunks[1]);
=======
    let status_line = Line::from(vec![
        Span::raw(app.status.as_str()),
        Span::raw("   "),
        Span::styled(vim_button_label, vim_button_style),
    ]);

    let status = Paragraph::new(status_line).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Help"),
    );
    frame.render_widget(status, chunks[3]);
>>>>>>> 41ee4543292daab38c988cb9aa124ea81a8c9273

    if app.status.starts_with("Save failed") || app.status.starts_with("Could not open") {
        let popup = centered_rect(75, 20, area);
        frame.render_widget(Clear, popup);
        let text = Paragraph::new(app.status.as_str())
            .block(Block::default().borders(Borders::ALL).title("Error"))
            .wrap(Wrap { trim: true });
        frame.render_widget(text, popup);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1]);
    horizontal[1].inner(Margin {
        vertical: 0,
        horizontal: 0,
    })
}

fn text_area_from_content(content: &str) -> TextArea<'static> {
    if content.is_empty() {
        TextArea::default()
    } else {
        let lines: Vec<String> = content.lines().map(ToString::to_string).collect();
        TextArea::from(lines)
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs()
}

fn format_relative_time(unix_ts: u64) -> String {
    let now = now_unix_secs();
    let diff = now.saturating_sub(unix_ts);

    if diff < 60 {
        return "just now".to_string();
    }
    if diff < 3600 {
        return format!("{}m ago", diff / 60);
    }
    if diff < 86_400 {
        return format!("{}h ago", diff / 3600);
    }

    let secs = UNIX_EPOCH + Duration::from_secs(unix_ts);
    let dt: chrono::DateTime<chrono::Local> = secs.into();
    dt.format("%Y-%m-%d %H:%M").to_string()
}

fn open_in_file_manager(path: &Path) -> Result<()> {
    let command = if cfg!(target_os = "linux") {
        "xdg-open"
    } else if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        anyhow::bail!("opening file manager is not supported on this platform")
    };

    Command::new(command)
        .arg(path)
        .spawn()
        .with_context(|| format!("failed to launch {command}"))?;
    Ok(())
}
