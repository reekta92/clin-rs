use anyhow::Result;
use crossterm::event::{self, Event};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::Stdout;
use std::time::Duration;

use crate::app_theme::AppThemeColors;
use crate::content_tree::state::ContentTreeState;
use crate::content_tree::{input, render};
use crate::keybinds::Keybinds;
use crate::storage::Storage;

pub enum ContentTreeResult {
    Back,
    JumpToLine { note_id: String, line: usize },
    HelpRequested,
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
            Ok(note) => ContentTreeState::new(id, &note.title, &note.content),
            Err(_) => ContentTreeState::error(id),
        }
    } else {
        ContentTreeState::error(String::new())
    };

    loop {
        terminal.draw(|f| {
            let area = f.area();
            render::draw_content_tree(f, area, &state, &theme, keybinds);
        })?;

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            // Check if key is release to avoid duplicate processing on some platforms
            if key.kind == crossterm::event::KeyEventKind::Release {
                continue;
            }
            match input::handle_input(&mut state, key, keybinds) {
                input::InputResult::Back => return Ok(ContentTreeResult::Back),
                input::InputResult::Help => return Ok(ContentTreeResult::HelpRequested),
                input::InputResult::Open => {
                    if !state.load_error && state.selected < state.nodes.len() {
                        let node = &state.nodes[state.selected];
                        return Ok(ContentTreeResult::JumpToLine {
                            note_id: state.note_id.clone(),
                            line: node.line,
                        });
                    } else {
                        return Ok(ContentTreeResult::Back);
                    }
                }
                input::InputResult::None => {}
            }
        }
    }
}
