use crossterm::event::Event;

use crate::content_tree::state::ContentTreeState;
use crate::content_tree::{input, render};

/// Shared mapping from input::ContentTreeInput to Option<OverlayResult>,
/// eliminating the identical match in the Key and Mouse dispatch arms.
fn map_input_result(
    state: &ContentTreeState,
    res: input::ContentTreeInput,
) -> Option<crate::overlay::OverlayResult> {
    use crate::overlay::OverlayResult;
    match res {
        input::ContentTreeInput::Back => Some(OverlayResult::Exit),
        input::ContentTreeInput::Help => {
            Some(OverlayResult::OpenHelp(crate::app::HelpTab::Notes))
        }
        input::ContentTreeInput::Open => {
            if !state.load_error && state.selected < state.nodes.len() {
                let line = state.nodes[state.selected].line;
                Some(OverlayResult::JumpToLine {
                    note_id: state.note_id.clone(),
                    line,
                })
            } else {
                Some(OverlayResult::Exit)
            }
        }
        input::ContentTreeInput::None => None,
    }
}

impl crate::overlay::OverlayView for ContentTreeState {
    fn overlay_render(
        &mut self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        theme: &crate::app_theme::AppThemeColors,
        config: &crate::config::ClinConfig,
        app_status: Option<&str>,
    ) {
        self.last_area = area;
        let keybinds = self.keybinds.clone();
        render::draw_content_tree(frame, area, self, theme, &keybinds, config, app_status);
    }

    fn overlay_handle_event(
        &mut self,
        event: crossterm::event::Event,
        _terminal: &ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
        config: &mut crate::config::ClinConfig,
    ) -> anyhow::Result<crate::overlay::OverlayResult> {
        let keybinds = self.keybinds.clone();
        match event {
            Event::Key(key) => {
                if key.kind == crossterm::event::KeyEventKind::Release {
                    return Ok(crate::overlay::OverlayResult::Continue);
                }
                let r = input::handle_input(self, key, &keybinds, config);
                if let Some(result) = map_input_result(self, r) {
                    return Ok(result);
                }
            }
            Event::Mouse(mouse) => {
                let term_area = self.last_area;
                let res = input::handle_content_tree_mouse(self, mouse, term_area);
                if let Some(result) = map_input_result(self, res) {
                    return Ok(result);
                }
            }
            _ => {}
        }
        Ok(crate::overlay::OverlayResult::Continue)
    }
}
