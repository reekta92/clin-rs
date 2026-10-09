use anyhow::{Context, Result, bail, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui_textarea::TextArea;
use serde_yaml_ng::Value;
use std::str::FromStr;
use yaml_edit::YamlFile;

use crate::app::{App, EditFocus, ViewMode};
use crate::app_theme::AppThemeColors;
use crate::frontmatter::{self, FrontmatterEdit};

use crate::property_model::PropertyKind as PropertyType;

pub(crate) struct PropertyRow {
    pub(crate) key_yaml: String,
    pub(crate) key: Value,
    pub(crate) value_yaml: String,
    pub(crate) value: Value,
    pub(crate) kind: PropertyType,
}
impl PropertyRow {
    pub(crate) fn name(&self) -> &str {
        self.key.as_str().unwrap_or_else(|| self.key_yaml.trim())
    }
    pub(crate) fn summary(&self) -> &str {
        self.value
            .as_str()
            .unwrap_or_else(|| self.value_yaml.trim())
            .lines()
            .next()
            .unwrap_or("null")
    }
}

pub(crate) struct PropertyDialog {
    pub(crate) key_yaml: Option<String>,
    pub(crate) name: TextArea<'static>,
    pub(crate) value: TextArea<'static>,
    pub(crate) kind: PropertyType,
    pub(crate) control: usize, // Name / Type / Value / Apply
    pub(crate) error: Option<String>,
    pub(crate) rects: [Rect; 4],
    pub(crate) choices: Vec<String>,
    pub(crate) choice: usize,
    pub(crate) choice_rect: Rect,
    pub(crate) choice_view_start: usize,
    definition_offer: Option<crate::property_model::PropertyDefinition>,
    definition_confirm: bool,
}
enum TypedPropertyCommit {
    Applied(bool),
    Offer(crate::property_model::PropertyDefinition),
}
impl PropertyDialog {
    fn advance(&mut self, direction: isize) {
        loop {
            self.control = (self.control as isize + direction).rem_euclid(4) as usize;
            if !(self.control == 0 && self.key_yaml.is_some()
                || self.control == 2 && self.kind == PropertyType::Null)
            {
                break;
            }
        }
    }
    fn edit(&self) -> Result<FrontmatterEdit> {
        let key_yaml = if let Some(key) = &self.key_yaml {
            key.clone()
        } else {
            let name = self.name.lines().join("\n");
            ensure!(
                !name.trim().is_empty() && !name.contains(['\r', '\n']),
                "Name must be nonempty and single-line"
            );
            serde_yaml_ng::to_string(&Value::String(name))?
        };
        Ok(FrontmatterEdit {
            key_yaml,
            value_yaml: Some(self.kind.encode(&self.value.lines().join("\n"))?),
            rename_from: None,
        })
    }
    fn picker(&self) -> bool {
        matches!(
            self.kind,
            PropertyType::Select
                | PropertyType::MultiSelect
                | PropertyType::NoteReference
                | PropertyType::NoteReferences
        )
    }
    fn toggle_boolean(&mut self) {
        let value = if self
            .value
            .lines()
            .first()
            .is_some_and(|value| value.trim() == "true")
        {
            "false"
        } else {
            "true"
        };
        self.value.select_all();
        self.value.insert_str(value);
    }
    fn choose(&mut self) {
        let Some(value) = self.choices.get(self.choice) else {
            return;
        };
        if matches!(
            self.kind,
            PropertyType::MultiSelect | PropertyType::NoteReferences
        ) {
            let encoded = self.value.lines().join("\n");
            let mut values: Vec<String> =
                match crate::property_model::parse_property_value(self.kind, &encoded) {
                    Ok(Value::Sequence(values)) => values
                        .into_iter()
                        .filter_map(|value| {
                            if let Value::String(value) = value {
                                Some(value)
                            } else {
                                None
                            }
                        })
                        .collect(),
                    Ok(_) => return,
                    Err(error) => {
                        self.error = Some(error.to_string());
                        return;
                    }
                };
            if values.iter().any(|candidate| candidate == value) {
                values.retain(|candidate| candidate != value);
            } else {
                values.push(value.clone());
            }
            self.value.select_all();
            self.value.insert_str(values.join("\n"));
        } else {
            self.value.select_all();
            self.value.insert_str(value);
        }
    }
}

pub(crate) enum PropertiesDialog {
    Edit(Box<PropertyDialog>),
    Delete {
        key_yaml: String,
        yes: bool,
        rects: [Rect; 2],
    },
}

#[derive(Default)]
pub(crate) struct PropertiesState {
    pub(crate) baseline: Option<String>,
    pub(crate) current: Option<String>,
    pub(crate) rows: Vec<PropertyRow>,
    pub(crate) selected: usize,
    pub(crate) scroll: usize,
    pub(crate) focused: bool,
    pub(crate) pending: Vec<FrontmatterEdit>,
    pub(crate) revision: u64,
    pub(crate) error: Option<String>,
    pub(crate) dialog: Option<PropertiesDialog>,
    pub(crate) focus_request: Option<EditFocus>,
    pub(crate) last_click: Option<(usize, std::time::Instant)>,
    pub(crate) visible_rows: usize,
    pub(crate) definitions: crate::property_model::PropertyDefinitions,
    pub(crate) known_keys: Vec<String>,
    renamed_keys: std::collections::HashSet<String>,
    pub(crate) values: crate::property_model::PropertyMap,
}
impl PropertiesState {
    pub(crate) fn load(
        header: Result<Option<String>>,
        definitions: &crate::property_model::PropertyDefinitions,
    ) -> Self {
        let mut state = Self {
            definitions: definitions.clone(),
            ..Default::default()
        };
        match header {
            Ok(header) => {
                state.baseline = header.clone();
                state.current = header;
                if let Err(error) = state.rebuild() {
                    state.error = Some(error.to_string());
                }
            }
            Err(error) => state.error = Some(error.to_string()),
        }
        state
    }
    fn rebuild(&mut self) -> Result<()> {
        let header = self.current.as_deref().unwrap_or("---\n---\n");
        let semantic = frontmatter::validate_header(header)?;
        let (Some(header), _) = frontmatter::split_header(header.as_bytes())? else {
            bail!("Missing header")
        };
        let opening = if header.starts_with("---\r\n") { 5 } else { 4 };
        let end = header.trim_end_matches(['\r', '\n']).len() - 3;
        let file = YamlFile::from_str(&header[opening..end])?;
        let mut rows = Vec::with_capacity(semantic.len());
        if let Some(mapping) = file.document().and_then(|document| document.as_mapping()) {
            for entry in mapping.entries() {
                let key_yaml = entry
                    .key_node()
                    .context("Unsupported property key")?
                    .to_string();
                let key: Value =
                    serde_yaml_ng::from_str(&key_yaml).context("Unsupported property key")?;
                if frontmatter::is_managed_key(&key) {
                    continue;
                }
                let value = semantic
                    .get(&key)
                    .context("Unsupported property syntax")?
                    .clone();
                let value_node = entry.value_node();
                let value_yaml = value_node
                    .as_ref()
                    .map_or_else(|| "null".into(), ToString::to_string);
                let mut kind = key
                    .as_str()
                    .and_then(|name| self.definitions.properties.get(name))
                    .map_or_else(|| PropertyType::of(&value), |definition| definition.kind);
                if value_node.is_some_and(|node| {
                    matches!(
                        node,
                        yaml_edit::YamlNode::Alias(_) | yaml_edit::YamlNode::TaggedNode(_)
                    )
                }) {
                    kind = PropertyType::Yaml;
                }
                rows.push(PropertyRow {
                    key_yaml,
                    key,
                    value_yaml,
                    value,
                    kind,
                });
            }
        }
        self.rows = rows;
        self.values = crate::property_model::properties_from_mapping(&semantic);
        self.selected = self.selected.min(self.rows.len()); // Add row follows entries.
        self.scroll = self.scroll.min(self.rows.len());
        self.error = None;
        Ok(())
    }
    pub(crate) fn refresh(
        &mut self,
        header: Result<Option<String>>,
        committed: bool,
    ) -> Result<()> {
        if committed {
            self.pending.clear();
            self.renamed_keys.clear();
            // Keep committed snapshot even when the post-write read fails.
            self.baseline = self.current.clone();
        }
        let header = header?;
        // Retain transaction order while renamed keys have pending edits.
        self.renamed_keys.extend(
            self.pending
                .iter()
                .filter(|edit| edit.rename_from.is_some())
                .filter_map(|edit| {
                    serde_yaml_ng::from_str::<Value>(&edit.key_yaml)
                        .ok()
                        .and_then(|key| key.as_str().map(str::to_owned))
                }),
        );
        let current = if self.pending.is_empty() {
            header.clone()
        } else {
            Some(frontmatter::apply_edits(
                header.as_deref().unwrap_or("---\n---\n"),
                &self.pending,
            )?)
        };
        self.baseline = header;
        self.current = current;
        self.rebuild()
    }
    pub(crate) fn commit(&mut self, edit: FrontmatterEdit, new: bool) -> Result<bool> {
        ensure!(
            self.error.is_none(),
            "Structured properties disabled: {}",
            self.error.as_deref().unwrap_or("")
        );
        let key: Value = serde_yaml_ng::from_str(&edit.key_yaml)?;
        ensure!(
            !frontmatter::is_managed_key(&key),
            "Managed by Clin; use existing note controls"
        );
        let before = frontmatter::validate_header(self.current.as_deref().unwrap_or("---\n---\n"))?;
        ensure!(
            !new || !before.contains_key(&key),
            "Property already exists"
        );
        let original =
            frontmatter::validate_header(self.baseline.as_deref().unwrap_or("---\n---\n"))?;
        let mut pending = self.pending.clone();
        if !key
            .as_str()
            .is_some_and(|key| self.renamed_keys.contains(key))
        {
            pending.retain(|delta| {
                serde_yaml_ng::from_str::<Value>(&delta.key_yaml)
                    .ok()
                    .as_ref()
                    != Some(&key)
            });
        }
        pending.push(edit);
        let candidate =
            frontmatter::apply_edits(self.baseline.as_deref().unwrap_or("---\n---\n"), &pending)?;
        let after = frontmatter::validate_header(&candidate)?;
        if before == after {
            return Ok(false);
        }
        if !key
            .as_str()
            .is_some_and(|key| self.renamed_keys.contains(key))
            && original.get(&key) == after.get(&key)
        {
            pending.pop();
        }
        let candidate =
            frontmatter::apply_edits(self.baseline.as_deref().unwrap_or("---\n---\n"), &pending)?;
        // Build a fresh tree, never clone mutable CST for rollback.
        let mut view = Self {
            current: Some(candidate),
            definitions: self.definitions.clone(),
            ..Default::default()
        };
        view.rebuild()?;
        ensure!(
            view.error.is_none(),
            "{}",
            view.error.as_deref().unwrap_or("Invalid properties")
        );
        self.current = view.current;
        self.rows = std::mem::take(&mut view.rows);
        self.values = std::mem::take(&mut view.values);
        self.pending = pending;
        self.selected = self.selected.min(self.rows.len());
        self.scroll = self.scroll.min(self.rows.len());
        self.revision = self.revision.wrapping_add(1);
        for row in &mut self.rows {
            if row.kind != PropertyType::Yaml
                && let Some(definition) = row
                    .key
                    .as_str()
                    .and_then(|key| self.definitions.properties.get(key))
            {
                row.kind = definition.kind;
            }
        }
        Ok(true)
    }
    pub(crate) fn set_definitions(
        &mut self,
        definitions: &crate::property_model::PropertyDefinitions,
    ) {
        self.definitions = definitions.clone();
        if let Err(error) = self.rebuild() {
            self.error = Some(error.to_string());
        }
    }
    pub(crate) fn rename(&mut self, new: &str) -> Result<bool> {
        let row = self
            .rows
            .get(self.selected)
            .context("Select property first")?;
        let old = row
            .key
            .as_str()
            .context("Rename supports string keys")?
            .to_owned();
        crate::property_model::validate_key(new)?;
        if old == new {
            return Ok(false);
        }
        let current = self.current.as_deref().unwrap_or("---\n---\n");
        frontmatter::rename_key(current, &old, new)?;
        let mut pending = self.pending.clone();
        pending.push(FrontmatterEdit {
            key_yaml: serde_yaml_ng::to_string(&Value::String(new.into()))?,
            value_yaml: None,
            rename_from: Some(row.key_yaml.clone()),
        });
        let candidate =
            frontmatter::apply_edits(self.baseline.as_deref().unwrap_or("---\n---\n"), &pending)?;
        self.current = Some(candidate);
        self.pending = pending;
        self.renamed_keys.insert(new.into());
        self.rebuild()?;
        self.revision = self.revision.wrapping_add(1);
        Ok(true)
    }
    pub(crate) fn begin_add(&mut self, theme: &AppThemeColors) {
        if self.error.is_some() {
            return;
        }
        self.dialog = Some(PropertiesDialog::Edit(Box::new(PropertyDialog {
            key_yaml: None,
            name: crate::ui::make_popup_textarea(theme, "Name"),
            value: crate::ui::make_popup_textarea(theme, "Value"),
            kind: PropertyType::String,
            control: 0,
            error: None,
            rects: [Rect::default(); 4],
            choices: Vec::new(),
            choice: 0,
            choice_rect: Rect::default(),
            choice_view_start: 0,
            definition_offer: None,
            definition_confirm: false,
        })));
    }
    pub(crate) fn begin_edit(&mut self, theme: &AppThemeColors, delete: bool) -> Result<()> {
        ensure!(self.error.is_none(), "Structured properties disabled");
        let row = self
            .rows
            .get(self.selected)
            .context("Select a property first")?;
        if delete {
            self.dialog = Some(PropertiesDialog::Delete {
                key_yaml: row.key_yaml.clone(),
                yes: false,
                rects: [Rect::default(); 2],
            });
        } else {
            let mut name = crate::ui::make_popup_textarea(theme, "");
            name.insert_str(row.name());
            let mut value = crate::ui::make_popup_textarea(theme, "Value");
            value.insert_str(&match (&row.kind, &row.value) {
                (
                    PropertyType::String
                    | PropertyType::Date
                    | PropertyType::DateTime
                    | PropertyType::Select
                    | PropertyType::NoteReference,
                    Value::String(value),
                ) => value.clone(),
                (
                    PropertyType::List | PropertyType::MultiSelect | PropertyType::NoteReferences,
                    Value::Sequence(values),
                ) if values.iter().all(|value| {
                    value
                        .as_str()
                        .is_some_and(|value| !value.is_empty() && !value.contains(['\r', '\n']))
                }) && (row.kind == PropertyType::NoteReferences
                    || values
                        .first()
                        .and_then(Value::as_str)
                        .is_none_or(|value| !value.trim_start().starts_with('['))) =>
                {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join("\n")
                }
                _ => row.value_yaml.trim_end_matches(['\r', '\n']).into(),
            });
            self.dialog = Some(PropertiesDialog::Edit(Box::new(PropertyDialog {
                key_yaml: Some(row.key_yaml.clone()),
                name,
                value,
                kind: row.kind,
                control: 2,
                error: None,
                rects: [Rect::default(); 4],
                choices: Vec::new(),
                choice: 0,
                choice_rect: Rect::default(),
                choice_view_start: 0,
                definition_offer: None,
                definition_confirm: false,
            })));
            if row.kind == PropertyType::Null
                && let Some(PropertiesDialog::Edit(dialog)) = &mut self.dialog
            {
                dialog.control = 1;
            }
        }
        Ok(())
    }
    pub(crate) fn paste(&mut self, data: &str) -> bool {
        if let Some(PropertiesDialog::Edit(dialog)) = &mut self.dialog {
            match dialog.control {
                0 if dialog.key_yaml.is_none() => {
                    dialog.name.insert_str(data.replace(['\r', '\n'], " "));
                }
                2 if dialog.kind != PropertyType::Null => {
                    dialog.value.insert_str(data);
                }
                _ => {}
            }
        }
        true // Lists and non-text dialog controls consume paste too.
    }
    fn navigate(&mut self, delta: isize) {
        self.selected = self
            .selected
            .saturating_add_signed(delta)
            .min(self.rows.len());
        self.scroll_to_selection();
    }
    pub(crate) fn scroll_to_selection(&mut self) {
        let height = self.visible_rows.max(1);
        if self.selected < self.scroll {
            self.scroll = self.selected;
        }
        if self.selected >= self.scroll + height {
            self.scroll = self.selected + 1 - height;
        }
    }
}

impl App {
    pub(crate) fn sync_property_editor(&mut self) {
        if self.editor.properties.definitions != self.property_definitions {
            self.editor
                .properties
                .set_definitions(&self.property_definitions);
        }
        let keys: std::collections::BTreeSet<_> = self
            .property_definitions
            .properties
            .keys()
            .chain(self.notes.iter().flat_map(|note| note.properties.keys()))
            .cloned()
            .collect();
        self.editor.properties.known_keys = keys.into_iter().collect();
    }
    fn prepare_property_choices(&self, input: &mut PropertyDialog, adopt_definition: bool) {
        let name = input.name.lines().join("\n");
        if let Some(definition) = self.property_definitions.properties.get(&name)
            && adopt_definition
            && (input.key_yaml.is_none() || input.kind != PropertyType::Yaml)
        {
            input.kind = definition.kind;
            if input.key_yaml.is_none()
                && input.value.lines().join("\n").is_empty()
                && let Some(default) = &definition.default
                && let Ok(value) = crate::property_model::toml_to_yaml(default)
            {
                input.value.insert_str(match &value {
                    Value::String(value) => value.clone(),
                    Value::Sequence(values) if values.iter().all(Value::is_string) => values
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join("\n"),
                    _ => serde_yaml_ng::to_string(&value)
                        .unwrap_or_default()
                        .trim()
                        .into(),
                });
            }
        }
        input.choices = if input.control == 0 {
            self.editor
                .properties
                .known_keys
                .iter()
                .filter(|key| key.starts_with(&name))
                .cloned()
                .collect()
        } else if matches!(
            input.kind,
            PropertyType::NoteReference | PropertyType::NoteReferences
        ) {
            self.visible_notes()
                .filter(|(_, note)| {
                    matches!(
                        std::path::Path::new(&note.id)
                            .extension()
                            .and_then(|ext| ext.to_str()),
                        Some("md" | "txt" | "clin")
                    )
                })
                .map(|(_, note)| note.id.clone())
                .collect()
        } else if let Some(definition) = self.property_definitions.properties.get(&name) {
            definition.options.clone()
        } else if matches!(input.kind, PropertyType::Select | PropertyType::MultiSelect) {
            self.notes
                .iter()
                .filter_map(|note| note.properties.get(&name))
                .flat_map(|value| match value {
                    crate::property_model::PropertyValue::String(value) => vec![value.clone()],
                    crate::property_model::PropertyValue::List(values) => values
                        .iter()
                        .filter_map(|value| {
                            if let crate::property_model::PropertyValue::String(value) = value {
                                Some(value.clone())
                            } else {
                                None
                            }
                        })
                        .collect(),
                    _ => Vec::new(),
                })
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect()
        } else {
            Vec::new()
        };
        input.choice = input.choice.min(input.choices.len().saturating_sub(1));
    }
    fn commit_typed_property(
        &mut self,
        edit: FrontmatterEdit,
        new: bool,
        kind: Option<PropertyType>,
        choices: &[String],
        consent: bool,
    ) -> Result<TypedPropertyCommit> {
        ensure!(
            self.property_definitions_error.is_none(),
            "Repair property definitions first"
        );
        ensure!(
            self.editor.properties.error.is_none(),
            "Repair note frontmatter first"
        );
        let key: Value = serde_yaml_ng::from_str(&edit.key_yaml)?;
        let name = key.as_str();
        let current = self
            .editor
            .properties
            .current
            .as_deref()
            .unwrap_or("---\n---\n");
        ensure!(
            !new || !frontmatter::validate_header(current)?.contains_key(&key),
            "Property already exists"
        );
        let candidate = frontmatter::apply_edits(current, std::slice::from_ref(&edit))?;
        let mapping = frontmatter::validate_header(&candidate)?;
        if let (Some(name), Some(value)) = (name, mapping.get(&key)) {
            if let Some(kind) = kind {
                if !self.property_definitions.properties.contains_key(name)
                    && matches!(
                        kind,
                        PropertyType::Date
                            | PropertyType::DateTime
                            | PropertyType::Select
                            | PropertyType::MultiSelect
                            | PropertyType::NoteReference
                            | PropertyType::NoteReferences
                    )
                {
                    self.ensure_catalog_ready()?;
                }
                if let Some(definition) = crate::property_model::inferred_definition(
                    &self.property_definitions,
                    self.notes
                        .iter()
                        .filter(|note| Some(note.id.as_str()) != self.editor.editing_id.as_deref()),
                    name,
                    kind,
                    value,
                    choices,
                )? {
                    if !consent {
                        return Ok(TypedPropertyCommit::Offer(definition));
                    }
                    let mut definitions = self.property_definitions.clone();
                    definitions.properties.insert(name.into(), definition);
                    self.save_property_definitions(definitions)?;
                }
            } else {
                self.property_definitions.validate(name, value)?;
            }
        }
        self.editor
            .properties
            .commit(edit, new)
            .map(TypedPropertyCommit::Applied)
    }
    pub fn toggle_properties(&mut self) {
        if self.mode == ViewMode::List {
            let Some(item) = self.list.visual_list.get(self.list.visual_index) else {
                self.set_temporary_status_static("Select a note first");
                return;
            };
            let crate::list_view::VisualItem::Note { summary_idx, .. } = item else {
                self.set_temporary_status_static("Properties are available for notes");
                return;
            };
            let Some(id) = self.notes.get(*summary_idx).map(|note| note.id.clone()) else {
                self.set_temporary_status_static("Select a note first");
                return;
            };
            if id.ends_with(".clin") {
                self.open_note_at_line(&id, None);
                return;
            }
            if !id.ends_with(".md") && !id.ends_with(".txt") {
                self.set_temporary_status_static("Properties are available for notes");
                return;
            }
            self.load_and_open_note(&id, None);
        }
        if self.mode != ViewMode::Edit || !self.properties_available() {
            self.set_temporary_status_static("Properties are available for notes");
            return;
        }
        if self.preview_fullscreen {
            self.toggle_preview_fullscreen();
        }
        self.set_sidebar(crate::editor::EditSidebar::Properties);
        self.editor.properties.focus_request = Some(
            if self.editor.sidebar == crate::editor::EditSidebar::Properties {
                EditFocus::Properties
            } else {
                EditFocus::Body
            },
        );
    }
    pub(crate) fn properties_available(&self) -> bool {
        self.editor.template_edit_path.is_none()
            && self.editor.editing_id.as_deref().is_some_and(|id| {
                let ext = std::path::Path::new(id)
                    .extension()
                    .and_then(|extension| extension.to_str());
                matches!(ext, None | Some("md" | "txt"))
            })
    }
    pub(crate) fn mark_properties_modified(&mut self) {
        self.editor.links = self.compute_links();
        self.editor.autosave_status = crate::editor::AutosaveStatus::Unsaved;
        self.editor.autosave_timer =
            Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
        *self.editor.modified_status_cache.borrow_mut() = None;
        self.write_draft();
    }
    pub(crate) fn handle_properties_dialog_key(&mut self, key: KeyEvent) -> bool {
        let Some(mut dialog) = self.editor.properties.dialog.take() else {
            return false;
        };
        self.seq_matcher.clear();
        if key.code == KeyCode::Esc {
            if let PropertiesDialog::Edit(input) = &mut dialog
                && input.definition_offer.take().is_some()
            {
                input.definition_confirm = false;
                self.editor.properties.dialog = Some(dialog);
            }
            return true;
        }
        if let PropertiesDialog::Edit(input) = &mut dialog
            && input.definition_offer.is_some()
        {
            match key.code {
                KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::BackTab => {
                    input.definition_confirm = !input.definition_confirm
                }
                KeyCode::Enter if input.definition_confirm => input.control = 3,
                KeyCode::Enter => {
                    input.definition_offer = None;
                    self.editor.properties.dialog = Some(dialog);
                    return true;
                }
                _ => {
                    self.editor.properties.dialog = Some(dialog);
                    return true;
                }
            }
            if key.code != KeyCode::Enter {
                self.editor.properties.dialog = Some(dialog);
                return true;
            }
        }
        if self
            .keybinds
            .matches_edit(crate::keybinds::EditAction::InsertDate, &key)
        {
            if let PropertiesDialog::Edit(input) = &mut dialog
                && input.control == 2
                && matches!(input.kind, PropertyType::String | PropertyType::Yaml)
            {
                input.value.insert_str(
                    chrono::Local::now()
                        .format(&self.config.editor.date_format)
                        .to_string(),
                );
            }
            self.editor.properties.dialog = Some(dialog);
            return true;
        }
        let mut edit = None;
        let mut close = false;
        match &mut dialog {
            PropertiesDialog::Delete { key_yaml, yes, .. } => match key.code {
                KeyCode::Left
                | KeyCode::Right
                | KeyCode::Tab
                | KeyCode::BackTab
                | KeyCode::Up
                | KeyCode::Down => *yes = !*yes,
                KeyCode::Enter => {
                    if *yes {
                        edit = Some(Ok((
                            FrontmatterEdit {
                                key_yaml: key_yaml.clone(),
                                value_yaml: None,
                                rename_from: None,
                            },
                            false,
                        )));
                    } else {
                        close = true;
                    }
                }
                _ => {}
            },
            PropertiesDialog::Edit(input) => match key.code {
                KeyCode::Enter
                    if key.modifiers.contains(KeyModifiers::CONTROL) || input.control == 3 =>
                {
                    edit = Some(input.edit().map(|edit| (edit, input.key_yaml.is_none())));
                }
                KeyCode::Tab => {
                    let adopt = input.control == 0;
                    input.advance(1);
                    self.prepare_property_choices(input, adopt);
                }
                KeyCode::BackTab => {
                    let adopt = input.control == 0;
                    input.advance(-1);
                    self.prepare_property_choices(input, adopt);
                }
                KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down
                    if input.control == 1 =>
                {
                    input.kind =
                        input
                            .kind
                            .cycle(if matches!(key.code, KeyCode::Left | KeyCode::Up) {
                                -1
                            } else {
                                1
                            });
                    if input.value.lines().join("\n").is_empty() {
                        match input.kind {
                            PropertyType::Boolean => {
                                input.value.insert_str("false");
                            }
                            PropertyType::Date => {
                                input.value.insert_str(
                                    chrono::Local::now().format("%Y-%m-%d").to_string(),
                                );
                            }
                            PropertyType::DateTime => {
                                input.value.insert_str(chrono::Local::now().to_rfc3339());
                            }
                            _ => {}
                        }
                    }
                    self.prepare_property_choices(input, false);
                }
                KeyCode::Up | KeyCode::Down
                    if input.control == 0
                        && input.key_yaml.is_none()
                        && !input.choices.is_empty() =>
                {
                    input.choice = (input.choice as isize
                        + if key.code == KeyCode::Up { -1 } else { 1 })
                    .rem_euclid(input.choices.len() as isize)
                        as usize;
                    input.name.select_all();
                    input.name.insert_str(&input.choices[input.choice]);
                    let choices = std::mem::take(&mut input.choices);
                    let choice = input.choice;
                    self.prepare_property_choices(input, true);
                    input.choices = choices;
                    input.choice = choice;
                }
                KeyCode::Up | KeyCode::Down
                    if input.control == 2 && input.picker() && !input.choices.is_empty() =>
                {
                    input.choice = (input.choice as isize
                        + if key.code == KeyCode::Up { -1 } else { 1 })
                    .rem_euclid(input.choices.len() as isize)
                        as usize;
                    if matches!(
                        input.kind,
                        PropertyType::Select | PropertyType::NoteReference
                    ) {
                        input.choose();
                    }
                }
                KeyCode::Char(' ')
                    if input.control == 2 && input.picker() && !input.choices.is_empty() =>
                {
                    input.choose()
                }
                KeyCode::Enter
                    if input.control == 2 && input.picker() && !input.choices.is_empty() =>
                {
                    input.choose();
                    input.advance(1);
                }
                KeyCode::Left
                | KeyCode::Right
                | KeyCode::Up
                | KeyCode::Down
                | KeyCode::Char(' ')
                    if input.control == 2 && input.kind == PropertyType::Boolean =>
                {
                    input.toggle_boolean()
                }
                KeyCode::Enter
                    if input.control != 2
                        || !matches!(
                            input.kind,
                            PropertyType::String
                                | PropertyType::Yaml
                                | PropertyType::List
                                | PropertyType::MultiSelect
                                | PropertyType::NoteReferences
                        ) =>
                {
                    input.advance(1)
                }
                _ => {
                    if input.control == 0 {
                        crate::text_edit::feed_key(&self.keybinds, &mut input.name, key);
                        self.prepare_property_choices(input, false);
                    } else if input.control == 2 && input.kind != PropertyType::Boolean {
                        crate::text_edit::feed_key(&self.keybinds, &mut input.value, key);
                    }
                }
            },
        }
        if let Some(edit) = edit {
            let (kind, choices) = match &dialog {
                PropertiesDialog::Edit(input) => (Some(input.kind), input.choices.as_slice()),
                _ => (None, &[][..]),
            };
            let consent = matches!(&dialog, PropertiesDialog::Edit(input) if input.definition_offer.is_some() && input.definition_confirm);
            match edit.and_then(|(edit, new)| {
                self.commit_typed_property(edit, new, kind, choices, consent)
            }) {
                Ok(TypedPropertyCommit::Offer(definition)) => {
                    if let PropertiesDialog::Edit(input) = &mut dialog {
                        input.definition_offer = Some(definition);
                        input.definition_confirm = false;
                        input.control = 3;
                    }
                }
                Ok(TypedPropertyCommit::Applied(changed)) => {
                    if changed {
                        self.mark_properties_modified();
                    }
                    close = true;
                }
                Err(error) => match &mut dialog {
                    PropertiesDialog::Edit(input) => input.error = Some(error.to_string()),
                    PropertiesDialog::Delete { .. } => {
                        self.set_temporary_status(&error.to_string())
                    }
                },
            }
        }
        if !close {
            self.editor.properties.dialog = Some(dialog);
        }
        true
    }
    pub(crate) fn handle_properties_list_key(&mut self, key: KeyEvent, focus: &mut EditFocus) {
        self.seq_matcher.clear();
        match key.code {
            KeyCode::Char('d') => {
                self.open_property_manager(
                    crate::property_management::PropertyManagerMode::Definitions,
                    None,
                );
                return;
            }
            KeyCode::Char('r') => {
                self.open_property_manager(
                    crate::property_management::PropertyManagerMode::Rename { global: false },
                    None,
                );
                return;
            }
            KeyCode::Char('R') => {
                self.open_property_manager(
                    crate::property_management::PropertyManagerMode::Rename { global: true },
                    None,
                );
                return;
            }
            KeyCode::Char('b') => {
                self.open_property_manager(
                    crate::property_management::PropertyManagerMode::Bulk,
                    None,
                );
                return;
            }
            _ => {}
        }
        if key.code == KeyCode::Char('a')
            || key.code == KeyCode::Enter
                && self.editor.properties.selected >= self.editor.properties.rows.len()
        {
            self.sync_property_editor();
        }
        let state = &mut self.editor.properties;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => state.navigate(-1),
            KeyCode::Down | KeyCode::Char('j') => state.navigate(1),
            KeyCode::Home => {
                state.selected = 0;
                state.scroll_to_selection();
            }
            KeyCode::End => {
                state.selected = state.rows.len();
                state.scroll_to_selection();
            }
            KeyCode::PageUp => state.navigate(-(state.visible_rows.max(1) as isize)),
            KeyCode::PageDown => state.navigate(state.visible_rows.max(1) as isize),
            KeyCode::Char('a') => state.begin_add(&self.app_theme),
            KeyCode::Enter if state.selected == state.rows.len() => {
                state.begin_add(&self.app_theme)
            }
            KeyCode::Enter | KeyCode::Delete => {
                if let Err(error) = state.begin_edit(&self.app_theme, key.code == KeyCode::Delete) {
                    self.set_temporary_status(&error.to_string());
                }
            }
            KeyCode::Char(' ') | KeyCode::Esc => {
                self.editor.sidebar = crate::editor::EditSidebar::None;
                *focus = EditFocus::Body;
            }
            _ => {}
        }
        if let Some(mut dialog) = self.editor.properties.dialog.take() {
            if let PropertiesDialog::Edit(input) = &mut dialog {
                self.prepare_property_choices(input, true);
            }
            self.editor.properties.dialog = Some(dialog);
        }
    }
}

pub(crate) fn draw_sidebar(
    frame: &mut ratatui::Frame,
    app: &mut App,
    focus: EditFocus,
    area: Rect,
) {
    use ratatui::{
        layout::{Constraint, Layout},
        style::{Modifier, Style},
        widgets::{Block, Paragraph, Wrap},
    };
    let theme = &app.app_theme;
    let state = &mut app.editor.properties;
    let background = theme.preview_bg_style();
    frame.render_widget(Block::default().style(background), area);
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .split(area);
    let title = format!("  PROPERTIES ({})", state.rows.len());
    let style = background
        .fg(if focus == EditFocus::Properties {
            theme.accent
        } else {
            theme.heading
        })
        .add_modifier(Modifier::BOLD);
    frame.render_widget(Paragraph::new(title).style(style), chunks[1]);
    let list = Rect::new(
        chunks[3].x.saturating_add(2),
        chunks[3].y,
        chunks[3].width.saturating_sub(2),
        chunks[3].height,
    );
    app.editor.sidebar_list_rect = list;
    state.visible_rows = list.height as usize;
    state.scroll = state
        .scroll
        .min((state.rows.len() + 1).saturating_sub(state.visible_rows.max(1)));
    if let Some(error) = &state.error {
        frame.render_widget(
            Paragraph::new(error.as_str())
                .style(background.fg(theme.destructive))
                .wrap(Wrap { trim: false }),
            list,
        );
        return;
    }
    for (visible, index) in (state.scroll..=state.rows.len())
        .take(state.visible_rows)
        .enumerate()
    {
        let text = if let Some(row) = state.rows.get(index) {
            format!("{}: {}  [{}]", row.name(), row.summary(), row.kind.label())
        } else {
            "+ Add property".into()
        };
        let rect = Rect::new(list.x, list.y + visible as u16, list.width, 1);
        let style = if index == state.selected && focus == EditFocus::Properties {
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
        } else if app
            .mouse_pos
            .is_some_and(|(x, y)| crate::events::contains_cell(rect, x, y))
        {
            theme.hover_style()
        } else {
            background
        };
        frame.render_widget(Paragraph::new(text).style(style), rect);
    }
}

pub(crate) fn draw_dialog(frame: &mut ratatui::Frame, app: &mut App) {
    use crate::ui::{PopupHints, PopupSize, draw_popup_frame};
    use ratatui::{
        layout::{Constraint, Layout},
        style::Style,
        widgets::{Block, Borders, Paragraph, Wrap},
    };
    let Some(dialog) = &mut app.editor.properties.dialog else {
        return;
    };
    let theme = &app.app_theme;
    let hints = [
        ("Tab".into(), "control"),
        ("Ctrl+Enter".into(), "apply"),
        ("Esc".into(), "cancel"),
    ];
    let content = draw_popup_frame(
        frame,
        frame.area(),
        "PROPERTIES",
        if matches!(dialog, PropertiesDialog::Delete { .. }) {
            PopupSize::Small
        } else {
            PopupSize::Large
        },
        PopupHints::Keybinds(&hints),
        theme,
    );
    let border = |title: &str, selected: bool| {
        Block::default()
            .borders(Borders::ALL)
            .title(title.to_owned())
            .style(theme.bg_style())
            .border_style(Style::default().fg(if selected { theme.accent } else { theme.muted }))
    };
    match dialog {
        PropertiesDialog::Edit(input) => {
            if let Some(definition) = &input.definition_offer {
                let chunks =
                    Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(content);
                frame.render_widget(Paragraph::new(format!(
                    "Create vault definition for {}?\nType: {}\nOptions: {:?}\nNo conflicting saved values found.\n\nDefinition affects every note in this vault. Confirmation applies definition and pending note edit. Escape returns to input; no writes yet.",
                    crate::fsutil::sanitize_for_terminal(&input.name.lines().join("")), definition.kind.label(), definition.options
                )).wrap(Wrap { trim: false }), chunks[0]);
                let buttons =
                    Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                        .split(chunks[1]);
                input.rects = [Rect::default(), Rect::default(), buttons[0], buttons[1]];
                for (index, label) in ["[ Create vault definition ]", "[ Back ]"]
                    .iter()
                    .enumerate()
                {
                    frame.render_widget(
                        Paragraph::new(*label).style(if (index == 0) == input.definition_confirm {
                            Style::default()
                                .fg(theme.highlight_fg)
                                .bg(theme.highlight_bg)
                        } else {
                            theme.bg_style()
                        }),
                        buttons[index],
                    );
                }
                return;
            }
            let chunks = Layout::vertical([
                Constraint::Length(3),
                Constraint::Length(3),
                if input.kind == PropertyType::Null {
                    Constraint::Length(0)
                } else {
                    Constraint::Min(3)
                },
                Constraint::Length(2),
                Constraint::Length(1),
            ])
            .split(content);
            input.rects = [chunks[0], chunks[1], chunks[2], chunks[4]];
            input.name.set_block(border(
                if input.key_yaml.is_some() {
                    "Name (read-only)"
                } else {
                    "Name"
                },
                input.control == 0,
            ));
            input.name.set_cursor_style(if input.control == 0 {
                Style::default()
                    .fg(theme.highlight_fg)
                    .bg(theme.highlight_bg)
            } else {
                theme.bg_style()
            });
            frame.render_widget(&input.name, chunks[0]);
            frame.render_widget(
                Paragraph::new(format!("{}  ←/→", input.kind.label()))
                    .block(border("Type", input.control == 1)),
                chunks[1],
            );
            let value_area = if !input.choices.is_empty() && (input.control == 0 || input.picker())
            {
                let panes =
                    Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
                        .split(chunks[2]);
                input.choice_rect = panes[1];
                let height = panes[1].height.saturating_sub(2) as usize;
                input.choice_view_start = input.choice.saturating_sub(height / 2);
                let items: Vec<ratatui::widgets::ListItem> = input
                    .choices
                    .iter()
                    .skip(input.choice_view_start)
                    .take(height)
                    .map(|choice| {
                        let chosen = input.value.lines().iter().any(|value| value == choice);
                        let marker = if matches!(
                            input.kind,
                            PropertyType::MultiSelect | PropertyType::NoteReferences
                        ) {
                            if chosen { "[x] " } else { "[ ] " }
                        } else {
                            ""
                        };
                        ratatui::widgets::ListItem::new(format!(
                            "{marker}{}",
                            crate::fsutil::sanitize_for_terminal(choice)
                        ))
                    })
                    .collect();
                let list = ratatui::widgets::List::new(items)
                    .block(border(
                        if input.control == 0 {
                            "Names ↑/↓"
                        } else {
                            "Choices ↑/↓ Space"
                        },
                        true,
                    ))
                    .highlight_style(
                        Style::default()
                            .fg(theme.highlight_fg)
                            .bg(theme.highlight_bg),
                    );
                let mut state = ratatui::widgets::ListState::default();
                state.select(Some(input.choice.saturating_sub(input.choice_view_start)));
                frame.render_stateful_widget(list, panes[1], &mut state);
                panes[0]
            } else {
                input.choice_rect = Rect::default();
                chunks[2]
            };
            input.rects[2] = value_area;
            if input.kind == PropertyType::Boolean {
                let value = input.value.lines().first().map_or("", String::as_str);
                let checkbox = format!(
                    "[{}] {}",
                    if value.trim() == "true" { "x" } else { " " },
                    crate::fsutil::sanitize_for_terminal(value)
                );
                frame.render_widget(
                    Paragraph::new(checkbox)
                        .block(border("Boolean: Space/click toggles", input.control == 2)),
                    value_area,
                );
            } else if input.kind != PropertyType::Null {
                let value_title = match input.kind {
                    PropertyType::Boolean => "Checkbox ←/→ toggles",
                    PropertyType::Date => "Date YYYY-MM-DD",
                    PropertyType::DateTime => "Date/time RFC 3339 (with offset)",
                    PropertyType::List => "List: one string item per line / YAML array",
                    PropertyType::MultiSelect | PropertyType::NoteReferences => {
                        "Selected items (Space toggles choice)"
                    }
                    _ => "Value",
                };
                input
                    .value
                    .set_block(border(value_title, input.control == 2));
                input.value.set_cursor_style(if input.control == 2 {
                    Style::default()
                        .fg(theme.highlight_fg)
                        .bg(theme.highlight_bg)
                } else {
                    theme.bg_style()
                });
                frame.render_widget(&input.value, value_area);
            }
            if let Some(error) = &input.error {
                frame.render_widget(
                    Paragraph::new(crate::fsutil::sanitize_for_terminal(error).to_string())
                        .style(Style::default().fg(ratatui::style::Color::Red))
                        .wrap(Wrap { trim: false }),
                    chunks[3],
                );
            } else {
                let name = input.name.lines().join("\n");
                let hint = app
                    .property_definitions
                    .properties
                    .get(&name)
                    .map(|definition| definition.description.as_str())
                    .unwrap_or_else(|| {
                        if matches!(
                            input.kind,
                            PropertyType::Date
                                | PropertyType::DateTime
                                | PropertyType::Select
                                | PropertyType::MultiSelect
                                | PropertyType::NoteReference
                                | PropertyType::NoteReferences
                        ) {
                            "Apply offers vault definition creation if undeclared; confirmation required."
                        } else {
                            ""
                        }
                    });
                frame.render_widget(
                    Paragraph::new(crate::fsutil::sanitize_for_terminal(hint).to_string())
                        .style(Style::default().fg(theme.muted)),
                    chunks[3],
                );
            }
            frame.render_widget(
                Paragraph::new("[ Apply ]").style(if input.control == 3 {
                    Style::default()
                        .fg(theme.highlight_fg)
                        .bg(theme.highlight_bg)
                } else {
                    theme.bg_style()
                }),
                chunks[4],
            );
        }
        PropertiesDialog::Delete {
            key_yaml,
            yes,
            rects,
        } => {
            let chunks =
                Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(content);
            frame.render_widget(
                Paragraph::new(format!("Delete property {}?", key_yaml.trim()))
                    .wrap(Wrap { trim: false }),
                chunks[0],
            );
            let buttons =
                Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(chunks[1]);
            *rects = [buttons[0], buttons[1]];
            for (index, label) in ["[ Yes ]", "[ No ]"].iter().enumerate() {
                frame.render_widget(
                    Paragraph::new(*label).style(if (index == 0) == *yes {
                        Style::default()
                            .fg(theme.highlight_fg)
                            .bg(theme.highlight_bg)
                    } else {
                        theme.bg_style()
                    }),
                    buttons[index],
                );
            }
        }
    }
}

pub(crate) fn handle_dialog_mouse(app: &mut App, mouse: crossterm::event::MouseEvent) -> bool {
    use crossterm::event::{MouseButton, MouseEventKind};
    let Some(dialog) = &mut app.editor.properties.dialog else {
        return false;
    };
    let mut apply = false;
    if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
        match dialog {
            PropertiesDialog::Edit(input) => {
                if input.definition_offer.is_some() {
                    for (index, rect) in input.rects[2..].iter().enumerate() {
                        if crate::events::contains_cell(*rect, mouse.column, mouse.row) {
                            input.definition_confirm = index == 0;
                            apply = true;
                        }
                    }
                } else if input.choice_rect.width > 0
                    && crate::events::contains_cell(input.choice_rect, mouse.column, mouse.row)
                {
                    input.choice = (input.choice_view_start
                        + mouse.row.saturating_sub(input.choice_rect.y + 1) as usize)
                        .min(input.choices.len().saturating_sub(1));
                    if input.control == 0 {
                        if let Some(choice) = input.choices.get(input.choice) {
                            input.name.select_all();
                            input.name.insert_str(choice);
                        }
                    } else {
                        input.control = 2;
                        input.choose();
                    }
                }
                if input.definition_offer.is_none()
                    && let Some(control) = input.rects.iter().position(|rect| {
                        crate::events::contains_cell(*rect, mouse.column, mouse.row)
                    })
                {
                    if control == 0 && input.key_yaml.is_some() {
                        return true;
                    }
                    input.control = control;
                    match control {
                        1 => input.kind = input.kind.cycle(1),
                        3 => apply = true,
                        2 if input.kind == PropertyType::Boolean => input.toggle_boolean(),
                        0 | 2 => {
                            let textarea = if control == 0 {
                                &mut input.name
                            } else {
                                &mut input.value
                            };
                            let rect = input.rects[control];
                            let inner = Rect::new(
                                rect.x + 1,
                                rect.y + 1,
                                rect.width.saturating_sub(2),
                                rect.height.saturating_sub(2),
                            );
                            crate::events::move_textarea_cursor_to_mouse(
                                textarea,
                                inner,
                                mouse.column,
                                mouse.row,
                                0,
                                0,
                            );
                        }
                        _ => {}
                    }
                }
            }
            PropertiesDialog::Delete { yes, rects, .. } => {
                if let Some(index) = rects
                    .iter()
                    .position(|rect| crate::events::contains_cell(*rect, mouse.column, mouse.row))
                {
                    *yes = index == 0;
                    apply = true;
                }
            }
        }
    }
    if let Some(mut dialog) = app.editor.properties.dialog.take() {
        if let PropertiesDialog::Edit(input) = &mut dialog {
            app.prepare_property_choices(input, input.control == 0 && input.key_yaml.is_none());
        }
        app.editor.properties.dialog = Some(dialog);
    }
    if apply {
        app.handle_properties_dialog_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    }
    true
}

pub(crate) fn handle_list_mouse(
    app: &mut App,
    mouse: crossterm::event::MouseEvent,
    area: Rect,
    focus: &mut EditFocus,
) -> bool {
    use crossterm::event::{MouseButton, MouseEventKind};
    if !crate::events::contains_cell(area, mouse.column, mouse.row) {
        return false;
    }
    let state = &mut app.editor.properties;
    match mouse.kind {
        MouseEventKind::ScrollUp => state.scroll = state.scroll.saturating_sub(1),
        MouseEventKind::ScrollDown => {
            state.scroll = (state.scroll + 1)
                .min((state.rows.len() + 1).saturating_sub(state.visible_rows.max(1)))
        }
        MouseEventKind::Down(MouseButton::Left) if mouse.row == area.y + 1 => {
            app.toggle_properties();
            if let Some(requested) = app.editor.properties.focus_request.take() {
                *focus = requested;
            }
        }
        MouseEventKind::Down(MouseButton::Left) if mouse.row >= area.y + 3 => {
            *focus = EditFocus::Properties;
            let index = state.scroll + (mouse.row - area.y - 3) as usize;
            if index <= state.rows.len() {
                let double = state.last_click.is_some_and(|(previous, time)| {
                    previous == index && time.elapsed().as_millis() < 500
                });
                state.selected = index;
                state.last_click = Some((index, std::time::Instant::now()));
                if index == state.rows.len() {
                    state.begin_add(&app.app_theme);
                } else if double && let Err(error) = state.begin_edit(&app.app_theme, false) {
                    app.set_temporary_status(&error.to_string());
                }
            }
        }
        _ => {}
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_dialogs_require_definition_consent_and_reopen_every_kind() -> Result<()> {
        let _guard = crate::config::ConfigTestGuard::lock();
        let dir = tempfile::tempdir()?;
        crate::config::set_config_path_override(dir.path().join("config.toml"));
        let vault = dir.path().join("vault");
        std::fs::create_dir_all(&vault)?;
        std::fs::write(vault.join("note.md"), "---\ntitle: Note\n---\nBody")?;
        std::fs::write(vault.join("target.md"), "---\ntitle: Target\n---\nTarget")?;
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
        app.load_and_open_note("note.md", None);
        let apply = KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL);
        for (index, (kind, value)) in [
            (PropertyType::String, "001"),
            (PropertyType::Number, "18446744073709551615"),
            (PropertyType::Boolean, "false"),
            (PropertyType::Null, ""),
            (PropertyType::Yaml, "{nested: [1, two]}"),
            (PropertyType::Date, "2026-10-09"),
            (PropertyType::DateTime, "2026-10-09T12:00:00+02:00"),
            (PropertyType::List, "[one, true, 0]"),
            (PropertyType::Select, "draft"),
            (PropertyType::MultiSelect, "draft\nreview"),
            (PropertyType::NoteReference, "target.md"),
            (PropertyType::NoteReferences, "target.md"),
        ]
        .into_iter()
        .enumerate()
        {
            let name = format!("field{index}");
            app.editor.properties.begin_add(&app.app_theme);
            if let Some(PropertiesDialog::Edit(input)) = &mut app.editor.properties.dialog {
                input.name.insert_str(&name);
                input.kind = kind;
                input.value.insert_str(value);
                input.control = 3;
            }
            let before = app.editor.properties.current.clone();
            app.handle_properties_dialog_key(apply);
            if app.editor.properties.dialog.is_some() {
                assert_eq!(app.editor.properties.current, before);
                assert!(!app.property_definitions.properties.contains_key(&name));
                app.handle_properties_dialog_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
                assert_eq!(app.editor.properties.current, before);
                app.handle_properties_dialog_key(apply);
                app.handle_properties_dialog_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
                app.handle_properties_dialog_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            }
            assert!(
                app.editor.properties.dialog.is_none(),
                "{} failed",
                kind.label()
            );
            app.autosave().map_err(anyhow::Error::msg)?;
            app.load_and_open_note("note.md", None);
            let row = app
                .editor
                .properties
                .rows
                .iter()
                .find(|row| row.name() == name)
                .expect("saved property");
            assert_eq!(row.kind, kind);
            assert_eq!(
                row.value,
                crate::property_model::parse_property_value(kind, value)?
            );
        }
        Ok(())
    }

    #[test]
    fn rename_transactions_preserve_comments_aliases_and_sequential_edits() {
        for newline in ["\n", "\r\n"] {
            let header = "---\nbase: &value ['001', two] # retain\ncopy: *value\n---\n"
                .replace('\n', newline);
            let mut state = PropertiesState::load(
                Ok(Some(header.clone())),
                &crate::property_model::PropertyDefinitions::default(),
            );
            assert!(state.rename("source").unwrap());
            assert_eq!(
                state.current.as_deref(),
                Some(header.replace("base:", "source:").as_str())
            );
            let before = state.current.clone();
            assert!(
                state
                    .commit(
                        crate::property_model::property_edit(
                            "source",
                            Some(&serde_yaml_ng::from_str("[new, two]").unwrap())
                        )
                        .unwrap(),
                        false
                    )
                    .is_err()
            );
            assert_eq!(state.current, before);
            assert!(state.rename("final").unwrap());
            let values = frontmatter::validate_header(state.current.as_deref().unwrap()).unwrap();
            assert_eq!(values["final"][0].as_str(), Some("001"));
            assert_eq!(values["copy"], values["final"]);
            assert!(state.current.as_deref().unwrap().contains("# retain"));
            assert!(state.current.as_deref().unwrap().contains("*value"));
            assert!(state.rename("copy").is_err());
        }
    }
    #[test]
    fn scalar_rename_followed_by_edit_keeps_transaction_order() {
        let mut state = PropertiesState::load(
            Ok(Some("---\nold: original # keep\n---\n".into())),
            &crate::property_model::PropertyDefinitions::default(),
        );
        assert!(state.rename("new").unwrap());
        assert!(
            state
                .commit(
                    crate::property_model::property_edit(
                        "new",
                        Some(&Value::String("changed".into()))
                    )
                    .unwrap(),
                    false
                )
                .unwrap()
        );
        assert!(state.rename("final").unwrap());
        assert!(
            state
                .commit(
                    crate::property_model::property_edit(
                        "final",
                        Some(&Value::String("original".into()))
                    )
                    .unwrap(),
                    false
                )
                .unwrap()
        );
        assert_eq!(
            frontmatter::validate_header(state.current.as_deref().unwrap()).unwrap()["final"]
                .as_str(),
            Some("original")
        );
        assert!(state.current.as_deref().unwrap().contains("# keep"));
    }

    #[test]
    fn properties_transaction_revert_types_and_errors() {
        let baseline = "---\n# retain\ncode: '001' # quote\nstatus: open\n---\n";
        let mut state = PropertiesState::load(
            Ok(Some(baseline.into())),
            &crate::property_model::PropertyDefinitions::default(),
        );
        let edit = |key: &str, value: &str| FrontmatterEdit {
            key_yaml: key.into(),
            value_yaml: Some(value.into()),
            rename_from: None,
        };
        assert!(state.commit(edit("status", "answered"), false).unwrap());
        assert_eq!(state.pending.len(), 1);
        assert!(state.commit(edit("status", "open"), false).unwrap());
        assert_eq!(state.pending.len(), 0);
        assert_eq!(state.current.as_deref(), Some(baseline));
        let revision = state.revision;
        assert!(!state.commit(edit("code", "\"001\""), false).unwrap());
        assert_eq!(state.revision, revision);
        for (key, value, new) in [
            ("title", "x", true),
            ("status", "x", true),
            ("bad", "[", true),
        ] {
            assert!(state.commit(edit(key, value), new).is_err());
        }
        assert_eq!(state.current.as_deref(), Some(baseline));
        assert_eq!(state.pending.len(), 0);
        let encoded = PropertyType::String.encode("true").unwrap();
        assert_eq!(
            serde_yaml_ng::from_str::<Value>(&encoded).unwrap(),
            Value::String("true".into())
        );
        assert!(PropertyType::Number.encode("001 text").is_err());
        assert!(PropertyType::Boolean.encode("yes").is_err());
        assert!(
            state
                .commit(
                    edit("flag", &PropertyType::Boolean.encode("true").unwrap()),
                    true
                )
                .unwrap()
        );
        assert_eq!(state.rows.last().unwrap().kind, PropertyType::Boolean);
        assert!(
            state
                .commit(
                    FrontmatterEdit {
                        key_yaml: "flag".into(),
                        value_yaml: None,
                        rename_from: None,
                    },
                    false
                )
                .unwrap()
        );
        assert_eq!(state.current.as_deref(), Some(baseline));
    }

    #[test]
    fn properties_sidebar_focus_layout_and_input() {
        use crate::app::EditSidebar;
        use crate::config::PreviewPosition;
        use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        let _guard = crate::config::ConfigTestGuard::lock();
        let dir = tempfile::tempdir().unwrap();
        crate::config::set_config_path_override(dir.path().join("config.toml"));
        let storage = crate::storage::Storage {
            data_dir: dir.path().into(),
            config_dir: dir.path().into(),
            notes_dir: dir.path().into(),
            templates_dir: dir.path().join("templates"),
            key: <[u8; 32]>::default(),
            skip_dir_patterns: vec![],
            rename_on_title_change: false,
        };
        std::fs::write(dir.path().join("note.md"),
            "---\ntitle: Note\nupdated_at: 42\ntags: [one]\npinned: true\nlinks: [other]\noriginal_ext: md\ntext_align: left\nstatus: open\ndetails: {score: 2}\n---\nbody text").unwrap();
        let mut app = App::new(storage).unwrap();
        app.load_and_open_note("note.md", None);
        assert_eq!(
            app.editor
                .properties
                .rows
                .iter()
                .map(PropertyRow::name)
                .collect::<Vec<_>>(),
            ["status", "details"]
        );
        let area = Rect::new(0, 0, 100, 30);
        let mut focus = EditFocus::Body;
        let mut selection = crate::text_edit::MouseTextSelection::default();
        let cycle = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL);
        for expected in [EditFocus::Properties, EditFocus::Title, EditFocus::Body] {
            crate::events::handle_edit_keys(&mut app, cycle, &mut focus);
            assert_eq!(focus, expected);
            assert_eq!(
                app.editor.sidebar == EditSidebar::Properties,
                expected == EditFocus::Properties
            );
        }
        app.app_theme.bg = Some(ratatui::style::Color::Rgb(40, 50, 60));
        for preview in [false, true] {
            for position in [PreviewPosition::Right, PreviewPosition::Left] {
                app.preview_position = position;
                app.editor.editor_preview_enabled = preview;
                crate::events::handle_edit_keys(&mut app, cycle, &mut focus);
                assert_eq!(focus, EditFocus::Properties);
                assert!(!app.editor.editor_preview_enabled);
                let body_area = crate::events::edit_view_outer_areas(area)[1];
                let layout = crate::events::compute_edit_layout(
                    body_area,
                    false,
                    app.editor.editor_preview_enabled,
                    app.editor.sidebar,
                    position,
                    0,
                );
                let sidebar = layout.sidebar.unwrap();
                assert_eq!(sidebar.y, body_area.y);
                assert_eq!(sidebar.height, body_area.height);
                assert_eq!(layout.body.y, body_area.y);
                assert_eq!(layout.body.height, body_area.height);
                assert!(layout.preview.is_none());
                match position {
                    PreviewPosition::Left => assert!(sidebar.right() <= layout.body.x),
                    PreviewPosition::Right => assert!(sidebar.x >= layout.body.right()),
                }
                let mut terminal =
                    ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30)).unwrap();
                terminal
                    .draw(|frame| crate::ui::draw_ui(frame, &mut app, focus))
                    .unwrap();
                let buffer = terminal.backend().buffer();
                for x in sidebar.x..sidebar.right() {
                    assert_eq!(buffer[(x, sidebar.y)].symbol(), " ");
                    assert_eq!(
                        buffer[(x, sidebar.y)].bg,
                        app.app_theme.preview_bg().unwrap()
                    );
                }
                let list = app.editor.sidebar_list_rect;
                let click = MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column: list.x,
                    row: list.y + 1,
                    modifiers: KeyModifiers::NONE,
                };
                crate::events::handle_edit_mouse(&mut app, click, area, &mut focus, &mut selection);
                assert_eq!(focus, EditFocus::Properties);
                assert_eq!(app.editor.properties.selected, 1);
                let (_, body, _) = crate::events::edit_view_input_areas(
                    area,
                    false,
                    false,
                    app.editor.body.lines().len(),
                    app.editor_show_line_numbers(),
                    app.editor.sidebar,
                    position,
                    app.editor.header_title_rect,
                    0,
                );
                crate::events::handle_edit_mouse(
                    &mut app,
                    MouseEvent {
                        column: body.x + 5,
                        row: body.y,
                        ..click
                    },
                    area,
                    &mut focus,
                    &mut selection,
                );
                assert_eq!(focus, EditFocus::Body);
                assert_eq!(app.editor.sidebar, EditSidebar::None);
                assert_eq!(
                    app.editor.body.cursor(),
                    crate::editor_document::TextPosition { row: 0, col: 5 }
                );
            }
        }
        app.toggle_outline_pane();
        crate::events::handle_edit_keys(&mut app, cycle, &mut focus);
        assert_eq!(focus, EditFocus::Sidebar);
        crate::events::handle_edit_keys(&mut app, cycle, &mut focus);
        assert_eq!(focus, EditFocus::Title);
        app.toggle_properties();
        assert_eq!(app.editor.sidebar, EditSidebar::Properties);
        app.editor.properties.focus_request = None;
        for (width, height) in [(8, 0), (8, 4), (40, 10)] {
            let layout = crate::events::compute_edit_layout(
                Rect::new(0, 0, width, height),
                false,
                false,
                EditSidebar::Properties,
                PreviewPosition::Right,
                0,
            );
            assert_eq!(layout.body.height, height);
            assert_eq!(layout.sidebar.unwrap().height, height);
        }
        let fullscreen = crate::events::compute_edit_layout(
            area,
            true,
            true,
            EditSidebar::Properties,
            PreviewPosition::Right,
            0,
        );
        assert!(fullscreen.sidebar.is_none());
        app.preview_fullscreen = true;
        focus = EditFocus::Body;
        crate::events::handle_edit_keys(&mut app, cycle, &mut focus);
        assert!(!app.preview_fullscreen);
        assert_eq!(focus, EditFocus::Properties);
        assert_eq!(app.editor.sidebar, EditSidebar::Properties);
        crate::events::handle_edit_keys(
            &mut app,
            KeyEvent::new(KeyCode::Char(';'), KeyModifiers::CONTROL),
            &mut focus,
        );
        assert_eq!(app.editor.body.lines(), &["body text"]);
        app.config.editor.date_format = "DATE".into();
        app.editor.properties.focused = true;
        crate::actions::execute_action("editor.insert_date", &mut app, None).unwrap();
        assert_eq!(app.editor.body.lines(), &["body text"]);
        app.editor.properties.selected = 0;
        app.editor
            .properties
            .begin_edit(&app.app_theme, false)
            .unwrap();
        app.handle_properties_dialog_key(KeyEvent::new(KeyCode::Char(';'), KeyModifiers::CONTROL));
        if let Some(PropertiesDialog::Edit(dialog)) = &mut app.editor.properties.dialog {
            assert_eq!(dialog.value.lines(), &["openDATE"]);
            dialog.value.select_all();
        } else {
            panic!("Property dialog closed during date insertion");
        }
        crate::actions::execute_action("editor.insert_date", &mut app, None).unwrap();
        if let Some(PropertiesDialog::Edit(dialog)) = &app.editor.properties.dialog {
            assert_eq!(dialog.value.lines(), &["DATE"]);
        }
        app.handle_properties_dialog_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.editor.properties.pending.len(), 0);
        crate::events::handle_edit_keys(
            &mut app,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &mut focus,
        );
        assert_eq!(focus, EditFocus::Body);
        assert_eq!(app.editor.sidebar, EditSidebar::None);
        assert_eq!(app.editor.body.lines(), &["body text"]);
    }
}
