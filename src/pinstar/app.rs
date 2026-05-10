use crate::keybinds::Keybinds;
use crate::pinstar::input::{handle_pinstar_event, handle_pinstar_mouse};
use crate::pinstar::render::draw_pinstar_view;
use crate::pinstar::state::PinstarState;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::Stdout;
use crate::app_theme::AppThemeColors;
use crate::storage::Storage;
use crossterm::event::{self, Event};
use std::time::Duration;

pub fn run_pinstar_view(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    storage: Storage,
    _keybinds: &Keybinds,
    file_id: Option<String>,
    theme: AppThemeColors,
) -> anyhow::Result<()> {
    let mut state = if let Some(id) = file_id {
        let path = storage.note_path(&id);
        PinstarState::load(&path)?
    } else {
        anyhow::bail!("No file ID provided for Pinstar view");
    };

    let mut running = true;

    while running {
        terminal.draw(|frame| {
            draw_pinstar_view(frame, &mut state, &theme);
        })?;

        if event::poll(Duration::from_millis(100))? {
            let area = terminal.size()?; 
            match event::read()? {
                Event::Key(key) => {
                    if !handle_pinstar_event(&mut state, key, &mut running) {
                        // event handled
                    }
                }
                Event::Mouse(mouse) => {
                    handle_pinstar_mouse(&mut state, mouse, area.into());
                }
                _ => {}
            }
        }
    }

    Ok(())
}
