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
    Theme(Box<ThemePopup>, Vec<[ratatui::style::Color; 4]>),
    Bases(Box<crate::bases::palette::Page>),
    Goal(GoalsPopupMode),
    Options(OptionKind),
    Create {
        folder: String,
        format: NoteFormat,
        task: Option<Box<crate::bases::palette::Creation>>,
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
}

#[derive(Clone, Copy)]
pub(crate) enum OptionKind {
    Sort,
    Icon,
    Hint,
    Preset,
}

impl OptionKind {
    pub(super) fn labels(self) -> Vec<&'static str> {
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
            Self::Preset => vec!["Default", "Helix", "Vim", "Emacs"],
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
        self.mouse_selection = crate::text_edit::MouseTextSelection::default();
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
            self.mouse_selection = crate::text_edit::MouseTextSelection::default();
            self.error = None;
            if self.workflow.is_none() {
                self.workflow_action = None;
            }
        }
    }

    pub(crate) fn workflow_title(&self) -> &str {
        match &self.workflow {
            Some(Workflow::Theme(..)) => "COMMANDS › Switch Theme",
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
            Some(Workflow::Bases(page)) => page.title(),
            None => "COMMANDS",
        }
    }

    pub(crate) fn is_submitting(&self) -> bool {
        matches!(self.workflow, Some(Workflow::Create { task: Some(_), .. }))
            || self.bases_page().is_some_and(|page| {
                matches!(
                    page,
                    crate::bases::palette::Page::Prompt {
                        submitting: true,
                        ..
                    }
                )
            })
    }

    pub(crate) fn workflow_hint(&self) -> &str {
        if let Some(error) = &self.error {
            return error;
        }
        if let Some(Workflow::Bases(page)) = &self.workflow {
            return page.hint();
        }
        if matches!(self.workflow, Some(Workflow::Create { task: Some(_), .. })) {
            return "Creating document…";
        }
        ""
    }

    pub(crate) fn is_value_input(&self) -> bool {
        if let Some(Workflow::Bases(page)) = &self.workflow {
            return page.is_prompt();
        }
        matches!(
            self.workflow,
            Some(
                Workflow::Goal(_)
                    | Workflow::Create { .. }
                    | Workflow::Import { .. }
                    | Workflow::VaultName { .. }
            )
        )
    }

    pub(crate) fn refresh_workflow(&mut self, app: &App) {
        let mut rows = self.manager_rows(app);
        if let Some(Workflow::Bases(page)) = &self.workflow {
            rows = page.rows();
        }
        if let Some(Workflow::Options(kind)) = &self.workflow {
            rows = kind
                .labels()
                .into_iter()
                .enumerate()
                .map(|(i, label)| {
                    (
                        i.to_string(),
                        label.to_owned(),
                        match kind {
                            OptionKind::Preset => [
                                "Arrow navigation · Ctrl shortcuts",
                                "Space leader · hjkl navigation",
                                ": command prefix · hjkl navigation",
                                "Ctrl+x prefix · Ctrl navigation",
                            ][i]
                                .into(),
                            _ => String::new(),
                        },
                    )
                })
                .collect();
        }
        if let Some(Workflow::Theme(popup, _)) = &self.workflow {
            let current = app
                .setup_state
                .as_ref()
                .filter(|_| app.mode == ViewMode::Setup)
                .map_or(app.config.ui.theme.as_str(), |state| {
                    state.themes[state.theme].as_str()
                });
            for (index, name) in popup.themes.iter().enumerate() {
                let source = if popup.is_custom[index] {
                    "Custom theme"
                } else {
                    "Built-in theme"
                };
                rows.push((
                    format!("theme:{index}"),
                    name.clone(),
                    if name == current {
                        format!("{source} · Current")
                    } else {
                        source.to_owned()
                    },
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
        let initial_selection = match &popup {
            ActivePopup::Theme(popup) => Some(format!("theme:{}", popup.selected)),
            ActivePopup::Sort(popup)
            | ActivePopup::IconMode(popup)
            | ActivePopup::HintBarStyle(popup)
            | ActivePopup::KeybindPreset(popup) => Some(popup.selected.to_string()),
            _ => None,
        };
        let (workflow, input) = match popup {
            ActivePopup::Theme(popup) => {
                let previews = popup
                    .themes
                    .iter()
                    .map(|name| {
                        let colors = crate::app_theme::AppThemeColors::from_config(
                            &crate::config::UiConfig {
                                theme: name.clone(),
                                ..Default::default()
                            },
                            &mut Vec::new(),
                        );
                        [colors.accent, colors.heading, colors.folder, colors.tag]
                    })
                    .collect();
                (
                    Workflow::Theme(Box::new(popup), previews),
                    crate::ui::make_popup_textarea(&self.app_theme, "Search themes..."),
                )
            }
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
                    task: None,
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
            popup => {
                self.command_palette = Some(palette);
                self.popups.active = Some(popup);
                return;
            }
        };
        palette.workflow_action = palette.launching_action.clone();
        palette.push_workflow(workflow, input, self);
        if let Some(id) = initial_selection {
            palette
                .state
                .select(palette.items.iter().position(|item| item.id == id));
        }
        self.command_palette = Some(palette);
    }

    pub(crate) fn leave_palette_step(&mut self, mut palette: CommandPalette, close: bool) {
        if matches!(
            palette.workflow,
            Some(Workflow::Create { task: Some(_), .. })
        ) {
            palette.error = Some("Creation in progress; wait for completion".into());
            self.command_palette = Some(palette);
            return;
        }
        if let Err(error) = self.leave_palette_bases(&palette) {
            palette.error = Some(format!("{error:#}"));
            self.command_palette = Some(palette);
            return;
        }
        if matches!(palette.workflow, Some(Workflow::Theme(..)))
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
        if matches!(palette.workflow, Some(Workflow::Bases(_))) {
            self.activate_palette_bases(palette, selected);
            return;
        }
        if matches!(
            palette.workflow,
            Some(Workflow::Vault(_) | Workflow::VaultName { .. })
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
                Workflow::Theme(popup, _) => {
                    let Some(item) = palette.items.get(selected) else {
                        return Ok(false);
                    };
                    popup.selected = item
                        .id
                        .strip_prefix("theme:")
                        .and_then(|id| id.parse().ok())
                        .ok_or_else(|| anyhow::anyhow!("Theme unavailable"))?;
                    popup.focus = ThemePopupFocus::ThemeList;
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
                Workflow::Create {
                    folder,
                    format,
                    task,
                } => {
                    self.validate_document_creation(
                        folder,
                        *format,
                        &palette.input.lines().join(""),
                    )?;
                    if *format == NoteFormat::Base {
                        anyhow::ensure!(
                            task.is_none(),
                            "Creation in progress; wait for completion"
                        );
                        self.save_before_session_replace()?;
                        anyhow::ensure!(self.ensure_bases_worker(), "{}", self.status);
                        let id = self.action_document_id(
                            folder,
                            *format,
                            &palette.input.lines().join(""),
                        )?;
                        *task = Some(Box::new(crate::bases::palette::Creation::start(self, id)));
                        return Ok(false);
                    }
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
                Workflow::Vault(_) | Workflow::VaultName { .. } => unreachable!("manager dispatch"),
                Workflow::Bases(_) => unreachable!("Bases dispatch"),
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
