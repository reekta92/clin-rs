use crossterm::event::Event;

use crate::pinstar::input::{handle_pinstar_event, handle_pinstar_mouse};
use crate::pinstar::render::draw_pinstar_view;
use crate::pinstar::state::PinstarState;

impl crate::overlay::OverlayView for PinstarState {
    fn overlay_render(
        &mut self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        theme: &crate::app_theme::AppThemeColors,
        _config: &crate::config::ClinConfig,
        _app_status: Option<&str>,
    ) {
        self.last_area = area;
        draw_pinstar_view(frame, self, theme, area);
    }

    fn overlay_handle_event(
        &mut self,
        event: crossterm::event::Event,
        _terminal: &ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
        config: &mut crate::config::ClinConfig,
    ) -> anyhow::Result<crate::overlay::OverlayResult> {
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
            return Ok(if self.help_requested {
                self.help_requested = false;
                crate::overlay::OverlayResult::OpenHelp(crate::app::HelpTab::Canvas)
            } else {
                crate::overlay::OverlayResult::Exit
            });
        }
        Ok(crate::overlay::OverlayResult::Continue)
    }
}
