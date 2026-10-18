use super::{
    CommandPalette, PaletteItem,
    workflows::{OptionKind, Workflow},
};
use crate::{
    app::{App, ViewMode},
    app_theme::AppThemeColors,
    popups::ThemePopupFocus,
};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
};

impl CommandPalette {
    pub(crate) fn workflow_bar(&self, app: &App) -> Vec<(Line<'static>, Option<ThemePopupFocus>)> {
        let current = match &self.workflow {
            Some(Workflow::Theme(popup, _)) => {
                let indicator = |label: &str, on| {
                    Line::from(vec![
                        Span::raw(format!("{label}: ")),
                        Span::styled(
                            if on { "On" } else { "Off" },
                            Style::default().fg(if on {
                                app.app_theme.success
                            } else {
                                app.app_theme.destructive
                            }),
                        ),
                    ])
                };
                let mut controls = vec![(
                    indicator("Background", popup.general_is_solid),
                    Some(ThemePopupFocus::GeneralBg),
                )];
                if !popup.setup_owned {
                    controls.push((
                        indicator("Graph background", popup.graph_is_solid),
                        Some(ThemePopupFocus::GraphBg),
                    ));
                }
                return controls;
            }
            Some(Workflow::Options(kind)) => match kind {
                OptionKind::Hint => app.config.ui.hint_bar_style.name().to_owned(),
                OptionKind::Icon => kind.labels()[match app.config.ui.icon_mode {
                    crate::config::IconMode::Nerd => 0,
                    crate::config::IconMode::Unicode => 1,
                    crate::config::IconMode::None => 2,
                }]
                .into(),
                OptionKind::Preset => {
                    if app.mode == ViewMode::Setup {
                        kind.labels()[app.setup_state.as_ref().map_or(0, |s| s.keybind_preset)]
                            .into()
                    } else {
                        app.config.core.keybind_preset.to_string()
                    }
                }
                OptionKind::Sort => {
                    use crate::list_view::{SortField, SortOrder};
                    kind.labels()[match (app.list.sort_field, app.list.sort_order) {
                        (SortField::Title, SortOrder::Ascending) => 0,
                        (SortField::Title, SortOrder::Descending) => 1,
                        (SortField::Modified, SortOrder::Descending) => 2,
                        (SortField::Modified, SortOrder::Ascending) => 3,
                    }]
                    .into()
                }
            },
            Some(Workflow::Goal(crate::popups::GoalsPopupMode::WordGoal)) => {
                app.config.goals.word_goal.to_string()
            }
            Some(Workflow::Goal(crate::popups::GoalsPopupMode::NoteGoal)) => {
                app.config.goals.note_goal.to_string()
            }
            Some(Workflow::Vault(_)) => app.storage.data_dir.to_string_lossy().into_owned(),
            Some(Workflow::Bases(page)) => {
                let Some(current) = page.current_value() else {
                    return Vec::new();
                };
                current.to_owned()
            }
            _ => return Vec::new(),
        };
        vec![(
            Line::from(format!(
                "Current: {}",
                crate::fsutil::sanitize_for_terminal(&current)
            )),
            None,
        )]
    }

    // Rendering and mouse routing must use the same optional bar/status rows.
    pub(crate) fn content_areas(&self, content: Rect, has_controls: bool) -> [Rect; 4] {
        let areas = Layout::vertical([
            Constraint::Length(self.input_height(content.height)),
            Constraint::Length(u16::from(self.workflow.is_none() || has_controls)),
            Constraint::Length(u16::from(
                !self.workflow_hint().is_empty() && self.workflow.is_some(),
            )),
            Constraint::Min(0),
        ])
        .split(content);
        [areas[0], areas[1], areas[2], areas[3]]
    }

    pub(crate) fn item_preview(&self, item: &PaletteItem, theme: &AppThemeColors) -> Line<'static> {
        let mut spans = Vec::new();
        match &self.workflow {
            Some(Workflow::Theme(_, previews)) => {
                if let Some(colors) = item
                    .id
                    .strip_prefix("theme:")
                    .and_then(|id| id.parse::<usize>().ok())
                    .and_then(|i| previews.get(i))
                {
                    for color in colors {
                        spans.push(Span::styled("██ ", Style::default().fg(*color)));
                    }
                }
            }
            Some(Workflow::Options(OptionKind::Icon)) => {
                use crate::config::IconMode;
                let mode = match item.id.as_str() {
                    "0" => IconMode::Nerd,
                    "1" => IconMode::Unicode,
                    _ => IconMode::None,
                };
                for (nerd, unicode, label, color) in [
                    ("\u{f15c}", "📄", "Note", theme.accent),
                    ("\u{f07b}", "📁", "Folder", theme.folder),
                    ("\u{f013}", "⚙", "Settings", theme.tag),
                ] {
                    let icon = crate::ui::get_icon(nerd, unicode, mode);
                    spans.push(Span::styled(
                        format!(
                            "{}{}{}   ",
                            icon,
                            if icon.is_empty() { "" } else { " " },
                            label
                        ),
                        Style::default().fg(color),
                    ));
                }
            }
            Some(Workflow::Options(OptionKind::Hint)) => {
                if let Ok(index) = item.id.parse::<usize>() {
                    let mut preview_theme = theme.clone();
                    preview_theme.hint_bar_style = crate::config::HintBarStyle::from_index(index);
                    let line = crate::ui::format_keybind_hints(
                        &preview_theme,
                        &[("Enter".into(), "select")],
                    );
                    spans.extend(
                        line.spans
                            .into_iter()
                            .map(|s| Span::styled(s.content.into_owned(), s.style)),
                    );
                }
            }
            _ => {}
        }
        spans.push(Span::styled(
            item.description.clone(),
            Style::default().fg(theme.muted),
        ));
        Line::from(spans)
    }
}

impl App {
    pub(crate) fn palette_theme_shortcut(&mut self, key: crossterm::event::KeyEvent) -> bool {
        use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};
        if key.kind != KeyEventKind::Press || key.modifiers != KeyModifiers::ALT {
            return false;
        }
        let focus = match key.code {
            KeyCode::Char('b' | 'B') => ThemePopupFocus::GeneralBg,
            KeyCode::Char('g' | 'G') => ThemePopupFocus::GraphBg,
            _ => return false,
        };
        if !self
            .command_palette
            .as_ref()
            .is_some_and(|p| matches!(p.workflow, Some(Workflow::Theme(..))))
        {
            return false;
        }
        let palette = self.command_palette.take().expect("theme palette");
        self.toggle_palette_theme_background(palette, focus);
        true
    }

    pub(crate) fn toggle_palette_theme_background(
        &mut self,
        mut palette: CommandPalette,
        focus: ThemePopupFocus,
    ) {
        if let Some(Workflow::Theme(popup, _)) = &mut palette.workflow
            && matches!(focus, ThemePopupFocus::GeneralBg | ThemePopupFocus::GraphBg)
            && !(popup.setup_owned && focus == ThemePopupFocus::GraphBg)
        {
            match crate::actions::validate_action_context("switch_theme", self, &palette.context) {
                Ok(()) => {
                    popup.focus = focus;
                    self.apply_theme_selection(popup);
                    popup.focus = ThemePopupFocus::ThemeList;
                    palette.error = None;
                }
                Err(error) => palette.error = Some(format!("{error:#}")),
            }
        }
        self.command_palette = Some(palette);
    }
}
