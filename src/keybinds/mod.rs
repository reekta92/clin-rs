use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::HashMap;

mod api;
mod combo;
mod defaults;
pub mod help_meta;
mod matcher;
mod types;

pub(crate) use api::repair_legacy_preset_sequences;
pub use combo::KeyCombo;
pub use matcher::{KeyMatcher, MatchOutcome};
pub use types::*;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeybindsToml {
    #[serde(default)]
    pub list: BTreeMap<ListAction, Vec<String>>,
    #[serde(default)]
    pub edit: BTreeMap<EditAction, Vec<String>>,
    #[serde(default)]
    pub help: BTreeMap<HelpAction, Vec<String>>,
    #[serde(default)]
    pub graph: BTreeMap<GraphAction, Vec<String>>,
    #[serde(default)]
    pub draw: BTreeMap<DrawAction, Vec<String>>,
    #[serde(default)]
    pub canvas: BTreeMap<CanvasAction, Vec<String>>,
    #[serde(default)]
    pub backup: BTreeMap<BackupAction, Vec<String>>,
    #[serde(default)]
    pub outline: BTreeMap<OutlineAction, Vec<String>>,
    #[serde(default)]
    pub setup: BTreeMap<SetupAction, Vec<String>>,
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
    pub outline: HashMap<OutlineAction, Vec<KeyCombo>>,
    pub setup: HashMap<SetupAction, Vec<KeyCombo>>,
}
