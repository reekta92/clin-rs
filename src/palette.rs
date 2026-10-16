use ratatui::style::Style;
use ratatui::widgets::ListState;
use ratatui::widgets::{Block, Borders};
use ratatui_textarea::TextArea;

/// (label, glyph, category-to-filter). Tab 0 = All (no filter).
pub type PaletteTab = (
    &'static str,
    &'static str,
    Option<crate::actions::ActionCategory>,
);

pub fn palette_tabs(app: &crate::app::App) -> Vec<PaletteTab> {
    if let Some(palette) = &app.command_palette {
        return palette.tabs.clone();
    }
    tabs_for_actions(app, &crate::actions::get_all_action_infos(app))
}

fn tabs_for_actions(
    app: &crate::app::App,
    actions: &[crate::actions::ActionInfo],
) -> Vec<PaletteTab> {
    let icon_mode = app.config.ui.icon_mode;
    let has_cat = |cat: crate::actions::ActionCategory| actions.iter().any(|a| a.category == cat);

    let mut tabs = vec![(
        "All",
        crate::ui::get_icon("\u{f0ca}", "\u{1f4cb}", icon_mode),
        None,
    )];

    if has_cat(crate::actions::ActionCategory::Notes) {
        tabs.push((
            "Notes",
            crate::ui::get_icon("\u{f15c}", "\u{1f4c4}", icon_mode),
            Some(crate::actions::ActionCategory::Notes),
        ));
    }
    if has_cat(crate::actions::ActionCategory::Import) {
        tabs.push((
            "Import",
            crate::ui::get_icon("\u{f019}", "\u{1f4e5}", icon_mode),
            Some(crate::actions::ActionCategory::Import),
        ));
    }
    if has_cat(crate::actions::ActionCategory::Append) {
        tabs.push((
            "Append",
            crate::ui::get_icon("\u{f067}", "\u{2795}", icon_mode),
            Some(crate::actions::ActionCategory::Append),
        ));
    }
    if has_cat(crate::actions::ActionCategory::Views) {
        tabs.push((
            "Views",
            crate::ui::get_icon("\u{f06e}", "\u{1f441}", icon_mode),
            Some(crate::actions::ActionCategory::Views),
        ));
    }
    if has_cat(crate::actions::ActionCategory::Settings) {
        tabs.push((
            "Settings",
            crate::ui::get_icon("\u{f013}", "\u{2699}", icon_mode),
            Some(crate::actions::ActionCategory::Settings),
        ));
    }
    tabs
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
    pub(crate) context: crate::actions::ActionContext,
    pub tabs: Vec<PaletteTab>,
    pub active_tab: usize,
    last_filter: Option<(String, usize)>,
    pub last_scroll: Option<crate::ui::scrollbar::ScrollbarMeta>,
    pub scroll_drag: Option<i32>,
    pub last_results_area: Option<ratatui::layout::Rect>,
    pub(crate) mouse_selection: crate::text_edit::MouseTextSelection,
}

impl CommandPalette {
    pub(crate) fn paste_text(&mut self, data: &str, app: &crate::app::App) {
        self.input
            .insert_str(data.replace("\r\n", " ").replace(['\r', '\n'], " "));
        self.refresh_items(app);
    }
    pub fn new(context_note_id: Option<String>, app: &crate::app::App) -> Self {
        Self::with_context(
            crate::actions::ActionContext::capture(app, None, context_note_id.as_deref()),
            app,
        )
    }

    pub(crate) fn with_context(
        context: crate::actions::ActionContext,
        app: &crate::app::App,
    ) -> Self {
        let mut input = crate::ui::make_popup_textarea(&app.app_theme, "Search commands...");
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
            context_note_id: context.note_id.clone(),
            context,
            tabs: Vec::new(),
            active_tab: 0,
            last_filter: None,
            last_scroll: None,
            scroll_drag: None,
            last_results_area: None,
            mouse_selection: crate::text_edit::MouseTextSelection::default(),
        };
        p.refresh_items(app);
        p
    }

    pub fn refresh_items(&mut self, app: &crate::app::App) {
        let query = self.input.lines()[0].as_str();
        let filter_changed = self
            .last_filter
            .as_ref()
            .is_none_or(|(previous, tab)| previous != query || *tab != self.active_tab);
        let selected = self
            .state
            .selected()
            .and_then(|i| self.items.get(i))
            .map(|item| item.id.clone());
        let actions = crate::actions::get_action_infos_in_context(app, &self.context);
        let mut matched = Vec::with_capacity(actions.len());
        self.tabs = tabs_for_actions(app, &actions);
        let tabs = &self.tabs;
        // Handle active_tab being out of bounds
        if self.active_tab >= tabs.len() {
            self.active_tab = 0;
        }
        self.last_filter = Some((query.to_owned(), self.active_tab));
        let category_filter = tabs[self.active_tab].2;
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
            self.state.select(
                selected
                    .and_then(|id| self.items.iter().position(|item| item.id == id))
                    .or_else(|| filter_changed.then_some(0)),
            );
        }
    }

    pub fn handle_input(&mut self, key: crossterm::event::KeyEvent, app: &crate::app::App) -> bool {
        use crossterm::event::{KeyCode, KeyEventKind};
        if key.kind != KeyEventKind::Press || key.code == KeyCode::F(6) {
            return false;
        }
        if crate::events::is_cancel_popup(&app.keybinds, &key, true) {
            return true;
        }
        match key.code {
            KeyCode::Tab => {
                let len = self.tabs.len();
                self.active_tab = (self.active_tab + 1) % len;
                self.refresh_items(app);
            }
            KeyCode::BackTab => {
                let len = self.tabs.len();
                if self.active_tab == 0 {
                    self.active_tab = len.saturating_sub(1);
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
                if app
                    .keybinds
                    .matches_edit(crate::keybinds::EditAction::Paste, &key)
                    && let Some(text) = crate::text_edit::read_system_clipboard()
                {
                    self.paste_text(&text, app);
                    return false;
                }
                crate::text_edit::feed_key(&app.keybinds, &mut self.input, key);
                self.refresh_items(app);
            }
        }
        false
    }
}
