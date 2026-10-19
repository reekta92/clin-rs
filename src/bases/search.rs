//! Session-only public search; Notes quick-search widget, Bases query ownership.
use super::state::BasesState;
use crate::{
    app::App,
    ui::quick_search::{
        QuickSearch, QuickSearchAction, draw_quick_search, handle_quick_search_keys,
        handle_quick_search_mouse,
    },
};
use crossterm::event::{Event, KeyEventKind};
use ratatui::{layout::Rect, text::Line};

impl BasesState {
    pub(super) fn open_search(&mut self, app: &mut App) -> anyhow::Result<()> {
        self.ensure_idle()?;
        self.search_before = (
            self.search_text.clone(),
            self.row().map(|row| row.id.clone()),
            self.focused_group()
                .and_then(|(index, _)| self.group_identity(index)),
        );
        let mut popup = QuickSearch::new("Bases search", &app.app_theme);
        popup.input.insert_str(&self.search_text);
        if let Some(result) = self.result() {
            popup.results = (0..result.rows.len()).collect();
        }
        popup.info = Some(format!("{} matches", popup.results.len()));
        self.search_popup = Some(popup);
        Ok(())
    }
    pub(super) fn render_search(&self, frame: &mut ratatui::Frame, area: Rect, app: &App) {
        if let Some(popup) = &self.search_popup {
            draw_quick_search(
                frame,
                area,
                popup,
                &app.app_theme,
                10.min(usize::from(area.height.saturating_sub(2))),
                |row, _, _| {
                    Line::from(
                        self.result()
                            .and_then(|result| result.rows.get(*row))
                            .map_or_else(String::new, |row| {
                                crate::fsutil::sanitize_for_terminal(&row.id).into_owned()
                            }),
                    )
                },
                app.config.ui.icon_mode,
            );
        }
    }
    pub(super) fn search_event(&mut self, event: &Event, app: &mut App, area: Rect) -> bool {
        let Some(popup) = self.search_popup.as_mut() else {
            return false;
        };
        let visible = 10.min(usize::from(area.height.saturating_sub(2)));
        let action =
            match event {
                Event::Key(key) if key.kind != KeyEventKind::Release => Some(
                    handle_quick_search_keys(popup, *key, &app.keybinds, visible),
                ),
                Event::Mouse(mouse) => {
                    handle_quick_search_mouse(popup, *mouse, area, visible, app.config.ui.icon_mode)
                }
                Event::Paste(text) => {
                    if text.len() + popup.query().len() > 64 * 1024 {
                        app.set_temporary_status_static("Bases search exceeds 64 KiB limit");
                        return true;
                    }
                    popup.input.insert_str(text.replace(['\r', '\n'], " "));
                    Some(QuickSearchAction::Edited)
                }
                _ => None,
            };
        match action {
            Some(QuickSearchAction::Edited) => {
                let query = popup.query();
                if query.len() > 64 * 1024 {
                    app.set_temporary_status_static("Bases search exceeds 64 KiB limit");
                } else if self.search_text != query {
                    self.search_text = query;
                    popup.selected = 0;
                    popup.scroll_offset = 0;
                    popup.info = Some("refreshing…".into());
                    self.refresh(app, false);
                }
            }
            Some(QuickSearchAction::Submit) => {
                if self.loading || popup.query() != self.search_text {
                    app.set_temporary_status_static(
                        "Wait for valid Bases search results before choosing a row",
                    );
                } else {
                    let row = popup.results.get(popup.selected).copied();
                    self.search_popup = None;
                    if let Some(row) = row {
                        self.focus_search_row(row);
                    }
                }
            }
            Some(QuickSearchAction::Cancel) => {
                self.search_popup = None;
                if self.search_text != self.search_before.0 || self.loading {
                    self.focus_id = self.search_before.1.clone();
                    self.focus_group = self.search_before.2.clone();
                    self.search_text = self.search_before.0.clone();
                    self.refresh(app, false);
                } else if let Some(row) = self.result().and_then(|result| {
                    result
                        .rows
                        .iter()
                        .position(|row| Some(&row.id) == self.search_before.1.as_ref())
                }) {
                    self.focus_search_row(row);
                } else {
                    self.rebuild_items(None, self.search_before.2.clone());
                }
            }
            _ => {}
        }
        true
    }
}
