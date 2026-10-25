//! Application chrome: every view receives content space above this shared footer.
use crate::{
    app::{App, ViewMode},
    keybinds::{
        BasesAction, CanvasAction, DrawAction, EditAction, GraphAction, HelpAction, ListAction,
        OutlineAction, SetupAction,
    },
    statusline::{StatuslineContext, active_note, render_footer},
};
use ratatui::{prelude::*, widgets::Paragraph};

pub(crate) fn content_area(area: Rect) -> Rect {
    Rect {
        height: super::chrome::rows(area)[2].y.saturating_sub(area.y),
        ..area
    }
}

pub(crate) fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    if area.height == 0 || app.mode == ViewMode::Setup {
        return;
    }
    let area = super::chrome::rows(area)[2];
    let theme = &app.app_theme;
    let mut ctx = StatuslineContext::for_view(app, app.mode);
    ctx.area = Some(area);
    ctx.note = active_note(app, app.mode);
    ctx.draw = app.draw_state.as_ref().map(|s| &s.state);
    ctx.canvas = app.canvas_state.as_ref().map(|s| &s.state);
    ctx.outline = app.outline_state.as_ref();
    ctx.bases = app.bases_state.as_ref();
    ctx.calendar = app.calendar_view.as_ref();
    ctx.setup = app.setup_state.as_ref();
    if app.mode == ViewMode::Bases {
        ctx.detail = app.bases_state.as_ref().map(|state| {
            vec![Span::styled(
                state.footer_detail(),
                Style::default().fg(theme.muted),
            )]
        });
    }
    let graph = app
        .graph_plugin
        .as_ref()
        .and_then(|p| p.graph_state.as_ref())
        .map(|s| s.read());
    ctx.graph = graph.as_deref();
    if let Some(plugin) = &app.graph_plugin {
        ctx.graph_fps = plugin.canvas_fps();
        ctx.graph_grid_visible = plugin.grid;
    }
    let mut footer_theme = theme.clone();
    if app.mode == ViewMode::Calendar
        && let Some(calendar) = &app.calendar_view
    {
        let (detail, warning) = calendar.footer_detail(app);
        if warning {
            footer_theme.accent = theme.warning;
        }
        ctx.detail = Some(vec![Span::styled(
            detail,
            Style::default().fg(if warning { theme.warning } else { theme.muted }),
        )]);
    }
    let matcher = match app.mode {
        ViewMode::Graph => app.graph_plugin.as_ref().map(|s| &s.seq_matcher),
        ViewMode::Canvas => app.canvas_state.as_ref().map(|s| &s.seq_matcher),
        ViewMode::Draw => app.draw_state.as_ref().map(|s| &s.seq_matcher),
        ViewMode::Outline => app.outline_state.as_ref().map(|s| &s.seq_matcher),
        ViewMode::Bases => app.bases_state.as_ref().map(|s| &s.matcher),
        _ => Some(&app.seq_matcher),
    };
    if let Some(pending) = matcher.and_then(crate::keybinds::KeyMatcher::pending_display) {
        ctx.pending = Some(vec![Span::styled(
            format!("{pending} "),
            Style::default().fg(theme.highlight_fg).bg(theme.accent),
        )]);
    }
    if app.mode == ViewMode::List {
        ctx.badge = Some(super::ext_badge_spans(
            app.editor.external_editor_enabled,
            theme,
            theme
                .hint_bar_style
                .has_filled_cells()
                .then_some(theme.accent),
        ));
    }
    ctx.hints = Some(hints(app).spans);
    let (left, right) = render_footer(&ctx, &app.config.statusline, app.mode, &footer_theme);
    // Clear the entire reserved row, including when a view has no active state.
    frame.render_widget(Paragraph::new("").style(theme.hint_line_bg_style()), area);
    super::draw_status_bar(frame, area, theme, left, right);
}

fn hints(app: &App) -> Line<'_> {
    let kb = match app.mode {
        ViewMode::Graph => app.graph_plugin.as_ref().map(|s| &s.keybinds),
        ViewMode::Draw => app.draw_state.as_ref().map(|s| &s.keybinds),
        ViewMode::Canvas => app.canvas_state.as_ref().map(|s| &s.keybinds),
        ViewMode::Outline => app.outline_state.as_ref().map(|s| &s.keybinds),
        _ => None,
    }
    .unwrap_or(&app.keybinds);
    let theme = &app.app_theme;
    let items = match app.mode {
        ViewMode::List if app.list.list_mode == crate::list_view::ListMode::Select => {
            return super::format_mode_keybind_hints(
                theme,
                &[
                    (kb.display_list(ListAction::ToggleSelectItem), "toggle"),
                    (kb.display_list(ListAction::MoveNote), "move"),
                    (kb.display_list(ListAction::Duplicate), "copy"),
                    (kb.display_list(ListAction::ManageTags), "tag"),
                    (kb.display_list(ListAction::RemoveTagsFromSelected), "untag"),
                    (kb.display_list(ListAction::Delete), "delete"),
                    (kb.display_list(ListAction::ToggleSelectMode), "exit"),
                ],
            );
        }
        ViewMode::List if app.layout_edit => {
            return super::format_mode_keybind_hints(
                theme,
                &[
                    ("←→ ↑↓/drag".into(), "resize"),
                    ("s".into(), "preview"),
                    ("c".into(), "widgets"),
                    ("Space/Ctrl+Space".into(), "cycle"),
                    ("Tab".into(), "swap"),
                    ("a".into(), "toggle"),
                    ("Esc".into(), "done"),
                ],
            );
        }
        ViewMode::List => vec![
            (
                format!(
                    "{}/{}",
                    kb.display_list(ListAction::MoveUp),
                    kb.display_list(ListAction::MoveDown)
                ),
                "move",
            ),
            (kb.display_list(ListAction::Open), "open"),
            (kb.display_list(ListAction::CreateNote), "new"),
            (kb.display_list(ListAction::Search), "search"),
            (app.palette_keys_display(ViewMode::List), "commands"),
        ],
        ViewMode::Edit => vec![
            (kb.display_edit(EditAction::CycleFocus), "focus"),
            (kb.display_edit(EditAction::Find), "find"),
            (
                kb.display_edit(EditAction::ToggleMarkdownPreview),
                "preview",
            ),
            (kb.display_edit(EditAction::Save), "save"),
            (kb.edit_keys_display(EditAction::Back), "back"),
        ],
        ViewMode::Help => vec![
            (
                format!(
                    "{}/{}",
                    kb.display_help(HelpAction::PrevTab),
                    kb.display_help(HelpAction::NextTab)
                ),
                "switch tab",
            ),
            (
                format!(
                    "{}/{}",
                    kb.display_help(HelpAction::ScrollUp),
                    kb.display_help(HelpAction::ScrollDown)
                ),
                "scroll",
            ),
            (kb.display_help(HelpAction::Search), "search"),
            (kb.display_help(HelpAction::Reroll), "reroll tips"),
            (
                format!("F1/{}", kb.help_keys_display(HelpAction::Close)),
                "close",
            ),
        ],
        ViewMode::Graph
            if app
                .graph_plugin
                .as_ref()
                .is_some_and(|p| !p.show_status_bar) =>
        {
            vec![(app.palette_keys_display(ViewMode::Graph), "commands")]
        }
        ViewMode::Graph => vec![
            (
                format!(
                    "{}/{}",
                    kb.display_graph(GraphAction::PanUp),
                    kb.display_graph(GraphAction::PanDown)
                ),
                "pan",
            ),
            (
                format!(
                    "{}/{}",
                    kb.display_graph(GraphAction::ZoomOut),
                    kb.display_graph(GraphAction::ZoomIn)
                ),
                "zoom",
            ),
            (kb.display_graph(GraphAction::ToggleLegend), "labels"),
            (kb.display_graph(GraphAction::AutoFit), "fit"),
            (kb.graph_keys_display(GraphAction::Quit), "quit"),
        ],
        ViewMode::Draw => vec![
            (kb.display_draw(DrawAction::SelectCursorTool), "cursor"),
            (kb.display_draw(DrawAction::SelectDrawTool), "draw"),
            (kb.display_draw(DrawAction::ToggleShapeSelector), "shape"),
            (kb.display_draw(DrawAction::SelectTextTool), "text"),
            (kb.draw_keys_display(DrawAction::Quit), "back"),
        ],
        ViewMode::Canvas => {
            if let Some(canvas) = &app.canvas_state
                && !canvas.state.footer_hint.is_empty()
            {
                return crate::statusline::line_from_segments(
                    &[crate::statusline::Segment::Text(
                        crate::fsutil::sanitize_for_terminal(&canvas.state.footer_hint)
                            .into_owned(),
                    )],
                    theme,
                    false,
                    false,
                );
            }
            vec![
                (
                    format!(
                        "{}/{}",
                        kb.display_canvas(CanvasAction::MoveUp),
                        kb.display_canvas(CanvasAction::MoveDown)
                    ),
                    "move",
                ),
                (kb.display_canvas(CanvasAction::OpenContextMenu), "menu"),
                (
                    format!(
                        "{}/{}",
                        kb.display_canvas(CanvasAction::ZoomOut),
                        kb.display_canvas(CanvasAction::ZoomIn)
                    ),
                    "zoom",
                ),
                (kb.canvas_keys_display(CanvasAction::Quit), "back"),
                (
                    format!("F1/{}", kb.canvas_keys_display(CanvasAction::Help)),
                    "help",
                ),
            ]
        }
        ViewMode::Bases => vec![
            (kb.display_bases(BasesAction::Back), "back"),
            (kb.display_bases(BasesAction::Open), "open"),
            (kb.display_bases(BasesAction::Edit), "edit"),
            (kb.display_bases(BasesAction::NextView), "views"),
            (kb.display_bases(BasesAction::Search), "search"),
        ],
        ViewMode::Outline => vec![
            (
                format!(
                    "{}/{}",
                    kb.display_outline(OutlineAction::MoveDown),
                    kb.display_outline(OutlineAction::MoveUp)
                ),
                "move",
            ),
            (kb.display_outline(OutlineAction::ToggleCollapse), "fold"),
            (kb.display_outline(OutlineAction::Open), "jump"),
            (kb.outline_keys_display(OutlineAction::Back), "back"),
            (
                format!("F1/{}", kb.outline_keys_display(OutlineAction::Help)),
                "help",
            ),
        ],
        ViewMode::Calendar => vec![
            ("hjkl/Arrows".into(), "move"),
            ("n/e".into(), "new/edit"),
            ("a/o".into(), "link/open note"),
            ("s".into(), "sync"),
            ("Esc".into(), "back"),
        ],
        ViewMode::Setup => vec![
            (
                format!(
                    "{}/{}",
                    kb.display_setup(SetupAction::Up),
                    kb.display_setup(SetupAction::Down)
                ),
                "move",
            ),
            (
                format!(
                    "{}/{}",
                    kb.display_setup(SetupAction::CyclePrev),
                    kb.display_setup(SetupAction::CycleNext)
                ),
                "change",
            ),
            (kb.display_setup(SetupAction::Activate), "select"),
            (kb.display_setup(SetupAction::Finish), "finish"),
            (app.palette_keys_display(ViewMode::Setup), "commands"),
        ],
    };
    super::format_keybind_hints(theme, &items)
}
