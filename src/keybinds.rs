use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};
use crate::config::KeybindPreset;

/// A single keystroke — one key code with optional modifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyStroke {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

/// A key combination, possibly a multi-key sequence like `"g g"` or `"Ctrl+x Ctrl+s"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    pub keys: Vec<KeyStroke>,
}

impl KeyStroke {
    /// Returns true if this single keystroke matches the given event.
    pub fn matches_event(&self, event: &KeyEvent) -> bool {
        if self.code != event.code {
            return false;
        }
        if self.code == KeyCode::BackTab {
            let self_mods = self.modifiers & !KeyModifiers::SHIFT;
            let event_mods = event.modifiers & !KeyModifiers::SHIFT;
            self_mods == event_mods
        } else {
            self.modifiers == event.modifiers
        }
    }
}

impl KeyCombo {
    /// Build a single-key combo with no modifiers.
    pub fn simple(code: KeyCode) -> Self {
        Self {
            keys: vec![KeyStroke {
                code,
                modifiers: KeyModifiers::NONE,
            }],
        }
    }

    /// Build a single-key combo with CONTROL modifier.
    pub fn ctrl(code: KeyCode) -> Self {
        Self {
            keys: vec![KeyStroke {
                code,
                modifiers: KeyModifiers::CONTROL,
            }],
        }
    }

    /// Build a single-key combo with SHIFT modifier.
    pub fn shift(code: KeyCode) -> Self {
        Self {
            keys: vec![KeyStroke {
                code,
                modifiers: KeyModifiers::SHIFT,
            }],
        }
    }

    /// Build a single-key combo with CONTROL|SHIFT modifiers.
    pub fn ctrl_shift(code: KeyCode) -> Self {
        Self {
            keys: vec![KeyStroke {
                code,
                modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            }],
        }
    }

    /// Parse a single keystroke token (e.g. `"Ctrl+q"`, `"g"`, `"Enter"`).
    fn parse_single_stroke(s: &str) -> Option<KeyStroke> {
        if s.is_empty() {
            return None;
        }
        let (modifiers_str, key_part) = if s == "+" {
            ("", "+")
        } else if let Some(stripped) = s.strip_suffix("++") {
            (stripped, "+")
        } else {
            match s.rfind('+') {
                Some(idx) => (&s[..idx], &s[idx + 1..]),
                None => ("", s),
            }
        };

        let mut modifiers = KeyModifiers::NONE;
        if !modifiers_str.is_empty() {
            let parts: Vec<&str> = modifiers_str.split('+').collect();
            for part in parts {
                let part_lower = part.to_lowercase();
                match part_lower.as_str() {
                    "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
                    "shift" => modifiers |= KeyModifiers::SHIFT,
                    "alt" => modifiers |= KeyModifiers::ALT,
                    "super" | "meta" | "cmd" => modifiers |= KeyModifiers::SUPER,
                    _ => return None,
                }
            }
        }

        let code = parse_key_code(key_part)?;
        Some(KeyStroke { code, modifiers })
    }

    /// Parse a key-combo string, possibly a multi-key sequence.
    /// Whitespace separates keys: `"g g"`, `"Space f"`, `"Ctrl+x Ctrl+s"`.
    pub fn parse(s: &str) -> Option<Self> {
        if s.is_empty() {
            return None;
        }
        let tokens: Vec<&str> = s.split_ascii_whitespace().collect();
        if tokens.is_empty() {
            return None;
        }
        let mut keys = Vec::with_capacity(tokens.len());
        for token in tokens {
            keys.push(Self::parse_single_stroke(token)?);
        }
        Some(Self { keys })
    }

    /// Format a single keystroke for display (e.g. `"Ctrl+q"`, `"g"`, `"Enter"`).
    fn stroke_to_string(s: &KeyStroke) -> String {
        let key = key_code_to_string(&s.code);
        let mut result = String::with_capacity(24);

        let mut need_sep = false;
        if s.modifiers.contains(KeyModifiers::CONTROL) {
            result.push_str("Ctrl");
            need_sep = true;
        }
        if s.modifiers.contains(KeyModifiers::SHIFT) {
            if need_sep {
                result.push('+');
            }
            result.push_str("Shift");
            need_sep = true;
        }
        if s.modifiers.contains(KeyModifiers::ALT) {
            if need_sep {
                result.push('+');
            }
            result.push_str("Alt");
            need_sep = true;
        }
        if s.modifiers.contains(KeyModifiers::SUPER) {
            if need_sep {
                result.push('+');
            }
            result.push_str("Super");
            need_sep = true;
        }
        if need_sep {
            result.push('+');
        }
        result.push_str(&key);

        result
    }

    /// Display string for this combo.
    /// Joins with no separator when every key is a single ASCII letter/digit (`gg`, `dd`),
    /// otherwise with a space (`g G`, `Space f`, `Ctrl+x Ctrl+s`).
    pub fn to_display_string(&self) -> String {
        let parts: Vec<String> = self.keys.iter().map(Self::stroke_to_string).collect();

        let all_simple = self.keys.iter().all(|s| {
            s.modifiers == KeyModifiers::NONE
                && if let KeyCode::Char(c) = s.code {
                    c.is_ascii_alphanumeric()
                } else {
                    false
                }
        });

        if all_simple {
            parts.join("")
        } else {
            parts.join(" ")
        }
    }

    /// Legacy single-key fast-path match.
    /// Returns `true` if this is a length-1 combo whose key matches the event.
    /// Multi-key combos always return `false` here; use `KeyMatcher::resolve` for sequences.
    pub fn matches(&self, event: &KeyEvent) -> bool {
        self.keys.len() == 1 && self.keys[0].matches_event(event)
    }
}

fn parse_key_code(s: &str) -> Option<KeyCode> {
    let s_lower = s.to_lowercase();
    match s_lower.as_str() {
        "enter" | "return" => Some(KeyCode::Enter),
        "esc" | "escape" => Some(KeyCode::Esc),
        "backspace" | "bs" => Some(KeyCode::Backspace),
        "tab" => Some(KeyCode::Tab),
        "backtab" => Some(KeyCode::BackTab),
        "space" | " " => Some(KeyCode::Char(' ')),
        "delete" | "del" => Some(KeyCode::Delete),
        "insert" | "ins" => Some(KeyCode::Insert),
        "home" => Some(KeyCode::Home),
        "end" => Some(KeyCode::End),
        "pageup" | "pgup" => Some(KeyCode::PageUp),
        "pagedown" | "pgdn" => Some(KeyCode::PageDown),
        "up" => Some(KeyCode::Up),
        "down" => Some(KeyCode::Down),
        "left" => Some(KeyCode::Left),
        "right" => Some(KeyCode::Right),

        "f1" => Some(KeyCode::F(1)),
        "f2" => Some(KeyCode::F(2)),
        "f3" => Some(KeyCode::F(3)),
        "f4" => Some(KeyCode::F(4)),
        "f5" => Some(KeyCode::F(5)),
        "f6" => Some(KeyCode::F(6)),
        "f7" => Some(KeyCode::F(7)),
        "f8" => Some(KeyCode::F(8)),
        "f9" => Some(KeyCode::F(9)),
        "f10" => Some(KeyCode::F(10)),
        "f11" => Some(KeyCode::F(11)),
        "f12" => Some(KeyCode::F(12)),

        _ if s.len() == 1 => {
            let c = s.chars().next()?;
            Some(KeyCode::Char(c.to_ascii_lowercase()))
        }

        _ => None,
    }
}

use std::borrow::Cow;

fn key_code_to_string(code: &KeyCode) -> Cow<'static, str> {
    match code {
        KeyCode::Enter => Cow::Borrowed("Enter"),
        KeyCode::Esc => Cow::Borrowed("Esc"),
        KeyCode::Backspace => Cow::Borrowed("Backspace"),
        KeyCode::Tab => Cow::Borrowed("Tab"),
        KeyCode::BackTab => Cow::Borrowed("BackTab"),
        KeyCode::Delete => Cow::Borrowed("Delete"),
        KeyCode::Insert => Cow::Borrowed("Insert"),
        KeyCode::Home => Cow::Borrowed("Home"),
        KeyCode::End => Cow::Borrowed("End"),
        KeyCode::PageUp => Cow::Borrowed("PageUp"),
        KeyCode::PageDown => Cow::Borrowed("PageDown"),
        KeyCode::Up => Cow::Borrowed("Up"),
        KeyCode::Down => Cow::Borrowed("Down"),
        KeyCode::Left => Cow::Borrowed("Left"),
        KeyCode::Right => Cow::Borrowed("Right"),
        KeyCode::F(n) => Cow::Owned(format!("F{n}")),
        KeyCode::Char(' ') => Cow::Borrowed("Space"),
        KeyCode::Char(c) => Cow::Owned(c.to_string()),
        _ => Cow::Borrowed("?"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListAction {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    Open,
    Delete,
    Quit,
    Help,
    OpenLocation,
    CycleFocus,
    Confirm,
    Cancel,
    ToggleExternalEditor,
    NewFromTemplate,
    CreateFolder,
    CreateNote,
    RenameFolder,
    MoveNote,
    ManageTags,
    CollapseFolder,
    ExpandFolder,
    OpenCommandPalette,

    Rename,
    Duplicate,
    TogglePin,
    CycleSort,
    Search,
    JumpToTop,
    JumpToBottom,
    PageUp,
    PageDown,
    OpenTrash,
    TogglePreview,
    TogglePreviewFullscreen,
    TogglePreviewWrap,
    OpenGraph,
    OpenCanvas,
    CreatePinstar,
    ToggleSelectMode,
    ToggleSelectItem,
    CollapseAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditAction {
    Quit,
    Back,
    CycleFocus,

    SelectAll,
    Copy,
    Cut,
    Paste,
    Undo,
    Redo,
    DeleteWord,
    DeleteNextWord,
    MoveToTop,
    MoveToBottom,
    ToggleMarkdownPreview,
    TogglePreviewFullscreen,
    TogglePreviewWrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HelpAction {
    Close,
    ScrollUp,
    ScrollDown,
    NextTab,
    PrevTab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphAction {
    Quit,
    PanUp,
    PanDown,
    PanLeft,
    PanRight,
    ZoomIn,
    ZoomOut,
    OpenNote,
    AutoFit,
    Help,
    ToggleSearch,
    ToggleMinimap,
    ToggleLegend,
    ToggleGrid,
    ToggleStatus,
    Refresh,
    ReloadConfig,
    TogglePreview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrawAction {
    Quit,
    SelectDrawTool,
    ToggleShapeSelector,
    SelectTextTool,
    SelectEraseTool,
    ShapeSelectorUp,
    ShapeSelectorDown,
    ShapeSelectorConfirm,
    ShapeSelectorCancel,
    TextEditorConfirm,
    TextEditorCancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasAction {
    Quit,
    Save,
    ZoomFineIn,
    ZoomFineOut,
    ZoomIn,
    ZoomOut,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    EditOrConnect,
    OpenContextMenu,
    ToggleGrid,
    ToggleEditorPane,
    CycleFocus,
    Help,
    RenameConfirm,
    RenameCancel,
    MenuClose,
    MenuUp,
    MenuDown,
    MenuSelect,
    CloseEditor,
    CloseEditorAlt,
    ConfirmResize,
    CancelResize,
    EditorUnfocus,
    EditorSyncRaw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupAction {
    Back,
    MoveDown,
    MoveUp,
    ScrollDiffDown,
    ScrollDiffUp,
    Refresh,
    EnterCommit,
    Push,
    OpenSettings,
    CycleSection,
    CancelCommit,
    ConfirmCommit,
    CloseSettings,
    ToggleFileSelect,
    NextField,
    PrevField,
    ActivateField,
    CancelEditField,
    ConfirmEditField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentTreeAction {
    MoveUp,
    MoveDown,
    ToggleCollapse,
    ExpandAll,
    CollapseAll,
    Open,
    Back,
    Help,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeybindsToml {
    #[serde(default)]
    pub list: HashMap<ListAction, Vec<String>>,
    #[serde(default)]
    pub edit: HashMap<EditAction, Vec<String>>,
    #[serde(default)]
    pub help: HashMap<HelpAction, Vec<String>>,
    #[serde(default)]
    pub graph: HashMap<GraphAction, Vec<String>>,
    #[serde(default)]
    pub draw: HashMap<DrawAction, Vec<String>>,
    #[serde(default)]
    pub canvas: HashMap<CanvasAction, Vec<String>>,
    #[serde(default)]
    pub backup: HashMap<BackupAction, Vec<String>>,
    #[serde(default)]
    pub content_tree: HashMap<ContentTreeAction, Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct Keybinds {
    pub list: HashMap<ListAction, Vec<KeyCombo>>,
    pub edit: HashMap<EditAction, Vec<KeyCombo>>,
    pub help: HashMap<HelpAction, Vec<KeyCombo>>,
    pub graph: HashMap<GraphAction, Vec<KeyCombo>>,
    pub draw: HashMap<DrawAction, Vec<KeyCombo>>,
    pub canvas: HashMap<CanvasAction, Vec<KeyCombo>>,
    pub backup: HashMap<BackupAction, Vec<KeyCombo>>,
    pub content_tree: HashMap<ContentTreeAction, Vec<KeyCombo>>,
}

impl Default for Keybinds {
    fn default() -> Self {
        let mut list = HashMap::new();
        list.insert(
            ListAction::MoveUp,
            vec![
                KeyCombo::simple(KeyCode::Up),
                KeyCombo::simple(KeyCode::Char('k')),
            ],
        );
        list.insert(
            ListAction::MoveDown,
            vec![
                KeyCombo::simple(KeyCode::Down),
                KeyCombo::simple(KeyCode::Char('j')),
            ],
        );
        list.insert(
            ListAction::MoveLeft,
            vec![
                KeyCombo::simple(KeyCode::Left),
                KeyCombo::simple(KeyCode::Char('h')),
            ],
        );
        list.insert(
            ListAction::MoveRight,
            vec![
                KeyCombo::simple(KeyCode::Right),
                KeyCombo::simple(KeyCode::Char('l')),
            ],
        );
        list.insert(ListAction::Open, vec![KeyCombo::simple(KeyCode::Enter)]);
        list.insert(
            ListAction::Delete,
            vec![
                KeyCombo::simple(KeyCode::Char('d')),
                KeyCombo::simple(KeyCode::Delete),
            ],
        );
        list.insert(ListAction::Quit, vec![KeyCombo::simple(KeyCode::Char('q'))]);
        list.insert(
            ListAction::Help,
            vec![
                KeyCombo::simple(KeyCode::Char('?')),
                KeyCombo::simple(KeyCode::F(1)),
            ],
        );
        list.insert(
            ListAction::OpenLocation,
            vec![KeyCombo::ctrl(KeyCode::Char('f'))],
        );
        list.insert(
            ListAction::CycleFocus,
            vec![
                KeyCombo::simple(KeyCode::Tab),
                KeyCombo::simple(KeyCode::BackTab),
            ],
        );
        list.insert(
            ListAction::Confirm,
            vec![
                KeyCombo::simple(KeyCode::Char('y')),
                KeyCombo::simple(KeyCode::Enter),
            ],
        );
        list.insert(
            ListAction::Cancel,
            vec![
                KeyCombo::simple(KeyCode::Char('n')),
                KeyCombo::simple(KeyCode::Esc),
            ],
        );
        list.insert(
            ListAction::ToggleExternalEditor,
            vec![KeyCombo::simple(KeyCode::Char('e'))],
        );
        list.insert(
            ListAction::NewFromTemplate,
            vec![KeyCombo::simple(KeyCode::Char('t'))],
        );
        list.insert(
            ListAction::CreateFolder,
            vec![KeyCombo::simple(KeyCode::Char('n'))],
        );
        list.insert(
            ListAction::CreateNote,
            vec![KeyCombo::simple(KeyCode::Char('a'))],
        );
        list.insert(
            ListAction::RenameFolder,
            vec![KeyCombo::simple(KeyCode::Char('r'))],
        );
        list.insert(
            ListAction::MoveNote,
            vec![KeyCombo::simple(KeyCode::Char('m'))],
        );
        list.insert(
            ListAction::ManageTags,
            vec![KeyCombo::simple(KeyCode::Char('.'))],
        );
        list.insert(
            ListAction::OpenCommandPalette,
            vec![
                KeyCombo::ctrl(KeyCode::Char('p')),
                KeyCombo::shift(KeyCode::Enter),
            ],
        );

        list.insert(
            ListAction::Rename,
            vec![KeyCombo::simple(KeyCode::Char('r'))],
        );
        list.insert(
            ListAction::Duplicate,
            vec![KeyCombo::simple(KeyCode::Char('y'))],
        );
        list.insert(
            ListAction::TogglePin,
            vec![KeyCombo::simple(KeyCode::Char('p'))],
        );
        list.insert(
            ListAction::CycleSort,
            vec![KeyCombo::simple(KeyCode::Char('s'))],
        );
        list.insert(
            ListAction::Search,
            vec![KeyCombo::simple(KeyCode::Char('f'))],
        );
        list.insert(
            ListAction::JumpToTop,
            vec![
                KeyCombo::parse("g g").unwrap(),
                KeyCombo::shift(KeyCode::Char('G')),
            ],
        );
        list.insert(ListAction::PageUp, vec![KeyCombo::ctrl(KeyCode::Char('u'))]);
        list.insert(
            ListAction::PageDown,
            vec![KeyCombo::ctrl(KeyCode::Char('d'))],
        );
        list.insert(
            ListAction::OpenTrash,
            vec![KeyCombo::shift(KeyCode::Char('T'))],
        );
        list.insert(
            ListAction::TogglePreview,
            vec![KeyCombo::shift(KeyCode::Char('P'))],
        );
        list.insert(
            ListAction::TogglePreviewFullscreen,
            vec![KeyCombo::ctrl(KeyCode::Char('e'))],
        );
        list.insert(
            ListAction::TogglePreviewWrap,
            vec![KeyCombo::ctrl(KeyCode::Char('w'))],
        );
        list.insert(
            ListAction::OpenGraph,
            vec![KeyCombo::ctrl(KeyCode::Char('g'))],
        );
        list.insert(
            ListAction::ToggleSelectMode,
            vec![KeyCombo::simple(KeyCode::Char('v'))],
        );
        list.insert(
            ListAction::ToggleSelectItem,
            vec![KeyCombo::simple(KeyCode::Char(' '))],
        );
        list.insert(
            ListAction::CollapseAll,
            vec![KeyCombo::parse("Esc Esc").unwrap()],
        );

        let mut edit = HashMap::new();
        edit.insert(EditAction::Back, vec![KeyCombo::simple(KeyCode::Esc)]);
        edit.insert(
            EditAction::CycleFocus,
            vec![
                KeyCombo::simple(KeyCode::Tab),
                KeyCombo::simple(KeyCode::BackTab),
            ],
        );
        edit.insert(
            EditAction::SelectAll,
            vec![KeyCombo::ctrl(KeyCode::Char('a'))],
        );
        edit.insert(
            EditAction::Copy,
            vec![
                KeyCombo::ctrl_shift(KeyCode::Char('c')),
                KeyCombo::ctrl(KeyCode::Insert),
            ],
        );
        edit.insert(
            EditAction::Cut,
            vec![
                KeyCombo::ctrl_shift(KeyCode::Char('x')),
                KeyCombo::shift(KeyCode::Delete),
            ],
        );
        edit.insert(
            EditAction::Paste,
            vec![
                KeyCombo::ctrl_shift(KeyCode::Char('v')),
                KeyCombo::shift(KeyCode::Insert),
            ],
        );
        edit.insert(EditAction::Undo, vec![KeyCombo::ctrl(KeyCode::Char('z'))]);
        edit.insert(
            EditAction::Redo,
            vec![
                KeyCombo::ctrl(KeyCode::Char('y')),
                KeyCombo::ctrl_shift(KeyCode::Char('z')),
            ],
        );
        edit.insert(
            EditAction::DeleteWord,
            vec![KeyCombo::ctrl(KeyCode::Backspace)],
        );
        edit.insert(
            EditAction::DeleteNextWord,
            vec![KeyCombo::ctrl(KeyCode::Delete)],
        );
        edit.insert(EditAction::MoveToTop, vec![KeyCombo::ctrl(KeyCode::Home)]);
        edit.insert(EditAction::MoveToBottom, vec![KeyCombo::ctrl(KeyCode::End)]);
        edit.insert(
            EditAction::ToggleMarkdownPreview,
            vec![KeyCombo::ctrl(KeyCode::Char('p'))],
        );
        edit.insert(
            EditAction::TogglePreviewFullscreen,
            vec![KeyCombo::ctrl(KeyCode::Char('e'))],
        );
        edit.insert(
            EditAction::TogglePreviewWrap,
            vec![KeyCombo::ctrl(KeyCode::Char('w'))],
        );

        let mut help = HashMap::new();
        help.insert(
            HelpAction::Close,
            vec![
                KeyCombo::simple(KeyCode::Esc),
                KeyCombo::simple(KeyCode::Char('q')),
                KeyCombo::simple(KeyCode::Char('?')),
                KeyCombo::simple(KeyCode::F(1)),
            ],
        );
        help.insert(
            HelpAction::NextTab,
            vec![
                KeyCombo::simple(KeyCode::Right),
                KeyCombo::simple(KeyCode::Char('l')),
                KeyCombo::simple(KeyCode::Tab),
            ],
        );
        help.insert(
            HelpAction::PrevTab,
            vec![
                KeyCombo::simple(KeyCode::Left),
                KeyCombo::simple(KeyCode::Char('h')),
                KeyCombo::simple(KeyCode::BackTab),
            ],
        );
        help.insert(
            HelpAction::ScrollUp,
            vec![
                KeyCombo::simple(KeyCode::Up),
                KeyCombo::simple(KeyCode::Char('k')),
            ],
        );
        help.insert(
            HelpAction::ScrollDown,
            vec![
                KeyCombo::simple(KeyCode::Down),
                KeyCombo::simple(KeyCode::Char('j')),
            ],
        );

        let mut graph = HashMap::new();
        graph.insert(
            GraphAction::Quit,
            vec![
                KeyCombo::simple(KeyCode::Esc),
                KeyCombo::simple(KeyCode::Char('q')),
            ],
        );
        graph.insert(
            GraphAction::PanUp,
            vec![
                KeyCombo::simple(KeyCode::Up),
                KeyCombo::simple(KeyCode::Char('k')),
            ],
        );
        graph.insert(
            GraphAction::PanDown,
            vec![
                KeyCombo::simple(KeyCode::Down),
                KeyCombo::simple(KeyCode::Char('j')),
            ],
        );
        graph.insert(
            GraphAction::PanLeft,
            vec![
                KeyCombo::simple(KeyCode::Left),
                KeyCombo::simple(KeyCode::Char('h')),
            ],
        );
        graph.insert(
            GraphAction::PanRight,
            vec![
                KeyCombo::simple(KeyCode::Right),
                KeyCombo::simple(KeyCode::Char('l')),
            ],
        );
        graph.insert(
            GraphAction::ZoomIn,
            vec![
                KeyCombo::simple(KeyCode::Char('+')),
                KeyCombo::ctrl(KeyCode::Char('j')),
            ],
        );
        graph.insert(
            GraphAction::ZoomOut,
            vec![
                KeyCombo::simple(KeyCode::Char('-')),
                KeyCombo::ctrl(KeyCode::Char('k')),
            ],
        );
        graph.insert(
            GraphAction::OpenNote,
            vec![KeyCombo::simple(KeyCode::Enter)],
        );
        graph.insert(
            GraphAction::AutoFit,
            vec![KeyCombo::simple(KeyCode::Char('a'))],
        );
        graph.insert(
            GraphAction::Help,
            vec![
                KeyCombo::simple(KeyCode::Char('?')),
                KeyCombo::simple(KeyCode::F(1)),
            ],
        );
        graph.insert(
            GraphAction::ToggleSearch,
            vec![KeyCombo::simple(KeyCode::Char('f'))],
        );
        graph.insert(
            GraphAction::ToggleMinimap,
            vec![KeyCombo::shift(KeyCode::Char('M'))],
        );
        graph.insert(
            GraphAction::ToggleLegend,
            vec![KeyCombo::shift(KeyCode::Char('L'))],
        );
        graph.insert(
            GraphAction::ToggleGrid,
            vec![KeyCombo::shift(KeyCode::Char('G'))],
        );
        graph.insert(
            GraphAction::ToggleStatus,
            vec![KeyCombo::shift(KeyCode::Char('S'))],
        );
        graph.insert(
            GraphAction::Refresh,
            vec![KeyCombo::simple(KeyCode::Char('r'))],
        );
        graph.insert(
            GraphAction::ReloadConfig,
            vec![KeyCombo::ctrl(KeyCode::Char('r'))],
        );
        graph.insert(
            GraphAction::TogglePreview,
            vec![KeyCombo::shift(KeyCode::Char('P'))],
        );

        let mut draw = HashMap::new();
        draw.insert(
            DrawAction::Quit,
            vec![
                KeyCombo::simple(KeyCode::Esc),
                KeyCombo::simple(KeyCode::Char('q')),
            ],
        );
        draw.insert(
            DrawAction::SelectDrawTool,
            vec![KeyCombo::simple(KeyCode::Char('d'))],
        );
        draw.insert(
            DrawAction::ToggleShapeSelector,
            vec![KeyCombo::simple(KeyCode::Char('s'))],
        );
        draw.insert(
            DrawAction::SelectTextTool,
            vec![KeyCombo::simple(KeyCode::Char('t'))],
        );
        draw.insert(
            DrawAction::SelectEraseTool,
            vec![KeyCombo::simple(KeyCode::Char('e'))],
        );
        draw.insert(
            DrawAction::ShapeSelectorUp,
            vec![KeyCombo::simple(KeyCode::Up)],
        );
        draw.insert(
            DrawAction::ShapeSelectorDown,
            vec![KeyCombo::simple(KeyCode::Down)],
        );
        draw.insert(
            DrawAction::ShapeSelectorConfirm,
            vec![KeyCombo::simple(KeyCode::Enter)],
        );
        draw.insert(
            DrawAction::ShapeSelectorCancel,
            vec![
                KeyCombo::simple(KeyCode::Esc),
                KeyCombo::simple(KeyCode::Char('q')),
            ],
        );
        draw.insert(
            DrawAction::TextEditorConfirm,
            vec![KeyCombo::simple(KeyCode::Enter)],
        );
        draw.insert(
            DrawAction::TextEditorCancel,
            vec![KeyCombo::simple(KeyCode::Esc)],
        );

        let mut canvas = HashMap::new();
        canvas.insert(
            CanvasAction::Quit,
            vec![
                KeyCombo::simple(KeyCode::Esc),
                KeyCombo::simple(KeyCode::Char('q')),
            ],
        );
        canvas.insert(CanvasAction::Save, vec![KeyCombo::ctrl(KeyCode::Char('s'))]);
        canvas.insert(
            CanvasAction::ZoomFineIn,
            vec![KeyCombo::ctrl(KeyCode::Char('j'))],
        );
        canvas.insert(
            CanvasAction::ZoomFineOut,
            vec![KeyCombo::ctrl(KeyCode::Char('k'))],
        );
        canvas.insert(
            CanvasAction::ZoomIn,
            vec![
                KeyCombo::simple(KeyCode::Char('+')),
                KeyCombo::simple(KeyCode::Char('=')),
            ],
        );
        canvas.insert(
            CanvasAction::ZoomOut,
            vec![
                KeyCombo::simple(KeyCode::Char('-')),
                KeyCombo::simple(KeyCode::Char('_')),
            ],
        );
        canvas.insert(
            CanvasAction::MoveLeft,
            vec![
                KeyCombo::simple(KeyCode::Left),
                KeyCombo::simple(KeyCode::Char('h')),
            ],
        );
        canvas.insert(
            CanvasAction::MoveRight,
            vec![
                KeyCombo::simple(KeyCode::Right),
                KeyCombo::simple(KeyCode::Char('l')),
            ],
        );
        canvas.insert(
            CanvasAction::MoveUp,
            vec![
                KeyCombo::simple(KeyCode::Up),
                KeyCombo::simple(KeyCode::Char('k')),
            ],
        );
        canvas.insert(
            CanvasAction::MoveDown,
            vec![
                KeyCombo::simple(KeyCode::Down),
                KeyCombo::simple(KeyCode::Char('j')),
            ],
        );
        canvas.insert(
            CanvasAction::EditOrConnect,
            vec![
                KeyCombo::simple(KeyCode::Char('i')),
                KeyCombo::simple(KeyCode::Enter),
            ],
        );
        canvas.insert(
            CanvasAction::OpenContextMenu,
            vec![KeyCombo::simple(KeyCode::Char('a'))],
        );
        canvas.insert(
            CanvasAction::ToggleGrid,
            vec![KeyCombo::ctrl(KeyCode::Char('g'))],
        );
        canvas.insert(
            CanvasAction::ToggleEditorPane,
            vec![KeyCombo::ctrl(KeyCode::Char('e'))],
        );
        canvas.insert(
            CanvasAction::CycleFocus,
            vec![
                KeyCombo::simple(KeyCode::Tab),
                KeyCombo::simple(KeyCode::BackTab),
            ],
        );
        canvas.insert(
            CanvasAction::Help,
            vec![KeyCombo::simple(KeyCode::Char('?'))],
        );
        canvas.insert(
            CanvasAction::RenameConfirm,
            vec![KeyCombo::simple(KeyCode::Enter)],
        );
        canvas.insert(
            CanvasAction::RenameCancel,
            vec![KeyCombo::simple(KeyCode::Esc)],
        );
        canvas.insert(
            CanvasAction::MenuClose,
            vec![
                KeyCombo::simple(KeyCode::Esc),
                KeyCombo::simple(KeyCode::Char('q')),
            ],
        );
        canvas.insert(
            CanvasAction::MenuUp,
            vec![
                KeyCombo::simple(KeyCode::Up),
                KeyCombo::simple(KeyCode::Char('k')),
            ],
        );
        canvas.insert(
            CanvasAction::MenuDown,
            vec![
                KeyCombo::simple(KeyCode::Down),
                KeyCombo::simple(KeyCode::Char('j')),
            ],
        );
        canvas.insert(
            CanvasAction::MenuSelect,
            vec![KeyCombo::simple(KeyCode::Enter)],
        );
        canvas.insert(
            CanvasAction::CloseEditor,
            vec![KeyCombo::simple(KeyCode::Esc)],
        );
        canvas.insert(
            CanvasAction::CloseEditorAlt,
            vec![KeyCombo::ctrl(KeyCode::Enter)],
        );
        canvas.insert(
            CanvasAction::ConfirmResize,
            vec![KeyCombo::simple(KeyCode::Enter)],
        );
        canvas.insert(
            CanvasAction::CancelResize,
            vec![
                KeyCombo::simple(KeyCode::Esc),
                KeyCombo::simple(KeyCode::Char('q')),
            ],
        );
        canvas.insert(
            CanvasAction::EditorUnfocus,
            vec![KeyCombo::simple(KeyCode::Esc)],
        );
        canvas.insert(
            CanvasAction::EditorSyncRaw,
            vec![KeyCombo::ctrl(KeyCode::Char('s'))],
        );

        let mut backup = HashMap::new();
        backup.insert(
            BackupAction::Back,
            vec![
                KeyCombo::simple(KeyCode::Esc),
                KeyCombo::simple(KeyCode::Char('q')),
            ],
        );
        backup.insert(
            BackupAction::MoveDown,
            vec![
                KeyCombo::simple(KeyCode::Char('j')),
                KeyCombo::simple(KeyCode::Down),
            ],
        );
        backup.insert(
            BackupAction::MoveUp,
            vec![
                KeyCombo::simple(KeyCode::Char('k')),
                KeyCombo::simple(KeyCode::Up),
            ],
        );
        backup.insert(
            BackupAction::ScrollDiffDown,
            vec![KeyCombo::simple(KeyCode::PageDown)],
        );
        backup.insert(
            BackupAction::ScrollDiffUp,
            vec![KeyCombo::simple(KeyCode::PageUp)],
        );
        backup.insert(
            BackupAction::Refresh,
            vec![KeyCombo::simple(KeyCode::Char('r'))],
        );
        backup.insert(
            BackupAction::EnterCommit,
            vec![KeyCombo::simple(KeyCode::Char('s'))],
        );
        backup.insert(
            BackupAction::Push,
            vec![KeyCombo::simple(KeyCode::Char('p'))],
        );
        backup.insert(
            BackupAction::OpenSettings,
            vec![KeyCombo::ctrl(KeyCode::Char('p'))],
        );
        backup.insert(
            BackupAction::ToggleFileSelect,
            vec![KeyCombo::simple(KeyCode::Char(' '))],
        );
        backup.insert(
            BackupAction::CycleSection,
            vec![
                KeyCombo::simple(KeyCode::Tab),
                KeyCombo::simple(KeyCode::BackTab),
            ],
        );
        backup.insert(
            BackupAction::CancelCommit,
            vec![KeyCombo::simple(KeyCode::Esc)],
        );
        backup.insert(
            BackupAction::ConfirmCommit,
            vec![KeyCombo::simple(KeyCode::Enter)],
        );
        backup.insert(
            BackupAction::CloseSettings,
            vec![
                KeyCombo::simple(KeyCode::Esc),
                KeyCombo::simple(KeyCode::Char('q')),
            ],
        );
        backup.insert(
            BackupAction::NextField,
            vec![
                KeyCombo::simple(KeyCode::Char('j')),
                KeyCombo::simple(KeyCode::Down),
            ],
        );
        backup.insert(
            BackupAction::PrevField,
            vec![
                KeyCombo::simple(KeyCode::Char('k')),
                KeyCombo::simple(KeyCode::Up),
            ],
        );
        backup.insert(
            BackupAction::ActivateField,
            vec![KeyCombo::simple(KeyCode::Enter)],
        );
        backup.insert(
            BackupAction::CancelEditField,
            vec![KeyCombo::simple(KeyCode::Esc)],
        );
        backup.insert(
            BackupAction::ConfirmEditField,
            vec![KeyCombo::simple(KeyCode::Enter)],
        );

        let mut content_tree = HashMap::new();
        content_tree.insert(
            ContentTreeAction::MoveUp,
            vec![
                KeyCombo::simple(KeyCode::Char('k')),
                KeyCombo::simple(KeyCode::Up),
            ],
        );
        content_tree.insert(
            ContentTreeAction::MoveDown,
            vec![
                KeyCombo::simple(KeyCode::Char('j')),
                KeyCombo::simple(KeyCode::Down),
            ],
        );
        content_tree.insert(
            ContentTreeAction::ToggleCollapse,
            vec![
                KeyCombo::simple(KeyCode::Tab),
                KeyCombo::simple(KeyCode::Left),
                KeyCombo::simple(KeyCode::Right),
            ],
        );
        content_tree.insert(
            ContentTreeAction::ExpandAll,
            vec![KeyCombo::simple(KeyCode::Char('e'))],
        );
        content_tree.insert(
            ContentTreeAction::CollapseAll,
            vec![KeyCombo::simple(KeyCode::Char('c'))],
        );
        content_tree.insert(
            ContentTreeAction::Open,
            vec![KeyCombo::simple(KeyCode::Enter)],
        );
        content_tree.insert(
            ContentTreeAction::Back,
            vec![
                KeyCombo::simple(KeyCode::Esc),
                KeyCombo::simple(KeyCode::Char('q')),
            ],
        );
        content_tree.insert(
            ContentTreeAction::Help,
            vec![
                KeyCombo::simple(KeyCode::Char('?')),
                KeyCombo::simple(KeyCode::F(1)),
            ],
        );

        Self {
            list,
            edit,
            help,
            graph,
            draw,
            canvas,
            backup,
            content_tree,
        }
    }
}

/// The result of trying to match a key event against a set of bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchOutcome<A> {
    /// The event (possibly combined with previous buffered events) matched an action.
    Matched(A),
    /// The event started a multi-key sequence but hasn't completed one yet; the event was consumed.
    Pending,
    /// No binding matched the event; fall through to hardcoded handling.
    NoMatch,
}

/// Per-view key-sequence matcher with timeout.
/// Buffers recent events and checks them against multi-key combos.
#[derive(Debug, Clone)]
pub struct KeyMatcher {
    pending: Vec<KeyEvent>,
    last_event_at: Option<std::time::Instant>,
    timeout: std::time::Duration,
}

impl KeyMatcher {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            last_event_at: None,
            timeout: std::time::Duration::from_millis(500),
        }
    }

    pub fn clear(&mut self) {
        self.pending.clear();
        self.last_event_at = None;
    }

    /// Resolve a key event against a binding map.
    ///
    /// When `sequences_enabled` is false, this is a simple length-1 match against all bindings.
    /// When true, it buffers events and tries to match multi-key sequences with a 500 ms timeout.
    pub fn resolve<A: Copy + Eq + std::hash::Hash>(
        &mut self,
        event: KeyEvent,
        bindings: &HashMap<A, Vec<KeyCombo>>,
        sequences_enabled: bool,
    ) -> MatchOutcome<A> {
        if !sequences_enabled {
            for (action, combos) in bindings {
                for combo in combos {
                    if combo.matches(&event) {
                        return MatchOutcome::Matched(*action);
                    }
                }
            }
            return MatchOutcome::NoMatch;
        }

        // Check timeout: if too long since last event, clear pending
        if let Some(last) = self.last_event_at
            && last.elapsed() > self.timeout
        {
            self.pending.clear();
        }

        // Push current event
        self.pending.push(event);
        self.last_event_at = Some(std::time::Instant::now());

        let mut pending_prefix = false;
        let mut full_match: Option<A> = None;

        'outer: for (action, combos) in bindings {
            for combo in combos {
                let keys = &combo.keys;
                let pending_len = self.pending.len();
                if keys.len() < pending_len {
                    continue;
                }
                let pending_slice = &self.pending[..pending_len.min(keys.len())];

                // Check if pending[..] matches keys[..pending_len]
                let all_match = keys
                    .iter()
                    .zip(pending_slice.iter())
                    .all(|(k, ev)| k.matches_event(ev));

                if all_match {
                    if keys.len() == self.pending.len() {
                        // Exact match
                        full_match = Some(*action);
                        break 'outer;
                    }
                    // Strict prefix match
                    pending_prefix = true;
                }
            }
        }

        if let Some(action) = full_match {
            self.pending.clear();
            self.last_event_at = None;
            return MatchOutcome::Matched(action);
        }

        if pending_prefix {
            return MatchOutcome::Pending;
        }

        // No prefix match — pop the last event and re-check as a single key
        self.pending.pop(); // remove last pushed event
        let last_event = self.pending.pop().unwrap_or(event); // use last buffered or the original
        self.pending.clear();
        self.last_event_at = None;

        // Re-check as length-1
        for (action, combos) in bindings {
            for combo in combos {
                if combo.matches(&last_event) {
                    return MatchOutcome::Matched(*action);
                }
            }
        }

        MatchOutcome::NoMatch
    }
}

impl KeybindPreset {
    /// Return the base bindings for this preset.
    /// The `edit` map is always `Keybinds::default().edit` (presets never affect text editing).
    pub fn base_keybinds(&self) -> Keybinds {
        let default_kb = Keybinds::default();
        match self {
            KeybindPreset::Default => default_kb,
            KeybindPreset::Helix => {
                let mut kb = default_kb;
                // List view
                kb.list.insert(ListAction::MoveUp, vec![
                    KeyCombo::simple(KeyCode::Char('k')),
                    KeyCombo::simple(KeyCode::Up),
                ]);
                kb.list.insert(ListAction::MoveDown, vec![
                    KeyCombo::simple(KeyCode::Char('j')),
                    KeyCombo::simple(KeyCode::Down),
                ]);
                kb.list.insert(ListAction::MoveLeft, vec![
                    KeyCombo::simple(KeyCode::Char('h')),
                    KeyCombo::simple(KeyCode::Left),
                ]);
                kb.list.insert(ListAction::MoveRight, vec![
                    KeyCombo::simple(KeyCode::Char('l')),
                    KeyCombo::simple(KeyCode::Right),
                ]);
                kb.list.insert(ListAction::Quit, vec![
                    KeyCombo::ctrl(KeyCode::Char('c')),
                    KeyCombo::simple(KeyCode::Char('q')),
                ]);
                kb.list.insert(ListAction::Search, vec![
                    KeyCombo::simple(KeyCode::Char('/')),
                ]);
                kb.list.insert(ListAction::JumpToTop, vec![
                    KeyCombo::parse("g g").unwrap(),
                    KeyCombo::shift(KeyCode::Char('G')),
                ]);
                kb.list.insert(ListAction::PageUp, vec![
                    KeyCombo::ctrl(KeyCode::Char('b')),
                ]);
                kb.list.insert(ListAction::PageDown, vec![
                    KeyCombo::ctrl(KeyCode::Char('f')),
                ]);
                kb.list.insert(ListAction::OpenCommandPalette, vec![
                    KeyCombo::parse("Space Space").unwrap(),
                ]);
                kb.list.insert(ListAction::NewFromTemplate, vec![
                    KeyCombo::parse("Space t").unwrap(),
                ]);
                kb.list.insert(ListAction::CreateNote, vec![
                    KeyCombo::parse("Space n").unwrap(),
                ]);
                kb.list.insert(ListAction::CreateFolder, vec![
                    KeyCombo::parse("Space N").unwrap(),
                ]);
                kb.list.insert(ListAction::TogglePin, vec![
                    KeyCombo::parse("Space p").unwrap(),
                ]);
                kb.list.insert(ListAction::OpenGraph, vec![
                    KeyCombo::parse("Space g").unwrap(),
                ]);
                kb.list.insert(ListAction::TogglePreview, vec![
                    KeyCombo::parse("Space P").unwrap(),
                ]);
                kb.list.insert(ListAction::OpenTrash, vec![
                    KeyCombo::parse("Space T").unwrap(),
                ]);
                kb.list.insert(ListAction::CycleSort, vec![
                    KeyCombo::parse("Space s").unwrap(),
                ]);
                kb.list.insert(ListAction::ManageTags, vec![
                    KeyCombo::parse("Space .").unwrap(),
                ]);
                kb.list.insert(ListAction::CollapseAll, vec![
                    KeyCombo::parse("Esc Esc").unwrap(),
                ]);
                // Graph view
                kb.graph.insert(GraphAction::PanUp, vec![
                    KeyCombo::simple(KeyCode::Char('k')),
                    KeyCombo::simple(KeyCode::Up),
                ]);
                kb.graph.insert(GraphAction::PanDown, vec![
                    KeyCombo::simple(KeyCode::Char('j')),
                    KeyCombo::simple(KeyCode::Down),
                ]);
                kb.graph.insert(GraphAction::PanLeft, vec![
                    KeyCombo::simple(KeyCode::Char('h')),
                    KeyCombo::simple(KeyCode::Left),
                ]);
                kb.graph.insert(GraphAction::PanRight, vec![
                    KeyCombo::simple(KeyCode::Char('l')),
                    KeyCombo::simple(KeyCode::Right),
                ]);
                kb.graph.insert(GraphAction::Quit, vec![KeyCombo::simple(KeyCode::Char('q'))]);
                kb.graph.insert(GraphAction::ToggleSearch, vec![KeyCombo::simple(KeyCode::Char('/'))]);
                kb.graph.insert(GraphAction::ZoomIn, vec![KeyCombo::simple(KeyCode::Char('='))]);
                kb.graph.insert(GraphAction::ZoomOut, vec![KeyCombo::simple(KeyCode::Char('-'))]);
                kb.graph.insert(GraphAction::AutoFit, vec![
                    KeyCombo::parse("Space a").unwrap(),
                ]);
                kb.graph.insert(GraphAction::Refresh, vec![
                    KeyCombo::parse("Space r").unwrap(),
                ]);
                kb.graph.insert(GraphAction::ToggleMinimap, vec![
                    KeyCombo::parse("Space m").unwrap(),
                ]);
                kb.edit = Keybinds::default().edit;
                kb
            }
            KeybindPreset::Vim => {
                let mut kb = default_kb;
                // List view
                kb.list.insert(ListAction::MoveUp, vec![
                    KeyCombo::simple(KeyCode::Char('k')),
                    KeyCombo::simple(KeyCode::Up),
                ]);
                kb.list.insert(ListAction::MoveDown, vec![
                    KeyCombo::simple(KeyCode::Char('j')),
                    KeyCombo::simple(KeyCode::Down),
                ]);
                kb.list.insert(ListAction::MoveLeft, vec![
                    KeyCombo::simple(KeyCode::Char('h')),
                    KeyCombo::simple(KeyCode::Left),
                ]);
                kb.list.insert(ListAction::MoveRight, vec![
                    KeyCombo::simple(KeyCode::Char('l')),
                    KeyCombo::simple(KeyCode::Right),
                ]);
                kb.list.insert(ListAction::Delete, vec![
                    KeyCombo::parse("d d").unwrap(),
                    KeyCombo::simple(KeyCode::Char('d')),
                    KeyCombo::simple(KeyCode::Delete),
                ]);
                kb.list.insert(ListAction::Quit, vec![
                    KeyCombo::parse(": q").unwrap(),
                    KeyCombo::simple(KeyCode::Char('q')),
                ]);
                kb.list.insert(ListAction::Search, vec![
                    KeyCombo::simple(KeyCode::Char('/')),
                ]);
                kb.list.insert(ListAction::JumpToTop, vec![
                    KeyCombo::parse("g g").unwrap(),
                    KeyCombo::shift(KeyCode::Char('G')),
                ]);
                kb.list.insert(ListAction::JumpToBottom, vec![
                    KeyCombo::shift(KeyCode::Char('G')),
                ]);
                kb.list.insert(ListAction::PageUp, vec![
                    KeyCombo::ctrl(KeyCode::Char('b')),
                ]);
                kb.list.insert(ListAction::PageDown, vec![
                    KeyCombo::ctrl(KeyCode::Char('f')),
                ]);
                kb.list.insert(ListAction::OpenCommandPalette, vec![
                    KeyCombo::parse(": ").unwrap(),
                ]);
                kb.list.insert(ListAction::CollapseAll, vec![
                    KeyCombo::parse("Esc Esc").unwrap(),
                ]);
                // Graph view — vim-style nav
                kb.graph.insert(GraphAction::PanUp, vec![
                    KeyCombo::simple(KeyCode::Char('k')),
                    KeyCombo::simple(KeyCode::Up),
                ]);
                kb.graph.insert(GraphAction::PanDown, vec![
                    KeyCombo::simple(KeyCode::Char('j')),
                    KeyCombo::simple(KeyCode::Down),
                ]);
                kb.graph.insert(GraphAction::PanLeft, vec![
                    KeyCombo::simple(KeyCode::Char('h')),
                    KeyCombo::simple(KeyCode::Left),
                ]);
                kb.graph.insert(GraphAction::PanRight, vec![
                    KeyCombo::simple(KeyCode::Char('l')),
                    KeyCombo::simple(KeyCode::Right),
                ]);
                kb.graph.insert(GraphAction::Quit, vec![
                    KeyCombo::parse(": q").unwrap(),
                    KeyCombo::simple(KeyCode::Char('q')),
                ]);
                kb.edit = Keybinds::default().edit;
                kb
            }
            KeybindPreset::Emacs => {
                let mut kb = default_kb;
                // List view
                kb.list.insert(ListAction::MoveUp, vec![
                    KeyCombo::ctrl(KeyCode::Char('p')),
                    KeyCombo::simple(KeyCode::Up),
                ]);
                kb.list.insert(ListAction::MoveDown, vec![
                    KeyCombo::ctrl(KeyCode::Char('n')),
                    KeyCombo::simple(KeyCode::Down),
                ]);
                kb.list.insert(ListAction::MoveLeft, vec![
                    KeyCombo::ctrl(KeyCode::Char('b')),
                    KeyCombo::simple(KeyCode::Left),
                ]);
                kb.list.insert(ListAction::MoveRight, vec![
                    KeyCombo::ctrl(KeyCode::Char('f')),
                    KeyCombo::simple(KeyCode::Right),
                ]);
                kb.list.insert(ListAction::Quit, vec![
                    KeyCombo::parse("Ctrl+x Ctrl+c").unwrap(),
                    KeyCombo::simple(KeyCode::Char('q')),
                ]);
                kb.list.insert(ListAction::Help, vec![
                    KeyCombo::ctrl(KeyCode::Char('h')),
                    KeyCombo::simple(KeyCode::F(1)),
                ]);
                kb.list.insert(ListAction::Search, vec![
                    KeyCombo::ctrl(KeyCode::Char('s')),
                ]);
                kb.list.insert(ListAction::PageUp, vec![
                    KeyCombo::ctrl(KeyCode::Char('v')),
                    KeyCombo::simple(KeyCode::PageUp),
                ]);
                kb.list.insert(ListAction::OpenCommandPalette, vec![
                    KeyCombo::parse("Ctrl+x Ctrl+p").unwrap(),
                ]);
                kb.list.insert(ListAction::Delete, vec![
                    KeyCombo::ctrl(KeyCode::Char('d')),
                    KeyCombo::simple(KeyCode::Delete),
                ]);
                kb.list.insert(ListAction::CollapseAll, vec![
                    KeyCombo::parse("Esc Esc").unwrap(),
                ]);
                // Graph view — Emacs nav
                kb.graph.insert(GraphAction::PanUp, vec![
                    KeyCombo::ctrl(KeyCode::Char('p')),
                    KeyCombo::simple(KeyCode::Up),
                ]);
                kb.graph.insert(GraphAction::PanDown, vec![
                    KeyCombo::ctrl(KeyCode::Char('n')),
                    KeyCombo::simple(KeyCode::Down),
                ]);
                kb.graph.insert(GraphAction::PanLeft, vec![
                    KeyCombo::ctrl(KeyCode::Char('b')),
                    KeyCombo::simple(KeyCode::Left),
                ]);
                kb.graph.insert(GraphAction::PanRight, vec![
                    KeyCombo::ctrl(KeyCode::Char('f')),
                    KeyCombo::simple(KeyCode::Right),
                ]);
                kb.graph.insert(GraphAction::Quit, vec![
                    KeyCombo::parse("Ctrl+x Ctrl+c").unwrap(),
                    KeyCombo::simple(KeyCode::Char('q')),
                ]);
                kb.edit = Keybinds::default().edit;
                kb
            }
        }
    }
}


impl Keybinds {
    pub fn load(path: &Path) -> Result<Self> {
        Self::load_layered(path, Self::default())
    }

    pub fn load_layered(path: &Path, base: Keybinds) -> Result<Self> {
        let mut keybinds = base;

        if !path.exists() {
            return Ok(keybinds);
        }

        let content = fs::read_to_string(path).context("failed to read keybinds file")?;

        let toml: KeybindsToml =
            toml::from_str(&content).context("failed to parse keybinds file")?;

        for (action, combos_str) in &toml.list {
            let combos: Vec<KeyCombo> = combos_str
                .iter()
                .filter_map(|s| KeyCombo::parse(s))
                .collect();
            if !combos.is_empty() {
                keybinds.list.insert(*action, combos);
            }
        }

        for (action, combos_str) in &toml.edit {
            let combos: Vec<KeyCombo> = combos_str
                .iter()
                .filter_map(|s| KeyCombo::parse(s))
                .collect();
            if !combos.is_empty() {
                keybinds.edit.insert(*action, combos);
            }
        }

        for (action, combos_str) in &toml.help {
            let combos: Vec<KeyCombo> = combos_str
                .iter()
                .filter_map(|s| KeyCombo::parse(s))
                .collect();
            if !combos.is_empty() {
                keybinds.help.insert(*action, combos);
            }
        }

        for (action, combos_str) in &toml.graph {
            let combos: Vec<KeyCombo> = combos_str
                .iter()
                .filter_map(|s| KeyCombo::parse(s))
                .collect();
            if !combos.is_empty() {
                keybinds.graph.insert(*action, combos);
            }
        }

        for (action, combos_str) in &toml.draw {
            let combos: Vec<KeyCombo> = combos_str
                .iter()
                .filter_map(|s| KeyCombo::parse(s))
                .collect();
            if !combos.is_empty() {
                keybinds.draw.insert(*action, combos);
            }
        }

        for (action, combos_str) in &toml.canvas {
            let combos: Vec<KeyCombo> = combos_str
                .iter()
                .filter_map(|s| KeyCombo::parse(s))
                .collect();
            if !combos.is_empty() {
                keybinds.canvas.insert(*action, combos);
            }
        }

        for (action, combos_str) in &toml.backup {
            let combos: Vec<KeyCombo> = combos_str
                .iter()
                .filter_map(|s| KeyCombo::parse(s))
                .collect();
            if !combos.is_empty() {
                keybinds.backup.insert(*action, combos);
            }
        }

        for (action, combos_str) in &toml.content_tree {
            let combos: Vec<KeyCombo> = combos_str
                .iter()
                .filter_map(|s| KeyCombo::parse(s))
                .collect();
            if !combos.is_empty() {
                keybinds.content_tree.insert(*action, combos);
            }
        }
        Ok(keybinds)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let toml = self.to_toml();
        let content = toml::to_string_pretty(&toml).context("failed to serialize keybinds")?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("failed to create keybinds directory")?;
        }

        crate::fsutil::atomic_write(path, content.as_bytes())?;

        Ok(())
    }

    pub fn to_toml(&self) -> KeybindsToml {
        let mut toml = KeybindsToml::default();

        for (action, combos) in &self.list {
            let values: Vec<String> = combos.iter().map(KeyCombo::to_display_string).collect();
            toml.list.insert(*action, values);
        }

        for (action, combos) in &self.edit {
            let values: Vec<String> = combos.iter().map(KeyCombo::to_display_string).collect();
            toml.edit.insert(*action, values);
        }

        for (action, combos) in &self.help {
            let values: Vec<String> = combos.iter().map(KeyCombo::to_display_string).collect();
            toml.help.insert(*action, values);
        }

        for (action, combos) in &self.graph {
            let values: Vec<String> = combos.iter().map(KeyCombo::to_display_string).collect();
            toml.graph.insert(*action, values);
        }

        for (action, combos) in &self.draw {
            let values: Vec<String> = combos.iter().map(KeyCombo::to_display_string).collect();
            toml.draw.insert(*action, values);
        }

        for (action, combos) in &self.canvas {
            let values: Vec<String> = combos.iter().map(KeyCombo::to_display_string).collect();
            toml.canvas.insert(*action, values);
        }

        for (action, combos) in &self.backup {
            let values: Vec<String> = combos.iter().map(KeyCombo::to_display_string).collect();
            toml.backup.insert(*action, values);
        }

        for (action, combos) in &self.content_tree {
            let values: Vec<String> = combos.iter().map(KeyCombo::to_display_string).collect();
            toml.content_tree.insert(*action, values);
        }
        toml
    }
    pub fn matches_list(&self, action: ListAction, event: &KeyEvent) -> bool {
        self.list
            .get(&action)
            .is_some_and(|combos| combos.iter().any(|c| c.matches(event)))
    }

    pub fn matches_edit(&self, action: EditAction, event: &KeyEvent) -> bool {
        self.edit
            .get(&action)
            .is_some_and(|combos| combos.iter().any(|c| c.matches(event)))
    }

    pub fn matches_draw(&self, action: DrawAction, event: &KeyEvent) -> bool {
        self.draw
            .get(&action)
            .is_some_and(|combos| combos.iter().any(|c| c.matches(event)))
    }

    pub fn matches_canvas(&self, action: CanvasAction, event: &KeyEvent) -> bool {
        self.canvas
            .get(&action)
            .is_some_and(|combos| combos.iter().any(|c| c.matches(event)))
    }

    pub fn matches_backup(&self, action: BackupAction, event: &KeyEvent) -> bool {
        self.backup
            .get(&action)
            .is_some_and(|combos| combos.iter().any(|c| c.matches(event)))
    }

    pub fn list_keys_display(&self, action: ListAction) -> String {
        self.list
            .get(&action)
            .map(|combos| {
                combos
                    .iter()
                    .map(KeyCombo::to_display_string)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default()
    }

    pub fn edit_keys_display(&self, action: EditAction) -> String {
        self.edit
            .get(&action)
            .map(|combos| {
                combos
                    .iter()
                    .map(KeyCombo::to_display_string)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default()
    }

    pub fn help_keys_display(&self, action: HelpAction) -> String {
        self.help
            .get(&action)
            .map(|combos| {
                combos
                    .iter()
                    .map(KeyCombo::to_display_string)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default()
    }

    pub fn graph_keys_display(&self, action: GraphAction) -> String {
        self.graph
            .get(&action)
            .map(|combos| {
                combos
                    .iter()
                    .map(KeyCombo::to_display_string)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default()
    }

    pub fn draw_keys_display(&self, action: DrawAction) -> String {
        self.draw
            .get(&action)
            .map(|combos| {
                combos
                    .iter()
                    .map(KeyCombo::to_display_string)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default()
    }

    pub fn canvas_keys_display(&self, action: CanvasAction) -> String {
        self.canvas
            .get(&action)
            .map(|combos| {
                combos
                    .iter()
                    .map(KeyCombo::to_display_string)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default()
    }

    pub fn backup_keys_display(&self, action: BackupAction) -> String {
        self.backup
            .get(&action)
            .map(|combos| {
                combos
                    .iter()
                    .map(KeyCombo::to_display_string)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default()
    }

    pub fn content_tree_keys_display(&self, action: ContentTreeAction) -> String {
        self.content_tree
            .get(&action)
            .map(|combos| {
                combos
                    .iter()
                    .map(KeyCombo::to_display_string)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default()
    }

    // -- Binding map accessors (used by KeyMatcher::resolve) --
    pub fn bindings_for_list(&self) -> &HashMap<ListAction, Vec<KeyCombo>> {
        &self.list
    }
    pub fn bindings_for_edit(&self) -> &HashMap<EditAction, Vec<KeyCombo>> {
        &self.edit
    }
    pub fn bindings_for_help(&self) -> &HashMap<HelpAction, Vec<KeyCombo>> {
        &self.help
    }
    pub fn bindings_for_graph(&self) -> &HashMap<GraphAction, Vec<KeyCombo>> {
        &self.graph
    }
    pub fn bindings_for_draw(&self) -> &HashMap<DrawAction, Vec<KeyCombo>> {
        &self.draw
    }
    pub fn bindings_for_canvas(&self) -> &HashMap<CanvasAction, Vec<KeyCombo>> {
        &self.canvas
    }
    pub fn bindings_for_backup(&self) -> &HashMap<BackupAction, Vec<KeyCombo>> {
        &self.backup
    }
    pub fn bindings_for_content_tree(&self) -> &HashMap<ContentTreeAction, Vec<KeyCombo>> {
        &self.content_tree
    }

    // -- Resolve wrappers (delegate to KeyMatcher::resolve) --
    pub fn resolve_list(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
    ) -> MatchOutcome<ListAction> {
        m.resolve(event, self.bindings_for_list(), seq)
    }
    pub fn resolve_edit(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
    ) -> MatchOutcome<EditAction> {
        m.resolve(event, self.bindings_for_edit(), seq)
    }
    pub fn resolve_help(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
    ) -> MatchOutcome<HelpAction> {
        m.resolve(event, self.bindings_for_help(), seq)
    }
    pub fn resolve_graph(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
    ) -> MatchOutcome<GraphAction> {
        m.resolve(event, self.bindings_for_graph(), seq)
    }
    pub fn resolve_draw(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
    ) -> MatchOutcome<DrawAction> {
        m.resolve(event, self.bindings_for_draw(), seq)
    }
    pub fn resolve_canvas(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
    ) -> MatchOutcome<CanvasAction> {
        m.resolve(event, self.bindings_for_canvas(), seq)
    }
    pub fn resolve_backup(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
    ) -> MatchOutcome<BackupAction> {
        m.resolve(event, self.bindings_for_backup(), seq)
    }
    pub fn resolve_content_tree(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
    ) -> MatchOutcome<ContentTreeAction> {
        m.resolve(event, self.bindings_for_content_tree(), seq)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parse_key_combo_simple() {
        let combo = KeyCombo::parse("q").unwrap();
        assert_eq!(combo.keys.len(), 1);
        assert_eq!(combo.keys[0].code, KeyCode::Char('q'));
        assert_eq!(combo.keys[0].modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn test_parse_key_combo_ctrl() {
        let combo = KeyCombo::parse("Ctrl+q").unwrap();
        assert_eq!(combo.keys.len(), 1);
        assert_eq!(combo.keys[0].code, KeyCode::Char('q'));
        assert_eq!(combo.keys[0].modifiers, KeyModifiers::CONTROL);
    }

    #[test]
    fn test_parse_key_combo_ctrl_shift() {
        let combo = KeyCombo::parse("Ctrl+Shift+z").unwrap();
        assert_eq!(combo.keys.len(), 1);
        assert_eq!(combo.keys[0].code, KeyCode::Char('z'));
        assert_eq!(combo.keys[0].modifiers, KeyModifiers::CONTROL | KeyModifiers::SHIFT);
    }

    #[test]
    fn test_parse_special_keys() {
        assert_eq!(KeyCombo::parse("Enter").unwrap().keys[0].code, KeyCode::Enter);
        assert_eq!(KeyCombo::parse("Esc").unwrap().keys[0].code, KeyCode::Esc);
        assert_eq!(KeyCombo::parse("F1").unwrap().keys[0].code, KeyCode::F(1));
        assert_eq!(KeyCombo::parse("Delete").unwrap().keys[0].code, KeyCode::Delete);
    }

    #[test]
    fn test_parse_sequence() {
        let combo = KeyCombo::parse("g g").unwrap();
        assert_eq!(combo.keys.len(), 2);
        assert_eq!(combo.keys[0].code, KeyCode::Char('g'));
        assert_eq!(combo.keys[0].modifiers, KeyModifiers::NONE);
        assert_eq!(combo.keys[1].code, KeyCode::Char('g'));
        assert_eq!(combo.keys[1].modifiers, KeyModifiers::NONE);

        let ctrl_combo = KeyCombo::parse("Ctrl+x Ctrl+s").unwrap();
        assert_eq!(ctrl_combo.keys.len(), 2);
        assert_eq!(ctrl_combo.keys[0].code, KeyCode::Char('x'));
        assert!(ctrl_combo.keys[0].modifiers.contains(KeyModifiers::CONTROL));
        assert_eq!(ctrl_combo.keys[1].code, KeyCode::Char('s'));
        assert!(ctrl_combo.keys[1].modifiers.contains(KeyModifiers::CONTROL));

        // single-key backward compat
        let single = KeyCombo::parse("q").unwrap();
        assert_eq!(single.keys.len(), 1);
        assert_eq!(single.keys[0].code, KeyCode::Char('q'));
    }

    #[test]
    fn test_key_combo_matches() {
        let combo = KeyCombo::ctrl(KeyCode::Char('q'));
        let event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
        assert!(combo.matches(&event));
        let wrong_event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(!combo.matches(&wrong_event));

        // BackTab matching should ignore the SHIFT modifier
        let bt_combo1 = KeyCombo::simple(KeyCode::BackTab);
        let bt_combo2 = KeyCombo::shift(KeyCode::BackTab);
        let bt_event1 = KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE);
        let bt_event2 = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
        assert!(bt_combo1.matches(&bt_event1));
        assert!(bt_combo1.matches(&bt_event2));
        assert!(bt_combo2.matches(&bt_event1));
        assert!(bt_combo2.matches(&bt_event2));

        // Other modifiers like CONTROL should still distinguish them
        let bt_ctrl_event = KeyEvent::new(KeyCode::BackTab, KeyModifiers::CONTROL);
        assert!(!bt_combo1.matches(&bt_ctrl_event));
    }

    #[test]
    fn test_display_single_and_multi() {
        let single = KeyCombo::simple(KeyCode::Char('q'));
        assert_eq!(single.to_display_string(), "q");

        let ctrl_single = KeyCombo::ctrl(KeyCode::Char('s'));
        assert_eq!(ctrl_single.to_display_string(), "Ctrl+s");

        // Multi-key simple (all ascii letters) joins without separator
        let gg = KeyCombo::parse("g g").unwrap();
        assert_eq!(gg.to_display_string(), "gg");

        // Multi-key with modifier uses space
        let ctrl_seq = KeyCombo::parse("Ctrl+x Ctrl+s").unwrap();
        assert_eq!(ctrl_seq.to_display_string(), "Ctrl+x Ctrl+s");

        // Space key in sequence
        let space_seq = KeyCombo::parse("Space f").unwrap();
        assert_eq!(space_seq.to_display_string(), "Space f");
    }

    #[test]
    fn test_default_keybinds() {
        let keybinds = Keybinds::default();
        assert!(!keybinds.list.is_empty());
        assert!(!keybinds.edit.is_empty());
        assert!(!keybinds.help.is_empty());
        assert!(!keybinds.draw.is_empty());
        assert!(!keybinds.canvas.is_empty());
        assert!(!keybinds.backup.is_empty());
        assert!(!keybinds.content_tree.is_empty());

        let toml = keybinds.to_toml();
        assert!(!toml.draw.is_empty());
        assert!(!toml.canvas.is_empty());
        assert!(!toml.content_tree.is_empty());
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("keybinds.toml");
        keybinds.save(&path).unwrap();
        let loaded_keybinds = Keybinds::load(&path).unwrap();
        assert_eq!(loaded_keybinds.draw, keybinds.draw);
        assert_eq!(loaded_keybinds.canvas, keybinds.canvas);
        assert_eq!(loaded_keybinds.backup, keybinds.backup);
        assert_eq!(loaded_keybinds.content_tree, keybinds.content_tree);
    }

    #[test]
    fn test_matches_list_action() {
        let keybinds = Keybinds::default();
        let event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(keybinds.matches_list(ListAction::Quit, &event));
    }

    #[test]
    fn test_new_action_displays() {
        let keybinds = Keybinds::default();
        assert_eq!(keybinds.draw_keys_display(DrawAction::SelectDrawTool), "d");
        assert_eq!(keybinds.canvas_keys_display(CanvasAction::Quit), "Esc/q");
        assert_eq!(keybinds.canvas_keys_display(CanvasAction::Save), "Ctrl+s");
    }

    #[test]
    fn test_matcher_sequences_disabled() {
        let mut matcher = KeyMatcher::new();
        let mut bindings: HashMap<ListAction, Vec<KeyCombo>> = HashMap::new();
        bindings.insert(ListAction::JumpToTop, vec![
            KeyCombo::parse("g g").unwrap(),
        ]);
        // With sequences disabled, "g" alone should not match JumpToTop
        let event = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        let result = matcher.resolve(event, &bindings, false);
        assert_eq!(result, MatchOutcome::NoMatch);
    }

    #[test]
    fn test_matcher_full_match() {
        let mut matcher = KeyMatcher::new();
        let mut bindings: HashMap<ListAction, Vec<KeyCombo>> = HashMap::new();
        bindings.insert(ListAction::JumpToTop, vec![
            KeyCombo::parse("g g").unwrap(),
        ]);

        // First 'g' should be Pending
        let e1 = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        let r1 = matcher.resolve(e1, &bindings, true);
        assert_eq!(r1, MatchOutcome::Pending);

        // Second 'g' within timeout should match
        let e2 = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        let r2 = matcher.resolve(e2, &bindings, true);
        assert_eq!(r2, MatchOutcome::Matched(ListAction::JumpToTop));
    }

    #[test]
    fn test_matcher_timeout() {
        let mut matcher = KeyMatcher {
            pending: Vec::new(),
            last_event_at: None,
            timeout: std::time::Duration::from_millis(1), // very short
        };
        let mut bindings: HashMap<ListAction, Vec<KeyCombo>> = HashMap::new();
        bindings.insert(ListAction::JumpToTop, vec![
            KeyCombo::parse("g g").unwrap(),
        ]);

        // First 'g' -> Pending
        let e1 = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        let r1 = matcher.resolve(e1, &bindings, true);
        assert_eq!(r1, MatchOutcome::Pending);

        // Wait longer than timeout
        std::thread::sleep(std::time::Duration::from_millis(5));

        // Second 'g' after timeout should NOT match (pending cleared)
        let e2 = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        let r2 = matcher.resolve(e2, &bindings, true);
        // Should re-start sequence
        assert_eq!(r2, MatchOutcome::Pending);
    }

    #[test]
    fn test_matcher_single_key_still_works() {
        let mut matcher = KeyMatcher::new();
        let mut bindings: HashMap<ListAction, Vec<KeyCombo>> = HashMap::new();
        bindings.insert(ListAction::Quit, vec![KeyCombo::simple(KeyCode::Char('q'))]);
        bindings.insert(ListAction::JumpToTop, vec![
            KeyCombo::parse("g g").unwrap(),
        ]);

        // 'q' alone should still match Quit (length-1)
        let event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        let result = matcher.resolve(event, &bindings, true);
        assert_eq!(result, MatchOutcome::Matched(ListAction::Quit));
    }

    #[test]
    fn test_preset_edit_unchanged() {
        let default_edit = &Keybinds::default().edit;
        for preset in &[KeybindPreset::Default, KeybindPreset::Helix, KeybindPreset::Vim, KeybindPreset::Emacs] {
            assert_eq!(&preset.base_keybinds().edit, default_edit, "preset {preset} must not change edit bindings");
        }
    }

    #[test]
    fn test_helix_preset_bindings() {
        let kb = KeybindPreset::Helix.base_keybinds();
        // gg → JumpToTop
        assert!(kb.list.get(&ListAction::JumpToTop).unwrap().iter().any(|c| c.to_display_string() == "gg"));
        // Space Space → OpenCommandPalette
        assert!(kb.list.get(&ListAction::OpenCommandPalette).unwrap().iter().any(|c| c.to_display_string() == "Space Space"));
        // Space t → NewFromTemplate
        assert!(kb.list.get(&ListAction::NewFromTemplate).unwrap().iter().any(|c| c.to_display_string() == "Space t"));
        // k → MoveUp (with arrow fallback)
        assert!(kb.list.get(&ListAction::MoveUp).unwrap().iter().any(|c| c.to_display_string() == "k"));
        assert!(kb.list.get(&ListAction::MoveUp).unwrap().iter().any(|c| c.to_display_string() == "Up"));
    }

    #[test]
    fn test_vim_preset_bindings() {
        let kb = KeybindPreset::Vim.base_keybinds();
        // : q → Quit
        assert!(kb.list.get(&ListAction::Quit).unwrap().iter().any(|c| c.to_display_string() == ": q"));
        // d d → Delete
        assert!(kb.list.get(&ListAction::Delete).unwrap().iter().any(|c| c.to_display_string() == "dd"));
        // gg → JumpToTop
        assert!(kb.list.get(&ListAction::JumpToTop).unwrap().iter().any(|c| c.to_display_string() == "gg"));
        // Shift+G → JumpToBottom
        assert!(kb.list.get(&ListAction::JumpToBottom).unwrap().iter().any(|c| c.to_display_string() == "Shift+G"));
    }

    #[test]
    fn test_emacs_preset_bindings() {
        let kb = KeybindPreset::Emacs.base_keybinds();
        // Ctrl+p → MoveUp
        assert!(kb.list.get(&ListAction::MoveUp).unwrap().iter().any(|c| c.to_display_string() == "Ctrl+p"));
        // Ctrl+h → Help
        assert!(kb.list.get(&ListAction::Help).unwrap().iter().any(|c| c.to_display_string() == "Ctrl+h"));
        // Ctrl+x Ctrl+c → Quit
        assert!(kb.list.get(&ListAction::Quit).unwrap().iter().any(|c| c.to_display_string() == "Ctrl+x Ctrl+c"));
        // Ctrl+d → Delete
        assert!(kb.list.get(&ListAction::Delete).unwrap().iter().any(|c| c.to_display_string() == "Ctrl+d"));
    }

}
