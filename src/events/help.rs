use crate::app::{App, HelpTab};
use crate::keybinds::HelpAction;
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

pub fn handle_help_keys(app: &mut App, key: KeyEvent) {
    if let Some(ref mut popup) = app.help_search.popup {
        app.seq_matcher.clear();
        match crate::ui::quick_search::handle_quick_search_keys(popup, key, &app.keybinds, 10) {
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
            KeyCode::Char('7') => app.switch_help_tab(HelpTab::ContentTree),
            KeyCode::Char('8') => app.switch_help_tab(HelpTab::Setup),
            KeyCode::Char('9') => app.switch_help_tab(HelpTab::Templates),
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

pub fn handle_help_mouse(app: &mut App, mouse_event: MouseEvent, area: Rect) {
    if let Some(ref mut popup) = app.help_search.popup {
        if let Some(action) = crate::ui::quick_search::handle_quick_search_mouse(
            popup,
            mouse_event,
            area,
            10,
            app.config.ui.icon_mode,
        ) {
            match action {
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
                crate::ui::quick_search::QuickSearchAction::Navigated => {}
            }
        }
        return;
    }

    let tab_bar_y = area.y;
    if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) && mouse_event.row == tab_bar_y {
        let tabs: Vec<(&str, Option<&str>)> = crate::ui::help_tab_names()
            .iter()
            .map(|&l| (l, None))
            .collect();
        let region = crate::ui::title_bar_tabs_region(area, "Help");
        if let Some(i) = crate::ui::hit_test_tabs(
            &tabs,
            area.x,
            area.width,
            region.x,
            mouse_event.column,
            app.config.ui.tab_icons_only,
            app.config.ui.icon_mode,
        ) {
            app.switch_help_tab(crate::app::HelpTab::from_index(i));
        }
    } else if mouse_event.kind == MouseEventKind::ScrollUp {
        app.help_page = app.help_page.saturating_sub(1);
    } else if mouse_event.kind == MouseEventKind::ScrollDown {
        let page_size = app.help_page_size as usize;
        let total = if page_size > 0 {
            app.get_help_rows().len().div_ceil(page_size)
        } else {
            1
        };
        let max_page = total.saturating_sub(1) as u16;
        app.help_page = app.help_page.saturating_add(1).min(max_page);
    }
}
