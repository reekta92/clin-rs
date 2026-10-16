//! Group authoring owns visible column order and typed assignments; storage owns every write.
use super::{
    authoring::{Dialog, EditIntent, Target},
    format::Layout,
    state::{BasesState, Item},
    write::{Operation, WriteTask},
};
use crate::app::App;
use anyhow::{Context, Result, bail};
use serde_yaml_ng::Value;

#[derive(Clone, Copy)]
pub enum GroupAction {
    Add,
    Hide,
    Left,
    Right,
    Automatic,
    HideAll,
}

impl BasesState {
    fn group_property(&self) -> Result<&str> {
        let loaded = self.loaded.as_ref().context("base has not loaded")?;
        Ok(&loaded.document.config.views[loaded.view]
            .group_by
            .as_ref()
            .context("view has no groupBy property")?
            .property)
    }
    fn group_yaml(&self, index: usize) -> Result<Value> {
        let loaded = self.loaded.as_ref().context("base has not loaded")?;
        let group = self
            .result()
            .context("query unavailable")?
            .groups
            .get(index)
            .context("group no longer exists")?;
        if matches!(group.value, super::value::Value::Null) {
            return Ok(Value::Null);
        }
        let property = self.group_property()?;
        if let Ok(key) = super::edits::property_key(property) {
            if let Some(row) = group
                .rows
                .first()
                .and_then(|row| self.result()?.rows.get(*row))
                && let Some(value) = loaded
                    .metadata
                    .file(&row.id)
                    .context("row is absent from public metadata")?
                    .properties()?
                    .get(key)
            {
                return super::edits::retarget_group_links(
                    &loaded.metadata,
                    value,
                    &row.id,
                    &self.id,
                );
            }
            if loaded.metadata.property_types.declared_type(key) == Some("date")
                && let super::value::Value::Date(date) = &group.value
            {
                return Ok(Value::String(date.format("%Y-%m-%d").to_string()));
            }
        }
        let source = match &group.value {
            super::value::Value::Link(link) => link.source.as_str(),
            super::value::Value::File(_) | super::value::Value::Host(_) => "",
            _ => group
                .rows
                .first()
                .and_then(|index| self.result()?.rows.get(*index))
                .map_or(self.id.as_str(), |row| row.id.as_str()),
        };
        super::edits::retarget_group_links(
            &loaded.metadata,
            &group.value.to_yaml()?,
            source,
            &self.id,
        )
    }
    pub(super) fn change_group_order(
        &mut self,
        app: &mut App,
        action: GroupAction,
        source: &str,
    ) -> Result<()> {
        self.ensure_idle()?;
        self.group_property()?;
        let result = self.result().context("query unavailable")?;
        let mut order = if matches!(action, GroupAction::Automatic | GroupAction::HideAll) {
            Vec::new()
        } else {
            (0..result.groups.len())
                .map(|index| self.group_yaml(index))
                .collect::<Result<Vec<_>>>()?
        };
        match action {
            GroupAction::Add => {
                let value = super::authoring::yaml_value(source)?;
                if order.contains(&value) {
                    bail!("group already exists");
                }
                let property = self.group_property()?;
                if let Ok(key) = super::edits::property_key(property)
                    && !value.is_null()
                {
                    self.loaded
                        .as_ref()
                        .context("base has not loaded")?
                        .metadata
                        .property_types
                        .validate(key, &value)?;
                }
                order.push(value);
            }
            GroupAction::Hide | GroupAction::Left | GroupAction::Right => {
                let (index, _) = self.focused_group().context("select a group column")?;
                match action {
                    GroupAction::Hide => {
                        order.remove(index);
                    }
                    GroupAction::Left | GroupAction::Right => {
                        let next = if matches!(action, GroupAction::Left) {
                            index.saturating_sub(1)
                        } else {
                            index.saturating_add(1).min(order.len().saturating_sub(1))
                        };
                        order.swap(index, next);
                    }
                    _ => unreachable!("group selection action"),
                }
            }
            GroupAction::Automatic | GroupAction::HideAll => {}
        }
        self.save_group_order(
            app,
            (!matches!(action, GroupAction::Automatic)).then_some(order),
        )
    }
    fn save_group_order(&mut self, app: &mut App, order: Option<Vec<Value>>) -> Result<()> {
        let loaded = self.loaded.as_ref().context("base has not loaded")?;
        let document = loaded.document.set_view_field(
            loaded.view,
            "groupOrder",
            order.map(Value::Sequence).as_ref(),
        )?;
        super::query::Query::compile(&document, loaded.view)?;
        let source = loaded.configuration_source(app, &document)?;
        self.write_task = Some(WriteTask::for_app(
            app,
            Operation::Save {
                baseline: loaded.snapshot.clone(),
                source,
                embedded: self.embedded_request.clone(),
            },
        ));
        Ok(())
    }
    pub(super) fn new_note_target(&self) -> Result<Target> {
        self.ensure_idle()?;
        let group = if matches!(self.layout(), Layout::Kanban) {
            let property = self.group_property()?;
            if property != "file.folder" {
                super::edits::property_key(property)?;
            }
            let (index, _) = self
                .focused_group()
                .context("select a group column before creating a note")?;
            let value = self.group_yaml(index)?;
            Some((property.to_owned(), (!value.is_null()).then_some(value)))
        } else {
            None
        };
        Ok(Target::NewNote { group })
    }
    pub(super) fn prompt_new_note(&mut self) -> Result<()> {
        let target = self.new_note_target()?;
        let initial = if let Target::NewNote {
            group: Some((property, Some(Value::String(folder)))),
        } = &target
        {
            if property == "file.folder" && !folder.trim_matches('/').is_empty() {
                format!("{}/New note.md", folder.trim_matches('/'))
            } else {
                "New note.md".into()
            }
        } else {
            "New note.md".into()
        };
        self.dialog = Some(Dialog::prompt(
            target,
            "New note in current view",
            "Vault-relative .md path; Kanban uses focused column's property or folder",
            &initial,
        ));
        Ok(())
    }
    pub(super) fn start_folder_move(
        &mut self,
        app: &mut App,
        ids: Vec<String>,
        folder: String,
    ) -> Result<()> {
        self.ensure_idle()?;
        let metadata = self
            .loaded
            .as_ref()
            .context("base has not loaded")?
            .metadata
            .clone();
        self.write_task = Some(WriteTask::for_app(
            app,
            Operation::MovePublic {
                ids,
                folder,
                metadata,
            },
        ));
        Ok(())
    }
    pub(super) fn drop_in_group(&mut self, app: &mut App, item: Item, target: usize) -> Result<()> {
        self.ensure_idle()?;
        let property = self.group_property()?;
        if property != "file.folder" {
            super::edits::property_key(property)?;
        }
        match item {
            Item::Group(source) => {
                if source == target {
                    return Ok(());
                }
                let mut order = (0..self.result().context("query unavailable")?.groups.len())
                    .map(|index| self.group_yaml(index))
                    .collect::<Result<Vec<_>>>()?;
                let value = order.remove(source);
                order.insert(target, value);
                self.save_group_order(app, Some(order))
            }
            Item::Row(row) => {
                let result = self.result().context("query unavailable")?;
                let source = result
                    .groups
                    .iter()
                    .position(|group| group.rows.contains(&row))
                    .context("card has no group")?;
                if source == target {
                    return Ok(());
                }
                let id = &result.rows.get(row).context("card no longer exists")?.id;
                let ids = if self.selected_ids.contains(id) {
                    result
                        .rows
                        .iter()
                        .filter(|row| self.selected_ids.contains(&row.id))
                        .map(|row| row.id.clone())
                        .collect()
                } else {
                    vec![id.clone()]
                };
                let value = self.group_yaml(target)?;
                if property == "file.folder" {
                    let folder = if value.is_null() {
                        String::new()
                    } else {
                        value
                            .as_str()
                            .context("folder group must be text")?
                            .to_owned()
                    };
                    return self.start_folder_move(app, ids, folder);
                }
                self.start_edit(
                    app,
                    EditIntent::Group {
                        ids,
                        value: (!value.is_null()).then_some(value),
                        metadata: self
                            .loaded
                            .as_ref()
                            .context("base has not loaded")?
                            .metadata
                            .clone(),
                        source: self.id.clone(),
                    },
                )
            }
        }
    }
}
