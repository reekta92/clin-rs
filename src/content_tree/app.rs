use crossterm::event::Event;

use crate::content_tree::state::ContentTreeState;
use crate::content_tree::{input, render};

pub enum ContentTreeResult {
    Back,
    JumpToLine { note_id: String, line: usize },
    HelpRequested,
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
                match input::handle_input(self, key, &keybinds, config) {
                    input::InputResult::Back => return Ok(Some(ContentTreeResult::Back)),
                    input::InputResult::Help => return Ok(Some(ContentTreeResult::HelpRequested)),
                    input::InputResult::Open => {
                        if !self.load_error && self.selected < self.nodes.len() {
                            let node = &self.nodes[self.selected];
                            return Ok(Some(ContentTreeResult::JumpToLine {
                                note_id: self.note_id.clone(),
                                line: node.line,
                            }));
                        } else {
                            return Ok(Some(ContentTreeResult::Back));
                        }
                    }
                    input::InputResult::None => {}
                }
            }
            Event::Mouse(mouse) => {
                let term_area = self.last_area;
                match input::handle_content_tree_mouse(self, mouse, term_area) {
                    input::InputResult::Back => return Ok(Some(ContentTreeResult::Back)),
                    input::InputResult::Help => return Ok(Some(ContentTreeResult::HelpRequested)),
                    input::InputResult::Open => {
                        if !self.load_error && self.selected < self.nodes.len() {
                            let node = &self.nodes[self.selected];
                            return Ok(Some(ContentTreeResult::JumpToLine {
                                note_id: self.note_id.clone(),
                                line: node.line,
                            }));
                        } else {
                            return Ok(Some(ContentTreeResult::Back));
                        }
                    }
                    input::InputResult::None => {}
                }
            }
            _ => {}
        }
        Ok(None)
    }
}
