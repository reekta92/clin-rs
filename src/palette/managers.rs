use super::{CommandPalette, workflows::Workflow};
use crate::{
    app::{
        App,
        vaults::{VaultSwitcher, same_vault},
    },
    popups::{ConfirmAction, SubnotesPopup},
};

impl CommandPalette {
    pub(crate) fn multiline_input(&self) -> bool {
        if self
            .bases_page()
            .is_some_and(crate::bases::palette::Page::is_prompt)
        {
            return true;
        }
        matches!(
            self.workflow,
            Some(Workflow::SubnoteField { content: true, .. })
        )
    }

    pub(crate) fn input_height(&self, height: u16) -> u16 {
        if self.multiline_input() {
            (height / 2).clamp(3, 10)
        } else {
            3
        }
    }

    pub(crate) fn manager_rows(&self, app: &App) -> Vec<(String, String, String)> {
        let clean = crate::fsutil::sanitize_for_terminal;
        match &self.workflow {
            Some(Workflow::Vault(switcher)) => {
                let mut rows: Vec<_> = switcher
                    .vaults
                    .iter()
                    .map(|row| {
                        let name = app
                            .config
                            .core
                            .vault_names
                            .get(&row.raw_path)
                            .cloned()
                            .unwrap_or_else(|| row.path.to_string_lossy().into_owned());
                        (
                            format!("vault:{}", row.raw_path),
                            clean(&name).into_owned(),
                            format!(
                                "{} · F2 rename · Delete remove",
                                clean(&row.path.to_string_lossy())
                            ),
                        )
                    })
                    .collect();
                rows.push((
                    "add".into(),
                    "+ Add new vault…".into(),
                    "Open system directory picker".into(),
                ));
                rows
            }
            Some(Workflow::Subnotes(popup)) => {
                let mut rows: Vec<_> = popup
                    .subnotes
                    .iter()
                    .map(|note| {
                        (
                            note.id.clone(),
                            clean(&note.title).into_owned(),
                            clean(note.content.lines().next().unwrap_or("")).into_owned(),
                        )
                    })
                    .collect();
                rows.push((
                    "add".into(),
                    "+ Add subnote".into(),
                    "Create and edit a subnote".into(),
                ));
                rows
            }
            Some(Workflow::SubnoteMenu { .. }) => [
                ("title", "Edit title"),
                ("content", "Edit content"),
                ("external", "Edit in external editor"),
                ("delete", "Delete subnote"),
            ]
            .into_iter()
            .map(|(id, name)| (id.into(), name.into(), String::new()))
            .collect(),
            _ => Vec::new(),
        }
    }

    fn subnotes_mut(&mut self) -> Option<&mut SubnotesPopup> {
        if let Some(Workflow::Subnotes(popup)) = &mut self.workflow {
            return Some(popup);
        }
        self.parents
            .iter_mut()
            .rev()
            .find_map(|parent| match &mut parent.workflow {
                Some(Workflow::Subnotes(popup)) => Some(&mut **popup),
                _ => None,
            })
    }

    pub(crate) fn finish_subnote_step(&mut self, app: &mut App, close: bool) -> anyhow::Result<()> {
        if let Some(Workflow::SubnoteField { id, content }) = &self.workflow {
            let id = id.clone();
            let content = *content;
            let value = self.input.lines().join(if content { "\n" } else { "" });
            let popup = self
                .subnotes_mut()
                .ok_or_else(|| anyhow::anyhow!("Subnote manager unavailable"))?;
            let note = popup
                .subnotes
                .iter_mut()
                .find(|note| note.id == id)
                .ok_or_else(|| anyhow::anyhow!("Subnote no longer exists"))?;
            if content {
                note.content = value;
            } else {
                note.title = value;
            }
            note.updated_at = crate::ui::now_unix_secs();
            popup.is_dirty = true;
        }
        if close || matches!(self.workflow, Some(Workflow::Subnotes(_))) {
            let dirty = self.subnotes_mut().is_some_and(|popup| popup.is_dirty);
            if dirty {
                crate::actions::validate_action_context(
                    "manage_subnotes_list",
                    app,
                    &self.context,
                )?;
            }
            if let Some(popup) = self.subnotes_mut() {
                app.save_subnotes_state(popup)?;
                popup.is_dirty = false;
            }
        }
        Ok(())
    }
}

impl App {
    pub(crate) fn present_vault_switcher(&mut self, switcher: VaultSwitcher) {
        if let Some(mut palette) = self.command_palette.take() {
            if palette.launching_action.is_some() {
                palette.workflow_action = palette.launching_action.clone();
                let input = crate::ui::make_popup_textarea(&self.app_theme, "Search vaults...");
                palette.push_workflow(Workflow::Vault(Box::new(switcher)), input, self);
                self.command_palette = Some(palette);
                return;
            }
            self.command_palette = Some(palette);
        }
        self.vault_switcher = Some(switcher);
    }

    pub(crate) fn remove_palette_vault(&mut self, path: &str) {
        if let Some(palette) = &mut self.command_palette
            && let Some(Workflow::Vault(switcher)) = &mut palette.workflow
        {
            switcher
                .vaults
                .retain(|row| !same_vault(&row.path, &crate::config::expand_path(path)));
        }
    }

    pub(crate) fn palette_manager_shortcut(&mut self, key: crossterm::event::KeyEvent) -> bool {
        use crossterm::event::{KeyCode, KeyEventKind};
        if key.kind != KeyEventKind::Press || !matches!(key.code, KeyCode::F(2) | KeyCode::Delete) {
            return false;
        }
        let Some(palette) = &self.command_palette else {
            return false;
        };
        if !matches!(palette.workflow, Some(Workflow::Vault(_))) {
            return false;
        }
        let target = palette
            .state
            .selected()
            .and_then(|i| palette.items.get(i))
            .and_then(|item| item.id.strip_prefix("vault:"));
        let Some(target) = target else { return true };
        let id = format!(
            "{}:{target}",
            if key.code == KeyCode::F(2) {
                "rename"
            } else {
                "remove"
            }
        );
        let palette = self.command_palette.take().expect("palette");
        self.activate_palette_manager(palette, &id);
        true
    }

    pub(crate) fn activate_palette_manager(&mut self, mut palette: CommandPalette, id: &str) {
        let mut workflow = palette.workflow.take().expect("manager");
        let mut next = None;
        let mut back = false;
        let result = (|| -> anyhow::Result<bool> {
            crate::actions::validate_action_context(
                palette.workflow_action.as_deref().unwrap_or(""),
                self,
                &palette.context,
            )?;
            match &mut workflow {
                Workflow::Vault(switcher) => {
                    if id == "add" {
                        if let Some(row) = self.pick_and_add_vault()? {
                            if !switcher
                                .vaults
                                .iter()
                                .any(|old| same_vault(&old.path, &row.path))
                            {
                                switcher.vaults.push(row);
                            }
                        }
                    } else {
                        let (action, key) = id
                            .split_once(':')
                            .ok_or_else(|| anyhow::anyhow!("Select a vault"))?;
                        let row = switcher
                            .vaults
                            .iter()
                            .find(|row| row.raw_path == key)
                            .ok_or_else(|| anyhow::anyhow!("Vault no longer exists"))?;
                        match action {
                            "rename" => {
                                let mut input = crate::ui::make_popup_textarea(
                                    &self.app_theme,
                                    "Vault display name",
                                );
                                if let Some(name) = self.config.core.vault_names.get(key) {
                                    input.insert_str(name);
                                }
                                next = Some((Workflow::VaultName { key: key.into() }, input));
                            }
                            "remove" => {
                                anyhow::ensure!(
                                    !same_vault(&row.path, &self.storage.data_dir),
                                    "Cannot remove the active vault"
                                );
                                self.show_confirm(ConfirmAction::RemoveVault {
                                    path: row.path.to_string_lossy().into_owned(),
                                });
                            }
                            _ => {
                                self.switch_vault(row.path.clone());
                                if self.setup_rebootstrap.is_some() {
                                    return Ok(true);
                                }
                                anyhow::ensure!(
                                    same_vault(&row.path, &self.storage.data_dir),
                                    "{}",
                                    self.status
                                );
                            }
                        }
                    }
                }
                Workflow::VaultName { key } => {
                    self.apply_vault_alias(key, &palette.input.lines().join(" "))?;
                    back = true;
                }
                Workflow::Subnotes(popup) => {
                    let id = if id == "add" {
                        let id = uuid::Uuid::new_v4().to_string();
                        popup.subnotes.push(crate::storage::SubNote {
                            id: id.clone(),
                            title: "New Note".into(),
                            content: String::new(),
                            updated_at: crate::ui::now_unix_secs(),
                        });
                        popup.is_dirty = true;
                        id
                    } else {
                        anyhow::ensure!(
                            popup.subnotes.iter().any(|note| note.id == id),
                            "Subnote no longer exists"
                        );
                        id.to_owned()
                    };
                    next = Some((
                        Workflow::SubnoteMenu { id },
                        crate::ui::make_popup_textarea(
                            &self.app_theme,
                            "Search subnote actions...",
                        ),
                    ));
                }
                Workflow::SubnoteMenu { id: note_id } => {
                    let popup = palette
                        .subnotes_mut()
                        .ok_or_else(|| anyhow::anyhow!("Subnote manager unavailable"))?;
                    let index = popup
                        .subnotes
                        .iter()
                        .position(|note| &note.id == note_id)
                        .ok_or_else(|| anyhow::anyhow!("Subnote no longer exists"))?;
                    match id {
                        "title" | "content" => {
                            let content = id == "content";
                            let mut input = crate::ui::make_popup_textarea(
                                &self.app_theme,
                                if content {
                                    "Subnote content · Ctrl+Enter apply"
                                } else {
                                    "Subnote title"
                                },
                            );
                            input.insert_str(if content {
                                &popup.subnotes[index].content
                            } else {
                                &popup.subnotes[index].title
                            });
                            next = Some((
                                Workflow::SubnoteField {
                                    id: note_id.clone(),
                                    content,
                                },
                                input,
                            ));
                        }
                        "delete" => {
                            popup.subnotes.remove(index);
                            popup.is_dirty = true;
                            back = true;
                        }
                        "external" => {
                            popup.selected = index;
                            popup.title_input = crate::ui::make_popup_textarea(&self.app_theme, "");
                            popup.title_input.insert_str(&popup.subnotes[index].title);
                            popup.content_input =
                                crate::ui::make_popup_textarea(&self.app_theme, "");
                            popup
                                .content_input
                                .insert_str(&popup.subnotes[index].content);
                            self.edit_subnote_external(popup);
                        }
                        _ => anyhow::bail!("Select a subnote action"),
                    }
                }
                Workflow::SubnoteField { .. } => {
                    back = true;
                }
                _ => unreachable!("manager dispatch"),
            }
            Ok(false)
        })();
        palette.workflow = Some(workflow);
        if let Err(error) = result {
            palette.error = Some(format!("{error:#}"));
        } else if matches!(result, Ok(true)) {
            self.dismiss_command_palette();
            return;
        } else if let Some((workflow, input)) = next {
            palette.push_workflow(workflow, input, self);
        } else if back {
            self.leave_palette_step(palette, false);
            return;
        }
        palette.refresh_items(self);
        self.command_palette = Some(palette);
    }
}
