use std::collections::HashMap;
use serde::{Deserialize, Serialize};

mod types;
mod combo;
mod matcher;
mod defaults;
mod api;

pub use types::*;
pub use combo::{KeyStroke, KeyCombo};
pub use matcher::{MatchOutcome, KeyMatcher};

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
