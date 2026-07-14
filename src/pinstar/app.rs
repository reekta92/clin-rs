use crossterm::event::Event;

use crate::pinstar::input::{handle_pinstar_event, handle_pinstar_mouse};
use crate::pinstar::render::draw_pinstar_view;
use crate::pinstar::state::PinstarState;

impl crate::overlay::OverlayView for PinstarState {
    fn overlay_render(
        &mut self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        app: &mut crate::app::App,
    ) {
        self.last_area = area;
        draw_pinstar_view(frame, self, app, area, self.mouse_pos);
    }

    fn overlay_handle_event(
        &mut self,
        event: crossterm::event::Event,
        app: &mut crate::app::App,
        _terminal: &ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    ) -> anyhow::Result<crate::overlay::OverlayResult> {
        let area = self.last_area;
        let keybinds = self.keybinds.clone();
        let mut running = true;
        match event {
            Event::Key(key) => {
                let _ = handle_pinstar_event(self, key, &mut running, area, &keybinds, app);
            }
            Event::Mouse(mouse) => {
                self.mouse_pos = Some((mouse.column, mouse.row));
                handle_pinstar_mouse(self, mouse, area, app);
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
