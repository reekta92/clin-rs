use crate::app::{App, ViewMode};
use crate::frontmatter::{self, FrontmatterEdit};
use crate::property_model::{PropertyDefinitions, PropertyKind, PropertyValue};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

mod guided;
mod recovery;
use guided::{GuidedAction, draw_guided_management};
use recovery::PreparedChange;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct HeaderChange {
    pub id: String,
    pub before: Option<String>,
    pub after: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BindingChange {
    pub path: std::path::PathBuf,
    #[serde(default)]
    pub create: bool,
    pub before: String,
    pub after: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PropertyBatch {
    #[serde(default)]
    pub version: Option<u32>,
    #[serde(default)]
    pub vault: Option<std::path::PathBuf>,
    #[serde(default)]
    pub writes: Vec<PreparedChange>,
    #[serde(default)]
    pub parents: std::collections::BTreeMap<String, String>,
    pub notes: Vec<HeaderChange>,
    pub bindings: Vec<BindingChange>,
}

impl App {
    pub fn ensure_catalog_ready(&mut self) -> Result<()> {
        if self.initial_load_done {
            return Ok(());
        }
        let load = crate::app::catalog::load_notes_blocking(
            &self.storage,
            &self.notes_worker_pool,
            self.list.show_hidden_files,
            self.list.show_all_files,
        )?;
        ensure!(
            load.complete,
            "Notes scan incomplete; cannot safely resolve note selection"
        );
        self.notes = load.summaries;
        self.catalog_folders = load.folders;
        self.note_stamps = load
            .map
            .into_iter()
            .map(|(id, (stamp, _))| (id, stamp))
            .collect();
        self.initial_load_done = true;
        self.catalog_status = None;
        self.notes_revision = self.notes_revision.wrapping_add(1);
        self.sort_notes();
        self.rebuild_note_index();
        self.refresh_visual_list();
        Ok(())
    }
    pub(crate) fn save_property_definitions(
        &mut self,
        definitions: PropertyDefinitions,
    ) -> Result<()> {
        let stamp = std::fs::metadata(PropertyDefinitions::path(&self.storage.data_dir))
            .ok()
            .map(|metadata| crate::storage::FileStamp {
                len: metadata.len(),
                modified_nanos: metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|time| time.as_nanos()),
            });
        ensure!(
            stamp == self.property_definitions_stamp,
            "Definitions changed since loaded; reload and retry"
        );
        definitions.save(&self.storage.data_dir)?;
        self.reload_property_definitions();
        self.enqueue_backup("properties: definitions");
        Ok(())
    }
    pub(crate) fn reload_property_definitions(&mut self) {
        let path = PropertyDefinitions::path(&self.storage.data_dir);
        self.property_definitions_stamp =
            std::fs::metadata(path)
                .ok()
                .map(|metadata| crate::storage::FileStamp {
                    len: metadata.len(),
                    modified_nanos: metadata
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|time| time.as_nanos()),
                });
        match PropertyDefinitions::load(&self.storage.data_dir) {
            Ok(definitions) => {
                self.property_definitions = definitions;
                self.property_definitions_error = None;
            }
            Err(error) => {
                let message = format!("Property definitions: {error:#}");
                self.property_definitions_error = Some(message.clone());
                self.messages
                    .push(message, crate::app::messages::MessageSeverity::Warning);
            }
        }
        self.notes_revision = self.notes_revision.wrapping_add(1);
        self.editor
            .properties
            .set_definitions(&self.property_definitions);
        if let Some(key) = &self.config.list.calendar_date_property
            && let Err(error) =
                crate::property_query::validate_date_binding(key, &self.property_definitions)
        {
            self.messages.push(
                format!("Calendar property {key}: {error}"),
                crate::app::messages::MessageSeverity::Warning,
            );
        }
        self.refresh_property_dependents();
    }
    pub(crate) fn refresh_property_dependents(&mut self) {
        self.sort_notes();
        self.rebuild_note_index();
        self.list.note_metrics = None;
        self.refresh_visual_list();
        self.editor.links = self.compute_links();
        self.graph_preview = None;
        if let Some(plugin) = &mut self.graph_plugin {
            plugin.notes.clone_from(&self.notes);
            plugin.refresh_simulation(&self.config);
        }
        if matches!(
            self.popups.active,
            Some(crate::popups::ActivePopup::Search(_))
        ) {
            self.update_search();
        }
        let mut diagnostics = self.notes.iter().flat_map(|note| {
            self.property_definitions
                .diagnostics(&note.properties)
                .into_iter()
                .map(move |warning| format!("{}: {warning}", note.id))
        });
        for warning in diagnostics.by_ref().take(10) {
            self.messages
                .push(warning, crate::app::messages::MessageSeverity::Warning);
        }
        let remaining = diagnostics.count();
        if remaining > 0 {
            self.messages.push(
                format!("…and {remaining} more invalid property values"),
                crate::app::messages::MessageSeverity::Warning,
            );
        }
    }
    pub(crate) fn check_property_definitions(&mut self) {
        let path = PropertyDefinitions::path(&self.storage.data_dir);
        let stamp = std::fs::metadata(path)
            .ok()
            .map(|metadata| crate::storage::FileStamp {
                len: metadata.len(),
                modified_nanos: metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|time| time.as_nanos()),
            });
        if stamp != self.property_definitions_stamp {
            self.reload_property_definitions();
        }
    }
    pub(crate) fn property_targets(&self) -> Vec<String> {
        let mut ids = std::collections::BTreeSet::new();
        if self.mode == ViewMode::Edit {
            if let Some(id) = &self.editor.editing_id {
                ids.insert(id.clone());
            }
        } else if !self.list.selected_indices.is_empty() {
            for index in &self.list.selected_indices {
                if let Some(crate::list_view::VisualItem::Note { summary_idx, .. }) =
                    self.list.visual_list.get(*index)
                    && let Some(note) = self.notes.get(*summary_idx)
                {
                    ids.insert(note.id.clone());
                }
            }
        } else if let Some(id) = self.get_selected_note_id() {
            ids.insert(id);
        }
        ids.into_iter()
            .filter(|id| {
                matches!(
                    std::path::Path::new(id)
                        .extension()
                        .and_then(|ext| ext.to_str()),
                    Some("md" | "txt" | "clin")
                )
            })
            .collect()
    }
    pub(crate) fn prepare_property_batch(
        &mut self,
        ids: &[String],
        edits: &[FrontmatterEdit],
    ) -> Result<PropertyBatch> {
        ensure!(
            !PropertyBatch::recovery_path(self).exists(),
            "Resume pending property batch before preparing a new batch"
        );
        if self.mode == ViewMode::Edit {
            self.autosave().map_err(anyhow::Error::msg)?;
        }
        ensure!(
            self.property_definitions_error.is_none(),
            "Repair property definitions before bulk editing"
        );
        let mut changes = Vec::new();
        for id in ids.iter().collect::<std::collections::BTreeSet<_>>() {
            let before = self.storage.load_frontmatter(id)?;
            self.storage.property_note_path(id)?;
            let after = frontmatter::apply_edits(before.as_deref().unwrap_or("---\n---\n"), edits)?;
            let values = frontmatter::checked_parse(&after)?;
            for edit in edits {
                let key: serde_yaml_ng::Value = serde_yaml_ng::from_str(&edit.key_yaml)?;
                if let Some(value) = values.extra.get(&key) {
                    self.property_definitions.validate(
                        key.as_str().context("Batch property needs string key")?,
                        value,
                    )?;
                }
            }
            if before.as_deref().unwrap_or("---\n---\n") != after {
                changes.push(HeaderChange {
                    id: id.clone(),
                    before,
                    after,
                });
            }
        }
        Ok(PropertyBatch {
            notes: changes,
            bindings: Vec::new(),
            ..Default::default()
        })
    }
    pub(crate) fn prepare_property_rename(
        &mut self,
        old: &str,
        new: &str,
    ) -> Result<PropertyBatch> {
        ensure!(
            !PropertyBatch::recovery_path(self).exists(),
            "Resume pending property batch before preparing a new batch"
        );
        if self.mode == ViewMode::Edit {
            self.autosave().map_err(anyhow::Error::msg)?;
        }
        self.ensure_catalog_ready()?;
        ensure!(
            self.property_definitions_error.is_none(),
            "Repair property definitions before renaming keys across the vault"
        );
        crate::property_model::validate_key(old)?;
        crate::property_model::validate_key(new)?;
        ensure!(old != new, "Choose a different property name");
        ensure!(
            !self.property_definitions.properties.contains_key(new),
            "Target definition already exists"
        );
        let mut batch = PropertyBatch::default();
        for id in self.storage.list_note_ids(true, false)? {
            if !matches!(
                std::path::Path::new(&id)
                    .extension()
                    .and_then(|value| value.to_str()),
                Some("md" | "txt" | "clin")
            ) {
                continue;
            }
            let before = self.storage.load_frontmatter(&id)?;
            if let Some(header) = &before {
                let mapping = frontmatter::validate_header(header)?;
                if mapping.contains_key(serde_yaml_ng::Value::String(old.into())) {
                    let after = frontmatter::rename_key(header, old, new)?;
                    batch.notes.push(HeaderChange { id, before, after });
                }
            }
        }
        let definition_path = PropertyDefinitions::path(&self.storage.data_dir);
        if definition_path.exists() {
            let before = std::fs::read_to_string(&definition_path)?;
            let mut doc: toml_edit::DocumentMut = before.parse()?;
            if let Some(table) = doc
                .get_mut("properties")
                .and_then(toml_edit::Item::as_table_like_mut)
                && let Some(value) = table.remove(old)
            {
                table.insert(new, value);
            }
            let after = doc.to_string();
            if before != after {
                batch.bindings.push(BindingChange {
                    create: false,
                    path: definition_path,
                    before,
                    after,
                });
            }
        }
        let config_path = crate::config::ClinConfig::config_path()?;
        if config_path.exists() {
            let before = std::fs::read_to_string(&config_path)?;
            let mut config = self.config.clone();
            for rule in &mut config.list.custom_smart_folders {
                for predicate in rule.all.iter_mut().chain(rule.any.iter_mut().flatten()) {
                    if predicate.property == old {
                        predicate.property = new.into();
                    }
                }
            }
            for binding in [
                &mut config.list.property_sort_key,
                &mut config.list.property_group_key,
                &mut config.list.calendar_date_property,
                &mut config.goals.note_word_goal_property,
            ] {
                if binding.as_deref() == Some(old) {
                    *binding = Some(new.into());
                }
            }
            for binding in &mut config.list.property_fields {
                if binding == old {
                    *binding = new.into();
                }
            }
            let mut desired: toml::Value = toml::Value::try_from(&config)?;
            replace_property_tokens(&mut desired, old, new);
            let desired: toml_edit::DocumentMut = toml::to_string(&desired)?.parse()?;
            let mut doc: toml_edit::DocumentMut = before.parse()?;
            for (key, item) in desired.iter() {
                if let Some(existing) = doc.get_mut(key) {
                    crate::config::merge::merge_edit_item(existing, item.clone());
                } else {
                    doc.insert(key, item.clone());
                }
            }
            let after = doc.to_string();
            if before != after {
                batch.bindings.push(BindingChange {
                    create: false,
                    path: config_path,
                    before,
                    after,
                });
            }
        }
        if self.storage.templates_dir.is_dir() {
            for entry in std::fs::read_dir(&self.storage.templates_dir)? {
                let path = entry?.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
                    continue;
                }
                let before = std::fs::read_to_string(&path)?;
                let template: crate::templates::Template = toml::from_str(&before)
                    .with_context(|| format!("Invalid template {}", path.display()))?;
                let mut doc: toml_edit::DocumentMut = before
                    .parse()
                    .with_context(|| format!("Invalid template {}", path.display()))?;
                if let Some(table) = doc
                    .get_mut("properties")
                    .and_then(toml_edit::Item::as_table_like_mut)
                {
                    ensure!(
                        !table.contains_key(new) || !table.contains_key(old),
                        "Template {} already defines {new}",
                        path.display()
                    );
                    if let Some(value) = table.remove(old) {
                        table.insert(new, value);
                    }
                }
                let (header, body) =
                    frontmatter::split_header(template.content.template.as_bytes())
                        .with_context(|| format!("Invalid template header {}", path.display()))?;
                if let Some(header) = header
                    && frontmatter::validate_header(header)?
                        .contains_key(serde_yaml_ng::Value::String(old.into()))
                {
                    let renamed = frontmatter::rename_key(header, old, new)?;
                    let item = &mut doc["content"]["template"];
                    let decor = item.as_value().map(|value| value.decor().clone());
                    *item = toml_edit::value(format!("{renamed}{}", std::str::from_utf8(body)?));
                    if let (Some(decor), Some(value)) = (decor, item.as_value_mut()) {
                        *value.decor_mut() = decor;
                    }
                }
                replace_document_tokens(&mut doc, old, new);
                let after = doc.to_string();
                if before != after {
                    batch.bindings.push(BindingChange {
                        create: false,
                        path,
                        before,
                        after,
                    });
                }
            }
        }
        Ok(batch)
    }
    pub(crate) fn apply_property_defaults(&mut self) -> Result<()> {
        let defaults = self.property_definitions.defaults()?;
        if self.mode == ViewMode::Edit {
            let mapping = frontmatter::validate_header(
                self.editor
                    .properties
                    .current
                    .as_deref()
                    .unwrap_or("---\n---\n"),
            )?;
            let mut changed = false;
            for edit in defaults {
                let key: serde_yaml_ng::Value = serde_yaml_ng::from_str(&edit.key_yaml)?;
                if !mapping.contains_key(&key) {
                    changed |= self.editor.properties.commit(edit, true)?;
                }
            }
            if changed {
                self.mark_properties_modified();
            }
        } else {
            anyhow::bail!("Open note editor to apply defaults");
        }
        Ok(())
    }
    pub(crate) fn current_property_value(&self, key: &str) -> Option<&PropertyValue> {
        if self.mode == ViewMode::Edit {
            self.editor.properties.values.get(key)
        } else if let Some(crate::list_view::VisualItem::Note { summary_idx, .. }) =
            self.list.visual_list.get(self.list.visual_index)
        {
            self.notes
                .get(*summary_idx)
                .and_then(|note| note.properties.get(key))
        } else {
            None
        }
    }
    pub(crate) fn note_word_goal(&self) -> Option<usize> {
        self.config
            .goals
            .note_word_goal_property
            .as_deref()
            .and_then(|key| self.current_property_value(key))
            .and_then(|value| value.word_goal())
            .filter(|target| *target > 0)
    }
}
fn replace_property_tokens(value: &mut toml::Value, old: &str, new: &str) {
    match value {
        toml::Value::String(text) => {
            *text = text.replace(&format!("{{prop:{old}}}"), &format!("{{prop:{new}}}"))
        }
        toml::Value::Array(values) => {
            for value in values {
                replace_property_tokens(value, old, new);
            }
        }
        toml::Value::Table(values) => {
            for (_, value) in values.iter_mut() {
                replace_property_tokens(value, old, new);
            }
        }
        _ => {}
    }
}
fn replace_document_tokens(document: &mut toml_edit::DocumentMut, old: &str, new: &str) {
    fn replace(item: &mut toml_edit::Item, old: &str, new: &str) {
        if let Some(value) = item.as_str() {
            let updated = value.replace(&format!("{{prop:{old}}}"), &format!("{{prop:{new}}}"));
            if updated != value {
                let decor = item.as_value().map(|value| value.decor().clone());
                *item = toml_edit::value(updated);
                if let (Some(decor), Some(value)) = (decor, item.as_value_mut()) {
                    *value.decor_mut() = decor;
                }
            }
        } else if let Some(table) = item.as_table_mut() {
            for (_, value) in table.iter_mut() {
                replace(value, old, new);
            }
        }
    }
    for (_, value) in document.iter_mut() {
        replace(value, old, new);
    }
}

#[derive(Clone, Copy)]
pub(crate) enum PropertyManagerMode {
    Definitions,
    Bulk,
    Rename { global: bool },
    Resume,
    References,
}
pub(crate) enum PropertyPreview {
    References {
        operation: ReferenceOperation,
        relocations: std::collections::HashMap<String, String>,
        batch: PropertyBatch,
    },
    Definitions {
        text: String,
        original: Option<String>,
    },
    Batch(PropertyBatch),
    Rename {
        old: String,
        new: String,
        header: Option<String>,
    },
}
pub(crate) struct PropertyManager {
    pub mode: PropertyManagerMode,
    pub input: ratatui_textarea::TextArea<'static>,
    pub targets: Vec<String>,
    pub original: Option<String>,
    pub preview: Option<PropertyPreview>,
    pub report: Vec<String>,
    pub error: Option<String>,
    pub confirm: bool,
    pub input_rect: ratatui::layout::Rect,
    pub apply_rect: ratatui::layout::Rect,
    pub scroll: u16,
    guided: Option<GuidedManager>,
}
enum GuidedManager {
    Definitions {
        definitions: PropertyDefinitions,
        keys: Vec<String>,
        selected: usize,
        form: Option<Box<ManagementForm>>,
    },
    Bulk(Box<ManagementForm>),
}
struct ManagementForm {
    name: ratatui_textarea::TextArea<'static>,
    value: ratatui_textarea::TextArea<'static>,
    description: ratatui_textarea::TextArea<'static>,
    options: ratatui_textarea::TextArea<'static>,
    kind: PropertyKind,
    enabled: bool, // Definition default enabled / bulk set (rather than unset).
    control: usize,
    rects: [ratatui::layout::Rect; 7],
    choices: Vec<String>,
    choice: usize,
}
impl ManagementForm {
    fn new(
        app: &App,
        name: &str,
        definition: Option<&crate::property_model::PropertyDefinition>,
    ) -> Result<Self> {
        let field = |text: &str| {
            let mut input = crate::ui::make_popup_textarea(&app.app_theme, "");
            input.insert_str(text);
            input
        };
        let value = definition
            .and_then(|definition| definition.default.as_ref())
            .map(crate::property_model::toml_to_yaml)
            .transpose()?
            .map(|value| match value {
                serde_yaml_ng::Value::String(value) => Ok(value),
                _ => serde_yaml_ng::to_string(&value),
            })
            .transpose()?
            .unwrap_or_default();
        Ok(Self {
            name: field(name),
            value: field(&value),
            description: field(definition.map_or("", |definition| &definition.description)),
            options: field(
                &definition.map_or_else(String::new, |definition| definition.options.join("\n")),
            ),
            kind: definition.map_or(PropertyKind::String, |definition| definition.kind),
            enabled: definition.is_some_and(|definition| definition.default.is_some()),
            control: 0,
            rects: [ratatui::layout::Rect::default(); 7],
            choices: Vec::new(),
            choice: 0,
        })
    }
    fn key(&self) -> String {
        self.name.lines().join("\n")
    }
    fn definition(&self) -> Result<crate::property_model::PropertyDefinition> {
        let value = if self.enabled {
            let value = crate::property_model::parse_property_value(
                self.kind,
                &self.value.lines().join("\n"),
            )?;
            Some(toml::Value::try_from(value).context(
                "Default must be representable in TOML (null/large unsigned defaults unsupported)",
            )?)
        } else {
            None
        };
        Ok(crate::property_model::PropertyDefinition {
            kind: self.kind,
            description: self.description.lines().join("\n"),
            options: self
                .options
                .lines()
                .iter()
                .filter(|option| !option.is_empty())
                .cloned()
                .collect(),
            default: value,
        })
    }
    fn bulk_text(&self) -> Result<String> {
        let mut table = toml::map::Map::new();
        table.insert("name".into(), toml::Value::String(self.key()));
        table.insert("type".into(), toml::Value::try_from(self.kind)?);
        if self.enabled {
            table.insert(
                "value".into(),
                toml::Value::String(self.value.lines().join("\n")),
            );
        } else {
            table.insert("unset".into(), toml::Value::Boolean(true));
        }
        Ok(toml::to_string(&table)?)
    }
    fn textarea(&mut self) -> Option<&mut ratatui_textarea::TextArea<'static>> {
        match self.control {
            0 => Some(&mut self.name),
            2 => Some(&mut self.value),
            3 => Some(&mut self.description),
            4 => Some(&mut self.options),
            _ => None,
        }
    }
    fn advance(&mut self, delta: isize, definition: bool) {
        loop {
            self.control = (self.control as isize + delta).rem_euclid(7) as usize;
            if definition || !matches!(self.control, 3 | 4) {
                break;
            }
        }
    }
    fn refresh_choices(&mut self, app: &App) {
        self.choices = if self.kind.is_reference() {
            app.visible_notes()
                .map(|(_, note)| note.id.clone())
                .filter(|id| {
                    matches!(
                        std::path::Path::new(id)
                            .extension()
                            .and_then(|value| value.to_str()),
                        Some("md" | "txt" | "clin")
                    )
                })
                .collect()
        } else {
            self.options
                .lines()
                .iter()
                .filter(|option| !option.is_empty())
                .cloned()
                .collect()
        };
        self.choice = self.choice.min(self.choices.len().saturating_sub(1));
    }
    fn choose(&mut self) {
        let Some(choice) = self.choices.get(self.choice).cloned() else {
            return;
        };
        if matches!(
            self.kind,
            PropertyKind::MultiSelect | PropertyKind::NoteReferences
        ) {
            let mut values = self.value.lines().to_vec();
            if values.contains(&choice) {
                values.retain(|value| value != &choice);
            } else {
                values.push(choice);
            }
            self.value.select_all();
            self.value.insert_str(
                values
                    .into_iter()
                    .filter(|value| !value.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        } else {
            self.value.select_all();
            self.value.insert_str(choice);
        }
    }
}
#[derive(Deserialize)]
struct BulkInput {
    name: String,
    #[serde(rename = "type", default)]
    kind: PropertyKind,
    value: Option<toml::Value>,
    #[serde(default)]
    unset: bool,
}
#[derive(Deserialize)]
struct RenameInput {
    old: String,
    new: String,
}
impl App {
    pub(crate) fn open_property_manager(
        &mut self,
        mode: PropertyManagerMode,
        context_note_id: Option<&str>,
    ) {
        let result = (|| -> Result<PropertyManager> {
            self.ensure_catalog_ready()?;
            let original = match mode {
                PropertyManagerMode::Definitions => {
                    match std::fs::read_to_string(PropertyDefinitions::path(&self.storage.data_dir))
                    {
                        Ok(text) => Some(text),
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                        Err(error) => return Err(error.into()),
                    }
                }
                _ => None,
            };
            let targets = if self.list.selected_indices.is_empty() {
                context_note_id
                    .map(|id| vec![id.into()])
                    .unwrap_or_else(|| self.property_targets())
            } else {
                self.property_targets()
            };
            if matches!(mode, PropertyManagerMode::Bulk) {
                ensure!(
                    !targets.is_empty(),
                    "Select notes first (Space marks notes)"
                );
            }
            let text = match mode {
                PropertyManagerMode::Definitions => original.clone().unwrap_or_else(|| "# Optional vault property definitions.\n# type: string, number, boolean, date, date_time, list, select,\n# multi_select, note_reference, note_references, null, yaml\n[properties.status]\ntype = \"select\"\noptions = [\"draft\", \"review\", \"done\"]\ndefault = \"draft\"\ndescription = \"Note workflow\"\n".into()),
                PropertyManagerMode::Bulk => "name = \"status\"\ntype = \"string\"\nvalue = \"draft\"\n# unset = true removes key; omit value when unsetting\n".into(),
                PropertyManagerMode::Rename { global } => {
                    let old = self.editor.properties.rows.get(self.editor.properties.selected).map(|row| row.name()).unwrap_or("");
                    if !global { ensure!(self.mode == ViewMode::Edit && !old.is_empty(), "Select property in note editor first"); }
                    toml::to_string(&toml::Value::Table(toml::map::Map::from_iter([("old".into(), toml::Value::String(old.into())), ("new".into(), toml::Value::String(String::new()))])))?
                },
                PropertyManagerMode::References => anyhow::bail!("Reference preview is opened by move/rename"),
                PropertyManagerMode::Resume => {
                    let batch: PropertyBatch = toml::from_str(&std::fs::read_to_string(PropertyBatch::recovery_path(self)).context("No pending property batch")?)?;
                    let mut manager = PropertyManager { mode, input: crate::ui::make_popup_textarea(&self.app_theme, ""), targets, original, preview: None, report: Vec::new(), error: None, confirm: false, input_rect: ratatui::layout::Rect::default(), apply_rect: ratatui::layout::Rect::default(), scroll: 0, guided: None };
                    manager.report = batch_report(&batch);
                    manager.preview = Some(PropertyPreview::Batch(batch));
                    return Ok(manager);
                },
            };
            let mut input = crate::ui::make_popup_textarea(&self.app_theme, "");
            input.insert_str(text);
            Ok(PropertyManager {
                mode,
                input,
                targets,
                original,
                preview: None,
                report: Vec::new(),
                error: None,
                confirm: false,
                input_rect: ratatui::layout::Rect::default(),
                apply_rect: ratatui::layout::Rect::default(),
                scroll: 0,
                guided: match mode {
                    PropertyManagerMode::Definitions => Some(GuidedManager::Definitions {
                        definitions: self.property_definitions.clone(),
                        keys: self
                            .property_definitions
                            .properties
                            .keys()
                            .chain(self.notes.iter().flat_map(|note| note.properties.keys()))
                            .cloned()
                            .collect::<std::collections::BTreeSet<_>>()
                            .into_iter()
                            .collect(),
                        selected: 0,
                        form: None,
                    }),
                    PropertyManagerMode::Bulk => {
                        let mut form = ManagementForm::new(self, "", None)?;
                        form.enabled = true;
                        Some(GuidedManager::Bulk(Box::new(form)))
                    }
                    _ => None,
                },
            })
        })();
        match result {
            Ok(manager) => self.property_manager = Some(manager),
            Err(error) => self.set_temporary_status(&format!("{error:#}")),
        }
    }
    fn preview_property_management(&mut self, manager: &mut PropertyManager) -> Result<()> {
        manager.sync_guided()?;
        let text = manager.input.lines().join("\n");
        manager.report.clear();
        manager.confirm = false;
        manager.scroll = 0;
        match manager.mode {
            PropertyManagerMode::Definitions => {
                let definitions: PropertyDefinitions = toml::from_str(&text)?;
                definitions.check()?;
                for (key, definition) in &definitions.properties {
                    manager
                        .report
                        .push(format!("{key}: {}", definition.kind.label()));
                    for note in &self.notes {
                        if let Some(value) = note.properties.get(key)
                            && let Err(error) = value
                                .to_yaml()
                                .and_then(|value| definition.validate(&value))
                        {
                            manager
                                .report
                                .push(format!("  conflict {}: {error}", note.id));
                        }
                    }
                }
                for key in self
                    .property_definitions
                    .properties
                    .keys()
                    .filter(|key| !definitions.properties.contains_key(*key))
                {
                    manager.report.push(format!(
                        "Remove definition {key}; note values remain unchanged"
                    ));
                }
                manager.report.push(
                    "Apply definitions only; conflicting note values remain visible and unchanged."
                        .into(),
                );
                manager.preview = Some(PropertyPreview::Definitions {
                    text,
                    original: manager.original.clone(),
                });
            }
            PropertyManagerMode::Bulk => {
                let request: BulkInput = toml::from_str(&text)?;
                ensure!(
                    request.unset == request.value.is_none(),
                    "Set needs value; unset must omit value"
                );
                let value = request
                    .value
                    .as_ref()
                    .map(|value| {
                        if value.is_str() {
                            crate::property_model::parse_property_value(
                                request.kind,
                                value.as_str().unwrap_or_default(),
                            )
                        } else {
                            let value = crate::property_model::toml_to_yaml(value)?;
                            crate::property_model::validate_kind(request.kind, &value)?;
                            Ok(value)
                        }
                    })
                    .transpose()?;
                if let Some(value) = &value {
                    self.property_definitions.validate(&request.name, value)?;
                }
                let edit = crate::property_model::property_edit(&request.name, value.as_ref())?;
                let mut batch = self.prepare_property_batch(&manager.targets, &[edit])?;
                if let Some(value) = &value {
                    self.ensure_catalog_ready()?;
                    if let Some(definition) = crate::property_model::inferred_definition(
                        &self.property_definitions,
                        self.notes
                            .iter()
                            .filter(|note| !manager.targets.contains(&note.id)),
                        &request.name,
                        request.kind,
                        value,
                        &[],
                    )? {
                        let mut definitions = self.property_definitions.clone();
                        definitions
                            .properties
                            .insert(request.name.clone(), definition);
                        let path = PropertyDefinitions::path(&self.storage.data_dir);
                        let original = match std::fs::read_to_string(&path) {
                            Ok(text) => Some(text),
                            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                            Err(error) => return Err(error.into()),
                        };
                        let mut document: toml_edit::DocumentMut =
                            original.as_deref().unwrap_or("").parse()?;
                        let desired: toml_edit::DocumentMut =
                            toml_edit::ser::to_string_pretty(&definitions)?.parse()?;
                        for (key, item) in desired.iter() {
                            if let Some(existing) = document.get_mut(key) {
                                crate::config::merge::merge_edit_item(existing, item.clone());
                            } else {
                                document.insert(key, item.clone());
                            }
                        }
                        batch.bindings.push(BindingChange {
                            path,
                            create: original.is_none(),
                            before: original.unwrap_or_default(),
                            after: document.to_string(),
                        });
                    }
                }
                manager.report = batch_report(&batch);
                manager.report.push(format!(
                    "{} unique targets, {} changes, {} no-ops",
                    manager.targets.len(),
                    batch.notes.len(),
                    manager.targets.len().saturating_sub(batch.notes.len())
                ));
                for id in &manager.targets {
                    manager.report.push(format!(
                        "Target: {id}{}",
                        if batch.notes.iter().any(|change| &change.id == id) {
                            ""
                        } else {
                            " (no-op)"
                        }
                    ));
                }
                if manager.targets.iter().any(|id| id.ends_with(".clin")) {
                    manager.report.push("Encrypted metadata remains plaintext; encryption and Git history do not hide these properties.".into());
                }
                if !batch.bindings.is_empty() {
                    manager.report.push("Create vault definition: affects all notes; existing values are not rewritten automatically.".into());
                }
                manager.preview = Some(PropertyPreview::Batch(batch));
            }
            PropertyManagerMode::Rename { global } => {
                let request: RenameInput = toml::from_str(&text)?;
                crate::property_model::validate_key(&request.old)?;
                crate::property_model::validate_key(&request.new)?;
                if global {
                    let batch = self.prepare_property_rename(&request.old, &request.new)?;
                    manager.report = batch_report(&batch);
                    manager.preview = Some(PropertyPreview::Batch(batch));
                } else {
                    let header = self.editor.properties.current.clone();
                    frontmatter::rename_key(
                        header.as_deref().context("Property missing")?,
                        &request.old,
                        &request.new,
                    )?;
                    if let Some(definition) = self.property_definitions.properties.get(&request.new)
                    {
                        let values =
                            frontmatter::checked_parse(header.as_deref().unwrap_or_default())?;
                        definition.validate(
                            values
                                .extra
                                .get(serde_yaml_ng::Value::String(request.old.clone()))
                                .context("Property missing")?,
                        )?;
                    }
                    manager.report.push(format!(
                        "Rename {} -> {} in current note (autosave/recovery retained)",
                        request.old, request.new
                    ));
                    if self
                        .property_definitions
                        .properties
                        .contains_key(&request.old)
                        && !self
                            .property_definitions
                            .properties
                            .contains_key(&request.new)
                    {
                        manager.report.push("Copy reusable type definition to new key; old definition retained for other notes.".into());
                    }
                    manager.preview = Some(PropertyPreview::Rename {
                        old: request.old,
                        new: request.new,
                        header,
                    });
                }
            }
            PropertyManagerMode::Resume | PropertyManagerMode::References => {}
        }
        Ok(())
    }
    fn confirm_property_management(&mut self, manager: &mut PropertyManager) -> Result<bool> {
        let Some(preview) = &mut manager.preview else {
            return Ok(false);
        };
        match preview {
            PropertyPreview::References {
                operation,
                relocations,
                batch,
            } => {
                ensure!(
                    self.storage.prepare_reference_relocation(relocations)? == *batch,
                    "References changed since preview; cancel and preview again"
                );
                match operation {
                    ReferenceOperation::Convert { id, encrypt } => {
                        self.apply_note_conversion(id, *encrypt)?
                    }
                    ReferenceOperation::SaveDraft { intent } => {
                        ensure!(
                            self.title_save_intent()? == *intent,
                            "Draft or source changed since preview; cancel and preview again"
                        );
                        self.editor.reference_save_decision = Some((intent.clone(), true));
                        if let Err(error) = self.autosave() {
                            self.editor.reference_save_decision = Some((intent.clone(), false));
                            anyhow::bail!(error);
                        }
                    }
                    ReferenceOperation::Move { mode, target } => {
                        self.perform_move(mode.clone(), target)
                    }
                    ReferenceOperation::RenameNote { id, title } => {
                        self.storage.rename_note(id, title)?;
                        self.request_notes_reconcile();
                    }
                    ReferenceOperation::RenameFolder { old, new } => {
                        self.storage.rename_folder(old, new)?;
                        self.request_notes_reconcile();
                    }
                }
                Ok(true)
            }
            PropertyPreview::Definitions { text, original } => {
                let path = PropertyDefinitions::path(&self.storage.data_dir);
                let actual = match std::fs::read_to_string(&path) {
                    Ok(text) => Some(text),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                    Err(error) => return Err(error.into()),
                };
                ensure!(
                    &actual == original,
                    "Definitions changed since preview; reopen manager"
                );
                let definitions: PropertyDefinitions = toml::from_str(text)?;
                definitions.check()?;
                std::fs::create_dir_all(self.storage.data_dir.join(".clin"))?;
                crate::fsutil::atomic_write_str(&path, text)?;
                self.reload_property_definitions();
                self.editor
                    .properties
                    .set_definitions(&self.property_definitions);
                self.enqueue_backup("properties: definitions");
                self.set_temporary_status_static("Property definitions saved");
                Ok(true)
            }
            PropertyPreview::Batch(batch) => {
                let count = batch.notes.len();
                let errors = batch.apply(self)?;
                if errors.is_empty() {
                    self.set_temporary_status(&format!("Property batch applied ({count} notes)"));
                    Ok(true)
                } else {
                    manager.report = errors;
                    manager.report.push("Partial changes retained. Enter Apply retries remaining changes; pending batch stored in .clin/property_batch.toml.".into());
                    manager.confirm = false;
                    Ok(false)
                }
            }
            PropertyPreview::Rename { old, new, header } => {
                ensure!(
                    &self.editor.properties.current == header,
                    "Properties changed since preview; reopen rename"
                );
                if !self.property_definitions.properties.contains_key(new)
                    && let Some(definition) = self.property_definitions.properties.get(old)
                {
                    let mut definitions = self.property_definitions.clone();
                    definitions
                        .properties
                        .insert(new.clone(), definition.clone());
                    self.save_property_definitions(definitions)?;
                    self.editor
                        .properties
                        .set_definitions(&self.property_definitions);
                }
                self.editor.properties.selected = self
                    .editor
                    .properties
                    .rows
                    .iter()
                    .position(|row| row.key.as_str() == Some(old))
                    .context("Property missing")?;
                if self.editor.properties.rename(new)? {
                    self.mark_properties_modified();
                }
                Ok(true)
            }
        }
    }
    pub(crate) fn handle_property_manager_event(&mut self, event: crossterm::event::Event) -> bool {
        use crossterm::event::{
            Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
        };
        let Some(mut manager) = self.property_manager.take() else {
            return false;
        };
        let mut close = false;
        let mut preview = false;
        let mut apply = false;
        if manager.preview.is_none() && manager.guided.is_some() {
            match self.handle_guided_management(&mut manager, &event) {
                Ok(GuidedAction::Handled) => {
                    self.property_manager = Some(manager);
                    return true;
                }
                Ok(GuidedAction::Preview) => preview = true,
                Ok(GuidedAction::Pass) => {}
                Err(error) => {
                    manager.error = Some(format!("{error:#}"));
                    self.property_manager = Some(manager);
                    return true;
                }
            }
        } else if manager.preview.is_none()
            && matches!(&event, Event::Key(key) if key.code == KeyCode::F(2))
        {
            match self.enable_guided_management(&mut manager) {
                Ok(()) => manager.error = None,
                Err(error) => manager.error = Some(format!("{error:#}")),
            }
            self.property_manager = Some(manager);
            return true;
        }
        match event {
            _ if preview => {}
            Event::Paste(text) if manager.preview.is_none() => {
                manager.input.insert_str(text);
            }
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Esc => {
                    if manager.preview.is_some()
                        && !matches!(
                            manager.mode,
                            PropertyManagerMode::Resume | PropertyManagerMode::References
                        )
                    {
                        manager.preview = None;
                        manager.confirm = false;
                    } else {
                        close = true;
                    }
                }
                KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if manager.preview.is_some() {
                        apply = true;
                    } else {
                        preview = true;
                    }
                }
                KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if manager.preview.is_some() {
                        apply = true;
                    } else {
                        preview = true;
                    }
                }
                KeyCode::Tab | KeyCode::BackTab if manager.preview.is_some() => {
                    manager.confirm = !manager.confirm
                }
                KeyCode::Enter if manager.preview.is_some() && manager.confirm => apply = true,
                KeyCode::Up if manager.preview.is_some() => {
                    manager.scroll = manager.scroll.saturating_sub(1)
                }
                KeyCode::Down if manager.preview.is_some() => {
                    manager.scroll = manager.scroll.saturating_add(1)
                }
                _ if manager.preview.is_none() => {
                    crate::text_edit::feed_key(&self.keybinds, &mut manager.input, key)
                }
                _ => {}
            },
            Event::Mouse(mouse) => {
                if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                    if crate::events::contains_cell(manager.apply_rect, mouse.column, mouse.row) {
                        if manager.preview.is_some() {
                            apply = true;
                        } else {
                            preview = true;
                        }
                    } else if manager.preview.is_none() {
                        crate::events::move_textarea_cursor_to_mouse(
                            &mut manager.input,
                            manager.input_rect,
                            mouse.column,
                            mouse.row,
                            0,
                            0,
                        );
                    }
                } else if mouse.kind == MouseEventKind::ScrollDown {
                    manager.scroll = manager.scroll.saturating_add(3);
                } else if mouse.kind == MouseEventKind::ScrollUp {
                    manager.scroll = manager.scroll.saturating_sub(3);
                }
            }
            _ => {}
        }
        if close && matches!(manager.mode, PropertyManagerMode::References) {
            if let Some(PropertyPreview::References {
                operation: ReferenceOperation::SaveDraft { intent },
                ..
            }) = manager.preview
            {
                self.editor.reference_save_decision = Some((intent, false));
                self.editor.autosave_status = crate::editor::AutosaveStatus::Unsaved;
                self.editor.autosave_timer = None;
                self.set_temporary_status_static(
                    "Title rename cancelled; unsaved draft retained. Save to preview again.",
                );
            }
            return true;
        }
        let result = if preview {
            self.preview_property_management(&mut manager)
                .map(|_| false)
        } else if apply {
            self.confirm_property_management(&mut manager)
        } else {
            Ok(false)
        };
        match result {
            Ok(done) => {
                close |= done;
                manager.error = None;
            }
            Err(error) => manager.error = Some(format!("{error:#}")),
        }
        if !close {
            self.property_manager = Some(manager);
        }
        true
    }
}
fn batch_report(batch: &PropertyBatch) -> Vec<String> {
    let mut lines = vec![format!(
        "{} prepared relocations/writes, {} note headers, {} bindings. No new writes until Apply.",
        batch.writes.len(),
        batch.notes.len(),
        batch.bindings.len()
    )];
    if let Some(version) = batch.version {
        lines.push(format!(
            "Recovery version {version}; vault {}",
            batch
                .vault
                .as_ref()
                .map_or_else(|| "(missing)".into(), |path| path.display().to_string())
        ));
    }
    for change in &batch.writes {
        lines.push(format!(
            "Prepared {}: {} -> {} (encrypted recovery artifact)",
            if change.folder { "folder" } else { "file" },
            change.source,
            change.target
        ));
    }
    for (source, target) in &batch.parents {
        lines.push(format!(
            "Pending draft/subnote identity: {source} -> {target}"
        ));
    }
    for change in &batch.notes {
        lines.push(format!("--- {}", change.id));
        lines.push(format!(
            "Before:\n{}",
            change.before.as_deref().unwrap_or("(no header)")
        ));
        lines.push(format!("After:\n{}", change.after));
    }
    for change in &batch.bindings {
        lines.push(format!(
            "--- {}\nBefore:\n{}\nAfter:\n{}",
            change.path.display(),
            change.before,
            change.after
        ));
    }
    lines
}
pub(crate) fn draw_property_manager(frame: &mut ratatui::Frame, app: &mut App) {
    use ratatui::{
        layout::{Constraint, Layout},
        style::Style,
        widgets::{Block, Borders, Paragraph, Wrap},
    };
    let Some(manager) = &mut app.property_manager else {
        return;
    };
    let title = match manager.mode {
        PropertyManagerMode::Definitions => "PROPERTY DEFINITIONS",
        PropertyManagerMode::Bulk => "BULK PROPERTIES",
        PropertyManagerMode::Rename { global: true } => "RENAME PROPERTY ACROSS VAULT",
        PropertyManagerMode::Rename { global: false } => "RENAME PROPERTY",
        PropertyManagerMode::Resume => "RESUME PROPERTY BATCH",
        PropertyManagerMode::References => "UPDATE NOTE REFERENCES",
    };
    let hints = [
        (
            "Ctrl+S".into(),
            if manager.preview.is_some() {
                "apply"
            } else {
                "preview"
            },
        ),
        ("Tab".into(), "confirm control"),
        ("Esc".into(), "back/cancel"),
    ];
    let area = crate::ui::draw_popup_frame(
        frame,
        frame.area(),
        title,
        crate::ui::PopupSize::Large,
        crate::ui::PopupHints::Keybinds(&hints),
        &app.app_theme,
    );
    let areas = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    manager.input_rect = areas[0];
    manager.apply_rect = areas[1];
    if manager.preview.is_some() {
        let text = manager
            .report
            .iter()
            .flat_map(|line| line.lines())
            .map(|line| crate::fsutil::sanitize_for_terminal(line).to_string())
            .collect::<Vec<_>>()
            .join("\n");
        frame.render_widget(
            Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .scroll((manager.scroll, 0))
                .style(app.app_theme.bg_style()),
            areas[0],
        );
    } else if manager.guided.is_some() {
        draw_guided_management(frame, manager, &app.notes, &app.app_theme, areas[0]);
    } else {
        manager.input.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title("Advanced TOML (F2 guided; Ctrl+S preview)"),
        );
        frame.render_widget(&manager.input, areas[0]);
    }
    frame.render_widget(
        Paragraph::new(if manager.preview.is_some() {
            if manager.confirm {
                "> Apply changes <"
            } else {
                "[ Apply changes ]"
            }
        } else {
            "[ Preview changes ]"
        })
        .style(Style::default().fg(app.app_theme.accent)),
        areas[1],
    );
    frame.render_widget(
        Paragraph::new(manager.error.as_deref().unwrap_or(""))
            .style(Style::default().fg(app.app_theme.destructive)),
        areas[2],
    );
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TitleSaveIntent {
    pub id: String,
    pub title: String,
    pub content: String,
    pub edits: Vec<FrontmatterEdit>,
    pub before: Option<Vec<u8>>,
}
#[derive(Clone)]
pub(crate) enum ReferenceOperation {
    SaveDraft {
        intent: TitleSaveIntent,
    },
    Convert {
        id: String,
        encrypt: bool,
    },
    Move {
        mode: crate::popups::FolderPickerMode,
        target: String,
    },
    RenameNote {
        id: String,
        title: String,
    },
    RenameFolder {
        old: String,
        new: String,
    },
}
impl App {
    pub(crate) fn convert_note_with_preview(&mut self, id: &str, encrypt: bool) -> Result<()> {
        let editor_target = self.editor.editing_id.as_deref() == Some(id);
        ensure!(
            self.finish_pending_editor_save(),
            "Unsaved draft retained; finish save before conversion"
        );
        let id = if editor_target {
            self.editor.editing_id.clone().unwrap_or_else(|| id.into())
        } else {
            id.into()
        };
        if !self.preview_reference_operation(ReferenceOperation::Convert {
            id: id.clone(),
            encrypt,
        })? {
            self.apply_note_conversion(&id, encrypt)?;
        }
        Ok(())
    }
    fn apply_note_conversion(&mut self, id: &str, encrypt: bool) -> Result<()> {
        let target = if encrypt {
            self.storage.encrypt_note(id)?
        } else {
            self.storage.decrypt_note(id)?
        };
        self.refresh_note_single(Some(id), &target);
        if self.mode == ViewMode::Edit && self.editor.editing_id.as_deref() == Some(id) {
            self.load_and_open_note(&target, None);
        }
        self.set_temporary_status(&format!(
            "Note {}: {target}",
            if encrypt { "encrypted" } else { "decrypted" }
        ));
        self.enqueue_backup(if encrypt {
            "note: encrypt"
        } else {
            "note: decrypt"
        });
        Ok(())
    }
    pub(crate) fn title_save_intent(&self) -> Result<TitleSaveIntent> {
        let id = self.editor.editing_id.clone().context("No editor note")?;
        let mut title = crate::events::get_title_text(&self.editor.title_editor)
            .trim()
            .to_owned();
        if title.is_empty() {
            title = "Untitled note".into();
        }
        let before = match std::fs::read(self.storage.property_note_path(&id)?) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        Ok(TitleSaveIntent {
            id,
            title,
            content: self.editor.body.lines().join("\n"),
            edits: self.editor.properties.pending.clone(),
            before,
        })
    }
    pub(crate) fn preview_reference_operation(
        &mut self,
        operation: ReferenceOperation,
    ) -> Result<bool> {
        use std::collections::HashMap;
        let mut relocations = HashMap::new();
        let mut folders = Vec::new();
        match &operation {
            ReferenceOperation::Convert { id, encrypt } => {
                relocations.insert(id.clone(), self.storage.conversion_target(id, *encrypt)?);
            }
            ReferenceOperation::SaveDraft { intent } => {
                let target = self.storage.rename_note_target(&intent.id, &intent.title);
                if target != intent.id {
                    relocations.insert(intent.id.clone(), target);
                }
            }
            ReferenceOperation::RenameNote { id, title } => {
                let target = self.storage.rename_note_target(id, title);
                if &target != id {
                    relocations.insert(id.clone(), target);
                }
            }
            ReferenceOperation::RenameFolder { old, new } => {
                folders.push((old.clone(), new.clone()));
            }
            ReferenceOperation::Move { mode, target } => {
                use crate::popups::FolderPickerMode;
                let (notes, paths): (&[String], &[String]) = match mode {
                    FolderPickerMode::MoveNote { note_id } => (std::slice::from_ref(note_id), &[]),
                    FolderPickerMode::MoveFolder { folder_path } => {
                        (&[], std::slice::from_ref(folder_path))
                    }
                    FolderPickerMode::BulkMoveNotes { note_ids } => (note_ids, &[]),
                    FolderPickerMode::BulkMoveFolders { folder_paths } => (&[], folder_paths),
                    FolderPickerMode::BulkMoveMixed {
                        note_ids,
                        folder_paths,
                    } => (note_ids, folder_paths),
                    _ => return Ok(false),
                };
                for id in notes {
                    let file = std::path::Path::new(id)
                        .file_name()
                        .and_then(|file| file.to_str())
                        .context("Missing note filename")?;
                    let new = if target.is_empty() {
                        file.to_owned()
                    } else {
                        format!("{target}/{file}")
                    };
                    if &new != id {
                        relocations.insert(id.clone(), new);
                    }
                }
                for path in paths {
                    let base = path.rsplit('/').next().unwrap_or(path);
                    let new = if target.is_empty() {
                        base.to_owned()
                    } else {
                        format!("{target}/{base}")
                    };
                    folders.push((path.clone(), new));
                }
            }
        }
        if !folders.is_empty() {
            for id in self.storage.list_note_ids(true, false)? {
                for (old, new) in &folders {
                    if old != new
                        && let Some(suffix) = id.strip_prefix(&format!("{old}/"))
                    {
                        relocations.insert(id.clone(), format!("{new}/{suffix}"));
                    }
                }
            }
        }
        let batch = self.storage.prepare_reference_relocation(&relocations)?;
        if batch.notes.is_empty() {
            return Ok(false);
        }
        let mut report = vec![
            "Relocate note IDs and update declared references. Esc cancels without writes.".into(),
        ];
        if matches!(operation, ReferenceOperation::Convert { encrypt: true, .. }) {
            report.push("Encryption protects body only. YAML properties, definitions/defaults remain plaintext.".into());
        }
        let mut targets: Vec<_> = relocations.iter().collect();
        targets.sort();
        report.extend(
            targets
                .into_iter()
                .map(|(old, new)| format!("{old} -> {new}")),
        );
        report.extend(batch_report(&batch));
        self.property_manager = Some(PropertyManager {
            mode: PropertyManagerMode::References,
            input: crate::ui::make_popup_textarea(&self.app_theme, ""),
            targets: Vec::new(),
            original: None,
            preview: Some(PropertyPreview::References {
                operation,
                relocations,
                batch,
            }),
            report,
            error: None,
            confirm: false,
            input_rect: ratatui::layout::Rect::default(),
            apply_rect: ratatui::layout::Rect::default(),
            scroll: 0,
            guided: None,
        });
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn creation_resets_pending_state_applies_defaults_and_splits_initial_header() -> Result<()> {
        let _guard = crate::config::ConfigTestGuard::lock();
        let dir = tempfile::tempdir()?;
        crate::config::set_config_path_override(dir.path().join("config.toml"));
        let vault = dir.path().join("vault");
        std::fs::create_dir_all(vault.join(".clin"))?;
        std::fs::write(
            vault.join(".clin/properties.toml"),
            "[properties.status]\ntype = 'string'\ndefault = 'draft'\n",
        )?;
        std::fs::write(
            vault.join("old.md"),
            "---\ntitle: Old\nold: true\n---\nOld body",
        )?;
        let storage = crate::storage::Storage {
            data_dir: vault.clone(),
            notes_dir: vault.clone(),
            config_dir: dir.path().into(),
            templates_dir: vault.join(".clin/templates"),
            key: [0; 32],
            skip_dir_patterns: Vec::new(),
            rename_on_title_change: false,
        };
        let mut app = App::new(storage)?;
        app.load_and_open_note("old.md", None);
        app.editor.properties.commit(
            crate::property_model::property_edit("stale", Some(&serde_yaml_ng::Value::Bool(true)))?,
            true,
        )?;
        app.editor.properties.begin_add(&app.app_theme);
        app.editor.template_edit_path = Some(dir.path().join("stale.toml"));
        app.start_note_with_content(String::new(), "Fresh".into(), "Fresh body".into());
        assert!(app.editor.template_edit_path.is_none());
        assert!(app.editor.properties.dialog.is_none());
        assert_eq!(
            app.editor.properties.values.get("status"),
            Some(&PropertyValue::String("draft".into()))
        );
        assert!(!app.editor.properties.values.contains_key("stale"));
        assert!(!app.editor.properties.values.contains_key("old"));
        assert!(app.storage.editor_draft_path().exists());
        app.autosave().map_err(anyhow::Error::msg)?;
        let id = app.editor.editing_id.clone().expect("new note");
        assert_eq!(app.storage.load_note(&id)?.content, "Fresh body");
        app.start_note_with_content(
            String::new(),
            "Header".into(),
            "---\nstatus: explicit # retain\n---\nSeparated body".into(),
        );
        assert_eq!(app.editor.body.lines().join("\n"), "Separated body");
        assert_eq!(
            app.editor.properties.values.get("status"),
            Some(&PropertyValue::String("explicit".into()))
        );
        let id = app.editor.editing_id.clone().expect("header note");
        assert!(
            app.storage
                .load_frontmatter(&id)?
                .expect("header")
                .contains("# retain")
        );
        let before = app.editor.editing_id.clone();
        app.property_definitions_error = Some("invalid definitions".into());
        app.start_blank_note_with_title(String::new(), "Rejected".into());
        assert_eq!(app.editor.editing_id, before);
        Ok(())
    }

    #[test]
    fn bulk_preflight_noops_and_partial_retry_preserve_notes() -> Result<()> {
        let _guard = crate::config::ConfigTestGuard::lock();
        let dir = tempfile::tempdir()?;
        let config_path = dir.path().join("config.toml");
        std::fs::write(&config_path, "[features]\nbackup = false\n")?;
        crate::config::set_config_path_override(config_path);
        let vault = dir.path().join("vault");
        std::fs::create_dir_all(&vault)?;
        for (name, text) in [
            ("a.md", "---\ntitle: A\nstatus: old # retain\n---\nBody A."),
            ("b.md", "---\ntitle: B\nstatus: old\n---\nBody B."),
            ("plain.txt", "Plain body."),
        ] {
            std::fs::write(vault.join(name), text)?;
        }
        let storage = crate::storage::Storage {
            data_dir: vault.clone(),
            notes_dir: vault.clone(),
            config_dir: dir.path().into(),
            templates_dir: vault.join(".clin/templates"),
            key: [0; 32],
            skip_dir_patterns: Vec::new(),
            rename_on_title_change: false,
        };
        let mut app = App::new(storage)?;
        app.ensure_catalog_ready()?;
        app.open_property_manager(PropertyManagerMode::Definitions, None);
        app.handle_property_manager_event(Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));
        if let Some(PropertyManager {
            guided:
                Some(GuidedManager::Definitions {
                    form: Some(form), ..
                }),
            ..
        }) = &mut app.property_manager
        {
            assert_eq!(form.key(), "status");
            form.kind = PropertyKind::Select;
            form.options.insert_str("draft\nreview");
        } else {
            anyhow::bail!("Guided adoption missing");
        }
        app.handle_property_manager_event(Event::Key(KeyEvent::new(
            KeyCode::Char('s'),
            KeyModifiers::CONTROL,
        )));
        assert!(
            app.property_manager
                .as_ref()
                .expect("manager")
                .report
                .iter()
                .any(|line| line.contains("conflict a.md"))
        );
        assert!(!PropertyDefinitions::path(&vault).exists());
        app.handle_property_manager_event(Event::Key(KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        )));
        app.handle_property_manager_event(Event::Key(KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        )));
        app.handle_property_manager_event(Event::Key(KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        )));
        assert!(app.property_manager.is_none());
        app.open_property_manager(PropertyManagerMode::Bulk, Some("a.md"));
        if let Some(PropertyManager {
            guided: Some(GuidedManager::Bulk(form)),
            ..
        }) = &mut app.property_manager
        {
            form.name.insert_str("status");
            form.value.insert_str("new");
        } else {
            anyhow::bail!("Guided bulk missing");
        }
        app.handle_property_manager_event(Event::Key(KeyEvent::new(
            KeyCode::Char('s'),
            KeyModifiers::CONTROL,
        )));
        assert!(
            app.property_manager
                .as_ref()
                .expect("preview")
                .preview
                .is_some()
        );
        assert!(std::fs::read_to_string(vault.join("a.md"))?.contains("status: old"));
        app.handle_property_manager_event(Event::Key(KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        )));
        app.handle_property_manager_event(Event::Key(KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        )));
        assert!(app.property_manager.is_none());
        let unset = crate::property_model::property_edit("absent", None)?;
        let batch = app.prepare_property_batch(&["plain.txt".into()], &[unset])?;
        assert_eq!(batch.notes.len(), 0);
        assert_eq!(std::fs::read(vault.join("plain.txt"))?, b"Plain body.");
        let edit = crate::property_model::property_edit(
            "status",
            Some(&serde_yaml_ng::Value::String("new".into())),
        )?;
        let mut batch = app.prepare_property_batch(&["a.md".into(), "b.md".into()], &[edit])?;
        assert!(std::fs::read_to_string(vault.join("a.md"))?.contains("status: old"));
        let original_b = std::fs::read_to_string(vault.join("b.md"))?;
        std::fs::write(
            vault.join("b.md"),
            original_b.replace("status: old", "status: external"),
        )?;
        let failures = batch.apply(&mut app)?;
        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("b.md"));
        assert_eq!(batch.notes.len(), 1);
        let a = std::fs::read_to_string(vault.join("a.md"))?;
        assert!(a.contains("status: new # retain"));
        assert!(a.ends_with("Body A."));
        assert!(std::fs::read_to_string(vault.join("b.md"))?.contains("status: external"));
        let recovery = PropertyBatch::recovery_path(&app);
        let mut pending: PropertyBatch = toml::from_str(&std::fs::read_to_string(&recovery)?)?;
        assert_eq!(pending, batch);
        assert!(app.prepare_property_batch(&["a.md".into()], &[]).is_err());
        std::fs::write(vault.join("b.md"), original_b)?;
        assert_eq!(pending.apply(&mut app)?.len(), 0);
        assert!(!recovery.exists());
        assert!(std::fs::read_to_string(vault.join("b.md"))?.contains("status: new"));
        assert!(std::fs::read_to_string(vault.join("b.md"))?.ends_with("Body B."));
        Ok(())
    }

    #[test]
    fn refresh_preserves_note_ids_search_generation_and_selected_goal_count() -> Result<()> {
        let _guard = crate::config::ConfigTestGuard::lock();
        let dir = tempfile::tempdir()?;
        crate::config::set_config_path_override(dir.path().join("config.toml"));
        let vault = dir.path().join("vault");
        std::fs::create_dir_all(&vault)?;
        for (name, text) in [
            (
                "a.md",
                "---\ntitle: A\npriority: 1\ngoal: 100\nstatus: done\n---\none two three four",
            ),
            (
                "b.md",
                "---\ntitle: B\npriority: 2\ngoal: 50\n---\nother body",
            ),
        ] {
            std::fs::write(vault.join(name), text)?;
        }
        let storage = crate::storage::Storage {
            data_dir: vault.clone(),
            notes_dir: vault.clone(),
            config_dir: dir.path().into(),
            templates_dir: vault.join(".clin/templates"),
            key: [0; 32],
            skip_dir_patterns: Vec::new(),
            rename_on_title_change: false,
        };
        let mut app = App::new(storage)?;
        app.ensure_catalog_ready()?;
        app.config.goals.note_word_goal_property = Some("goal".into());
        app.config.list.property_sort_key = Some("priority".into());
        app.list.sort_field = crate::list_view::SortField::Property;
        app.refresh_property_dependents();
        let row = app.list.visual_list.iter().position(|item| matches!(item, crate::list_view::VisualItem::Note { summary_idx, .. } if app.notes[*summary_idx].id == "a.md")).expect("A row");
        app.list.visual_index = row;
        app.list.selected_indices.insert(row);
        app.request_preview_update();
        let mut context = crate::statusline::StatuslineContext::for_view(&app, ViewMode::List);
        context.note = crate::statusline::active_note(&app, ViewMode::List);
        assert_eq!(
            context.resolve("note_goal_progress").as_deref(),
            Some("4/100")
        );
        app.list.sort_order = crate::list_view::SortOrder::Ascending;
        app.refresh_property_dependents();
        assert_eq!(app.get_selected_note_id().as_deref(), Some("a.md"));
        assert_eq!(app.property_targets(), ["a.md"]);
        app.begin_search();
        if let Some(crate::popups::ActivePopup::Search(popup)) = &mut app.popups.active {
            popup.input.insert_str("prop:status=done");
        }
        app.update_search();
        let generation = app
            .search_query_generation
            .load(std::sync::atomic::Ordering::SeqCst);
        std::fs::create_dir_all(vault.join(".clin"))?;
        std::fs::write(
            vault.join(".clin/properties.toml"),
            "[properties.status]\ntype = 'boolean'\n",
        )?;
        app.reload_property_definitions();
        assert!(
            app.search_query_generation
                .load(std::sync::atomic::Ordering::SeqCst)
                > generation
        );
        assert!(app.search_status.is_some());
        app.popups.active = None;
        let changed = std::fs::read_to_string(vault.join("a.md"))?
            .replace("priority: 1", "priority: 9")
            .replace("one two three four", "one two");
        std::fs::write(vault.join("a.md"), changed)?;
        app.refresh_note_single(None, "a.md");
        assert_eq!(app.get_selected_note_id().as_deref(), Some("a.md"));
        let mut context = crate::statusline::StatuslineContext::for_view(&app, ViewMode::List);
        context.note = crate::statusline::active_note(&app, ViewMode::List);
        assert_eq!(
            context.resolve("note_goal_progress").as_deref(),
            Some("2/100")
        );
        app.notes
            .iter_mut()
            .find(|note| note.id == "a.md")
            .expect("A")
            .id = "a.clin".into();
        app.preview_encryption = true;
        app.refresh_property_dependents();
        let row = app.list.visual_list.iter().position(|item| matches!(item, crate::list_view::VisualItem::Note { summary_idx, .. } if app.notes[*summary_idx].id == "a.clin")).expect("encrypted row");
        app.list.visual_index = row;
        app.request_preview_update();
        let mut context = crate::statusline::StatuslineContext::for_view(&app, ViewMode::List);
        context.note = crate::statusline::active_note(&app, ViewMode::List);
        assert_eq!(
            context.resolve("note_goal_progress").as_deref(),
            Some("—/100")
        );
        Ok(())
    }

    #[test]
    fn reference_preview_cancel_apply_backlinks_and_graph_union() -> Result<()> {
        let _guard = crate::config::ConfigTestGuard::lock();
        let dir = tempfile::tempdir()?;
        crate::config::set_config_path_override(dir.path().join("config.toml"));
        let vault = dir.path().join("vault");
        std::fs::create_dir_all(vault.join(".clin"))?;
        std::fs::write(
            vault.join(".clin/properties.toml"),
            "[properties.parent]\ntype = 'note_reference'\n",
        )?;
        std::fs::write(
            vault.join("target.md"),
            "---\ntitle: Target\nupdated_at: 1\n---\ntarget",
        )?;
        std::fs::write(
            vault.join("child.md"),
            "---\ntitle: Child\nupdated_at: 1\nparent: target.md\nraw: '[[Ghost]]'\n---\n[[Target]]",
        )?;
        let storage = crate::storage::Storage {
            data_dir: vault.clone(),
            notes_dir: vault.clone(),
            config_dir: dir.path().into(),
            templates_dir: vault.join(".clin/templates"),
            key: [0; 32],
            skip_dir_patterns: Vec::new(),
            rename_on_title_change: false,
        };
        let mut app = App::new(storage)?;
        app.ensure_catalog_ready()?;
        let child = app
            .notes
            .iter()
            .find(|note| note.id == "child.md")
            .expect("child");
        assert_eq!(child.property_links, ["target.md"]);
        let specs = crate::graf_adapter::note_specs(&app.notes, &app.config.features);
        assert_eq!(
            specs
                .iter()
                .find(|note| note.id == "child.md")
                .expect("child")
                .links,
            ["target.md"]
        );
        app.load_and_open_note("target.md", None);
        let links = app.compute_links();
        assert!(
            links
                .iter()
                .any(|link| link.id == "child.md" && link.is_backlink && link.is_property)
        );
        app.mode = ViewMode::List;
        let operation = ReferenceOperation::Move {
            mode: crate::popups::FolderPickerMode::MoveNote {
                note_id: "target.md".into(),
            },
            target: "archive".into(),
        };
        let original = std::fs::read(vault.join("child.md"))?;
        assert!(app.preview_reference_operation(operation.clone())?);
        assert_eq!(std::fs::read(vault.join("child.md"))?, original);
        app.handle_property_manager_event(Event::Key(KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        )));
        assert!(app.property_manager.is_none());
        assert!(vault.join("target.md").exists());
        assert!(app.preview_reference_operation(operation)?);
        app.handle_property_manager_event(Event::Key(KeyEvent::new(
            KeyCode::Tab,
            KeyModifiers::NONE,
        )));
        app.handle_property_manager_event(Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));
        assert!(app.property_manager.is_none());
        assert!(vault.join("archive/target.md").exists());
        let values = frontmatter::checked_parse(
            &app.storage.load_frontmatter("child.md")?.expect("header"),
        )?
        .extra;
        assert_eq!(values["parent"].as_str(), Some("archive/target.md"));
        assert_eq!(values["raw"].as_str(), Some("[[Ghost]]"));
        Ok(())
    }
}
