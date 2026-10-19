//! Clin owns document IDs, key resolution, chrome and clipboard transport.
//! The upstream `draw` crate owns the drawing document and its interactions.
use crate::{
    app::{App, HelpTab, ViewMode},
    keybinds::{DrawAction, KeyMatcher, Keybinds, MatchOutcome},
    overlay::{OverlayResult, OverlayView},
    storage::Storage,
};
use ::draw::{DrawEventAction, DrawHints, DrawState, DrawTool, ThemeColors};
use crossterm::event::{Event, KeyEvent, MouseButton, MouseEventKind};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Span,
};

pub struct DrawPlugin {
    pub state: DrawState,
    pub current_file: String,
    pub keybinds: Keybinds,
    pub seq_matcher: KeyMatcher,
}

impl DrawPlugin {
    pub fn load(
        storage: &Storage,
        id: &str,
        keybinds: Keybinds,
        seq_matcher: KeyMatcher,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            std::path::Path::new(id)
                .extension()
                .is_some_and(|ext| ext == "draw"),
            "Not a drawing: {id}"
        );
        let mut state = DrawState::load(&storage.document_path(id)?)?;
        let target = id.to_owned();
        let storage = storage.clone();
        state.set_save_handler(move |before, after| {
            storage.replace_document(&target, before, after)
        });
        Ok(Self {
            state,
            current_file: id.into(),
            keybinds,
            seq_matcher,
        })
    }

    pub(crate) fn palette_input_available(&self) -> bool {
        self.state.input_available()
    }

    pub fn save_for_navigation(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.state.input_available(),
            "Finish the drawing interaction before leaving"
        );
        self.state.save()
    }

    pub fn sync_header_status(&mut self, app: &mut App) {
        if let Some(message) = self.state.take_notice() {
            app.set_temporary_status_static(message);
        } else if app.status_until.is_none() {
            if let Some(message) = self.state.active_mode_message() {
                app.status = std::borrow::Cow::Borrowed(message);
            } else if ["ROTATE MODE:", "SCALE MODE:", "PASTE MODE:"]
                .iter()
                .any(|prefix| app.status.starts_with(prefix))
            {
                app.set_default_status();
            }
        }
    }

    fn apply(
        &mut self,
        action: DrawAction,
        clipboard: &mut Option<::draw::DrawItem>,
    ) -> anyhow::Result<DrawEventAction> {
        ::draw::apply_action(&mut self.state, to_lib_action(action), clipboard)
    }

    fn direct(&self, key: &KeyEvent, actions: &[DrawAction]) -> Option<DrawAction> {
        actions
            .iter()
            .copied()
            .find(|a| self.keybinds.matches_draw(*a, key))
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut App) -> anyhow::Result<DrawEventAction> {
        if self.state.text_editor.is_some() {
            self.seq_matcher.clear();
            if let Some(action) = self.direct(
                &key,
                &[DrawAction::TextEditorCancel, DrawAction::TextEditorConfirm],
            ) {
                return self.apply(action, &mut app.draw_clipboard);
            }
            if let Some(editor) = self.state.text_editor_mut()
                && !crate::text_edit::apply_text_shortcuts(&self.keybinds, editor, key)
            {
                editor.input(key);
            }
            return Ok(DrawEventAction::Continue);
        }
        if self.state.show_shape_selector {
            self.seq_matcher.clear();
            if let Some(action) = self.direct(
                &key,
                &[
                    DrawAction::ShapeSelectorCancel,
                    DrawAction::ShapeSelectorConfirm,
                    DrawAction::ShapeSelectorUp,
                    DrawAction::ShapeSelectorDown,
                ],
            ) {
                return self.apply(action, &mut app.draw_clipboard);
            }
        }
        if self.state.show_color_selector {
            self.seq_matcher.clear();
            if let Some(action) = self.direct(
                &key,
                &[
                    DrawAction::ColorSelectorCancel,
                    DrawAction::ColorSelectorConfirm,
                    DrawAction::ColorSelectorUp,
                    DrawAction::ColorSelectorDown,
                ],
            ) {
                return self.apply(action, &mut app.draw_clipboard);
            }
        }
        if self.keybinds.matches_draw(DrawAction::Paste, &key) && app.draw_clipboard.is_some() {
            self.seq_matcher.clear();
            return self.apply(DrawAction::Paste, &mut app.draw_clipboard);
        }
        if self.state.context_menu.is_some() {
            self.seq_matcher.clear();
            if let Some(action) = self.direct(
                &key,
                &[
                    DrawAction::MenuClose,
                    DrawAction::MenuUp,
                    DrawAction::MenuDown,
                    DrawAction::MenuSelect,
                ],
            ) {
                return self.apply(action, &mut app.draw_clipboard);
            }
            ::draw::handle_menu_shortcut(key, &mut self.state, &mut app.draw_clipboard)?;
            return Ok(DrawEventAction::Continue);
        }
        if self.keybinds.matches_draw(DrawAction::MenuClose, &key)
            && ::draw::cancel_interaction_or_selection(&mut self.state)
        {
            return Ok(DrawEventAction::Continue);
        }
        if ::draw::handle_selected_menu_shortcut(key, &mut self.state, &mut app.draw_clipboard)? {
            self.seq_matcher.clear();
            return Ok(DrawEventAction::Continue);
        }
        if crate::events::is_universal_quit_key(&key) {
            return Ok(DrawEventAction::Quit);
        }
        match self.keybinds.resolve_draw(
            &mut self.seq_matcher,
            key,
            app.config.sequences_enabled(),
            app.config.counts_enabled(),
        ) {
            MatchOutcome::Matched(action, _count) => self.apply(action, &mut app.draw_clipboard),
            MatchOutcome::Pending | MatchOutcome::NoMatch => Ok(DrawEventAction::Continue),
        }
    }
}

pub(crate) fn theme_colors(theme: &crate::app_theme::AppThemeColors) -> ThemeColors {
    ThemeColors {
        bg: theme.bg,
        accent: theme.accent,
        muted: theme.muted,
        text: theme.text,
        popup_bg: theme.preview_bg(),
        selected: Style::default()
            .fg(theme.highlight_fg)
            .bg(theme.highlight_bg),
        hover: theme.hover_style(),
    }
}

impl OverlayView for DrawPlugin {
    fn overlay_render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let [canvas, status_area] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(area);
        let hints = DrawHints {
            shape_confirm: self.keybinds.display_draw(DrawAction::ShapeSelectorConfirm),
            shape_cancel: self.keybinds.display_draw(DrawAction::ShapeSelectorCancel),
            color_confirm: self.keybinds.display_draw(DrawAction::ColorSelectorConfirm),
            color_cancel: self.keybinds.display_draw(DrawAction::ColorSelectorCancel),
            text_confirm: self.keybinds.display_draw(DrawAction::TextEditorConfirm),
            text_cancel: self.keybinds.display_draw(DrawAction::TextEditorCancel),
        };
        ::draw::draw_view(
            frame,
            &mut self.state,
            canvas,
            &theme_colors(&app.app_theme),
            &hints,
            app.mouse_pos,
        );
        let hints_items = [
            (
                self.keybinds.display_draw(DrawAction::SelectCursorTool),
                "cursor",
            ),
            (
                self.keybinds.display_draw(DrawAction::SelectDrawTool),
                "draw",
            ),
            (
                self.keybinds.display_draw(DrawAction::ToggleShapeSelector),
                "shape",
            ),
            (
                self.keybinds.display_draw(DrawAction::SelectTextTool),
                "text",
            ),
            (
                self.keybinds.display_draw(DrawAction::SelectEraseTool),
                "erase",
            ),
            (self.keybinds.draw_keys_display(DrawAction::Quit), "back"),
            (
                format!("F1/{}", self.keybinds.draw_keys_display(DrawAction::Help)),
                "help",
            ),
            ("F2".to_string(), "keybinds"),
        ];
        let hint_line = crate::ui::format_keybind_hints(&app.app_theme, &hints_items);
        let mut ctx =
            crate::statusline::StatuslineContext::for_overlay(&app.config, ViewMode::Draw);
        ctx.area = Some(status_area);
        ctx.draw = Some(&self.state);
        ctx.hints = Some(hint_line.spans);
        if let Some(p) = &self.seq_matcher.pending_display() {
            ctx.pending = Some(vec![Span::styled(
                format!("{} ", p),
                Style::default()
                    .fg(app.app_theme.highlight_fg)
                    .bg(app.app_theme.accent),
            )]);
        }

        let (left_line, right_line) = crate::statusline::render_footer(
            &ctx,
            &app.config.statusline,
            ViewMode::Draw,
            &app.app_theme,
        );
        crate::ui::draw_status_bar(frame, status_area, &app.app_theme, left_line, right_line);
    }

    fn overlay_handle_event(
        &mut self,
        event: Event,
        app: &mut App,
        term_area: Rect,
    ) -> anyhow::Result<OverlayResult> {
        let [header, canvas, _footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(term_area);
        self.state.set_canvas_area(canvas);
        let result = match event {
            Event::Key(key) => self.handle_key(key, app),
            Event::Mouse(mouse) => {
                if self.state.input_available()
                    && mouse.kind == MouseEventKind::Down(MouseButton::Left)
                    && header.contains((mouse.column, mouse.row).into())
                {
                    let icon_mode = app.config.ui.icon_mode;
                    let tabs_arr = draw_tool_tabs(icon_mode);
                    let tabs = crate::ui::tab_vec_from_array(&tabs_arr);
                    let region = crate::ui::title_bar_tabs_region(header, "Draw");
                    if let Some(index) = crate::ui::hit_test_tabs(
                        &tabs,
                        header.x,
                        header.width,
                        region.x,
                        mouse.column,
                        false,
                        icon_mode,
                    ) {
                        let tool = DRAW_TAB_TOOLS[index];
                        let action = match tool {
                            DrawTool::Draw if self.state.active_tool == DrawTool::Draw => {
                                DrawAction::ToggleColorSelector
                            }
                            DrawTool::Cursor => DrawAction::SelectCursorTool,
                            DrawTool::Draw => DrawAction::SelectDrawTool,
                            DrawTool::Shape => DrawAction::ToggleShapeSelector,
                            DrawTool::Text => DrawAction::SelectTextTool,
                            DrawTool::Erase => DrawAction::SelectEraseTool,
                        };
                        self.apply(action, &mut app.draw_clipboard)
                    } else {
                        Ok(DrawEventAction::Continue)
                    }
                } else {
                    ::draw::handle_draw_mouse(&mut self.state, mouse, &mut app.draw_clipboard)
                }
            }
            Event::Paste(text) => {
                self.state.insert_text(&text);
                Ok(DrawEventAction::Continue)
            }
            _ => Ok(DrawEventAction::Continue),
        };
        let result = result.and_then(|action| match action {
            DrawEventAction::Quit => {
                self.save_for_navigation()?;
                Ok(OverlayResult::Exit)
            }
            DrawEventAction::OpenHelp => Ok(OverlayResult::OpenHelp(HelpTab::Draw)),
            DrawEventAction::CopyText(text) => {
                let notice = crate::text_edit::clipboard_notice(
                    crate::text_edit::write_system_clipboard(&text),
                );
                app.set_temporary_status_static(notice);
                Ok(OverlayResult::Continue)
            }
            DrawEventAction::Continue => Ok(OverlayResult::Continue),
        });
        match result {
            Ok(result) => {
                self.sync_header_status(app);
                Ok(result)
            }
            Err(error) => {
                self.state.take_notice();
                app.set_temporary_status(&format!("{error:#}"));
                Ok(OverlayResult::Continue)
            }
        }
    }
}

fn to_lib_action(action: DrawAction) -> ::draw::DrawAction {
    match action {
        DrawAction::Quit => ::draw::DrawAction::Quit,
        DrawAction::Help => ::draw::DrawAction::Help,
        DrawAction::SelectCursorTool => ::draw::DrawAction::SelectCursorTool,
        DrawAction::SelectDrawTool => ::draw::DrawAction::SelectDrawTool,
        DrawAction::ToggleShapeSelector => ::draw::DrawAction::ToggleShapeSelector,
        DrawAction::SelectTextTool => ::draw::DrawAction::SelectTextTool,
        DrawAction::SelectEraseTool => ::draw::DrawAction::SelectEraseTool,
        DrawAction::ShapeSelectorUp => ::draw::DrawAction::ShapeSelectorUp,
        DrawAction::ShapeSelectorDown => ::draw::DrawAction::ShapeSelectorDown,
        DrawAction::ShapeSelectorConfirm => ::draw::DrawAction::ShapeSelectorConfirm,
        DrawAction::ShapeSelectorCancel => ::draw::DrawAction::ShapeSelectorCancel,
        DrawAction::ToggleColorSelector => ::draw::DrawAction::ToggleColorSelector,
        DrawAction::ColorSelectorUp => ::draw::DrawAction::ColorSelectorUp,
        DrawAction::ColorSelectorDown => ::draw::DrawAction::ColorSelectorDown,
        DrawAction::ColorSelectorConfirm => ::draw::DrawAction::ColorSelectorConfirm,
        DrawAction::ColorSelectorCancel => ::draw::DrawAction::ColorSelectorCancel,
        DrawAction::TextEditorConfirm => ::draw::DrawAction::TextEditorConfirm,
        DrawAction::TextEditorCancel => ::draw::DrawAction::TextEditorCancel,
        DrawAction::MenuClose => ::draw::DrawAction::MenuClose,
        DrawAction::MenuUp => ::draw::DrawAction::MenuUp,
        DrawAction::MenuDown => ::draw::DrawAction::MenuDown,
        DrawAction::MenuSelect => ::draw::DrawAction::MenuSelect,
        DrawAction::Copy => ::draw::DrawAction::Copy,
        DrawAction::Paste => ::draw::DrawAction::Paste,
        DrawAction::Undo => ::draw::DrawAction::Undo,
        DrawAction::Redo => ::draw::DrawAction::Redo,
        DrawAction::ToggleGrid => ::draw::DrawAction::ToggleGrid,
    }
}

/// Draw-view tool tab (label, glyph) pairs, in toolbar order. Shared by
/// `draw_ui` header render (via `ui/mod.rs`) and the draw mouse hit-test
/// so they never drift — same pattern as other view tab bars.
pub fn draw_tool_tabs(icon_mode: crate::config::IconMode) -> [(&'static str, &'static str); 5] {
    [
        (
            "Cursor",
            crate::ui::get_icon("\u{f245}", "\u{25b6}", icon_mode),
        ),
        (
            "Draw",
            crate::ui::get_icon("\u{f040}", "\u{270f}", icon_mode),
        ),
        (
            "Shape",
            crate::ui::get_icon("\u{f0c8}", "\u{25a0}", icon_mode),
        ),
        (
            "Text",
            crate::ui::get_icon("\u{f031}", "\u{1f4dd}", icon_mode),
        ),
        (
            "Erase",
            crate::ui::get_icon("\u{f1f8}", "\u{1f5d1}", icon_mode),
        ),
    ]
}
/// Tab order is fixed and intentionally not tied to `DrawTool` declaration
/// order. Keep this array as the single index-to-tool source of truth.
pub const DRAW_TAB_TOOLS: [DrawTool; 5] = [
    DrawTool::Cursor,
    DrawTool::Draw,
    DrawTool::Shape,
    DrawTool::Text,
    DrawTool::Erase,
];

/// Index of a tool within the tab order, for `build_tab_spans(active)`.
pub fn draw_tool_tab_index(tool: DrawTool) -> usize {
    DRAW_TAB_TOOLS.iter().position(|t| *t == tool).unwrap_or(0)
}
