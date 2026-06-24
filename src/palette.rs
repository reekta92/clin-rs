use ratatui::style::Style;
use ratatui::widgets::ListState;
use ratatui::widgets::{Block, Borders};
use ratatui_textarea::TextArea;

/// (label, glyph, category-to-filter). Tab 0 = All (no filter).
pub fn palette_tabs(icon_mode: crate::config::IconMode) -> Vec<(&'static str, &'static str, Option<crate::actions::ActionCategory>)> {
    vec![
        ("All", crate::ui::get_icon("\u{f0ca}", "\u{1f4cb}", icon_mode), None),
        (
            "Notes",
            crate::ui::get_icon("\u{f15c}", "\u{1f4c4}", icon_mode),
            Some(crate::actions::ActionCategory::Notes),
        ),
        (
            "Import",
            crate::ui::get_icon("\u{f019}", "\u{1f4e5}", icon_mode),
            Some(crate::actions::ActionCategory::Import),
        ),
        (
            "Append",
            crate::ui::get_icon("\u{f067}", "\u{2795}", icon_mode),
            Some(crate::actions::ActionCategory::Append),
        ),
        (
            "Views",
            crate::ui::get_icon("\u{f06e}", "\u{1f441}", icon_mode),
            Some(crate::actions::ActionCategory::Views),
        ),
        (
            "Settings",
            crate::ui::get_icon("\u{f013}", "\u{2699}", icon_mode),
            Some(crate::actions::ActionCategory::Settings),
        ),
    ]
}

pub struct PaletteItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub glyph: String,
    pub score: i64,
}

pub struct CommandPalette {
    pub input: TextArea<'static>,
    pub items: Vec<PaletteItem>,
    pub state: ListState,
    pub context_note_id: Option<String>,
    pub active_tab: usize,
}
impl CommandPalette {
    pub fn new(context_note_id: Option<String>, app: &crate::app::App) -> Self {
        let mut input = TextArea::default();
        input.set_cursor_line_style(Style::default());
        input.set_placeholder_text("Search commands...");
        input.set_style(app.app_theme.bg_style());
        input.set_block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.app_theme.muted)),
        );

        let mut p = Self {
            input,
            items: Vec::new(),
            state: ListState::default(),
            context_note_id,
            active_tab: 0,
        };
        p.refresh_items(app);
        p
    }

    pub fn refresh_items(&mut self, app: &crate::app::App) {
        let query = self.input.lines()[0].as_str();
        let actions = crate::actions::get_all_action_infos(app);
        let mut matched = Vec::with_capacity(actions.len());

        let category_filter = palette_tabs(app.config.ui.icon_mode)[self.active_tab].2;

        if query.is_empty() {
            for action in actions {
                if category_filter.is_some_and(|cat| action.category != cat) {
                    continue;
                }
                matched.push(PaletteItem {
                    id: action.id.clone(),
                    name: action.name.clone(),
                    description: action.description.clone(),
                    glyph: action.glyph.clone(),
                    score: 0,
                });
            }
        } else {
            use fuzzy_matcher::FuzzyMatcher;
            use fuzzy_matcher::skim::SkimMatcherV2;
            let matcher = SkimMatcherV2::default();
            for action in actions {
                if category_filter.is_some_and(|cat| action.category != cat) {
                    continue;
                }
                if let Some(score) = matcher.fuzzy_match(&action.name, query) {
                    matched.push(PaletteItem {
                        id: action.id.clone(),
                        name: action.name.clone(),
                        description: action.description.clone(),
                        glyph: action.glyph.clone(),
                        score,
                    });
                }
            }
            matched.sort_by_key(|b| std::cmp::Reverse(b.score));
        }

        self.items = matched;
        if self.items.is_empty() {
            self.state.select(None);
        } else {
            self.state.select(Some(0));
        }
    }

    pub fn handle_input(&mut self, key: crossterm::event::KeyEvent, app: &crate::app::App) -> bool {
        use crossterm::event::KeyCode;
        if crate::events::is_cancel_popup(&app.keybinds, &key, true) { return true; }
        match key.code {
            KeyCode::Tab => {
                self.active_tab = (self.active_tab + 1) % palette_tabs(app.config.ui.icon_mode).len();
                self.refresh_items(app);
            }
            KeyCode::BackTab => {
                if self.active_tab == 0 {
                    self.active_tab = palette_tabs(app.config.ui.icon_mode).len() - 1;
                } else {
                    self.active_tab -= 1;
                }
                self.refresh_items(app);
            }
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
                return true;
            }
            _ => {
                self.input.input(key);
                self.refresh_items(app);
            }
        }
        false
    }
}
