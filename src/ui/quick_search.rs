use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Clear, List, ListItem, Paragraph},
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
    pub scroll_offset: usize,
    /// Optional match-count info rendered right-aligned in the header bar,
    /// e.g. "3/7". Populated by the find-popup Edited handler.
    pub info: Option<String>,
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
            scroll_offset: 0,
            info: None,
        }
    }

    /// Safely extract the first line of the input text.
    pub fn query(&self) -> String {
        self.input.lines().first().cloned().unwrap_or_default()
    }

    pub fn scroll_to_selected(&mut self, max_visible: usize) {
        if max_visible == 0 {
            return;
        }
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + max_visible {
            self.scroll_offset = self.selected - max_visible + 1;
        }
    }
}

/// Shared key routing for any quick search.
pub fn handle_quick_search_keys<T>(
    popup: &mut QuickSearch<T>,
    key: KeyEvent,
    keybinds: &Keybinds,
    max_visible: usize,
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
            popup.scroll_to_selected(max_visible);
            QuickSearchAction::Navigated
        }
        KeyCode::Down => {
            if !popup.results.is_empty() && popup.selected + 1 < popup.results.len() {
                popup.selected += 1;
            }
            popup.scroll_to_selected(max_visible);
            QuickSearchAction::Navigated
        }
        KeyCode::BackTab | KeyCode::Tab if shift => {
            if !popup.results.is_empty() {
                popup.selected = popup
                    .selected
                    .checked_sub(1)
                    .unwrap_or(popup.results.len().saturating_sub(1));
            }
            popup.scroll_to_selected(max_visible);
            QuickSearchAction::Navigated
        }
        KeyCode::Tab => {
            if !popup.results.is_empty() {
                popup.selected = (popup.selected + 1) % popup.results.len();
            }
            popup.scroll_to_selected(max_visible);
            QuickSearchAction::Navigated
        }
        KeyCode::Char('a') if ctrl => QuickSearchAction::Edited,
        KeyCode::Char('e') if ctrl => QuickSearchAction::Edited,
        _ => {
            handle_popup_text_input(key, &mut popup.input, keybinds);
            QuickSearchAction::Edited
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
    icon_mode: crate::config::IconMode,
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

    // --- Icon + "Find:" label + centered input field ---
    let icon = crate::ui::get_icon("\u{f002}", "\u{1f50d}", icon_mode);
    let label_text = format!("{} Find: ", icon);
    let label_width = label_text.chars().count() as u16;
    let input_width = 50u16.min(frame_area.width.saturating_sub(label_width));
    let combo_width = label_width + input_width;
    let start_x = frame_area.x + (frame_area.width.saturating_sub(combo_width)) / 2;
    let label_area = Rect::new(start_x, frame_area.y, label_width, 1);
    let input_area = Rect::new(start_x + label_width, frame_area.y, input_width, 1);

    let label_style = Style::default().fg(theme.highlight_fg);
    let find_label = Paragraph::new(Line::from(Span::styled(label_text, label_style)));
    frame.render_widget(find_label, label_area);
    let mut input_widget = popup.input.clone();
    input_widget.set_block(Block::default());
    let input_style = Style::default().bg(theme.accent).fg(theme.highlight_fg);
    input_widget.set_style(input_style);
    input_widget.set_cursor_line_style(input_style);
    frame.render_widget(&input_widget, input_area);
    // --- Match-count info (right-aligned after input) ---
    if let Some(info) = &popup.info {
        let info_style = Style::default().fg(theme.highlight_fg).bg(theme.accent);
        let info_span = Span::styled(format!(" {}", info), info_style);
        let info_width = info.len() as u16 + 1;
        let info_x = start_x + combo_width + 1;
        if info_x + info_width <= frame_area.right() {
            let info_area = Rect::new(info_x, frame_area.y, info_width, 1);
            frame.render_widget(Paragraph::new(Line::from(info_span)), info_area);
        }
    }

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
        frame.render_widget(
            Block::default().style(Style::default().bg(theme.accent)),
            dropdown_area,
        );

        if result_count == 0 {
            let no_match = Paragraph::new(Line::styled(
                "  No matches",
                Style::default().fg(theme.highlight_fg),
            ));
            frame.render_widget(no_match, dropdown_area);
        } else {
            let scroll_offset = popup.scroll_offset;
            let items: Vec<ListItem<'static>> = popup
                .results
                .iter()
                .enumerate()
                .skip(scroll_offset)
                .take(max_visible)
                .map(|(i, item)| {
                    let is_selected = i == popup.selected;
                    let line = render_item(item, is_selected, theme);
                    let mut list_item = ListItem::new(line);
                    if is_selected {
                        list_item = list_item.style(Style::default().bg(theme.heading));
                    }
                    list_item
                })
                .collect();

            let list = List::new(items);
            frame.render_widget(list, dropdown_area);
        }
    }
}

/// Shared mouse routing for any quick search.
pub fn handle_quick_search_mouse<T>(
    popup: &mut QuickSearch<T>,
    event: crossterm::event::MouseEvent,
    frame_area: Rect,
    max_visible: usize,
    icon_mode: crate::config::IconMode,
) -> Option<QuickSearchAction> {
    let result_count = popup.results.len();
    let query = popup.query();
    let has_query = !query.is_empty();

    let header_rect = Rect::new(frame_area.x, frame_area.y, frame_area.width, 1);

    let icon = crate::ui::get_icon("\u{f002}", "\u{1f50d}", icon_mode);
    let label_text = format!("{} Find: ", icon);
    let label_width = label_text.chars().count() as u16;
    let input_width = 50u16.min(frame_area.width.saturating_sub(label_width));
    let combo_width = label_width + input_width;
    let start_x = frame_area.x + (frame_area.width.saturating_sub(combo_width)) / 2;

    let visible_count = result_count.min(max_visible);
    let dropdown_height = if visible_count > 0 {
        visible_count
    } else if has_query {
        1 // show "No matches"
    } else {
        0
    };

    let dropdown_area = if dropdown_height > 0 {
        Some(Rect::new(
            start_x,
            frame_area.y + 1,
            combo_width,
            dropdown_height as u16,
        ))
    } else {
        None
    };

    let over_header = crate::events::contains_cell(header_rect, event.column, event.row);
    let over_dropdown = dropdown_area.map_or(false, |area| {
        crate::events::contains_cell(area, event.column, event.row)
    });

    match event.kind {
        crossterm::event::MouseEventKind::Moved => {
            if over_dropdown && result_count > 0 {
                let dropdown_rect = dropdown_area.unwrap();
                let visual_row = event.row.saturating_sub(dropdown_rect.y) as usize;
                let target_index = popup.scroll_offset + visual_row;
                if target_index < result_count && popup.selected != target_index {
                    popup.selected = target_index;
                    return Some(QuickSearchAction::Navigated);
                }
            }
        }
        crossterm::event::MouseEventKind::ScrollUp => {
            if over_dropdown && result_count > 0 {
                if popup.selected > 0 {
                    popup.selected -= 1;
                    popup.scroll_to_selected(max_visible);
                    return Some(QuickSearchAction::Navigated);
                }
            }
        }
        crossterm::event::MouseEventKind::ScrollDown => {
            if over_dropdown && result_count > 0 {
                if popup.selected + 1 < result_count {
                    popup.selected += 1;
                    popup.scroll_to_selected(max_visible);
                    return Some(QuickSearchAction::Navigated);
                }
            }
        }
        crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
            if over_dropdown && result_count > 0 {
                let dropdown_rect = dropdown_area.unwrap();
                let visual_row = event.row.saturating_sub(dropdown_rect.y) as usize;
                let target_index = popup.scroll_offset + visual_row;
                if target_index < result_count {
                    popup.selected = target_index;
                    return Some(QuickSearchAction::Submit);
                }
            } else if !over_header && !over_dropdown {
                return Some(QuickSearchAction::Cancel);
            }
        }
        _ => {}
    }

    None
}
