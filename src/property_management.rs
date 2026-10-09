use crate::app::{App, ViewMode};
use crate::frontmatter::{self, FrontmatterEdit};
use crate::property_model::{PropertyDefinitions, PropertyKind, PropertyValue};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct HeaderChange {
    pub id: String,
    pub before: Option<String>,
    pub after: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct BindingChange {
    pub path: std::path::PathBuf,
    pub before: String,
    pub after: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct PropertyBatch {
    pub notes: Vec<HeaderChange>,
    pub bindings: Vec<BindingChange>,
}
impl PropertyBatch {
    fn recovery_path(app: &App) -> std::path::PathBuf {
        app.storage
            .data_dir
            .join(".clin")
            .join("property_batch.toml")
    }
    pub fn save_recovery(&self, app: &App) -> Result<()> {
        std::fs::create_dir_all(app.storage.data_dir.join(".clin"))?;
        crate::fsutil::atomic_write_str(&Self::recovery_path(app), &toml::to_string(self)?)
    }
    pub fn apply(&mut self, app: &mut App) -> Result<Vec<String>> {
        self.save_recovery(app)?;
        let mut failures = Vec::new();
        self.notes.retain(|change| {
            match app.storage.replace_property_header(
                &change.id,
                change.before.as_deref(),
                &change.after,
            ) {
                Ok(()) => false,
                Err(error) => {
                    failures.push(format!("{}: {error:#}", change.id));
                    true
                }
            }
        });
        if self.notes.is_empty() {
            let config_path = crate::config::ClinConfig::config_path()?;
            let vault = std::fs::canonicalize(&app.storage.data_dir)?;
            self.bindings.retain(|change| {
                let result = (|| -> Result<()> {
                    let is_config = change.path == config_path;
                    let parent = std::fs::canonicalize(
                        change.path.parent().context("Missing binding parent")?,
                    )?;
                    ensure!(
                        is_config || parent.starts_with(&vault),
                        "Binding target outside vault"
                    );
                    let actual = std::fs::read_to_string(&change.path)?;
                    if actual == change.after {
                        return Ok(());
                    }
                    ensure!(
                        actual == change.before,
                        "Binding changed since preview; inspect and retry"
                    );
                    crate::fsutil::atomic_write_str(&change.path, &change.after)
                })();
                match result {
                    Ok(()) => false,
                    Err(error) => {
                        failures.push(format!("{}: {error:#}", change.path.display()));
                        true
                    }
                }
            });
        }
        if self.notes.is_empty() && self.bindings.is_empty() {
            crate::fsutil::remove_file_if_exists(&Self::recovery_path(app))?;
        } else {
            self.save_recovery(app)?;
        }
        app.request_notes_reconcile();
        app.reload_config();
        app.reload_property_definitions();
        app.enqueue_backup("properties: batch edit");
        Ok(failures)
    }
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
        self.sort_notes();
        self.rebuild_note_index();
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
        self.graph_preview = None;
        self.refresh_visual_list();
        self.list.pending_preview_update = true;
        for note in &self.notes {
            for warning in self
                .property_definitions
                .diagnostics(&note.properties)
                .into_iter()
                .take(3)
            {
                self.messages.push(
                    format!("{}: {warning}", note.id),
                    crate::app::messages::MessageSeverity::Warning,
                );
            }
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
        if self.mode == ViewMode::Edit {
            self.autosave().map_err(anyhow::Error::msg)?;
        }
        ensure!(
            self.property_definitions_error.is_none(),
            "Repair property definitions before bulk editing"
        );
        let mut changes = Vec::new();
        for id in ids {
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
            if before.as_deref() != Some(after.as_str()) {
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
        })
    }
    pub(crate) fn prepare_property_rename(
        &mut self,
        old: &str,
        new: &str,
    ) -> Result<PropertyBatch> {
        if self.mode == ViewMode::Edit {
            self.autosave().map_err(anyhow::Error::msg)?;
        }
        self.ensure_catalog_ready()?;
        crate::property_model::validate_key(old)?;
        crate::property_model::validate_key(new)?;
        ensure!(old != new, "Choose a different property name");
        ensure!(
            !self.property_definitions.properties.contains_key(new),
            "Target definition already exists"
        );
        let mut batch = PropertyBatch::default();
        for note in &self.notes {
            if !matches!(
                std::path::Path::new(&note.id)
                    .extension()
                    .and_then(|value| value.to_str()),
                Some("md" | "txt" | "clin")
            ) {
                continue;
            }
            let before = self.storage.load_frontmatter(&note.id)?;
            if let Some(header) = &before {
                let mapping = frontmatter::validate_header(header)?;
                if mapping.contains_key(serde_yaml_ng::Value::String(old.into())) {
                    let after = frontmatter::rename_key(header, old, new)?;
                    batch.notes.push(HeaderChange {
                        id: note.id.clone(),
                        before,
                        after,
                    });
                }
            }
        }
        let definition_path = PropertyDefinitions::path(&self.storage.data_dir);
        if definition_path.exists() {
            let before = std::fs::read_to_string(&definition_path)?;
            let mut doc: toml_edit::DocumentMut = before.parse()?;
            if let Some(table) = doc
                .get_mut("properties")
                .and_then(toml_edit::Item::as_table_mut)
                && let Some(value) = table.remove(old)
            {
                table.insert(new, value);
            }
            let after = doc.to_string();
            if before != after {
                batch.bindings.push(BindingChange {
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
                let mut doc: toml_edit::DocumentMut = before
                    .parse()
                    .with_context(|| format!("Invalid template {}", path.display()))?;
                if let Some(table) = doc
                    .get_mut("properties")
                    .and_then(toml_edit::Item::as_table_mut)
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
                replace_document_tokens(&mut doc, old, new);
                let after = doc.to_string();
                if before != after {
                    batch.bindings.push(BindingChange {
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
}
pub(crate) enum PropertyPreview {
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
                PropertyManagerMode::Resume => {
                    let batch: PropertyBatch = toml::from_str(&std::fs::read_to_string(PropertyBatch::recovery_path(self)).context("No pending property batch")?)?;
                    let mut manager = PropertyManager { mode, input: crate::ui::make_popup_textarea(&self.app_theme, ""), targets, original, preview: None, report: Vec::new(), error: None, confirm: false, input_rect: Default::default(), apply_rect: Default::default(), scroll: 0 };
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
                input_rect: Default::default(),
                apply_rect: Default::default(),
                scroll: 0,
            })
        })();
        match result {
            Ok(manager) => self.property_manager = Some(manager),
            Err(error) => self.set_temporary_status(&format!("{error:#}")),
        }
    }
    fn preview_property_management(&mut self, manager: &mut PropertyManager) -> Result<()> {
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
                        if matches!(
                            request.kind,
                            PropertyKind::Yaml
                                | PropertyKind::List
                                | PropertyKind::MultiSelect
                                | PropertyKind::NoteReferences
                        ) && value.is_str()
                        {
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
                let batch = self.prepare_property_batch(&manager.targets, &[edit])?;
                manager.report = batch_report(&batch);
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
            PropertyManagerMode::Resume => {}
        }
        Ok(())
    }
    fn confirm_property_management(&mut self, manager: &mut PropertyManager) -> Result<bool> {
        let Some(preview) = &mut manager.preview else {
            return Ok(false);
        };
        match preview {
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
                    definitions.save(&self.storage.data_dir)?;
                    self.property_definitions = definitions;
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
        match event {
            Event::Paste(text) if manager.preview.is_none() => {
                manager.input.insert_str(text);
            }
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Esc => {
                    if manager.preview.is_some()
                        && !matches!(manager.mode, PropertyManagerMode::Resume)
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
        "{} note changes, {} binding changes. No writes until Apply.",
        batch.notes.len(),
        batch.bindings.len()
    )];
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
    } else {
        manager.input.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title("TOML (Ctrl+S previews; no writes yet)"),
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
