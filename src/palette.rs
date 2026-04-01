use ratatui::style::Style;
use ratatui::widgets::ListState;
use ratatui::widgets::{Block, Borders};
use tui_textarea::TextArea;

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
        input.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Command Palette "),
        );

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
        let query = self.input.lines()[0].as_str();
        let actions = crate::actions::get_cached_action_infos();
        let mut matched = Vec::with_capacity(actions.len());

        if query.is_empty() {
            for action in actions {
                matched.push(PaletteItem {
                    id: action.id.clone(),
                    name: action.name.clone(),
                    description: action.description.clone(),
                    score: 0,
                });
            }
        } else {
            use fuzzy_matcher::skim::SkimMatcherV2;
            use fuzzy_matcher::FuzzyMatcher;
            let matcher = SkimMatcherV2::default();
            for action in actions {
                if let Some(score) = matcher.fuzzy_match(&action.name, query) {
                    matched.push(PaletteItem {
                        id: action.id.clone(),
                        name: action.name.clone(),
                        description: action.description.clone(),
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
