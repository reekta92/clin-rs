//! Palette continuations retain Bases targets; authoring and write tasks stay in Bases.
use super::{
    authoring::{COMMANDS, Command, Dialog, EditIntent, Target},
    state::BasesState,
    write::{Operation, Outcome, WriteTask},
};
use crate::{
    app::App,
    palette::{CommandPalette, workflows::Workflow},
    storage::documents::DocumentSnapshot,
};
use anyhow::{Context, Result, ensure};

#[derive(Clone, PartialEq)]
pub(crate) struct Selection {
    view: usize,
    row: Option<String>,
    property: Option<String>,
    group: Option<super::eval::Identity>,
    ids: Vec<String>,
}

impl Selection {
    fn capture(state: &BasesState) -> Self {
        let mut ids: Vec<_> = state.selected_ids.iter().cloned().collect();
        ids.sort();
        Self {
            view: state.loaded.as_ref().map_or(0, |loaded| loaded.view),
            row: state.row().map(|row| row.id.clone()),
            property: state
                .loaded
                .as_ref()
                .and_then(|loaded| loaded.columns.get(state.column))
                .cloned(),
            group: state
                .focused_group()
                .and_then(|(group, _)| state.group_identity(group)),
            ids,
        }
    }
}

pub(crate) enum Page {
    Menu {
        baseline: DocumentSnapshot,
        view: usize,
    },
    Preparing {
        selection: Selection,
    },
    Prompt {
        target: Target,
        title: String,
        hint: String,
        selection: Selection,
        submitting: bool,
    },
}

impl Page {
    pub(crate) fn title(&self) -> &str {
        match self {
            Self::Menu { .. } => "COMMANDS › Bases configuration",
            Self::Preparing { .. } => "COMMANDS › Bases › Prepare property",
            Self::Prompt { title, .. } => title,
        }
    }
    pub(crate) fn hint(&self) -> &str {
        match self {
            Self::Prompt { hint, .. } => hint,
            Self::Preparing { .. } => "Preparing captured public property…",
            Self::Menu { .. } => "Choose an existing configuration command",
        }
    }
    pub(crate) fn is_prompt(&self) -> bool {
        matches!(self, Self::Prompt { .. })
    }
    pub(crate) fn rows(&self) -> Vec<(String, String, String)> {
        match self {
            Self::Menu { .. } => COMMANDS
                .iter()
                .enumerate()
                .map(|(i, (_, name, hint))| (i.to_string(), (*name).into(), (*hint).into()))
                .collect(),
            Self::Preparing { .. } => vec![(
                "retry".into(),
                "Retry preparation".into(),
                "Wait for the current read, or retry after an error".into(),
            )],
            Self::Prompt { .. } => Vec::new(),
        }
    }
}

impl BasesState {
    pub(crate) fn present_dialog(&mut self, app: &mut App, dialog: Dialog) {
        let confirmation = matches!(
            &dialog,
            Dialog::Prompt {
                target: Target::Cell { clear: true, .. }
                    | Target::Config {
                        command: Command::RemoveView,
                        ..
                    },
                ..
            }
        );
        let integrated = app.command_palette.as_ref().is_some_and(|palette| {
            palette
                .launching_action
                .as_deref()
                .or(palette.workflow_action.as_deref())
                .is_some_and(|id| {
                    matches!(
                        id,
                        "bases.configure"
                            | "bases.edit"
                            | "bases.new_note"
                            | "bases.export"
                            | "bases.move_to_group"
                    )
                })
        });
        if !integrated || confirmation || matches!(dialog, Dialog::Image { .. }) {
            self.dialog = Some(dialog);
            return;
        }
        let mut palette = app
            .command_palette
            .take()
            .expect("integrated Bases palette");
        let (page, input) = match dialog {
            Dialog::Menu { .. } => {
                let Some(loaded) = &self.loaded else {
                    app.command_palette = Some(palette);
                    return;
                };
                (
                    Page::Menu {
                        baseline: loaded.snapshot.clone(),
                        view: loaded.view,
                    },
                    crate::ui::make_popup_textarea(
                        &app.app_theme,
                        "Search configuration commands...",
                    ),
                )
            }
            Dialog::Prompt {
                target,
                title,
                hint,
                input,
                ..
            } => {
                if let Some(Workflow::Bases(page)) = &palette.workflow
                    && let Page::Preparing { selection } = &**page
                {
                    if *selection != Selection::capture(self) {
                        palette.error =
                            Some("Bases selection changed while preparing the property".into());
                        app.command_palette = Some(palette);
                        return;
                    }
                    let action = palette.workflow_action.clone();
                    palette.pop_workflow();
                    palette.workflow_action = action;
                }
                (
                    Page::Prompt {
                        target,
                        title,
                        hint,
                        selection: Selection::capture(self),
                        submitting: false,
                    },
                    *input,
                )
            }
            Dialog::Image { .. } => unreachable!("image handled separately"),
        };
        if palette.workflow_action.is_none() {
            palette.workflow_action = palette.launching_action.clone();
        }
        palette.push_workflow(Workflow::Bases(Box::new(page)), input, app);
        app.command_palette = Some(palette);
    }

    /// Keyboard/mouse legacy menus and palette menus call the same authoring transition.
    pub(crate) fn run_configuration_command(&mut self, app: &mut App, index: usize) -> Result<()> {
        self.ensure_idle()?;
        let &(command, title, hint) = COMMANDS
            .get(index)
            .context("Configuration command unavailable")?;
        let target = match command {
            Command::Source => {
                self.write_task = Some(WriteTask::for_app(
                    app,
                    Operation::OpenSource {
                        id: self.id.clone(),
                    },
                ));
                return Ok(());
            }
            Command::NewNote => return self.prompt_new_note(app),
            Command::Export => Target::Export,
            Command::Group(action) => {
                if !matches!(action, super::groups::GroupAction::Add) {
                    return self.change_group_order(app, action, "");
                }
                Target::GroupOrder {
                    action,
                    baseline: self
                        .loaded
                        .as_ref()
                        .context("Base has not loaded")?
                        .snapshot
                        .clone(),
                }
            }
            _ => {
                let loaded = self.loaded.as_ref().context("Base has not loaded")?;
                Target::Config {
                    command,
                    baseline: loaded.snapshot.clone(),
                    view: loaded.view,
                }
            }
        };
        self.present_dialog(app, Dialog::prompt(target, title, hint, ""));
        Ok(())
    }

    pub(crate) fn palette_preparing(&self, app: &mut App) {
        if self.edit_intent.is_none() {
            return;
        }
        if let Some(mut palette) = app.command_palette.take() {
            if palette.launching_action.as_deref() == Some("bases.edit") {
                palette.workflow_action = palette.launching_action.clone();
                let input = crate::ui::make_popup_textarea(&app.app_theme, "Preparing property...");
                palette.push_workflow(
                    Workflow::Bases(Box::new(Page::Preparing {
                        selection: Selection::capture(self),
                    })),
                    input,
                    app,
                );
            }
            app.command_palette = Some(palette);
        }
    }
}

impl CommandPalette {
    pub(crate) fn bases_page(&self) -> Option<&Page> {
        match &self.workflow {
            Some(Workflow::Bases(page)) => Some(page),
            _ => None,
        }
    }
}

impl App {
    pub(crate) fn palette_bases_dialog_open(&self) -> bool {
        self.command_palette
            .as_ref()
            .is_some_and(|p| p.bases_page().is_some())
            && self
                .bases_state
                .as_ref()
                .is_some_and(|state| state.dialog.is_some())
    }

    pub(crate) fn leave_palette_bases(&mut self, palette: &CommandPalette) -> Result<()> {
        if palette.bases_page().is_some()
            && let Some(state) = &mut self.bases_state
        {
            ensure!(
                !state.is_writing(),
                "Write in progress; wait before leaving Bases"
            );
            state.write_task = None;
            state.edit_intent = None;
        }
        Ok(())
    }

    pub(crate) fn activate_palette_bases(&mut self, mut palette: CommandPalette, selected: usize) {
        let result = (|| -> Result<()> {
            crate::actions::validate_action_context(
                palette.workflow_action.as_deref().unwrap_or(""),
                self,
                &palette.context,
            )?;
            let state = self
                .bases_state
                .as_ref()
                .context("Bases view unavailable")?;
            match palette.bases_page().context("Bases step unavailable")? {
                Page::Menu { baseline, view } => {
                    let loaded = state.loaded.as_ref().context("Base has not loaded")?;
                    ensure!(
                        loaded.snapshot.id() == baseline.id()
                            && loaded.snapshot.source() == baseline.source()
                            && loaded.view == *view,
                        "Bases document or view changed; reopen configuration"
                    );
                }
                Page::Preparing { selection } | Page::Prompt { selection, .. } => {
                    ensure!(
                        *selection == Selection::capture(state),
                        "Bases selection changed; reopen the action"
                    );
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            palette.error = Some(format!("{error:#}"));
            self.command_palette = Some(palette);
            return;
        }
        let index = palette
            .items
            .get(selected)
            .and_then(|item| item.id.parse::<usize>().ok());
        self.command_palette = Some(palette);
        let mut state = self.bases_state.take().expect("validated Bases state");
        let mut palette = self.command_palette.take().expect("palette");
        let result = match palette.workflow.as_mut().expect("Bases workflow") {
            Workflow::Bases(page) => match &mut **page {
                Page::Menu { .. } => {
                    self.command_palette = Some(palette);
                    state.run_configuration_command(self, index.unwrap_or(usize::MAX))
                }
                Page::Preparing { .. } => {
                    self.command_palette = Some(palette);
                    state.start_edit(self, EditIntent::Edit)
                }
                Page::Prompt {
                    target, submitting, ..
                } => {
                    let result = state.submit(self, target, &palette.input.lines().join("\n"));
                    if result.is_ok() {
                        *submitting = true;
                    }
                    self.command_palette = Some(palette);
                    result
                }
            },
            _ => unreachable!("Bases dispatch"),
        };
        self.bases_state = Some(state);
        if let Err(error) = result
            && let Some(palette) = &mut self.command_palette
        {
            palette.error = Some(format!("{error:#}"));
        }
    }

    pub(crate) fn finish_palette_bases_write(&mut self, error: Option<String>) {
        if self
            .command_palette
            .as_ref()
            .is_none_or(|p| p.bases_page().is_none())
        {
            return;
        }
        if let Some(error) = error {
            let palette = self.command_palette.as_mut().expect("Bases palette");
            palette.error = Some(error);
            if let Some(Workflow::Bases(page)) = &mut palette.workflow
                && let Page::Prompt { submitting, .. } = &mut **page
            {
                *submitting = false;
            }
        } else {
            self.dismiss_command_palette();
        }
    }
}

pub(crate) struct Creation {
    task: WriteTask,
}

impl Creation {
    pub(crate) fn start(app: &App, id: String) -> Self {
        Self {
            task: WriteTask::for_app(app, Operation::CreateBase { id }),
        }
    }
}

impl App {
    pub(crate) fn poll_palette_base_creation(&mut self) -> bool {
        let Some(palette) = &mut self.command_palette else {
            return false;
        };
        let Some(Workflow::Create {
            task: Some(creation),
            ..
        }) = &palette.workflow
        else {
            return false;
        };
        let Some(result) = creation.task.poll() else {
            return false;
        };
        match result {
            Ok(Outcome::CreatedBase(snapshot)) => {
                self.open_bases_view(snapshot.id(), None);
                self.request_notes_reconcile();
                self.dismiss_command_palette();
            }
            result => {
                palette.error = Some(match result {
                    Err(error) => format!("{error:#}"),
                    _ => "Unexpected creation result".into(),
                });
                if let Some(Workflow::Create { task, .. }) = &mut palette.workflow {
                    *task = None;
                }
            }
        }
        true
    }
}
