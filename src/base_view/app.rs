use crate::base_view::state::BaseState;
use crate::base_view::{input, render};
use crate::overlay::{OverlayResult, OverlayView};
use crossterm::event::Event;

fn map_input_result(state: &BaseState, res: input::BaseInput) -> Option<OverlayResult> {
    match res {
        input::BaseInput::Back => Some(OverlayResult::Exit),
        input::BaseInput::Help => Some(OverlayResult::OpenHelp(crate::app::HelpTab::Base)),
        input::BaseInput::Open => state
            .selected_row()
            .map(|row| OverlayResult::NoteOpened(row.id.clone())),
        input::BaseInput::None | input::BaseInput::Refresh => None,
        input::BaseInput::NewNote => Some(OverlayResult::NewNoteFromBase),
    }
}

impl OverlayView for BaseState {
    fn overlay_render(
        &mut self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        theme: &crate::app_theme::AppThemeColors,
        _config: &crate::config::ClinConfig,
        _app_status: Option<&str>,
    ) {
        self.last_area = area;
        let keybinds = self.keybinds.clone();
        render::draw_base_view(frame, area, self, theme, &keybinds);
    }

    fn overlay_handle_event(
        &mut self,
        event: crossterm::event::Event,
        _terminal: &ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
        config: &mut crate::config::ClinConfig,
    ) -> anyhow::Result<OverlayResult> {
        let keybinds = self.keybinds.clone();
        match event {
            Event::Key(key) => {
                if key.kind == crossterm::event::KeyEventKind::Release {
                    return Ok(OverlayResult::Continue);
                }
                let r = input::handle_input(self, key, &keybinds, config);
                if let Some(result) = map_input_result(self, r) {
                    return Ok(result);
                }
            }
            Event::Mouse(mouse) => {
                let term_area = self.last_area;
                let res = input::handle_base_mouse(self, mouse, term_area);
                if let Some(result) = map_input_result(self, res) {
                    return Ok(result);
                }
            }
            _ => {}
        }
        Ok(OverlayResult::Continue)
    }
}
