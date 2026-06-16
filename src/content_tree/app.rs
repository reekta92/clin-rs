use anyhow::Result;
use crossterm::event::Event;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::Stdout;
use std::time::Duration;

use crate::app_theme::AppThemeColors;
use crate::content_tree::state::ContentTreeState;
use crate::content_tree::{input, render};
use crate::keybinds::Keybinds;
use crate::overlay::OverlayView;
use crate::storage::Storage;

pub enum ContentTreeResult {
    Back,
    JumpToLine { note_id: String, line: usize },
    HelpRequested,
}

impl OverlayView<ContentTreeResult> for ContentTreeState {
    fn render(
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

    fn handle_event(
        &mut self,
        event: crossterm::event::Event,
        _terminal: &ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
        _config: &mut crate::config::ClinConfig,
    ) -> anyhow::Result<Option<ContentTreeResult>> {
        let keybinds = self.keybinds.clone();
        match event {
            Event::Key(key) => {
                if key.kind == crossterm::event::KeyEventKind::Release {
                    return Ok(None);
                }
                match input::handle_input(self, key, &keybinds) {
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
                match input::handle_content_tree_mouse(self, mouse, term_area.into()) {
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

    fn title(&self) -> String {
        format!("CONTENT TREE — {}", self.note_title)
    }
}

pub fn run_content_tree_view(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    storage: Storage,
    note_id: Option<String>,
    keybinds: &Keybinds,
    theme: AppThemeColors,
) -> Result<ContentTreeResult> {
    let mut state = if let Some(id) = note_id {
        match storage.load_note(&id) {
            Ok(note) => ContentTreeState::new(id, &note.title, &note.content, keybinds.clone()),
            Err(_) => ContentTreeState::error(id, keybinds.clone()),
        }
    } else {
        ContentTreeState::error(String::new(), keybinds.clone())
    };

    let mut config = crate::config::ClinConfig::default();
    crate::overlay::run_overlay(
        terminal,
        &mut state,
        &mut config,
        &theme,
        Duration::from_millis(100),
    )
}
