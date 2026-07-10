use crate::app::{App, HelpTab};
use crate::keybinds::HelpAction;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn handle_help_keys(app: &mut App, key: KeyEvent) {
    if app.help_search.active {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            _ if crate::events::is_cancel_popup(&app.keybinds, &key, true) => {
                app.help_search = crate::app::HelpSearchState::default();
            }
            KeyCode::Enter => {
                if let Some(&(idx, _)) = app.help_search.results.get(app.help_search.selected) {
                    let page_size = app.help_page_size.max(1) as usize;
                    app.help_page = (idx / page_size) as u16;
                    app.help_search.highlight_row = Some(idx);
                }
                app.help_search.active = false;
                app.help_search.query.clear();
                app.help_search.results.clear();
                app.help_search.selected = 0;
                app.help_search.cursor = 0;
            }
            KeyCode::Up => {
                if app.help_search.selected > 0 {
                    app.help_search.selected -= 1;
                }
            }
            KeyCode::Down => {
                if !app.help_search.results.is_empty()
                    && app.help_search.selected < app.help_search.results.len() - 1
                {
                    app.help_search.selected += 1;
                }
            }
            KeyCode::BackTab | KeyCode::Tab if shift => {
                if !app.help_search.results.is_empty() {
                    app.help_search.selected = app
                        .help_search
                        .selected
                        .checked_sub(1)
                        .unwrap_or(app.help_search.results.len() - 1);
                }
            }
            KeyCode::Tab => {
                if !app.help_search.results.is_empty() {
                    app.help_search.selected =
                        (app.help_search.selected + 1) % app.help_search.results.len();
                }
            }
            KeyCode::Backspace => {
                if app.help_search.cursor > 0 {
                    let prev = app.help_search.query[..app.help_search.cursor]
                        .char_indices()
                        .last()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    app.help_search
                        .query
                        .replace_range(prev..app.help_search.cursor, "");
                    app.help_search.cursor = prev;
                    app.update_help_search();
                }
            }
            KeyCode::Delete => {
                if app.help_search.cursor < app.help_search.query.len() {
                    let next = app.help_search.query[app.help_search.cursor..]
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| app.help_search.cursor + i)
                        .unwrap_or(app.help_search.query.len());
                    app.help_search
                        .query
                        .replace_range(app.help_search.cursor..next, "");
                    app.update_help_search();
                }
            }
            KeyCode::Left => {
                if app.help_search.cursor > 0 {
                    app.help_search.cursor = app.help_search.query[..app.help_search.cursor]
                        .char_indices()
                        .last()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                }
            }
            KeyCode::Right => {
                if app.help_search.cursor < app.help_search.query.len() {
                    app.help_search.cursor = app.help_search.query[app.help_search.cursor..]
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| app.help_search.cursor + i)
                        .unwrap_or(app.help_search.query.len());
                }
            }
            KeyCode::Home => {
                app.help_search.cursor = 0;
            }
            KeyCode::End => {
                app.help_search.cursor = app.help_search.query.len();
            }
            KeyCode::Char('h') if ctrl => {
                if app.help_search.cursor > 0 {
                    let prev = app.help_search.query[..app.help_search.cursor]
                        .char_indices()
                        .last()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    app.help_search
                        .query
                        .replace_range(prev..app.help_search.cursor, "");
                    app.help_search.cursor = prev;
                    app.update_help_search();
                }
            }
            KeyCode::Char('w') if ctrl => {
                if app.help_search.cursor > 0 {
                    let prev = app.help_search.query[..app.help_search.cursor]
                        .char_indices()
                        .last()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    app.help_search
                        .query
                        .replace_range(prev..app.help_search.cursor, "");
                    app.help_search.cursor = prev;
                    app.update_help_search();
                }
            }
            KeyCode::Char('u') if ctrl => {
                app.help_search.query.clear();
                app.help_search.cursor = 0;
                app.update_help_search();
            }
            KeyCode::Char('a') if ctrl => {
                app.help_search.cursor = 0;
            }
            KeyCode::Char('e') if ctrl => {
                app.help_search.cursor = app.help_search.query.len();
            }
            KeyCode::Char(c) if !ctrl => {
                const MAX_SEARCH_LEN: usize = 256;
                if app.help_search.query.len() < MAX_SEARCH_LEN {
                    app.help_search.query.insert(app.help_search.cursor, c);
                    app.help_search.cursor += c.len_utf8();
                    app.update_help_search();
                }
            }
            _ => {}
        }
        return;
    }

    let seq = app.config.sequences_enabled();
    let counts = app.config.counts_enabled();
    match app
        .keybinds
        .resolve_help(&mut app.seq_matcher, key, seq, counts)
    {
        crate::keybinds::MatchOutcome::Matched(action, _count) => match action {
            HelpAction::Close => {
                app.close_help_page();
            }
            HelpAction::ScrollDown => {
                let page_size = app.help_page_size as usize;
                let total = if page_size > 0 {
                    app.get_help_rows().len().div_ceil(page_size)
                } else {
                    1
                };
                let max_page = total.saturating_sub(1) as u16;
                app.help_page = app.help_page.saturating_add(1).min(max_page);
            }
            HelpAction::ScrollUp => {
                app.help_page = app.help_page.saturating_sub(1);
            }
            HelpAction::NextTab => {
                app.switch_help_tab(app.help_tab.next());
            }
            HelpAction::PrevTab => {
                app.switch_help_tab(app.help_tab.prev());
            }
            HelpAction::Search => {
                app.help_search.active = true;
                app.help_search.query.clear();
                app.help_search.cursor = 0;
                app.help_search.results.clear();
                app.help_search.selected = 0;
                app.help_search.highlight_row = None;
            }
            HelpAction::Reroll => {
                app.reroll_help_suggestions();
            }
        },
        crate::keybinds::MatchOutcome::Pending => {}
        crate::keybinds::MatchOutcome::NoMatch => match key.code {
            KeyCode::Char('1') => app.switch_help_tab(HelpTab::Notes),
            KeyCode::Char('2') => app.switch_help_tab(HelpTab::Editor),
            KeyCode::Char('3') => app.switch_help_tab(HelpTab::Graph),
            KeyCode::Char('4') => app.switch_help_tab(HelpTab::Draw),
            KeyCode::Char('5') => app.switch_help_tab(HelpTab::Canvas),
            KeyCode::Char('6') => app.switch_help_tab(HelpTab::Backup),
            KeyCode::Char('7') => app.switch_help_tab(HelpTab::Templates),
            KeyCode::Char('8') => app.switch_help_tab(HelpTab::ContentTree),
            KeyCode::Char('9') => app.switch_help_tab(HelpTab::About),
            _ => {}
        },
    }
}
