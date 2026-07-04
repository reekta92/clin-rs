use serde::{Deserialize, Serialize};
use std::collections::HashMap;

mod api;
mod combo;
mod defaults;
mod matcher;
mod types;

pub use combo::KeyCombo;
pub use matcher::{KeyMatcher, MatchOutcome};
pub use types::*;

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
    #[serde(default)]
    pub setup: HashMap<SetupAction, Vec<String>>,
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
    pub setup: HashMap<SetupAction, Vec<KeyCombo>>,
}
