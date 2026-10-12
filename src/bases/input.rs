use super::state::BasesState;
use crate::{
    app::App,
    keybinds::{BasesAction, MatchOutcome},
    overlay::{OverlayResult, OverlayView},
};
use crossterm::event::{Event, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::Line,
    widgets::Paragraph,
};

impl OverlayView for BasesState {
    fn overlay_render(&mut self, frame: &mut ratatui::Frame, area: Rect, app: &mut App) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(area);
        let view = self.loaded.as_ref().map_or("", |loaded| {
            loaded.document.config.views[loaded.view].name.as_str()
        });
        let title = format!(
            "Bases · {} · {}{}",
            crate::fsutil::sanitize_for_terminal(&self.id),
            crate::fsutil::sanitize_for_terminal(view),
            if self.loading {
                " · refreshing…"
            } else {
                ""
            }
        );
        let count = self.result().map_or_else(String::new, |result| {
            format!(
                "{} / {} files · {} selected",
                result.rows.len(),
                result.matched,
                self.selected_ids.len()
            )
        });
        crate::ui::draw_view_title_bar(
            frame,
            chunks[0],
            &app.app_theme,
            Line::from(title),
            Some(Line::from(count)),
            Some(app.status.as_ref()),
            app.load_spinner_tick,
        );
        super::render::render(frame, chunks[1], self, &app.app_theme);
        let hints = format!(
            "{} back  {} open  {} edit  {} views  {} config  {} refresh",
            app.keybinds.display_bases(BasesAction::Back),
            app.keybinds.display_bases(BasesAction::Open),
            app.keybinds.display_bases(BasesAction::Edit),
            app.keybinds.display_bases(BasesAction::NextView),
            app.keybinds.display_bases(BasesAction::Configure),
            app.keybinds.display_bases(BasesAction::Refresh)
        );
        frame.render_widget(
            Paragraph::new(hints).style(app.app_theme.hint_line_bg_style()),
            chunks[2],
        );
    }
    fn overlay_handle_event(
        &mut self,
        event: Event,
        app: &mut App,
        _area: Rect,
    ) -> anyhow::Result<OverlayResult> {
        let action = match event {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                match app.keybinds.resolve_bases(
                    &mut self.matcher,
                    key,
                    app.config.sequences_enabled(),
                    app.config.counts_enabled(),
                ) {
                    MatchOutcome::Matched(action, count) => Some((action, count.unwrap_or(1))),
                    _ => None,
                }
            }
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollDown => Some((BasesAction::MoveDown, 3)),
                MouseEventKind::ScrollUp => Some((BasesAction::MoveUp, 3)),
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some((_, item, column)) = self
                        .hit_rows
                        .iter()
                        .find(|(rect, _, _)| rect.contains((mouse.column, mouse.row).into()))
                    {
                        self.selected = *item;
                        self.column = *column;
                    }
                    None
                }
                _ => None,
            },
            _ => None,
        };
        let Some((action, count)) = action else {
            return Ok(OverlayResult::Continue);
        };
        match action {
            BasesAction::Back => return Ok(OverlayResult::Exit),
            BasesAction::Help => return Ok(OverlayResult::OpenHelp(crate::app::HelpTab::Notes)),
            BasesAction::MoveUp => self.selected = self.selected.saturating_sub(count as usize),
            BasesAction::MoveDown => {
                self.selected =
                    (self.selected + count as usize).min(self.items.len().saturating_sub(1))
            }
            BasesAction::First => self.selected = 0,
            BasesAction::Last => self.selected = self.items.len().saturating_sub(1),
            BasesAction::PageUp => {
                self.selected = self
                    .selected
                    .saturating_sub(usize::from(self.body_area.height).saturating_sub(1))
            }
            BasesAction::PageDown => {
                self.selected = (self.selected
                    + usize::from(self.body_area.height).saturating_sub(1))
                .min(self.items.len().saturating_sub(1))
            }
            BasesAction::MoveLeft => self.column = self.column.saturating_sub(count as usize),
            BasesAction::MoveRight => {
                self.column = (self.column + count as usize).min(
                    self.loaded
                        .as_ref()
                        .map_or(0, |loaded| loaded.columns.len().saturating_sub(1)),
                )
            }
            BasesAction::Refresh => self.refresh(app, true),
            BasesAction::NextView | BasesAction::PreviousView => {
                if let Some(loaded) = &self.loaded {
                    let length = loaded.document.config.views.len();
                    let index = if matches!(action, BasesAction::NextView) {
                        (loaded.view + 1) % length
                    } else {
                        (loaded.view + length - 1) % length
                    };
                    self.named_view = Some(loaded.document.config.views[index].name.clone());
                    self.environment.random_seed = rand::random();
                    self.refresh(app, false);
                }
            }
            BasesAction::ToggleSelect => {
                if let Some(id) = self.row().map(|row| row.id.clone())
                    && !self.selected_ids.remove(&id)
                {
                    self.selected_ids.insert(id);
                }
            }
            BasesAction::SelectAll => {
                if let Some(result) = self.result() {
                    self.selected_ids = result.rows.iter().map(|row| row.id.clone()).collect();
                }
            }
            _ => app.set_temporary_status_static("Bases authoring controls are not wired yet"),
        }
        Ok(OverlayResult::Continue)
    }
}
impl BasesState {
    pub fn refresh(&mut self, app: &mut App, invalidate: bool) {
        self.loading = true;
        if let Some(worker) = app.bases_worker.as_mut() {
            worker.request(
                &self.id,
                self.named_view.clone(),
                self.environment.clone(),
                invalidate,
            );
        }
    }
}
