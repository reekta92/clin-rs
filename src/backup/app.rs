use crossterm::event::Event;

use crate::backup::input::{self, InputResult};
use crate::backup::render;
use crate::backup::state::BackupState;

impl crate::overlay::OverlayView for BackupState {
    fn overlay_render(
        &mut self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        app: &mut crate::app::App,
    ) {
        self.last_area = Some(area);
        let app_status = app.status.as_ref();
        render::draw_dashboard(frame, self, area, &app.config, Some(app_status));
    }

    fn overlay_handle_event(
        &mut self,
        event: crossterm::event::Event,
        app: &mut crate::app::App,
        _term_area: ratatui::layout::Rect,
    ) -> anyhow::Result<crate::overlay::OverlayResult> {
        match event {
            Event::Key(key) => {
                let keybinds = self.keybinds.clone();
                match input::handle_input(self, key, &keybinds, &app.config) {
                    InputResult::Back => {
                        return Ok(crate::overlay::OverlayResult::Exit);
                    }
                    InputResult::Refresh => self.refresh_git_info(),
                    InputResult::Help => {
                        return Ok(crate::overlay::OverlayResult::OpenHelp(
                            crate::app::HelpTab::Backup,
                        ));
                    }
                    InputResult::None => {}
                }
            }
            Event::Mouse(mouse) => {
                if let InputResult::Refresh =
                    input::handle_mouse(self, mouse, app.config.ui.icon_mode)
                {
                    self.refresh_git_info();
                }
            }
            _ => {}
        }
        Ok(crate::overlay::OverlayResult::Continue)
    }
}
