use crate::app::{App, HelpTab};
use crate::keybinds::HelpAction;
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle_help_keys(app: &mut App, key: KeyEvent) {
    if let Some(ref mut popup) = app.help_search.popup {
        app.seq_matcher.clear();
        match crate::ui::quick_search::handle_quick_search_keys(popup, key, &app.keybinds) {
            crate::ui::quick_search::QuickSearchAction::Submit => {
                if let Some(&(idx, _)) = popup.results.get(popup.selected) {
                    let page_size = app.help_page_size.max(1) as usize;
                    app.help_page = (idx / page_size) as u16;
                    app.help_search.highlight_row = Some(idx);
                }
                app.help_search.popup = None;
            }
            crate::ui::quick_search::QuickSearchAction::Cancel => {
                app.help_search.popup = None;
            }
            crate::ui::quick_search::QuickSearchAction::Edited => {
                app.update_help_search();
            }
            _ => {}
        }
        return;
    }
    // Universal back/quit (override-proof): bare q/Esc closes help.
    if crate::events::is_universal_quit_key(&key) {
        app.close_help_page();
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
                let theme = &app.app_theme;
                app.help_search.popup =
                    Some(crate::ui::quick_search::QuickSearch::new("Search", theme));
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
            KeyCode::Char('8') => app.switch_help_tab(HelpTab::About),
            KeyCode::Char('n') => {
                let len = crate::ui::help_content::tab_popup_descriptions(app.help_tab).len();
                if len > 0 {
                    app.help_info_active = (app.help_info_active + 1) % len;
                }
            }
            KeyCode::Char('N') => {
                let len = crate::ui::help_content::tab_popup_descriptions(app.help_tab).len();
                if len > 0 {
                    app.help_info_active = (app.help_info_active + len - 1) % len;
                }
            }
            _ => {}
        },
    }
}
