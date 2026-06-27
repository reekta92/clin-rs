use crossterm::event::Event;

use crate::content_tree::state::ContentTreeState;
use crate::content_tree::{input, render};

pub enum ContentTreeResult {
    Back,
    JumpToLine { note_id: String, line: usize },
    HelpRequested,
}

/// Shared mapping from input::ContentTreeInput to Option<ContentTreeResult>,
/// eliminating the identical match in the Key and Mouse dispatch arms.
fn map_input_result(state: &ContentTreeState, res: input::ContentTreeInput) -> Option<ContentTreeResult> {
    match res {
        input::ContentTreeInput::Back => Some(ContentTreeResult::Back),
        input::ContentTreeInput::Help => Some(ContentTreeResult::HelpRequested),
        input::ContentTreeInput::Open => {
            if !state.load_error && state.selected < state.nodes.len() {
                let line = state.nodes[state.selected].line;
                Some(ContentTreeResult::JumpToLine {
                    note_id: state.note_id.clone(),
                    line,
                })
            } else {
                Some(ContentTreeResult::Back)
            }
        }
        input::ContentTreeInput::None => None,
    }
}

impl ContentTreeState {
    pub fn overlay_render(
        &mut self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        theme: &crate::app_theme::AppThemeColors,
        _config: &crate::config::ClinConfig,
    ) {
        self.last_area = area;
        let keybinds = self.keybinds.clone();
        render::draw_content_tree(frame, area, self, theme, &keybinds);
    }

    pub fn overlay_handle_event(
        &mut self,
        event: crossterm::event::Event,
        _terminal: &ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
        config: &mut crate::config::ClinConfig,
    ) -> anyhow::Result<Option<ContentTreeResult>> {
        let keybinds = self.keybinds.clone();
        match event {
            Event::Key(key) => {
                if key.kind == crossterm::event::KeyEventKind::Release {
                    return Ok(None);
                }
                let r = input::handle_input(self, key, &keybinds, config);
                if let Some(result) = map_input_result(self, r) {
                    return Ok(Some(result));
                }
            }
            Event::Mouse(mouse) => {
                let term_area = self.last_area;
                let res = input::handle_content_tree_mouse(self, mouse, term_area);
                if let Some(result) = map_input_result(self, res) {
                    return Ok(Some(result));
                }
            }
            _ => {}
        }
        Ok(None)
    }
}
