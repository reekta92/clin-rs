use std::io::Stdout;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::Event;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::app_theme::AppThemeColors;
use crate::backup::input::{self, InputResult};
use crate::backup::render;
use crate::backup::state::BackupState;
use crate::config::ClinConfig;
use crate::keybinds::{BackupAction, Keybinds};
use crate::overlay::OverlayView;

pub enum BackupResult {
    Back,
}

impl OverlayView<BackupResult> for BackupState {
    fn render(
        &mut self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        _theme: &crate::app_theme::AppThemeColors,
        _config: &crate::config::ClinConfig,
    ) {
        self.last_area = Some(area);
        render::draw_dashboard(frame, self, area);
    }

    fn handle_event(
        &mut self,
        event: crossterm::event::Event,
        _terminal: &ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
        _config: &mut crate::config::ClinConfig,
    ) -> anyhow::Result<Option<BackupResult>> {
        match event {
            Event::Key(key) => {
                let keybinds = self.keybinds.clone();
                match input::handle_input(self, key, &keybinds) {
                    InputResult::Back => return Ok(Some(BackupResult::Back)),
                    InputResult::Refresh => self.refresh_git_info(),
                    InputResult::None => {}
                }
            }
            Event::Mouse(mouse) => {
                if let InputResult::Refresh = input::handle_mouse(self, mouse) {
                    self.refresh_git_info();
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn title(&self) -> String {
        "Backup".to_string()
    }

    fn render_title(
        &self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        _theme: &crate::app_theme::AppThemeColors,
    ) {
        render::draw_header(frame, area, self);
    }
}

pub fn run_backup_view(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    vault_path: PathBuf,
    config: &ClinConfig,
    keybinds: &Keybinds,
    app_theme: &AppThemeColors,
) -> Result<BackupResult> {
    let mut state = BackupState::new(
        vault_path,
        &config.backup,
        app_theme.clone(),
        keybinds.clone(),
        config.ui.tab_icons_only,
    );
    state.footer_hint = format!(
        "{}: commit · {}: push · {}: refresh · {}: settings · {}: ←",
        keybinds.backup_keys_display(BackupAction::EnterCommit),
        keybinds.backup_keys_display(BackupAction::Push),
        keybinds.backup_keys_display(BackupAction::Refresh),
        keybinds.backup_keys_display(BackupAction::OpenSettings),
        keybinds.backup_keys_display(BackupAction::Back),
    );

    // Cast or clone config as mutable to fit run_overlay signature
    let mut config_mut = config.clone();
    crate::overlay::run_overlay(
        terminal,
        &mut state,
        &mut config_mut,
        app_theme,
        Duration::from_millis(100),
    )
}
