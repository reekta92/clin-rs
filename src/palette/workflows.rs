use super::{CommandPalette, PaletteItem};
use crate::{
    app::{App, ViewMode},
    popups::{
        ActivePopup, GoalsPopupMode, ImportSource, ImportTarget, NoteFormat, ThemePopup,
        ThemePopupFocus,
    },
};
use ratatui::widgets::ListState;
use ratatui_textarea::TextArea;

pub(crate) enum Workflow {
    Theme(Box<ThemePopup>),
    Goal(GoalsPopupMode),
    Options(OptionKind),
    Create {
        folder: String,
        format: NoteFormat,
    },
    Import {
        source: ImportSource,
        target: ImportTarget,
        note_id: Option<String>,
    },
    Vault(Box<crate::app::vaults::VaultSwitcher>),
    VaultName {
        key: String,
    },
    Subnotes(Box<crate::popups::SubnotesPopup>),
    SubnoteMenu {
        id: String,
    },
    SubnoteField {
        id: String,
        content: bool,
    },
}

#[derive(Clone, Copy)]
pub(crate) enum OptionKind {
    Sort,
    Icon,
    Hint,
    Preset,
}

impl OptionKind {
    fn labels(self) -> Vec<&'static str> {
        match self {
            Self::Sort => vec![
                "Title (A-Z)",
                "Title (Z-A)",
                "Modified (newest)",
                "Modified (oldest)",
            ],
            Self::Icon => vec!["Nerd Font", "Unicode", "None"],
            Self::Hint => crate::config::HintBarStyle::ALL
                .iter()
                .map(|style| style.name())
                .collect(),
            Self::Preset => vec![
                "default — Default CUA",
                "helix — Space leader",
                "vim — : commands",
                "emacs — Ctrl-x prefix",
            ],
        }
    }
}

pub(super) struct PaletteParent {
    input: TextArea<'static>,
    items: Vec<PaletteItem>,
    state: ListState,
    pub(super) workflow: Option<Workflow>,
    last_filter: Option<(String, usize)>,
}

impl CommandPalette {
    pub(crate) fn push_workflow(
        &mut self,
        workflow: Workflow,
        input: TextArea<'static>,
        app: &App,
    ) {
        self.parents.push(PaletteParent {
            input: std::mem::replace(&mut self.input, input),
            items: std::mem::take(&mut self.items),
            state: std::mem::take(&mut self.state),
            workflow: self.workflow.replace(workflow),
            last_filter: self.last_filter.take(),
        });
        self.last_scroll = None;
        self.scroll_drag = None;
        self.mouse_selection = Default::default();
        self.error = None;
        self.refresh_items(app);
    }

    pub(crate) fn pop_workflow(&mut self) {
        if let Some(parent) = self.parents.pop() {
            self.input = parent.input;
            self.items = parent.items;
            self.state = parent.state;
            self.workflow = parent.workflow;
            self.last_filter = parent.last_filter;
            self.last_scroll = None;
            self.scroll_drag = None;
            self.mouse_selection = Default::default();
            self.error = None;
            if self.workflow.is_none() {
                self.workflow_action = None;
            }
        }
    }

    pub(crate) fn workflow_title(&self) -> &str {
        match &self.workflow {
            Some(Workflow::Theme(_)) => "COMMANDS › Switch Theme",
            Some(Workflow::Goal(GoalsPopupMode::WordGoal)) => "COMMANDS › Daily Word Goal",
            Some(Workflow::Goal(GoalsPopupMode::NoteGoal)) => "COMMANDS › Daily Note Goal",
            Some(Workflow::Options(OptionKind::Sort)) => "COMMANDS › Sort Order",
            Some(Workflow::Options(OptionKind::Icon)) => "COMMANDS › Icon Mode",
            Some(Workflow::Options(OptionKind::Hint)) => "COMMANDS › Hint Bar Style",
            Some(Workflow::Options(OptionKind::Preset)) => "COMMANDS › Navigation Preset",
            Some(Workflow::Create {
                format: NoteFormat::Draw,
                ..
            }) => "COMMANDS › New Drawing Name",
            Some(Workflow::Create {
                format: NoteFormat::Canvas,
                ..
            }) => "COMMANDS › New Canvas Name",
            Some(Workflow::Create { .. }) => "COMMANDS › New Bases File Name",
            Some(Workflow::Import { .. }) => "COMMANDS › URL",
            Some(Workflow::Vault(_)) => "COMMANDS › Switch Vault",
            Some(Workflow::VaultName { .. }) => "COMMANDS › Switch Vault › Rename",
            Some(Workflow::Subnotes(_)) => "COMMANDS › Subnotes",
            Some(Workflow::SubnoteMenu { .. }) => "COMMANDS › Subnotes › Selected subnote",
            Some(Workflow::SubnoteField { content: false, .. }) => "COMMANDS › Subnotes › Title",
            Some(Workflow::SubnoteField { content: true, .. }) => "COMMANDS › Subnotes › Content",
            None => "COMMANDS",
        }
    }

    pub(crate) fn is_value_input(&self) -> bool {
        matches!(
            self.workflow,
            Some(
                Workflow::Goal(_)
                    | Workflow::Create { .. }
                    | Workflow::Import { .. }
                    | Workflow::VaultName { .. }
                    | Workflow::SubnoteField { .. }
            )
        )
    }

    pub(crate) fn refresh_workflow(&mut self, app: &App) {
        let mut rows = self.manager_rows(app);
        if let Some(Workflow::Options(kind)) = &self.workflow {
            rows = kind
                .labels()
                .into_iter()
                .enumerate()
                .map(|(i, label)| (i.to_string(), label.to_owned(), "Apply this option".into()))
                .collect();
        }
        if let Some(Workflow::Theme(popup)) = &self.workflow {
            let current = app
                .setup_state
                .as_ref()
                .filter(|_| app.mode == ViewMode::Setup)
                .map_or(app.config.ui.theme.as_str(), |state| {
                    state.themes[state.theme].as_str()
                });
            for (index, name) in popup.themes.iter().enumerate() {
                rows.push((
                    format!("theme:{index}"),
                    name.clone(),
                    if name == current {
                        "Current theme"
                    } else if popup.is_custom[index] {
                        "Custom theme"
                    } else {
                        "Built-in theme"
                    }
                    .to_owned(),
                ));
            }
            rows.push((
                "background".into(),
                "General background".into(),
                if popup.general_is_solid {
                    "Solid"
                } else {
                    "Transparent"
                }
                .into(),
            ));
            if !popup.setup_owned {
                rows.push((
                    "graph_background".into(),
                    "Graph background".into(),
                    if popup.graph_is_solid {
                        "Solid"
                    } else {
                        "Transparent"
                    }
                    .into(),
                ));
            }
        }
        if self.is_value_input() {
            self.items = vec![PaletteItem {
                id: "submit".into(),
                name: "Apply".into(),
                description: "Submit entered value".into(),
                glyph: String::new(),
                score: 0,
            }];
            self.state.select(Some(0));
        } else {
            self.filter_workflow_rows(rows);
        }
    }

    pub(crate) fn filter_workflow_rows(&mut self, rows: Vec<(String, String, String)>) {
        use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
        let query = &self.input.lines()[0];
        let changed = self
            .last_filter
            .as_ref()
            .is_none_or(|(previous, _)| previous != query);
        let selected = self
            .state
            .selected()
            .and_then(|i| self.items.get(i))
            .map(|item| item.id.clone());
        let matcher = SkimMatcherV2::default();
        self.items = rows
            .into_iter()
            .filter_map(|(id, name, description)| {
                let score = if query.is_empty() {
                    0
                } else {
                    matcher.fuzzy_match(&name, query)?
                };
                Some(PaletteItem {
                    id,
                    name,
                    description,
                    glyph: String::new(),
                    score,
                })
            })
            .collect();
        if !query.is_empty() {
            self.items.sort_by_key(|item| std::cmp::Reverse(item.score));
        }
        self.last_filter = Some((query.to_owned(), self.active_tab));
        self.state.select(
            selected
                .and_then(|id| self.items.iter().position(|item| item.id == id))
                .or_else(|| (changed && !self.items.is_empty()).then_some(0)),
        );
    }
}

impl App {
    /// Only explicit palette action launches opt into an integrated continuation.
    pub(crate) fn present_action_popup(&mut self, popup: ActivePopup) {
        let Some(mut palette) = self.command_palette.take() else {
            self.popups.active = Some(popup);
            return;
        };
        if palette.launching_action.is_none() {
            self.command_palette = Some(palette);
            self.popups.active = Some(popup);
            return;
        }
        let (workflow, input) = match popup {
            ActivePopup::Theme(popup) => (
                Workflow::Theme(Box::new(popup)),
                crate::ui::make_popup_textarea(&self.app_theme, "Search themes..."),
            ),
            ActivePopup::Goals(popup) => (Workflow::Goal(popup.mode), popup.input),
            ActivePopup::Sort(_) => (
                Workflow::Options(OptionKind::Sort),
                crate::ui::make_popup_textarea(&self.app_theme, "Search sort orders..."),
            ),
            ActivePopup::IconMode(_) => (
                Workflow::Options(OptionKind::Icon),
                crate::ui::make_popup_textarea(&self.app_theme, "Search icon modes..."),
            ),
            ActivePopup::HintBarStyle(_) => (
                Workflow::Options(OptionKind::Hint),
                crate::ui::make_popup_textarea(&self.app_theme, "Search hint styles..."),
            ),
            ActivePopup::KeybindPreset(_) => (
                Workflow::Options(OptionKind::Preset),
                crate::ui::make_popup_textarea(&self.app_theme, "Search navigation presets..."),
            ),
            ActivePopup::CreateNote(popup, format) => (
                Workflow::Create {
                    folder: popup.folder,
                    format,
                },
                popup.input,
            ),
            ActivePopup::Import(popup) => (
                Workflow::Import {
                    source: popup.source,
                    target: popup.target,
                    note_id: popup.note_id,
                },
                popup.input,
            ),
            ActivePopup::Subnotes(popup) => (
                Workflow::Subnotes(popup),
                crate::ui::make_popup_textarea(&self.app_theme, "Search subnotes..."),
            ),
            popup => {
                self.command_palette = Some(palette);
                self.popups.active = Some(popup);
                return;
            }
        };
        palette.workflow_action = palette.launching_action.clone();
        palette.push_workflow(workflow, input, self);
        self.command_palette = Some(palette);
    }

    pub(crate) fn leave_palette_step(&mut self, mut palette: CommandPalette, close: bool) {
        if let Err(error) = palette.finish_subnote_step(self, close) {
            palette.error = Some(format!("{error:#}"));
            self.command_palette = Some(palette);
            return;
        }
        if matches!(palette.workflow, Some(Workflow::Theme(_)))
            && self.mode != ViewMode::Setup
            && let Err(error) = self.config.save()
        {
            palette.error = Some(format!("Failed to save theme: {error}"));
            self.command_palette = Some(palette);
            return;
        }
        if close || palette.workflow.is_none() {
            self.dismiss_command_palette();
        } else {
            palette.pop_workflow();
            if palette.workflow.is_some() {
                palette.refresh_items(self);
            }
            self.command_palette = Some(palette);
        }
    }

    pub(crate) fn activate_palette_workflow(
        &mut self,
        mut palette: CommandPalette,
        selected: usize,
    ) {
        if matches!(
            palette.workflow,
            Some(
                Workflow::Vault(_)
                    | Workflow::VaultName { .. }
                    | Workflow::Subnotes(_)
                    | Workflow::SubnoteMenu { .. }
                    | Workflow::SubnoteField { .. }
            )
        ) {
            let id = palette
                .items
                .get(selected)
                .map(|item| item.id.clone())
                .unwrap_or_default();
            self.activate_palette_manager(palette, &id);
            return;
        }
        let result = (|| -> anyhow::Result<bool> {
            crate::actions::validate_action_context(
                palette
                    .workflow_action
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("Missing workflow action"))?,
                self,
                &palette.context,
            )?;
            match palette.workflow.as_mut().expect("workflow") {
                Workflow::Theme(popup) => {
                    let Some(item) = palette.items.get(selected) else {
                        return Ok(false);
                    };
                    popup.focus = match item.id.as_str() {
                        "background" => ThemePopupFocus::GeneralBg,
                        "graph_background" => ThemePopupFocus::GraphBg,
                        id => {
                            popup.selected = id
                                .strip_prefix("theme:")
                                .and_then(|id| id.parse().ok())
                                .ok_or_else(|| anyhow::anyhow!("Theme unavailable"))?;
                            ThemePopupFocus::ThemeList
                        }
                    };
                    self.apply_theme_selection(popup);
                    Ok(false)
                }
                Workflow::Goal(mode) => {
                    self.apply_goal(*mode, &palette.input.lines().join(""))?;
                    Ok(true)
                }
                Workflow::Options(kind) => {
                    let Some(item) = palette.items.get(selected) else {
                        return Ok(false);
                    };
                    let index = item.id.parse::<usize>()?;
                    match kind {
                        OptionKind::Sort => self.apply_sort_selection(index),
                        OptionKind::Icon => self.apply_icon_selection(index)?,
                        OptionKind::Hint => self.apply_hint_selection(index)?,
                        OptionKind::Preset => self.apply_preset_selection(index)?,
                    }
                    Ok(matches!(kind, OptionKind::Sort | OptionKind::Icon))
                }
                Workflow::Create { folder, format } => {
                    self.validate_document_creation(
                        folder,
                        *format,
                        &palette.input.lines().join(""),
                    )?;
                    self.apply_note_creation(
                        folder.clone(),
                        *format,
                        &palette.input.lines().join(""),
                    )?;
                    Ok(self.mode != palette.context.mode)
                }
                Workflow::Import {
                    source,
                    target,
                    note_id,
                } => {
                    self.apply_import(
                        *source,
                        *target,
                        note_id.as_deref(),
                        &palette.input.lines().join(""),
                    )?;
                    Ok(true)
                }
                Workflow::Vault(_)
                | Workflow::VaultName { .. }
                | Workflow::Subnotes(_)
                | Workflow::SubnoteMenu { .. }
                | Workflow::SubnoteField { .. } => unreachable!("manager dispatch"),
            }
        })();
        match result {
            Ok(true) => self.dismiss_command_palette(),
            result => {
                palette.error = result.err().map(|error| format!("{error:#}"));
                palette.refresh_items(self);
                self.command_palette = Some(palette);
            }
        }
    }
}
