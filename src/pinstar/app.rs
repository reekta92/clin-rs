use crossterm::event::Event;

use crate::pinstar::render::draw_pinstar_view;
use crate::pinstar::input::{handle_pinstar_event, handle_pinstar_mouse};
use crate::pinstar::state::PinstarState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinstarResult {
    Normal,
    HelpRequested,
}

impl PinstarState {
    pub fn overlay_render(
        &mut self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        theme: &crate::app_theme::AppThemeColors,
        _config: &crate::config::ClinConfig,
    ) {
        self.last_area = area;
        draw_pinstar_view(frame, self, theme, area);
    }

    pub fn overlay_handle_event(
        &mut self,
        event: crossterm::event::Event,
        _terminal: &ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
        config: &mut crate::config::ClinConfig,
    ) -> anyhow::Result<Option<PinstarResult>> {
        let area = self.last_area;
        let keybinds = self.keybinds.clone();
        let mut running = true;
        match event {
            Event::Key(key) => {
                let _ = handle_pinstar_event(self, key, &mut running, area, &keybinds, config);
            }
            Event::Mouse(mouse) => {
                handle_pinstar_mouse(self, mouse, area);
            }
            _ => {}
        }
        if !running {
            let res = if self.help_requested {
                PinstarResult::HelpRequested
            } else {
                PinstarResult::Normal
            };
            return Ok(Some(res));
        }
        Ok(None)
    }
}
