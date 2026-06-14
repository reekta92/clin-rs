use crate::app_theme::AppThemeColors;
use crate::keybinds::{CanvasAction, Keybinds};
use crate::pinstar::input::{handle_pinstar_event, handle_pinstar_mouse};
use crate::pinstar::render::draw_pinstar_view;
use crate::pinstar::state::PinstarState;
use crate::storage::Storage;
use crossterm::event::{self, Event};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::Stdout;
use std::time::Duration;

pub enum PinstarResult {
    Normal,
    HelpRequested,
}

pub fn run_pinstar_view(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    storage: Storage,
    keybinds: &Keybinds,
    file_id: Option<String>,
    theme: AppThemeColors,
) -> anyhow::Result<PinstarResult> {
    let mut state = if let Some(id) = file_id {
        let path = storage.note_path(&id);
        PinstarState::load(&path)?
    } else {
        anyhow::bail!("No file ID provided for Pinstar view");
    };

    state.footer_hint = format!(
        "{} switch focus · {} back · Arrows select · {} edit · {} save",
        keybinds.canvas_keys_display(CanvasAction::CycleFocus),
        keybinds.canvas_keys_display(CanvasAction::Quit),
        keybinds.canvas_keys_display(CanvasAction::EditOrConnect),
        keybinds.canvas_keys_display(CanvasAction::Save),
    );
    let mut running = true;

    while running {

        terminal.draw(|frame| {
            let full = frame.area();
            let outer = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Length(1),
                    ratatui::layout::Constraint::Min(0),
                ])
                .split(full);
            crate::ui::draw_view_title_bar(frame, outer[0], "Canvas", &theme);
            draw_pinstar_view(frame, &mut state, &theme, outer[1]);
        })?;

        if event::poll(Duration::from_millis(100))? {
            let mut pending = true;
            while pending {
                let term_area: ratatui::layout::Rect = terminal.size()?.into();
                let outer = ratatui::layout::Layout::default()
                    .direction(ratatui::layout::Direction::Vertical)
                    .constraints([
                        ratatui::layout::Constraint::Length(1),
                        ratatui::layout::Constraint::Min(0),
                    ])
                    .split(term_area);
                let area = outer[1];
                match event::read()? {
                    Event::Key(key) => {
                        if !handle_pinstar_event(&mut state, key, &mut running, area, keybinds) {}
                    }
                    Event::Mouse(mouse) => {
                        handle_pinstar_mouse(&mut state, mouse, area);
                    }
                    Event::Resize(_, _) => {
                        terminal.autoresize()?;
                        let _ = terminal.clear();
                    }
                    _ => {}
                }
                pending = event::poll(Duration::ZERO)?;
            }
        }
    }

    if state.help_requested {
        Ok(PinstarResult::HelpRequested)
    } else {
        Ok(PinstarResult::Normal)
    }
}
