//! Keybind configuration module
//!
//! This module handles customizable keyboard shortcuts for the application.
//! Keybinds are stored in <`storage_path>/keybinds.toml` and can be customized
//! by the user. Vim mode keybindings are NOT affected by this system.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};

/// A key combination (e.g., "Ctrl+q", "Enter", "Shift+Delete")
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyCombo {
    pub fn simple(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::NONE,
        }
    }

    pub fn ctrl(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::CONTROL,
        }
    }

    pub fn shift(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::SHIFT,
        }
    }

    pub fn ctrl_shift(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        }
    }

    /// Parse a key combo from a string like "Ctrl+Shift+a", "Enter", "F1"
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('+').collect();
        let mut modifiers = KeyModifiers::NONE;
        let mut key_part = "";

        for (i, part) in parts.iter().enumerate() {
            let part_lower = part.to_lowercase();
            if i == parts.len() - 1 {
                // Last part is the key
                key_part = part;
            } else {
                // Modifier
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
        Some(Self { code, modifiers })
    }

    /// Convert to a display string
    pub fn to_display_string(&self) -> String {
        let mut parts = Vec::new();

        if self.modifiers.contains(KeyModifiers::CONTROL) {
            parts.push("Ctrl");
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            parts.push("Shift");
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            parts.push("Alt");
        }
        if self.modifiers.contains(KeyModifiers::SUPER) {
            parts.push("Super");
        }

        parts.push(key_code_to_string(&self.code));
        parts.join("+")
    }

    /// Check if this combo matches a key event
    pub fn matches(&self, event: &KeyEvent) -> bool {
        self.code == event.code && self.modifiers == event.modifiers
    }
}

fn parse_key_code(s: &str) -> Option<KeyCode> {
    let s_lower = s.to_lowercase();
    match s_lower.as_str() {
        // Special keys
        "enter" | "return" => Some(KeyCode::Enter),
        "esc" | "escape" => Some(KeyCode::Esc),
        "backspace" | "bs" => Some(KeyCode::Backspace),
        "tab" => Some(KeyCode::Tab),
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

        // Function keys
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

        // Single character
        _ if s.len() == 1 => {
            let c = s.chars().next()?;
            Some(KeyCode::Char(c.to_ascii_lowercase()))
        }

        _ => None,
    }
}

fn key_code_to_string(code: &KeyCode) -> &'static str {
    match code {
        KeyCode::Enter => "Enter",
        KeyCode::Esc => "Esc",
        KeyCode::Backspace => "Backspace",
        KeyCode::Tab => "Tab",
        KeyCode::Delete => "Delete",
        KeyCode::Insert => "Insert",
        KeyCode::Home => "Home",
        KeyCode::End => "End",
        KeyCode::PageUp => "PageUp",
        KeyCode::PageDown => "PageDown",
        KeyCode::Up => "Up",
        KeyCode::Down => "Down",
        KeyCode::Left => "Left",
        KeyCode::Right => "Right",
        KeyCode::F(1) => "F1",
        KeyCode::F(2) => "F2",
        KeyCode::F(3) => "F3",
        KeyCode::F(4) => "F4",
        KeyCode::F(5) => "F5",
        KeyCode::F(6) => "F6",
        KeyCode::F(7) => "F7",
        KeyCode::F(8) => "F8",
        KeyCode::F(9) => "F9",
        KeyCode::F(10) => "F10",
        KeyCode::F(11) => "F11",
        KeyCode::F(12) => "F12",
        KeyCode::Char(' ') => "Space",
        KeyCode::Char('?') => "?",
        KeyCode::Char(c) => {
            // This is a bit of a hack for static strings
            match c {
                'a' => "a",
                'b' => "b",
                'c' => "c",
                'd' => "d",
                'e' => "e",
                'f' => "f",
                'g' => "g",
                'h' => "h",
                'i' => "i",
                'j' => "j",
                'k' => "k",
                'l' => "l",
                'm' => "m",
                'n' => "n",
                'o' => "o",
                'p' => "p",
                'q' => "q",
                'r' => "r",
                's' => "s",
                't' => "t",
                'u' => "u",
                'v' => "v",
                'w' => "w",
                'x' => "x",
                'y' => "y",
                'z' => "z",
                '0' => "0",
                '1' => "1",
                '2' => "2",
                '3' => "3",
                '4' => "4",
                '5' => "5",
                '6' => "6",
                '7' => "7",
                '8' => "8",
                '9' => "9",
                _ => "?",
            }
        }
        _ => "?",
    }
}

/// Actions that can be bound to keys in list view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListAction {
    MoveUp,
    MoveDown,
    Open,
    Delete,
    Quit,
    Help,
    OpenLocation,
    CycleFocus,
    ConfirmDelete,
    CancelDelete,
    ToggleButton,
    NewFromTemplate,
}

/// Actions that can be bound to keys in edit view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditAction {
    Quit,
    Back,
    CycleFocus,
    ToggleButton,
    // Text editing shortcuts
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
}

/// Actions that can be bound to keys in help view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HelpAction {
    Close,
    ScrollUp,
    ScrollDown,
}

/// TOML representation of keybinds for serialization
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeybindsToml {
    #[serde(default)]
    pub list: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub edit: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub help: HashMap<String, Vec<String>>,
}

/// Main keybinds configuration
#[derive(Debug, Clone)]
pub struct Keybinds {
    pub list: HashMap<ListAction, Vec<KeyCombo>>,
    pub edit: HashMap<EditAction, Vec<KeyCombo>>,
    pub help: HashMap<HelpAction, Vec<KeyCombo>>,
}

impl Default for Keybinds {
    fn default() -> Self {
        let mut list = HashMap::new();
        list.insert(ListAction::MoveUp, vec![KeyCombo::simple(KeyCode::Up)]);
        list.insert(ListAction::MoveDown, vec![KeyCombo::simple(KeyCode::Down)]);
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
            vec![KeyCombo::simple(KeyCode::Char('f'))],
        );
        list.insert(ListAction::CycleFocus, vec![KeyCombo::simple(KeyCode::Tab)]);
        list.insert(
            ListAction::ConfirmDelete,
            vec![
                KeyCombo::simple(KeyCode::Char('y')),
                KeyCombo::simple(KeyCode::Enter),
            ],
        );
        list.insert(
            ListAction::CancelDelete,
            vec![
                KeyCombo::simple(KeyCode::Char('n')),
                KeyCombo::simple(KeyCode::Esc),
            ],
        );
        list.insert(
            ListAction::ToggleButton,
            vec![
                KeyCombo::simple(KeyCode::Enter),
                KeyCombo::simple(KeyCode::Char(' ')),
            ],
        );
        list.insert(
            ListAction::NewFromTemplate,
            vec![KeyCombo::simple(KeyCode::Char('t'))],
        );

        let mut edit = HashMap::new();
        edit.insert(EditAction::Quit, vec![KeyCombo::ctrl(KeyCode::Char('q'))]);
        edit.insert(EditAction::Back, vec![KeyCombo::simple(KeyCode::Esc)]);
        edit.insert(EditAction::CycleFocus, vec![KeyCombo::simple(KeyCode::Tab)]);
        edit.insert(
            EditAction::ToggleButton,
            vec![
                KeyCombo::simple(KeyCode::Enter),
                KeyCombo::simple(KeyCode::Char(' ')),
            ],
        );
        edit.insert(
            EditAction::SelectAll,
            vec![KeyCombo::ctrl(KeyCode::Char('a'))],
        );
        edit.insert(
            EditAction::Copy,
            vec![
                KeyCombo::ctrl(KeyCode::Char('c')),
                KeyCombo::ctrl(KeyCode::Insert),
            ],
        );
        edit.insert(
            EditAction::Cut,
            vec![
                KeyCombo::ctrl(KeyCode::Char('x')),
                KeyCombo::shift(KeyCode::Delete),
            ],
        );
        edit.insert(
            EditAction::Paste,
            vec![
                KeyCombo::ctrl(KeyCode::Char('v')),
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

        Self { list, edit, help }
    }
}

impl Keybinds {
    /// Load keybinds from file, merging with defaults
    pub fn load(path: &Path) -> Result<Self> {
        let mut keybinds = Self::default();

        if !path.exists() {
            return Ok(keybinds);
        }

        let content = fs::read_to_string(path).context("failed to read keybinds file")?;

        let toml: KeybindsToml =
            toml::from_str(&content).context("failed to parse keybinds file")?;

        // Apply overrides from TOML
        for (action_str, combos_str) in &toml.list {
            if let Some(action) = parse_list_action(action_str) {
                let combos: Vec<KeyCombo> = combos_str
                    .iter()
                    .filter_map(|s| KeyCombo::parse(s))
                    .collect();
                if !combos.is_empty() {
                    keybinds.list.insert(action, combos);
                }
            }
        }

        for (action_str, combos_str) in &toml.edit {
            if let Some(action) = parse_edit_action(action_str) {
                let combos: Vec<KeyCombo> = combos_str
                    .iter()
                    .filter_map(|s| KeyCombo::parse(s))
                    .collect();
                if !combos.is_empty() {
                    keybinds.edit.insert(action, combos);
                }
            }
        }

        for (action_str, combos_str) in &toml.help {
            if let Some(action) = parse_help_action(action_str) {
                let combos: Vec<KeyCombo> = combos_str
                    .iter()
                    .filter_map(|s| KeyCombo::parse(s))
                    .collect();
                if !combos.is_empty() {
                    keybinds.help.insert(action, combos);
                }
            }
        }

        Ok(keybinds)
    }

    /// Save keybinds to file
    pub fn save(&self, path: &Path) -> Result<()> {
        let toml = self.to_toml();
        let content = toml::to_string_pretty(&toml).context("failed to serialize keybinds")?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("failed to create keybinds directory")?;
        }

        let mut file = fs::File::create(path).context("failed to create keybinds file")?;
        file.write_all(content.as_bytes())
            .context("failed to write keybinds file")?;

        Ok(())
    }

    /// Convert to TOML representation
    pub fn to_toml(&self) -> KeybindsToml {
        let mut toml = KeybindsToml::default();

        for (action, combos) in &self.list {
            let key = list_action_to_string(*action);
            let values: Vec<String> = combos.iter().map(KeyCombo::to_display_string).collect();
            toml.list.insert(key.to_string(), values);
        }

        for (action, combos) in &self.edit {
            let key = edit_action_to_string(*action);
            let values: Vec<String> = combos.iter().map(KeyCombo::to_display_string).collect();
            toml.edit.insert(key.to_string(), values);
        }

        for (action, combos) in &self.help {
            let key = help_action_to_string(*action);
            let values: Vec<String> = combos.iter().map(KeyCombo::to_display_string).collect();
            toml.help.insert(key.to_string(), values);
        }

        toml
    }

    /// Check if a key event matches a list action
    pub fn matches_list(&self, action: ListAction, event: &KeyEvent) -> bool {
        self.list
            .get(&action)
            .is_some_and(|combos| combos.iter().any(|c| c.matches(event)))
    }

    /// Check if a key event matches an edit action
    pub fn matches_edit(&self, action: EditAction, event: &KeyEvent) -> bool {
        self.edit
            .get(&action)
            .is_some_and(|combos| combos.iter().any(|c| c.matches(event)))
    }

    /// Check if a key event matches a help action
    pub fn matches_help(&self, action: HelpAction, event: &KeyEvent) -> bool {
        self.help
            .get(&action)
            .is_some_and(|combos| combos.iter().any(|c| c.matches(event)))
    }

    /// Get display string for a list action's keybinds
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

    /// Get display string for an edit action's keybinds
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

    /// Get display string for a help action's keybinds
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
}

fn parse_list_action(s: &str) -> Option<ListAction> {
    match s {
        "move_up" => Some(ListAction::MoveUp),
        "move_down" => Some(ListAction::MoveDown),
        "open" => Some(ListAction::Open),
        "delete" => Some(ListAction::Delete),
        "quit" => Some(ListAction::Quit),
        "help" => Some(ListAction::Help),
        "open_location" => Some(ListAction::OpenLocation),
        "cycle_focus" => Some(ListAction::CycleFocus),
        "confirm_delete" => Some(ListAction::ConfirmDelete),
        "cancel_delete" => Some(ListAction::CancelDelete),
        "toggle_button" => Some(ListAction::ToggleButton),
        "new_from_template" => Some(ListAction::NewFromTemplate),
        _ => None,
    }
}

fn parse_edit_action(s: &str) -> Option<EditAction> {
    match s {
        "quit" => Some(EditAction::Quit),
        "back" => Some(EditAction::Back),
        "cycle_focus" => Some(EditAction::CycleFocus),
        "toggle_button" => Some(EditAction::ToggleButton),
        "select_all" => Some(EditAction::SelectAll),
        "copy" => Some(EditAction::Copy),
        "cut" => Some(EditAction::Cut),
        "paste" => Some(EditAction::Paste),
        "undo" => Some(EditAction::Undo),
        "redo" => Some(EditAction::Redo),
        "delete_word" => Some(EditAction::DeleteWord),
        "delete_next_word" => Some(EditAction::DeleteNextWord),
        "move_to_top" => Some(EditAction::MoveToTop),
        "move_to_bottom" => Some(EditAction::MoveToBottom),
        _ => None,
    }
}

fn parse_help_action(s: &str) -> Option<HelpAction> {
    match s {
        "close" => Some(HelpAction::Close),
        "scroll_up" => Some(HelpAction::ScrollUp),
        "scroll_down" => Some(HelpAction::ScrollDown),
        _ => None,
    }
}

fn list_action_to_string(action: ListAction) -> &'static str {
    match action {
        ListAction::MoveUp => "move_up",
        ListAction::MoveDown => "move_down",
        ListAction::Open => "open",
        ListAction::Delete => "delete",
        ListAction::Quit => "quit",
        ListAction::Help => "help",
        ListAction::OpenLocation => "open_location",
        ListAction::CycleFocus => "cycle_focus",
        ListAction::ConfirmDelete => "confirm_delete",
        ListAction::CancelDelete => "cancel_delete",
        ListAction::ToggleButton => "toggle_button",
        ListAction::NewFromTemplate => "new_from_template",
    }
}

fn edit_action_to_string(action: EditAction) -> &'static str {
    match action {
        EditAction::Quit => "quit",
        EditAction::Back => "back",
        EditAction::CycleFocus => "cycle_focus",
        EditAction::ToggleButton => "toggle_button",
        EditAction::SelectAll => "select_all",
        EditAction::Copy => "copy",
        EditAction::Cut => "cut",
        EditAction::Paste => "paste",
        EditAction::Undo => "undo",
        EditAction::Redo => "redo",
        EditAction::DeleteWord => "delete_word",
        EditAction::DeleteNextWord => "delete_next_word",
        EditAction::MoveToTop => "move_to_top",
        EditAction::MoveToBottom => "move_to_bottom",
    }
}

fn help_action_to_string(action: HelpAction) -> &'static str {
    match action {
        HelpAction::Close => "close",
        HelpAction::ScrollUp => "scroll_up",
        HelpAction::ScrollDown => "scroll_down",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key_combo_simple() {
        let combo = KeyCombo::parse("q").unwrap();
        assert_eq!(combo.code, KeyCode::Char('q'));
        assert_eq!(combo.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn test_parse_key_combo_ctrl() {
        let combo = KeyCombo::parse("Ctrl+q").unwrap();
        assert_eq!(combo.code, KeyCode::Char('q'));
        assert_eq!(combo.modifiers, KeyModifiers::CONTROL);
    }

    #[test]
    fn test_parse_key_combo_ctrl_shift() {
        let combo = KeyCombo::parse("Ctrl+Shift+z").unwrap();
        assert_eq!(combo.code, KeyCode::Char('z'));
        assert_eq!(combo.modifiers, KeyModifiers::CONTROL | KeyModifiers::SHIFT);
    }

    #[test]
    fn test_parse_special_keys() {
        assert_eq!(KeyCombo::parse("Enter").unwrap().code, KeyCode::Enter);
        assert_eq!(KeyCombo::parse("Esc").unwrap().code, KeyCode::Esc);
        assert_eq!(KeyCombo::parse("F1").unwrap().code, KeyCode::F(1));
        assert_eq!(KeyCombo::parse("Delete").unwrap().code, KeyCode::Delete);
    }

    #[test]
    fn test_key_combo_matches() {
        let combo = KeyCombo::ctrl(KeyCode::Char('q'));
        let event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
        assert!(combo.matches(&event));

        let wrong_event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(!combo.matches(&wrong_event));
    }

    #[test]
    fn test_default_keybinds() {
        let keybinds = Keybinds::default();
        assert!(!keybinds.list.is_empty());
        assert!(!keybinds.edit.is_empty());
        assert!(!keybinds.help.is_empty());
    }

    #[test]
    fn test_matches_list_action() {
        let keybinds = Keybinds::default();
        let event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(keybinds.matches_list(ListAction::Quit, &event));
    }
}
