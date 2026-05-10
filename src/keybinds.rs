





use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};


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

    
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('+').collect();
        let mut modifiers = KeyModifiers::NONE;
        let mut key_part = "";

        for (i, part) in parts.iter().enumerate() {
            let part_lower = part.to_lowercase();
            if i == parts.len() - 1 {
                
                key_part = part;
            } else {
                
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

    
    pub fn to_display_string(&self) -> String {
        let key = key_code_to_string(&self.code);
        let mut result = String::with_capacity(24);

        let mut need_sep = false;
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            result.push_str("Ctrl");
            need_sep = true;
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            if need_sep {
                result.push('+');
            }
            result.push_str("Shift");
            need_sep = true;
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            if need_sep {
                result.push('+');
            }
            result.push_str("Alt");
            need_sep = true;
        }
        if self.modifiers.contains(KeyModifiers::SUPER) {
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

    
    pub fn matches(&self, event: &KeyEvent) -> bool {
        self.code == event.code && self.modifiers == event.modifiers
    }
}

fn parse_key_code(s: &str) -> Option<KeyCode> {
    let s_lower = s.to_lowercase();
    match s_lower.as_str() {
        
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
        KeyCode::F(n) => Cow::Owned(format!("F{}", n)),
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
    Open,
    Delete,
    Quit,
    Help,
    OpenLocation,
    CycleFocus,
    Confirm,
    Cancel,
    ToggleButton,
    NewFromTemplate,
    CreateFolder,
    CreateNote,
    RenameFolder,
    MoveNote,
    ManageTags,
    FilterTags,
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
    OpenGraph,
    OpenCanvas,
    CreatePinstar,
    }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditAction {
    Quit,
    Back,
    CycleFocus,
    ToggleButton,
    
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
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HelpAction {
    Close,
    ScrollUp,
    ScrollDown,
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
}


#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeybindsToml {
    #[serde(default)]
    pub list: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub edit: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub help: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub graph: HashMap<String, Vec<String>>,
}


#[derive(Debug, Clone)]
pub struct Keybinds {
    pub list: HashMap<ListAction, Vec<KeyCombo>>,
    pub edit: HashMap<EditAction, Vec<KeyCombo>>,
    pub help: HashMap<HelpAction, Vec<KeyCombo>>,
    pub graph: HashMap<GraphAction, Vec<KeyCombo>>,
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
            ListAction::FilterTags,
            vec![KeyCombo::simple(KeyCode::Char('/'))],
        );
        list.insert(
            ListAction::CollapseFolder,
            vec![KeyCombo::simple(KeyCode::Char('h'))],
        );
        list.insert(
            ListAction::ExpandFolder,
            vec![KeyCombo::simple(KeyCode::Char('l'))],
        );
        list.insert(
            ListAction::OpenCommandPalette,
            vec![
                KeyCombo::ctrl(KeyCode::Char('p')),
                KeyCombo::shift(KeyCode::Enter),
            ],
        );
        list.insert(
            ListAction::ExpandFolder,
            vec![KeyCombo::simple(KeyCode::Char('l'))],
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
        list.insert(ListAction::Search, vec![KeyCombo::ctrl(KeyCode::Char('f'))]);
        list.insert(
            ListAction::JumpToTop,
            vec![KeyCombo::shift(KeyCode::Char('G'))],
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
            ListAction::OpenGraph,
            vec![KeyCombo::ctrl(KeyCode::Char('g'))],
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
        edit.insert(
            EditAction::ToggleMarkdownPreview,
            vec![KeyCombo::ctrl(KeyCode::Char('p'))],
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
        graph.insert(GraphAction::Quit, vec![KeyCombo::simple(KeyCode::Esc)]);
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

        Self {
            list,
            edit,
            help,
            graph,
        }
    }
}

impl Keybinds {
    
    pub fn load(path: &Path) -> Result<Self> {
        let mut keybinds = Self::default();

        if !path.exists() {
            return Ok(keybinds);
        }

        let content = fs::read_to_string(path).context("failed to read keybinds file")?;

        let toml: KeybindsToml =
            toml::from_str(&content).context("failed to parse keybinds file")?;

        
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

        for (action_str, combos_str) in &toml.graph {
            if let Some(action) = parse_graph_action(action_str) {
                let combos: Vec<KeyCombo> = combos_str
                    .iter()
                    .filter_map(|s| KeyCombo::parse(s))
                    .collect();
                if !combos.is_empty() {
                    keybinds.graph.insert(action, combos);
                }
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

        let mut file = fs::File::create(path).context("failed to create keybinds file")?;
        file.write_all(content.as_bytes())
            .context("failed to write keybinds file")?;

        Ok(())
    }

    
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

        for (action, combos) in &self.graph {
            let key = graph_action_to_string(*action);
            let values: Vec<String> = combos.iter().map(KeyCombo::to_display_string).collect();
            toml.graph.insert(key.to_string(), values);
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

    
    pub fn matches_help(&self, action: HelpAction, event: &KeyEvent) -> bool {
        self.help
            .get(&action)
            .is_some_and(|combos| combos.iter().any(|c| c.matches(event)))
    }

    pub fn matches_graph(&self, action: GraphAction, event: &KeyEvent) -> bool {
        self.graph
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
        "confirm" => Some(ListAction::Confirm),
        "cancel" => Some(ListAction::Cancel),
        "toggle_button" => Some(ListAction::ToggleButton),
        "new_from_template" => Some(ListAction::NewFromTemplate),
        "create_folder" => Some(ListAction::CreateFolder),
        "create_note" => Some(ListAction::CreateNote),
        "rename_folder" => Some(ListAction::RenameFolder),
        "move_note" => Some(ListAction::MoveNote),
        "manage_tags" => Some(ListAction::ManageTags),
        "filter_tags" => Some(ListAction::FilterTags),
        "collapse_folder" => Some(ListAction::CollapseFolder),
        "expand_folder" => Some(ListAction::ExpandFolder),
        "open_graph" => Some(ListAction::OpenGraph),
        "open_canvas" => Some(ListAction::OpenCanvas),
        "create_pinstar" => Some(ListAction::CreatePinstar),
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
        "toggle_markdown_preview" => Some(EditAction::ToggleMarkdownPreview),
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

fn parse_graph_action(s: &str) -> Option<GraphAction> {
    match s {
        "quit" => Some(GraphAction::Quit),
        "pan_up" => Some(GraphAction::PanUp),
        "pan_down" => Some(GraphAction::PanDown),
        "pan_left" => Some(GraphAction::PanLeft),
        "pan_right" => Some(GraphAction::PanRight),
        "zoom_in" => Some(GraphAction::ZoomIn),
        "zoom_out" => Some(GraphAction::ZoomOut),
        "open_note" => Some(GraphAction::OpenNote),
        "auto_fit" => Some(GraphAction::AutoFit),
        "help" => Some(GraphAction::Help),
        "toggle_search" => Some(GraphAction::ToggleSearch),
        "toggle_minimap" => Some(GraphAction::ToggleMinimap),
        "toggle_legend" => Some(GraphAction::ToggleLegend),
        "toggle_grid" => Some(GraphAction::ToggleGrid),
        "toggle_status" => Some(GraphAction::ToggleStatus),
        "refresh" => Some(GraphAction::Refresh),
        "reload_config" => Some(GraphAction::ReloadConfig),
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
        ListAction::Confirm => "confirm",
        ListAction::Cancel => "cancel",
        ListAction::ToggleButton => "toggle_button",
        ListAction::NewFromTemplate => "new_from_template",
        ListAction::CreateFolder => "create_folder",
        ListAction::CreateNote => "create_note",
        ListAction::RenameFolder => "rename_folder",
        ListAction::MoveNote => "move_note",
        ListAction::ManageTags => "manage_tags",
        ListAction::FilterTags => "filter_tags",
        ListAction::CollapseFolder => "collapse_folder",
        ListAction::ExpandFolder => "expand_folder",
        ListAction::OpenCommandPalette => "open_command_palette",
        
        ListAction::Rename => "rename",
        ListAction::Duplicate => "duplicate",
        ListAction::TogglePin => "toggle_pin",
        ListAction::CycleSort => "cycle_sort",
        ListAction::Search => "search",
        ListAction::JumpToTop => "jump_to_top",
        ListAction::JumpToBottom => "jump_to_bottom",
        ListAction::PageUp => "page_up",
        ListAction::PageDown => "page_down",
        ListAction::OpenTrash => "open_trash",
        ListAction::TogglePreview => "toggle_preview",
        ListAction::OpenGraph => "open_graph",
        ListAction::OpenCanvas => "open_canvas",
        ListAction::CreatePinstar => "create_pinstar",
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
        EditAction::ToggleMarkdownPreview => "toggle_markdown_preview",
    }
}

fn help_action_to_string(action: HelpAction) -> &'static str {
    match action {
        HelpAction::Close => "close",
        HelpAction::ScrollUp => "scroll_up",
        HelpAction::ScrollDown => "scroll_down",
    }
}

fn graph_action_to_string(action: GraphAction) -> &'static str {
    match action {
        GraphAction::Quit => "quit",
        GraphAction::PanUp => "pan_up",
        GraphAction::PanDown => "pan_down",
        GraphAction::PanLeft => "pan_left",
        GraphAction::PanRight => "pan_right",
        GraphAction::ZoomIn => "zoom_in",
        GraphAction::ZoomOut => "zoom_out",
        GraphAction::OpenNote => "open_note",
        GraphAction::AutoFit => "auto_fit",
        GraphAction::Help => "help",
        GraphAction::ToggleSearch => "toggle_search",
        GraphAction::ToggleMinimap => "toggle_minimap",
        GraphAction::ToggleLegend => "toggle_legend",
        GraphAction::ToggleGrid => "toggle_grid",
        GraphAction::ToggleStatus => "toggle_status",
        GraphAction::Refresh => "refresh",
        GraphAction::ReloadConfig => "reload_config",
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
