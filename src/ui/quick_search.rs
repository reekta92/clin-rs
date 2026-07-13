use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::app_theme::AppThemeColors;
use crate::events::handle_popup_text_input;
use crate::keybinds::Keybinds;

/// Unified quick search that replaces duplicated search/find implementations.
/// Renders into the header bar with a dropdown for results.
/// Uses `ratatui_textarea::TextArea` for robust text input handling.
#[derive(Debug, Clone)]
pub struct QuickSearch<T> {
    pub title: String,
    pub input: ratatui_textarea::TextArea<'static>,
    pub results: Vec<T>,
    pub selected: usize,
}

/// Actions returned by `handle_quick_search_keys`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickSearchAction {
    /// Enter key pressed — confirm selection.
    Submit,
    /// Cancel/close key pressed.
    Cancel,
    /// Text input changed (character inserted, deleted, etc.).
    Edited,
    /// List navigation (Up/Down/Tab).
    Navigated,
}
impl<T> QuickSearch<T> {
    pub fn new(title: &str, theme: &AppThemeColors) -> Self {
        let input = crate::ui::popups::make_popup_textarea(theme, "Search…");
        Self {
            title: title.to_string(),
            input,
            results: Vec::new(),
            selected: 0,
        }
    }

    /// Safely extract the first line of the input text.
    pub fn query(&self) -> String {
        self.input.lines().first().cloned().unwrap_or_default()
    }
}

/// Shared key routing for any quick search.
pub fn handle_quick_search_keys<T>(
    popup: &mut QuickSearch<T>,
    key: KeyEvent,
    keybinds: &Keybinds,
) -> QuickSearchAction {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    if crate::events::is_cancel_popup(keybinds, &key, true) {
        return QuickSearchAction::Cancel;
    }

    match key.code {
        KeyCode::Enter => QuickSearchAction::Submit,
        KeyCode::Up => {
            if popup.selected > 0 {
                popup.selected -= 1;
            }
            QuickSearchAction::Navigated
        }
        KeyCode::Down => {
            if !popup.results.is_empty() && popup.selected + 1 < popup.results.len() {
                popup.selected += 1;
            }
            QuickSearchAction::Navigated
        }
        KeyCode::BackTab | KeyCode::Tab if shift => {
            if !popup.results.is_empty() {
                popup.selected = popup
                    .selected
                    .checked_sub(1)
                    .unwrap_or(popup.results.len().saturating_sub(1));
            }
            QuickSearchAction::Navigated
        }
        KeyCode::Tab => {
            if !popup.results.is_empty() {
                popup.selected = (popup.selected + 1) % popup.results.len();
            }
            QuickSearchAction::Navigated
        }
        KeyCode::Char('a') if ctrl => {
            return QuickSearchAction::Edited;
        }
        KeyCode::Char('e') if ctrl => {
            return QuickSearchAction::Edited;
        }
        _ => {
            handle_popup_text_input(key, &mut popup.input, keybinds);
            return QuickSearchAction::Edited;
        }
    }
}

/// Shared rendering for any quick search.
///
/// Renders an accent-colored full-width header bar with a centered input field,
/// and a dropdown results list immediately beneath it.
///
/// `render_item` converts a result item into a styled `Line`; it receives
/// the item, whether it is currently selected, and the theme colors.
pub fn draw_quick_search<T, F>(
    frame: &mut Frame,
    _area: Rect,
    popup: &QuickSearch<T>,
    theme: &AppThemeColors,
    max_visible: usize,
    render_item: F,
) where
    F: Fn(&T, bool, &AppThemeColors) -> Line<'static>,
{
    let frame_area = frame.area();
    let result_count = popup.results.len();
    let query = popup.query();
    let has_query = !query.is_empty();

    // --- Header bar: full-width accent background ---
    let header_rect = Rect::new(frame_area.x, frame_area.y, frame_area.width, 1);
    let header_block = Block::default().style(Style::default().bg(theme.accent));
    frame.render_widget(Clear, header_rect);
    frame.render_widget(&header_block, header_rect);

    // --- "Find:" label + centered input field ---
    let label_width = 6u16; // "Find: "
    let input_width = 50u16.min(frame_area.width.saturating_sub(label_width));
    let combo_width = label_width + input_width;
    let start_x = frame_area.x + (frame_area.width.saturating_sub(combo_width)) / 2;
    let label_area = Rect::new(start_x, frame_area.y, label_width, 1);
    let input_area = Rect::new(start_x + label_width, frame_area.y, input_width, 1);

    let label_style = Style::default().fg(theme.highlight_fg);
    let find_label = Paragraph::new(Line::from(Span::styled("Find: ", label_style)));
    frame.render_widget(find_label, label_area);

    let mut input_widget = popup.input.clone();
    input_widget.set_block(Block::default());
    let input_style = Style::default()
        .bg(theme.accent)
        .fg(theme.highlight_fg);
    input_widget.set_style(input_style);
    input_widget.set_cursor_line_style(input_style);
    frame.render_widget(&input_widget, input_area);

    // --- Dropdown results list ---
    let visible_count = result_count.min(max_visible);
    let dropdown_height = if visible_count > 0 {
        visible_count
    } else if has_query {
        1 // show "No matches"
    } else {
        0
    };

    if dropdown_height > 0 {
        let dropdown_area = Rect::new(
            start_x,
            frame_area.y + 1,
            combo_width,
            dropdown_height as u16,
        );

        // Clear the area behind the dropdown
        frame.render_widget(Clear, dropdown_area);
        // Fill dropdown area with accent to match header bar
        frame.render_widget(Block::default().style(Style::default().bg(theme.accent)), dropdown_area);

        if result_count == 0 {
            let no_match = Paragraph::new(Line::styled(
                "  No matches",
                Style::default().fg(theme.text),
            ));
            frame.render_widget(no_match, dropdown_area);
        } else {
            let scroll_offset = popup.selected.saturating_sub(max_visible.saturating_sub(1));
            let items: Vec<ListItem<'static>> = popup
                .results
                .iter()
                .enumerate()
                .skip(scroll_offset)
                .take(max_visible)
                .map(|(i, item)| {
                    let is_selected = i == popup.selected;
                    let line = render_item(item, is_selected, theme);
                    ListItem::new(line)
                })
                .collect();

            let list = List::new(items);
            frame.render_widget(list, dropdown_area);
        }
    }
}
