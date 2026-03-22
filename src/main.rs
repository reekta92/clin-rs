use crate::vim::VimOutput;
mod helpers;
mod vim;

use std::fs;
use std::io::{self, Stdout};
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{env, process};

use crate::vim::VimMode;
use anyhow::{Context, Result, anyhow};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
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
use tui_textarea::CursorMove;
use tui_textarea::{Input, TextArea};
use uuid::Uuid;

const FILE_MAGIC: &[u8; 5] = b"CLIN1";
const NONCE_LEN: usize = 12;
const LIST_HELP_HINTS: &str = "Up/Down move   Enter open/new   Del/d delete   f location   ? help   Tab change focus   q quit";
const EDIT_HELP_HINTS: &str = "Esc back   Tab change focus   Ctrl+Q quit";
const VIM_EDIT_HELP_HINTS: &str = ":q quit  Tab change focus";
const HELP_PAGE_HINTS: &str = "Esc/q/?/F1 close help";
const _HELP_PAGE_TEXT: &str = "clin help\n\
\n\
Core features\n\
- Encrypted local notes stored as binary .clin files\n\
- Notes shown in a list, open/edit directly in terminal UI\n\
- Continual Auto-save\n\
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
- Esc: return to notes view\n\
- Ctrl+Q: quit app\n\
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppSettings {
    vim_enabled: bool,
    #[serde(default = "default_encryption_enabled")]
    encryption_enabled: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            vim_enabled: false,
            encryption_enabled: true,
        }
    }
}

fn default_encryption_enabled() -> bool {
    true
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
        self.notes_dir.join(id)
    }

    fn list_note_ids(&self) -> Result<Vec<String>> {
        let mut ids = Vec::new();
        for entry in fs::read_dir(&self.notes_dir).context("failed reading notes directory")? {
            let entry = entry.context("failed to read note entry")?;
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext == "clin" || ext == "md" || ext == "txt" {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        ids.push(name.to_string());
                    }
                }
            }
        }
        Ok(ids)
    }

    fn load_note(&self, id: &str) -> Result<Note> {
        let path = self.note_path(id);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        
        if ext == "clin" {
            let encrypted = fs::read(&path).context("failed to read note")?;
            let plain = self.decrypt(&encrypted)?;
            let (note, _) =
                bincode::serde::decode_from_slice::<Note, _>(&plain, bincode::config::standard())
                    .context("failed to decode note")?;
            Ok(note)
        } else {
            let content = fs::read_to_string(&path).context("failed to read plain note")?;
            let title = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Untitled note").to_string();
            let updated_at = fs::metadata(&path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
            Ok(Note {
                title,
                content,
                updated_at,
            })
        }
    }

    fn save_note(&self, id: &str, note: &Note, encryption_enabled: bool) -> Result<String> {
        let preferred_stem = self.note_file_stem_from_title(&note.title);
        
        let old_path = self.note_path(id);
        let old_ext = old_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        
        // If we are currently saving an unencrypted file and encryption is OFF, keep its extension.
        // Otherwise default to .md if no extension matched.
        let target_ext = if encryption_enabled {
            "clin"
        } else if old_ext == "txt" || old_ext == "md" {
            old_ext
        } else {
            "md"
        };

        let target_id = self.unique_note_id(&preferred_stem, target_ext, id);

        if target_ext == "clin" {
            let bytes = bincode::serde::encode_to_vec(note, bincode::config::standard())
                .context("failed to encode note")?;
            let encrypted = self.encrypt(&bytes)?;
            fs::write(self.note_path(&target_id), encrypted).context("failed to write note")?;
        } else {
            fs::write(self.note_path(&target_id), &note.content).context("failed to write plain note")?;
        }

        // We only remove the old file if the target_id differs AND we are not creating an encrypted copy of a plain text file.
        // Wait, the prompt says: "when encryption on if user tries to open a unencrypted file the program should create a encrypted copy... and edit that copy not the original unencrypted file".
        // App handles setting `editing_id` to a newly generated ID when opening to avoid overwriting. But here, just in case, if `id` != `target_id` we delete the old one, UNLESS we specifically want to avoid it.
        // If App changes `editing_id` to `uuid.clin` before saving, then `id` (the current editing state) is already `uuid.clin`, it won't equal `old_path` in name so... Wait.
        // If App changes `app.editing_id = Some(new_id)`, then the first time it autosaves, it calls `save_note(new_id, note, true)`. So `id` == `target_id` (likely), and the old file is untouched because `id` doesn't point to the original file anymore!
        // So `save_note` can safely delete `old_path` if `id != target_id` because `id` represents the file WE ARE CURRENTLY EDITING which is getting renamed.
        if id != target_id {
            let old_path_to_remove = self.note_path(id);
            if old_path_to_remove.exists() {
                fs::remove_file(&old_path_to_remove).context("failed to rename note file")?;
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

    fn unique_note_id(&self, preferred_stem: &str, ext: &str, current_id: &str) -> String {
        let mut candidate_stem = preferred_stem.to_string();
        let mut candidate = format!("{candidate_stem}.{ext}");
        let mut counter = 2_u32;

        while candidate != current_id && self.note_path(&candidate).exists() {
            candidate_stem = format!("{preferred_stem} ({counter})");
            candidate = format!("{candidate_stem}.{ext}");
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
    EncryptionToggle,
}

struct ContextMenu {
    x: u16,
    y: u16,
    selected: usize,
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
    encryption_enabled: bool,
    vim_title: crate::vim::Vim,
    vim_body: crate::vim::Vim,
    status: String,
    status_until: Option<Instant>,
    pending_delete_note_id: Option<String>,
    help_scroll: u16,
    context_menu: Option<ContextMenu>,
}

enum CliCommand {
    Run {
        edit_title: Option<String>,
    },
    NewAndOpen {
        title: Option<String>,
    },
    QuickNote {
        content: String,
        title: Option<String>,
    },
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
            encryption_enabled: settings.encryption_enabled,
            vim_title: crate::vim::Vim::new(),
            vim_body: crate::vim::Vim::new(),
            status: String::from(LIST_HELP_HINTS),
            status_until: None,
            pending_delete_note_id: None,
            help_scroll: 0,
            context_menu: None,
        };
        app.context_menu = None;
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
            let is_clin = summary.id.ends_with(".clin");
            if !self.encryption_enabled && is_clin {
                self.status = "Cannot open encrypted notes while encryption is disabled.".to_string();
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
        self.editor
            .set_cursor_style(Style::default().fg(Color::Black).bg(Color::Cyan));
        self.editor
            .set_cursor_line_style(Style::default().bg(Color::Rgb(32, 36, 44)));
        self.reset_vim_states_for_edit();
        self.set_default_status();
    }

    fn autosave(&mut self) {
        let Some(id) = self.editing_id.clone() else {
            return;
        };
        let mut title = get_title_text(&self.title_editor).trim().to_string();
        if title.is_empty() {
            title = String::from("Untitled note");
        }
        let note = Note {
            title,
            content: self.editor.lines().join(
                "
",
            ),
            updated_at: now_unix_secs(),
        };
        if let Ok(saved_id) = self.storage.save_note(&id, &note, self.encryption_enabled) {
            self.editing_id = Some(saved_id);
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

    fn handle_menu_action(&mut self, action: usize, focus: &mut EditFocus) {
        match action {
            0 => {
                match focus {
                    EditFocus::Title => { self.title_editor.copy(); }
                    EditFocus::Body => { self.editor.copy(); }
                    _ => {}
                }
            }
            1 => {
                match focus {
                    EditFocus::Title => { self.title_editor.cut(); }
                    EditFocus::Body => { self.editor.cut(); }
                    _ => {}
                }
            }
            2 => {
                match focus {
                    EditFocus::Title => { self.title_editor.paste(); }
                    EditFocus::Body => { self.editor.paste(); }
                    _ => {}
                }
            }
            3 => {
                match focus {
                    EditFocus::Title => { self.title_editor.select_all(); }
                    EditFocus::Body => { self.editor.select_all(); }
                    _ => {}
                }
            }
            _ => {}
        }
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
        self.vim_title.reset();
        self.vim_body.reset();

        if self.vim_enabled {
            self.title_editor
                .set_cursor_style(self.vim_title.mode.cursor_style());
            self.editor
                .set_cursor_style(self.vim_body.mode.cursor_style());
        } else {
            self.title_editor
                .set_cursor_style(Style::default().fg(Color::Black).bg(Color::Cyan));
            self.editor
                .set_cursor_style(Style::default().fg(Color::Black).bg(Color::Cyan));
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
            encryption_enabled: self.encryption_enabled,
        });
    }

    fn toggle_encryption_mode(&mut self) {
        self.encryption_enabled = !self.encryption_enabled;
        self.set_default_status();
        self.storage.save_settings(&AppSettings {
            vim_enabled: self.vim_enabled,
            encryption_enabled: self.encryption_enabled,
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
            ViewMode::Edit => {
                if self.vim_enabled {
                    VIM_EDIT_HELP_HINTS
                } else {
                    EDIT_HELP_HINTS
                }
            }
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
    EncryptionToggle,
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
            let saved_id = storage.save_note(&id, &note, true)?;
            println!("Saved quick note \"{}\" as {}.clin", final_title, saved_id);
            Ok(())
        }
        CliCommand::NewAndOpen { title } => {
            let storage = Storage::init()?;
            let settings = storage.load_settings();
            let final_title = title
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| String::from("Untitled note"));

            let note = Note {
                title: final_title.clone(),
                content: String::new(),
                updated_at: now_unix_secs(),
            };
            
            let ext = if settings.encryption_enabled { "clin" } else { "md" };
            let base_id = storage.new_note_id();
            let id = format!("{}.{}", base_id, ext);
            let saved_id = storage.save_note(&id, &note, settings.encryption_enabled)?;

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

    let run_result = {
        let mut terminal_safe = std::panic::AssertUnwindSafe(&mut terminal);
        let mut app_safe = std::panic::AssertUnwindSafe(&mut *app);
        let res = std::panic::catch_unwind(move || run_app(*terminal_safe, *app_safe));

        if app.mode == ViewMode::Edit {
            app.autosave();
        }

        match res {
            Ok(r) => r,
            Err(err) => std::panic::resume_unwind(err),
        }
    };

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

fn run_app(
    terminal: &mut Terminal<ratatui::backend::CrosstermBackend<Stdout>>,
    app: &mut App,
) -> Result<()> {
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
                Event::Paste(data) if app.mode == ViewMode::Edit => match focus {
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
                    EditFocus::EncryptionToggle => {}
                },
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
            ListFocus::VimToggle => ListFocus::EncryptionToggle,
            ListFocus::EncryptionToggle => ListFocus::Notes,
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
    if app.list_focus == ListFocus::EncryptionToggle {
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                app.toggle_encryption_mode();
            }
            KeyCode::Char('q') => return true,
            _ => {}
        }
        return false;
    }
    if app.list_focus == ListFocus::EncryptionToggle {
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                app.toggle_encryption_mode();
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
    if let Some(mut menu) = app.context_menu.take() {
        match key.code {
            KeyCode::Up => {
                menu.selected = menu.selected.saturating_sub(1);
                app.context_menu = Some(menu);
            }
            KeyCode::Down => {
                if menu.selected < 3 {
                    menu.selected += 1;
                }
                app.context_menu = Some(menu);
            }
            KeyCode::Enter => {
                app.handle_menu_action(menu.selected, focus);
            }
            KeyCode::Esc => {
                app.context_menu = None;
            }
            _ => {
                app.context_menu = Some(menu);
            }
        }
        return false;
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
        app.autosave();
        return true;
    }

    if key.code == KeyCode::Tab {
        *focus = match *focus {
            EditFocus::Title => EditFocus::Body,
            EditFocus::Body => EditFocus::VimToggle,
            EditFocus::VimToggle => EditFocus::EncryptionToggle,
            EditFocus::EncryptionToggle => EditFocus::Title,
        };
        return false;
    }

    if app.vim_enabled && *focus != EditFocus::VimToggle && *focus != EditFocus::EncryptionToggle {
        let (output, did_transition) = match *focus {
            EditFocus::Title => {
                let out = app.vim_title.transition(key, &mut app.title_editor);
                (out, true)
            }
            EditFocus::Body => {
                let out = app.vim_body.transition(key, &mut app.editor);
                (out, true)
            }
            _ => (VimOutput::Unhandled, false),
        };

        if did_transition {
            match output {
                VimOutput::Command(cmd) => {
                    if cmd == ":q" || cmd == ":q!" || cmd == ":wq" || cmd == ":x" {
                        app.autosave();
                        app.back_to_list();
                        *focus = EditFocus::Body;
                    }
                    return false;
                }
                VimOutput::Handled => return false,
                VimOutput::Unhandled => {
                    // fall through
                }
            }
        }
    }

    if key.code == KeyCode::Esc {
        if !app.vim_enabled {
            app.autosave();
            app.back_to_list();
            *focus = EditFocus::Body;
        }
        return false;
    }

    match *focus {
        EditFocus::Title => {
            if key.code == KeyCode::Enter
                && (!app.vim_enabled || app.vim_title.mode != VimMode::Normal)
            {
                *focus = EditFocus::Body;
                return false;
            }

            if handle_os_shortcuts(&mut app.title_editor, key) {
                return false;
            }

            if app.title_editor.input(Input::from(key)) && app.title_editor.lines().len() > 1 {
                let normalized = get_title_text(&app.title_editor).replace(['\r', '\n'], " ");
                app.title_editor = make_title_editor(&normalized);
            }
        }
        EditFocus::Body => {
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
        EditFocus::EncryptionToggle => {
            if key.code == KeyCode::Enter || key.code == KeyCode::Char(' ') {
                app.toggle_encryption_mode();
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
    if let Some(menu) = &app.context_menu {
        let menu_rect = Rect::new(menu.x, menu.y, 14, 6);
        if contains_cell(menu_rect, mouse_event.column, mouse_event.row) {
            if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
                let clicked_idx = mouse_event.row.saturating_sub(menu.y).saturating_sub(1) as usize;
                if clicked_idx < 4 {
                    app.handle_menu_action(clicked_idx, focus);
                }
                app.context_menu = None;
            } else if mouse_event.kind == MouseEventKind::ScrollUp {
                let mut menu_copy = app.context_menu.take().unwrap();
                menu_copy.selected = menu_copy.selected.saturating_sub(1);
                app.context_menu = Some(menu_copy);
            } else if mouse_event.kind == MouseEventKind::ScrollDown {
                let mut menu_copy = app.context_menu.take().unwrap();
                if menu_copy.selected < 3 {
                    menu_copy.selected += 1;
                }
                app.context_menu = Some(menu_copy);
            }
            return;
        } else if matches!(mouse_event.kind, MouseEventKind::Down(_)) {
            app.context_menu = None;
            if mouse_event.kind != MouseEventKind::Down(MouseButton::Right) {
                return;
            }
        } else {
            return;
        }
    }

    if mouse_event.kind == MouseEventKind::Down(MouseButton::Right) {
        let (title_inner, body_inner) = edit_view_input_areas(terminal_area);
        
        if contains_cell(title_inner, mouse_event.column, mouse_event.row) {
            *focus = EditFocus::Title;
            move_textarea_cursor_to_mouse(&mut app.title_editor, title_inner, mouse_event.column, mouse_event.row);
        } else if contains_cell(body_inner, mouse_event.column, mouse_event.row) {
            *focus = EditFocus::Body;
            move_textarea_cursor_to_mouse(&mut app.editor, body_inner, mouse_event.column, mouse_event.row);
        }

        let max_x = terminal_area.width.saturating_sub(14);
        let max_y = terminal_area.height.saturating_sub(6);
        app.context_menu = Some(ContextMenu {
            x: mouse_event.column.min(max_x),
            y: mouse_event.row.min(max_y),
            selected: 0,
        });
        return;
    }

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
                move_textarea_cursor_to_mouse(&mut app.editor, body_inner, mouse_event.column, mouse_event.row);
                app.editor.start_selection();
                *mouse_selecting = true;
            } else if contains_cell(title_inner, mouse_event.column, mouse_event.row) {
                *focus = EditFocus::Title;
                move_textarea_cursor_to_mouse(&mut app.title_editor, title_inner, mouse_event.column, mouse_event.row);
                app.title_editor.start_selection();
                *mouse_selecting = true;
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if *mouse_selecting {
                if *focus == EditFocus::Body {
                    move_textarea_cursor_to_mouse(&mut app.editor, body_inner, mouse_event.column, mouse_event.row);
                } else {
                    move_textarea_cursor_to_mouse(&mut app.title_editor, title_inner, mouse_event.column, mouse_event.row);
                }
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            *mouse_selecting = false;
        }
        _ => {}
    }
}

fn move_textarea_cursor_to_mouse(textarea: &mut TextArea, body_inner: Rect, mouse_col: u16, mouse_row: u16) {
    if textarea.lines().is_empty() || body_inner.width == 0 || body_inner.height == 0 {
        return;
    }

    let mut scroll_row = 0;
    let mut scroll_col = 0;
    let debug_str = format!("{:?}", textarea);
    if let Some(start) = debug_str.find("viewport: Viewport(") {
        let after_start = &debug_str[start + "viewport: Viewport(".len()..];
        if let Some(end) = after_start.find(')') {
            let number_str = &after_start[..end];
            if let Ok(number) = number_str.parse::<u64>() {
                scroll_row = (number >> 16) as usize;
                scroll_col = (number & 0xFFFF) as usize;
            }
        }
    }

    let row = mouse_row.saturating_sub(body_inner.y) as usize + scroll_row;
    let col = mouse_col.saturating_sub(body_inner.x) as usize + scroll_col;

    let max_row = textarea.lines().len().saturating_sub(1);
    let target_row = row.min(max_row);
    let max_col = textarea.lines()[target_row].chars().count();
    let target_col = col.min(max_col);

    textarea.move_cursor(CursorMove::Jump(target_row as u16, target_col as u16));
}

fn edit_view_input_areas(area: Rect) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(area);

    let title_inner = chunks[0].inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let body_inner = chunks[1].inner(Margin {
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
            Span::styled(
                "clin",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Help", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        help_heading("Core Features"),
        help_item("Encrypted local note files (.clin)", None),
        help_item(
            "In-terminal note list, full text editor, and continual auto-save",
            None,
        ),
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
        help_item("Return to notes (continually auto-saved)", Some("Esc")),
        help_item("Save and quit", Some("Ctrl+Q")),
        help_item("Copy / Cut / Paste", Some("Ctrl+C / Ctrl+X / Ctrl+V")),
        help_item(
            "Alt clipboard keys",
            Some("Ctrl+Insert / Shift+Insert / Shift+Delete"),
        ),
        help_item("Select all / Undo / Redo", Some("Ctrl+A / Ctrl+Z / Ctrl+Y")),
        help_item("Redo alternate", Some("Ctrl+Shift+Z")),
        help_item(
            "Delete prev/next word",
            Some("Ctrl+Backspace / Ctrl+Delete"),
        ),
        Line::from(""),
        help_heading("Editor (Vim ON)"),
        help_item("Modes", Some("NORMAL, INSERT, VISUAL, OPERATOR, REPLACE")),
        help_item("Movement", Some("h j k l, w e b, 0/^, $, %")),
        help_item("Visual line", Some("V then j/k, apply y/d/c/x/p")),
        help_item("Enter insert mode", Some("i I a A o O")),
        help_item("Visual select", Some("v (char), Esc to leave")),
        help_item("Operators", Some("d y c, plus dd yy cc and dw/cw/yw")),
        help_item(
            "Inner object ops",
            Some("ciw/diw, ci(/di(, ci[/di[, ci{/di{, ci</di<"),
        ),
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
            Span::styled(
                key,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
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
        Span::styled(
            "clin",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  encrypted terminal notes"),
    ]))
    .block(Block::default().borders(Borders::ALL).title("Notes"));
    frame.render_widget(header, chunks[0]);

    let mut items = Vec::new();
    for summary in &app.notes {
        let when = format_relative_time(summary.updated_at);
        let mut text_style = Style::default().add_modifier(Modifier::BOLD);
        let mut title = summary.title.clone();
        
        let is_clin = summary.id.ends_with(".clin");
        if !app.encryption_enabled && is_clin {
            text_style = text_style.fg(Color::Red);
            title = format!("[ENC] {title}");
        }
        
        items.push(ListItem::new(Line::from(vec![
            Span::styled(
                title,
                text_style,
            ),
            Span::raw(format!("  ({when})")),
        ])));
    }
    items.push(ListItem::new(Line::from(vec![Span::styled(
        "+ Create a new note",
        Style::default().fg(Color::Green),
    )])));

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
        Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else if app.vim_enabled {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    };
    
    let enc_button_label = if app.encryption_enabled {
        "[ Enc: ON ]"
    } else {
        "[ Enc: OFF ]"
    };
    let enc_button_style = if app.list_focus == ListFocus::EncryptionToggle {
        Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else if app.encryption_enabled {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    };

    let footer_line = Line::from(vec![
        Span::styled(vim_button_label, vim_button_style),
        Span::raw("   "),
        Span::styled(enc_button_label, enc_button_style),
        Span::raw("   "),
        Span::raw(app.status.as_str()),
    ]);

    let footer =
        Paragraph::new(footer_line).block(Block::default().borders(Borders::ALL).title("Help"));
    frame.render_widget(footer, chunks[2]);
}

fn draw_edit_view(frame: &mut Frame, app: &App, focus: EditFocus) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(area);

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
    frame.render_widget(&title_editor, chunks[0]);

    if get_title_text(&app.title_editor).is_empty() {
        let title_inner = chunks[0].inner(Margin {
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
    frame.render_widget(&editor, chunks[1]);

    let vim_button_label = if app.vim_enabled {
        let active_mode = match focus {
            EditFocus::Title => app.vim_title.mode.name(),
            EditFocus::Body => app.vim_body.mode.name(),
            EditFocus::VimToggle => app.vim_body.mode.name(),
            EditFocus::EncryptionToggle => app.vim_body.mode.name(),
        };
        format!("[ Vim: {} ]", active_mode)
    } else {
        "[ Vim: OFF ]".to_string()
    };
    let vim_button_style = if focus == EditFocus::VimToggle {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if app.vim_enabled {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    };
    let status_line = Line::from(vec![
        Span::styled(vim_button_label, vim_button_style),
        Span::raw("   "),
        Span::raw(app.status.as_str()),
    ]);

    let status =
        Paragraph::new(status_line).block(Block::default().borders(Borders::ALL).title("Help"));
    frame.render_widget(status, chunks[2]);

    if app.status.starts_with("Save failed") || app.status.starts_with("Could not open") {
        let popup = centered_rect(75, 20, area);
        frame.render_widget(Clear, popup);
        let text = Paragraph::new(app.status.as_str())
            .block(Block::default().borders(Borders::ALL).title("Error"))
            .wrap(Wrap { trim: true });
        frame.render_widget(text, popup);
    }

    if let Some(menu) = &app.context_menu {
        let items = vec![
            ListItem::new(" Copy       "),
            ListItem::new(" Cut        "),
            ListItem::new(" Paste      "),
            ListItem::new(" Select All "),
        ];
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        
        let menu_area = Rect::new(menu.x, menu.y, 14, 6);
        let mut state = ListState::default();
        state.select(Some(menu.selected));
        
        frame.render_widget(Clear, menu_area);
        frame.render_stateful_widget(list, menu_area, &mut state);
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
