use crate::app_theme::AppThemeColors;
use crate::keybinds::{CanvasAction, Keybinds};
use crate::overlay::OverlayView;
use crate::pinstar::input::{handle_pinstar_event, handle_pinstar_mouse};
use crate::pinstar::render::draw_pinstar_view;
use crate::pinstar::state::PinstarState;
use crate::storage::Storage;
use crossterm::event::Event;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::Stdout;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinstarResult {
    Normal,
    HelpRequested,
}

impl OverlayView<PinstarResult> for PinstarState {
    fn render(
        &mut self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        theme: &crate::app_theme::AppThemeColors,
        _config: &crate::config::ClinConfig,
    ) {
        self.last_area = area;
        draw_pinstar_view(frame, self, theme, area);
    }

    fn handle_event(
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

    fn title(&self) -> String {
        "Canvas".to_string()
    }
}

pub fn run_pinstar_view(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    storage: Storage,
    keybinds: &Keybinds,
    file_id: Option<String>,
    theme: AppThemeColors,
    seq_matcher: &mut crate::keybinds::KeyMatcher,
) -> anyhow::Result<PinstarResult> {
    let mut state = if let Some(id) = file_id {
        let path = storage.note_path(&id);
        PinstarState::load(&path, keybinds.clone(), seq_matcher.clone())?
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

    let mut config = crate::config::ClinConfig::default();
    crate::overlay::run_overlay(
        terminal,
        &mut state,
        &mut config,
        &theme,
        Duration::from_millis(100),
    )
}
