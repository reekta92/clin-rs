use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::app::{App, HelpTab};
use crate::keybinds::HelpAction;

pub fn handle_help_keys(app: &mut App, key: KeyEvent) {
    // Ctrl+C → quit (interactive path, raw mode delivers Ctrl+C as key event)
    if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
        app.initiate_quit();
        return;
    }

    let seq = app.config.core.enable_key_sequences;
    match app.keybinds.resolve_help(&mut app.seq_matcher, key, seq) {
        crate::keybinds::MatchOutcome::Matched(action) => match action {
            HelpAction::Close => {
                app.close_help_page();
            }
            HelpAction::ScrollDown => {
                app.help_scroll = app.help_scroll.saturating_add(1);
            }
            HelpAction::ScrollUp => {
                app.help_scroll = app.help_scroll.saturating_sub(1);
            }
            HelpAction::NextTab => {
                app.switch_help_tab(app.help_tab.next());
            }
            HelpAction::PrevTab => {
                app.switch_help_tab(app.help_tab.prev());
            }
        },
        crate::keybinds::MatchOutcome::Pending => {}
        crate::keybinds::MatchOutcome::NoMatch => match key.code {
            KeyCode::Char('1') => app.switch_help_tab(HelpTab::Notes),
            KeyCode::Char('2') => app.switch_help_tab(HelpTab::Editor),
            KeyCode::Char('3') => app.switch_help_tab(HelpTab::Graph),
            KeyCode::Char('4') => app.switch_help_tab(HelpTab::Draw),
            KeyCode::Char('5') => app.switch_help_tab(HelpTab::Canvas),
            KeyCode::Char('6') => app.switch_help_tab(HelpTab::Templates),
            _ => {}
        },
    }
}
