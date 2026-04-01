use crate::actions::get_all_actions;
use ratatui::widgets::ListState;
use tui_textarea::TextArea;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders};

pub struct PaletteItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub score: i64,
}

pub struct CommandPalette {
    pub input: TextArea<'static>,
    pub items: Vec<PaletteItem>,
    pub state: ListState,
    pub context_note_id: Option<String>,
}

impl CommandPalette {
    pub fn new(context_note_id: Option<String>) -> Self {
        let mut input = TextArea::default();
        input.set_cursor_line_style(Style::default());
        input.set_placeholder_text("Search commands...");
        input.set_block(Block::default().borders(Borders::ALL).title(" Command Palette "));

        let mut p = Self {
            input,
            items: Vec::new(),
            state: ListState::default(),
            context_note_id,
        };
        p.refresh_items();
        p
    }

    pub fn refresh_items(&mut self) {
        let query = self.input.lines()[0].clone();
        let actions = get_all_actions();
        let mut matched = Vec::new();

        if query.is_empty() {
            for action in actions {
                matched.push(PaletteItem {
                    id: action.id().into_owned(),
                    name: action.name().into_owned(),
                    description: action.description().into_owned(),
                    score: 0,
                });
            }
        } else {
            use fuzzy_matcher::FuzzyMatcher;
            use fuzzy_matcher::skim::SkimMatcherV2;
            let matcher = SkimMatcherV2::default();
            for action in actions {
                let name = action.name();
                if let Some(score) = matcher.fuzzy_match(&name, &query) {
                    matched.push(PaletteItem {
                        id: action.id().into_owned(),
                        name: name.into_owned(),
                        description: action.description().into_owned(),
                        score,
                    });
                }
            }
            matched.sort_by(|a, b| b.score.cmp(&a.score));
        }

        self.items = matched;
        if self.items.is_empty() {
            self.state.select(None);
        } else {
            self.state.select(Some(0));
        }
    }

    pub fn handle_input(&mut self, key: crossterm::event::KeyEvent) -> bool {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => return true,
            KeyCode::Down => {
                let i = match self.state.selected() {
                    Some(i) => {
                        if i >= self.items.len().saturating_sub(1) {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                if !self.items.is_empty() {
                    self.state.select(Some(i));
                }
            }
            KeyCode::Up => {
                let i = match self.state.selected() {
                    Some(i) => {
                        if i == 0 {
                            self.items.len().saturating_sub(1)
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                if !self.items.is_empty() {
                    self.state.select(Some(i));
                }
            }
            KeyCode::Enter => {
                // Return true, but we need to signal that we picked an item
                // This is a quick way to handle it: return true and the caller checks state
                return true;
            }
            _ => {
                self.input.input(key);
                self.refresh_items();
            }
        }
        false
    }
}
