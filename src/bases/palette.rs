//! Palette continuations retain Bases targets; authoring and write tasks stay in Bases.
use super::{
    authoring::{Command, Dialog, EditIntent, Target},
    state::BasesState,
    write::{Operation, Outcome, WriteTask},
};
use crate::{
    app::App,
    palette::{CommandPalette, workflows::Workflow},
};
use anyhow::{Context, Result, ensure};

pub(crate) const COMMANDS: &[(&str, Command, &str, &str)] = &[
    (
        "bases.config.rename_view",
        Command::View("name"),
        "Rename view",
        "View name",
    ),
    (
        "bases.config.layout",
        Command::View("type"),
        "Layout",
        "table, list, cards or kanban",
    ),
    (
        "bases.config.global_filter",
        Command::GlobalFilter,
        "Global filter",
        "Expression; blank removes filter",
    ),
    (
        "bases.config.view_filter",
        Command::View("filters"),
        "View filter",
        "Expression; blank removes filter",
    ),
    (
        "bases.config.properties",
        Command::View("order"),
        "Displayed properties",
        "YAML list: [file.name, status, formula.Total]",
    ),
    (
        "bases.config.sort",
        Command::View("sort"),
        "Row sorting",
        "YAML list: [{property: score, direction: DESC}]",
    ),
    (
        "bases.config.group_by",
        Command::View("groupBy"),
        "Grouping",
        "YAML mapping: {property: status, direction: ASC}",
    ),
    (
        "bases.config.group_order",
        Command::View("groupOrder"),
        "Manual group order",
        "YAML list: [Open, Done, null]; blank = automatic",
    ),
    (
        "bases.config.limit",
        Command::View("limit"),
        "Row limit",
        "Non-negative integer; blank = unlimited",
    ),
    (
        "bases.config.row_height",
        Command::View("rowHeight"),
        "Table row height",
        "medium, tall or extra; blank = short",
    ),
    (
        "bases.config.column_widths",
        Command::View("columnSize"),
        "Table column widths",
        "YAML pixel-width mapping: {file.name: 240, score: 120}; blank = default",
    ),
    (
        "bases.config.markers",
        Command::View("markers"),
        "List markers",
        "bullet, number or none; blank = bullet",
    ),
    (
        "bases.config.indent_properties",
        Command::View("indentProperties"),
        "Indent list properties",
        "true or false; blank = false",
    ),
    (
        "bases.config.separator",
        Command::View("separator"),
        "List property separator",
        "YAML text; blank or empty text = comma and space",
    ),
    (
        "bases.config.card_width",
        Command::View("cardSize"),
        "Card width",
        "Width in pixels; blank = default",
    ),
    (
        "bases.config.kanban_width",
        Command::View("columnWidth"),
        "Kanban column width",
        "Width in pixels; blank = default",
    ),
    (
        "bases.config.cover",
        Command::View("image"),
        "Cover property",
        "Property name, e.g. note.cover or formula.Cover; blank = none",
    ),
    (
        "bases.config.image_fit",
        Command::View("imageFit"),
        "Image fit",
        "cover or contain; blank = default",
    ),
    (
        "bases.config.image_aspect_ratio",
        Command::View("imageAspectRatio"),
        "Image aspect ratio",
        "Width / height; positive number; blank = 1",
    ),
    (
        "bases.config.column_summaries",
        Command::View("summaries"),
        "Column summaries",
        "YAML mapping: {score: Average}",
    ),
    (
        "bases.config.formula",
        Command::Formula,
        "Formula",
        "name = expression; blank expression removes formula",
    ),
    (
        "bases.config.summary",
        Command::Summary,
        "Custom summary",
        "name = expression; blank expression removes summary",
    ),
    (
        "bases.config.property_label",
        Command::Label,
        "Property label",
        "property = display name",
    ),
    (
        "bases.config.add_view",
        Command::AddView,
        "Add view",
        "layout = name; layout: table, list, cards, kanban",
    ),
    (
        "bases.config.remove_view",
        Command::RemoveView,
        "Remove current view",
        "Type DELETE to confirm; last view cannot be removed",
    ),
    (
        "bases.config.new_note",
        Command::NewNote,
        "New note",
        "Vault-relative Markdown path; existing files never overwritten",
    ),
    (
        "bases.config.export",
        Command::Export,
        "Export visible rows",
        "Vault-relative .csv or .md path; existing files never overwritten",
    ),
    (
        "bases.config.add_group",
        Command::Group(super::groups::GroupAction::Add),
        "Add empty group",
        "YAML value for new group; null = None",
    ),
    (
        "bases.config.hide_group",
        Command::Group(super::groups::GroupAction::Hide),
        "Hide focused group",
        "",
    ),
    (
        "bases.config.group_left",
        Command::Group(super::groups::GroupAction::Left),
        "Move focused group left",
        "",
    ),
    (
        "bases.config.group_right",
        Command::Group(super::groups::GroupAction::Right),
        "Move focused group right",
        "",
    ),
    (
        "bases.config.automatic_groups",
        Command::Group(super::groups::GroupAction::Automatic),
        "Automatic group order / show all",
        "",
    ),
    (
        "bases.config.hide_groups",
        Command::Group(super::groups::GroupAction::HideAll),
        "Hide all groups",
        "",
    ),
    (
        "bases.config.source",
        Command::Source,
        "Edit Bases source",
        "Open public source for repair",
    ),
];
pub(crate) fn configuration_command(id: &str) -> Option<usize> {
    COMMANDS
        .iter()
        .position(|(command_id, ..)| *command_id == id)
}

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

pub(crate) struct Choices {
    options: &'static [(&'static str, &'static str)],
    current: String,
}

impl Choices {
    fn for_target(target: &Target, state: &BasesState) -> Option<Self> {
        let Target::Config {
            command: Command::View(key),
            view,
            ..
        } = target
        else {
            return None;
        };
        let options: &[(&str, &str)] = match *key {
            "type" => &[
                ("table", "Table"),
                ("list", "List"),
                ("cards", "Cards"),
                ("kanban", "Kanban"),
            ],
            "rowHeight" => &[
                ("", "Short (default)"),
                ("medium", "Medium"),
                ("tall", "Tall"),
                ("extra", "Extra"),
            ],
            "markers" => &[
                ("", "Bullet (default)"),
                ("number", "Numbered"),
                ("none", "None"),
            ],
            "indentProperties" => &[("", "Off (default)"), ("true", "On")],
            "imageFit" => &[("", "Cover (default)"), ("contain", "Contain")],
            _ => return None,
        };
        let view = state.loaded.as_ref()?.document.config.views.get(*view)?;
        let current = match (*key, view.options.get(*key)) {
            (_, None) | ("indentProperties", Some(serde_yaml_ng::Value::Bool(false))) => "",
            ("indentProperties", Some(serde_yaml_ng::Value::Bool(true))) => "true",
            ("indentProperties", _) => "Invalid value",
            ("markers", Some(serde_yaml_ng::Value::String(value))) if value == "bullet" => "",
            ("markers", Some(serde_yaml_ng::Value::String(value))) if value.is_empty() => {
                "Invalid value"
            }
            ("imageFit", Some(serde_yaml_ng::Value::String(value))) if value == "cover" => "",
            (_, Some(serde_yaml_ng::Value::String(value))) => value.as_str(),
            _ => "Invalid value",
        }
        .to_owned();
        Some(Self { options, current })
    }
}

pub(crate) enum Page {
    Running {
        title: String,
        selection: Selection,
    },
    Preparing {
        selection: Selection,
    },
    Prompt {
        target: Target,
        title: String,
        selection: Selection,
        submitting: bool,
        choices: Option<Choices>,
    },
}

impl Page {
    pub(crate) fn is_destructive(&self) -> bool {
        matches!(
            self,
            Self::Prompt {
                target: Target::Cell { clear: true, .. }
                    | Target::Config {
                        command: Command::RemoveView,
                        ..
                    },
                ..
            }
        )
    }

    pub(crate) fn title(&self) -> &str {
        match self {
            Self::Running { title, .. } => title,
            Self::Preparing { .. } => "COMMANDS › Bases › Prepare property",
            Self::Prompt { title, .. } => title,
        }
    }
    pub(crate) fn hint(&self) -> &str {
        match self {
            Self::Prompt { .. } => "",
            Self::Preparing { .. } => "Preparing captured public property…",
            Self::Running { .. } => "Operation in progress…",
        }
    }
    pub(crate) fn is_prompt(&self) -> bool {
        matches!(self, Self::Prompt { choices: None, .. })
    }
    pub(crate) fn current_value(&self) -> Option<&str> {
        let Self::Prompt {
            choices: Some(choices),
            ..
        } = self
        else {
            return None;
        };
        Some(
            choices
                .options
                .iter()
                .find(|(value, _)| *value == choices.current)
                .map_or(choices.current.as_str(), |(_, name)| *name),
        )
    }
    pub(crate) fn rows(&self) -> Vec<(String, String, String)> {
        match self {
            Self::Prompt {
                choices: Some(choices),
                ..
            } => choices
                .options
                .iter()
                .map(|(value, name)| {
                    (
                        (*value).into(),
                        (*name).into(),
                        if *value == choices.current {
                            "Current".into()
                        } else {
                            String::new()
                        },
                    )
                })
                .collect(),
            Self::Preparing { .. } => vec![(
                "retry".into(),
                "Retry preparation".into(),
                "Wait for the current read, or retry after an error".into(),
            )],
            Self::Prompt { .. } | Self::Running { .. } => Vec::new(),
        }
    }
}

impl BasesState {
    pub(crate) fn present_dialog(&mut self, app: &mut App, dialog: Dialog) {
        let confirmation = matches!(
            &dialog,
            Dialog::Prompt {
                target: Target::Cell { clear: true, .. },
                ..
            }
        );
        let integrated = app.command_palette.as_ref().is_some_and(|palette| {
            palette
                .launching_action
                .as_deref()
                .or(palette.workflow_action.as_deref())
                .is_some_and(|id| {
                    configuration_command(id).is_some()
                        || matches!(
                            id,
                            "bases.edit"
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
            Dialog::Prompt {
                target,
                title,
                hint,
                input,
                ..
            } => {
                if let Some(Workflow::Bases(page)) = &palette.workflow
                    && let Page::Preparing { selection } | Page::Running { selection, .. } = &**page
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
                let mut styled_input = crate::ui::make_popup_textarea(&app.app_theme, &hint);
                styled_input.insert_str(input.lines().join("\n"));
                (
                    Page::Prompt {
                        choices: Choices::for_target(&target, self),
                        target,
                        title: format!("COMMANDS › Bases › {title}"),
                        selection: Selection::capture(self),
                        submitting: false,
                    },
                    styled_input,
                )
            }
            Dialog::Image { .. } => unreachable!("image handled separately"),
        };
        if palette.workflow_action.is_none() {
            palette.workflow_action = palette.launching_action.clone();
        }
        let initial_choice = match &page {
            Page::Prompt {
                choices: Some(choices),
                ..
            } => Some(choices.current.clone()),
            _ => None,
        };
        let input = if initial_choice.is_some() {
            crate::ui::make_popup_textarea(&app.app_theme, "Search options...")
        } else {
            input
        };
        palette.push_workflow(Workflow::Bases(Box::new(page)), input, app);
        if let Some(current) = initial_choice
            && let Some(index) = palette.items.iter().position(|item| item.id == current)
        {
            palette.state.select(Some(index));
        }
        app.command_palette = Some(palette);
    }

    /// Keyboard/mouse palette commands call the same authoring transition.
    pub(crate) fn run_configuration_command(&mut self, app: &mut App, index: usize) -> Result<()> {
        self.ensure_idle()?;
        let &(id, command, title, hint) = COMMANDS
            .get(index)
            .context("Configuration command unavailable")?;
        if let Some(mut palette) = app.command_palette.take() {
            if palette.launching_action.as_deref() == Some(id) {
                palette.workflow_action = Some(id.into());
                palette.push_workflow(
                    Workflow::Bases(Box::new(Page::Running {
                        title: format!("COMMANDS › Bases › {title}"),
                        selection: Selection::capture(self),
                    })),
                    crate::ui::make_popup_textarea(&app.app_theme, ""),
                    app,
                );
            }
            app.command_palette = Some(palette);
        }
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

    pub(crate) fn finish_palette_bases_confirmation(&mut self) {
        if self
            .bases_state
            .as_ref()
            .is_some_and(|state| state.dialog.is_none() && !state.is_writing())
            && let Some(palette) = &mut self.command_palette
            && matches!(palette.bases_page(), Some(Page::Running { .. }))
        {
            palette.pop_workflow();
        }
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
                Page::Preparing { selection }
                | Page::Prompt { selection, .. }
                | Page::Running { selection, .. } => {
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
        self.command_palette = Some(palette);
        let mut state = self.bases_state.take().expect("validated Bases state");
        let mut palette = self.command_palette.take().expect("palette");
        let option = palette.items.get(selected).map(|item| item.id.clone());
        let result = match palette.workflow.as_mut().expect("Bases workflow") {
            Workflow::Bases(page) => match &mut **page {
                Page::Running { .. } => {
                    self.command_palette = Some(palette);
                    Ok(())
                }
                Page::Preparing { .. } => {
                    self.command_palette = Some(palette);
                    state.start_edit(self, EditIntent::Edit)
                }
                Page::Prompt {
                    target,
                    submitting,
                    choices,
                    ..
                } => {
                    let source = if let Some(choices) = choices {
                        option
                            .filter(|id| choices.options.iter().any(|(value, _)| *value == id))
                            .context("Configuration option unavailable")
                    } else {
                        Ok(palette.input.lines().join("\n"))
                    };
                    let result = source.and_then(|source| state.submit(self, target, &source));
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
