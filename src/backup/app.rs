use std::io::Stdout;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use crossterm::event::{self, Event};

use crate::app_theme::AppThemeColors;
use crate::backup::render;
use crate::backup::input::{self, InputResult};
use crate::backup::state::BackupState;
use crate::config::ClinConfig;
use crate::keybinds::Keybinds;

pub enum BackupResult {
    Back,
}

pub fn run_backup_view(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    vault_path: PathBuf,
    config: &ClinConfig,
    _keybinds: &Keybinds,
    app_theme: &AppThemeColors,
) -> Result<BackupResult> {
    let mut state = BackupState::new(vault_path, &config.backup, app_theme.clone());

    loop {
        state.last_area = Some(terminal.size()?.into());
        terminal.draw(|frame| render::draw_dashboard(frame, &state))?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    match input::handle_input(&mut state, key) {
                        InputResult::Back => return Ok(BackupResult::Back),
                        InputResult::Refresh => state.refresh_git_info(),
                        InputResult::None => {}
                    }
                }
                Event::Mouse(mouse) => {
                    match input::handle_mouse(&mut state, mouse) {
                        InputResult::Refresh => state.refresh_git_info(),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
}
