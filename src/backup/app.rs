
use crossterm::event::Event;

use crate::backup::input::{self, InputResult};
use crate::backup::render;
use crate::backup::state::BackupState;

pub enum BackupResult {
    Back,
}

impl BackupState {
    pub fn overlay_render(
        &mut self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        _theme: &crate::app_theme::AppThemeColors,
        _config: &crate::config::ClinConfig,
    ) {
        self.last_area = Some(area);
        render::draw_dashboard(frame, self, area);
    }

    pub fn overlay_handle_event(
        &mut self,
        event: crossterm::event::Event,
        _terminal: &ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
        config: &mut crate::config::ClinConfig,
    ) -> anyhow::Result<Option<BackupResult>> {
        match event {
            Event::Key(key) => {
                let keybinds = self.keybinds.clone();
                match input::handle_input(self, key, &keybinds, config) {
                    InputResult::Back => return Ok(Some(BackupResult::Back)),
                    InputResult::Refresh => self.refresh_git_info(),
                    InputResult::None => {}
                }
            }
            Event::Mouse(mouse) => {
                if let InputResult::Refresh = input::handle_mouse(self, mouse, config.ui.icon_mode) {
                    self.refresh_git_info();
                }
            }
            _ => {}
        }
        Ok(None)
    }
}
