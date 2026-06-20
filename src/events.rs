use crate::app::ContextMenu;
use crate::app::{App, EditFocus, HelpTab};
use crate::keybinds::*;
use crate::list_view::ListMode;
pub use crate::text_edit::apply_text_shortcuts;
use crossterm::event::*;
use ratatui::layout::Margin;
use ratatui::prelude::*;
use ratatui_textarea::*;

pub fn handle_popup_text_input(
    key: KeyEvent,
    input: &mut TextArea<'static>,
    keybinds: &Keybinds,
) -> bool {
    if !apply_text_shortcuts(keybinds, input, key) {
        input.input(Input::from(key));
    }
    true
}

pub fn handle_list_keys(app: &mut App, key: KeyEvent) -> bool {
    // Ctrl+C → quit (interactive path, raw mode delivers Ctrl+C as key event)
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.initiate_quit();
        return false;
    }

    if let Some(mut palette) = app.command_palette.take() {
        if palette.handle_input(key, app) {
            if key.code == KeyCode::Enter
                && let Some(selected_idx) = palette.state.selected()
                && let Some(item) = palette.items.get(selected_idx)
            {
                let action_id = item.id.clone();
                let note_id = palette.context_note_id.clone();
                if let Err(e) = crate::actions::execute_action(&action_id, app, note_id.as_deref())
                {
                    app.set_temporary_status(&format!("Action failed: {e}"));
                }
            }
            return false;
        }
        app.command_palette = Some(palette);
        return false;
    }

    let q_exit = key.code == KeyCode::Char('q') && !app.popups.has_text_input();
    if app.popups.has_any() && (key.code == KeyCode::Esc || q_exit) {
        app.popups.clear_all();
        return false;
    }

    if let Some((mut popup, format)) = app.popups.create_note.take() {
        if key.code == KeyCode::Esc {
            app.popups.create_note = None;
        } else if key.code == KeyCode::Enter {
            app.popups.create_note = Some((popup, format));
            app.confirm_create_note();
        } else {
            handle_popup_text_input(key, &mut popup.input, &app.keybinds);
            app.popups.create_note = Some((popup, format));
        }
        return false;
    }

    if let Some(mut popup) = app.popups.import.take() {
        if key.code == KeyCode::Esc {
            app.popups.import = None;
        } else if key.code == KeyCode::Enter {
            app.popups.import = Some(popup);
            app.confirm_import();
        } else {
            handle_popup_text_input(key, &mut popup.input, &app.keybinds);
            app.popups.import = Some(popup);
        }
        return false;
    }

    if let Some(mut popup) = app.popups.folder.take() {
        if key.code == KeyCode::Esc {
            app.popups.folder = None;
        } else if key.code == KeyCode::Enter {
            app.popups.folder = Some(popup);
            app.confirm_folder_popup();
        } else {
            handle_popup_text_input(key, &mut popup.input, &app.keybinds);
            app.popups.folder = Some(popup);
        }
        return false;
    }

    if let Some(mut popup) = app.popups.tag.take() {
        if app.popups.confirm.is_some() {
            app.popups.tag = Some(popup);
            let confirm_key = key;
            if confirm_key.code == KeyCode::Left || confirm_key.code == KeyCode::Char('h') {
                app.confirm_popup_select_confirm();
            } else if confirm_key.code == KeyCode::Right || confirm_key.code == KeyCode::Char('l') {
                app.confirm_popup_select_cancel();
            } else if confirm_key.code == KeyCode::Tab {
                app.confirm_popup_toggle_button();
            } else if confirm_key.code == KeyCode::Enter
                || confirm_key.code == KeyCode::Char('y')
                || confirm_key.code == KeyCode::Char('Y')
            {
                app.confirm_popup_activate();
            } else if confirm_key.code == KeyCode::Char('n')
                || confirm_key.code == KeyCode::Char('N')
                || confirm_key.code == KeyCode::Esc
            {
                app.cancel_confirm();
            }
            return false;
        }

        if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
            let tag_text = popup.input.lines().join("");
            let tag = tag_text
                .split(',')
                .next()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            if let Some(tag) = tag {
                app.list.tag_to_assign = Some(tag);
                app.popups.tag = None;
                app.list.list_mode = crate::list_view::ListMode::Select;
                app.list.selected_indices.clear();
                app.list.selected_indices.insert(app.list.visual_index);
                app.set_temporary_status_static(
                    "TAG MODE: Select notes to apply tag, Enter to confirm, Esc to cancel",
                );
            } else {
                app.popups.tag = None;
                app.set_temporary_status_static("Enter a tag name first");
            }
            return false;
        }

        match key.code {
            KeyCode::Esc => {
                app.popups.tag = None;
            }
            KeyCode::Tab => {
                if popup.focus == crate::popups::TagPopupFocus::Input {
                    if popup.suggestions.is_empty() {
                        popup.focus = crate::popups::TagPopupFocus::AllTagsList;
                    } else {
                        app.popups.tag = Some(popup);
                        app.accept_tag_suggestion();
                        return false;
                    }
                } else {
                    popup.focus = crate::popups::TagPopupFocus::Input;
                }
                app.popups.tag = Some(popup);
            }
            KeyCode::BackTab => {
                popup.focus = match popup.focus {
                    crate::popups::TagPopupFocus::Input => {
                        crate::popups::TagPopupFocus::AllTagsList
                    }
                    crate::popups::TagPopupFocus::AllTagsList => {
                        crate::popups::TagPopupFocus::Input
                    }
                };
                app.popups.tag = Some(popup);
            }
            _ => match popup.focus {
                crate::popups::TagPopupFocus::Input => {
                    if key.code == KeyCode::Enter {
                        app.popups.tag = Some(popup);
                        app.confirm_manage_tags();
                    } else if key.code == KeyCode::Char('D')
                        && key.modifiers.contains(KeyModifiers::SHIFT)
                    {
                        if let Some(tag) = popup.suggestions.get(popup.suggestion_index).cloned() {
                            app.begin_delete_tag_with_name(tag);
                        }
                    } else {
                        if !apply_text_shortcuts(&app.keybinds, &mut popup.input, key) {
                            popup.input.input(Input::from(key));
                        }
                        app.popups.tag = Some(popup);
                        app.update_tag_suggestions();
                    }
                }
                crate::popups::TagPopupFocus::AllTagsList => match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        popup.all_tags_selected = popup.all_tags_selected.saturating_sub(1);
                        app.popups.tag = Some(popup);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if popup.all_tags_selected + 1 < popup.all_tags.len() {
                            popup.all_tags_selected += 1;
                        }
                        app.popups.tag = Some(popup);
                    }
                    KeyCode::Char('d') | KeyCode::Delete => {
                        if let Some(tag) = popup.all_tags.get(popup.all_tags_selected).cloned() {
                            app.popups.tag = Some(popup);
                            app.begin_delete_tag_with_name(tag);
                        }
                    }
                    _ => {
                        app.popups.tag = Some(popup);
                    }
                },
            },
        }
        return false;
    }

    if let Some(mut popup) = app.popups.note_rename.take() {
        if key.code == KeyCode::Esc {
            app.popups.note_rename = None;
        } else if key.code == KeyCode::Enter {
            app.popups.note_rename = Some(popup);
            app.confirm_rename_note();
        } else {
            if !apply_text_shortcuts(&app.keybinds, &mut popup.input, key) {
                popup.input.input(Input::from(key));
            }
            app.popups.note_rename = Some(popup);
        }
        return false;
    }

    if let Some(mut popup) = app.popups.search.take() {
        let has_title = !popup.title_results.is_empty();
        let has_grep = !popup.grep_results.is_empty();
        let has_results = has_title || has_grep;

        let grep_prev_visible = |p: &crate::popups::SearchPopup, cur: usize| -> usize {
            if cur == 0 {
                return 0;
            }
            let mut i = cur - 1;
            loop {
                if p.grep_is_header[i] {
                    return i;
                }
                let mut parent = i;
                while parent > 0 && !p.grep_is_header[parent] {
                    parent -= 1;
                }
                if p.grep_expanded.contains(&parent) {
                    return i;
                }
                if i == 0 {
                    return 0;
                }
                i -= 1;
            }
        };
        let grep_next_visible = |p: &crate::popups::SearchPopup, cur: usize| -> usize {
            let mut i = cur + 1;
            while i < p.grep_results.len() {
                if p.grep_is_header[i] {
                    return i;
                }
                let mut parent = i;
                while parent > 0 && !p.grep_is_header[parent] {
                    parent -= 1;
                }
                if p.grep_expanded.contains(&parent) {
                    return i;
                }
                i += 1;
            }
            cur
        };

        match key.code {
            KeyCode::Esc => {
                app.popups.search = Some(popup);
                app.cancel_search();
            }
            KeyCode::Tab | KeyCode::BackTab => {
                popup.focus = match popup.focus {
                    crate::popups::SearchFocus::Input if has_results => {
                        crate::popups::SearchFocus::Results
                    }
                    _ => crate::popups::SearchFocus::Input,
                };
                app.popups.search = Some(popup);
            }
            KeyCode::Enter => {
                if popup.focus == crate::popups::SearchFocus::Results && has_results {
                    app.popups.search = Some(popup);
                    app.jump_to_selected_result();
                    app.confirm_search();
                } else {
                    app.popups.search = Some(popup);
                    app.confirm_search();
                }
            }
            KeyCode::Char('l') => {
                if popup.focus == crate::popups::SearchFocus::Input {
                    if !apply_text_shortcuts(&app.keybinds, &mut popup.input, key) {
                        popup.input.input(Input::from(key));
                    }
                    app.popups.search = Some(popup);
                    app.update_search();
                } else if has_grep
                    && popup
                        .grep_is_header
                        .get(popup.grep_selected)
                        .copied()
                        .unwrap_or(false)
                {
                    if popup.grep_expanded.contains(&popup.grep_selected) {
                        popup.grep_expanded.remove(&popup.grep_selected);
                    } else {
                        popup.grep_expanded.insert(popup.grep_selected);
                    }
                    app.popups.search = Some(popup);
                } else if has_results {
                    app.popups.search = Some(popup);
                    app.jump_to_selected_result();
                    app.confirm_search();
                } else {
                    app.popups.search = Some(popup);
                    app.update_search();
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if popup.focus == crate::popups::SearchFocus::Input {
                    if !apply_text_shortcuts(&app.keybinds, &mut popup.input, key) {
                        popup.input.input(Input::from(key));
                    }
                    app.popups.search = Some(popup);
                    app.update_search();
                } else if has_grep {
                    popup.grep_selected = grep_prev_visible(&popup, popup.grep_selected);
                    app.popups.search = Some(popup);
                } else if has_title {
                    popup.title_selected = popup.title_selected.saturating_sub(1);
                    app.popups.search = Some(popup);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if popup.focus == crate::popups::SearchFocus::Input {
                    if !apply_text_shortcuts(&app.keybinds, &mut popup.input, key) {
                        popup.input.input(Input::from(key));
                    }
                    app.popups.search = Some(popup);
                    app.update_search();
                } else if has_grep {
                    popup.grep_selected = grep_next_visible(&popup, popup.grep_selected);
                    app.popups.search = Some(popup);
                } else if has_title {
                    if popup.title_selected + 1 < popup.title_results.len() {
                        popup.title_selected += 1;
                    }
                    app.popups.search = Some(popup);
                }
            }
            KeyCode::Right | KeyCode::Char(' ') => {
                if popup.focus == crate::popups::SearchFocus::Input {
                    if !apply_text_shortcuts(&app.keybinds, &mut popup.input, key) {
                        popup.input.input(Input::from(key));
                    }
                    app.popups.search = Some(popup);
                    app.update_search();
                } else if has_grep
                    && popup
                        .grep_is_header
                        .get(popup.grep_selected)
                        .copied()
                        .unwrap_or(false)
                {
                    popup.grep_expanded.insert(popup.grep_selected);
                    app.popups.search = Some(popup);
                } else {
                    popup.focus = crate::popups::SearchFocus::Input;
                    if !apply_text_shortcuts(&app.keybinds, &mut popup.input, key) {
                        popup.input.input(Input::from(key));
                    }
                    app.popups.search = Some(popup);
                    app.update_search();
                }
            }
            KeyCode::Left => {
                if popup.focus == crate::popups::SearchFocus::Input {
                    if !apply_text_shortcuts(&app.keybinds, &mut popup.input, key) {
                        popup.input.input(Input::from(key));
                    }
                    app.popups.search = Some(popup);
                    app.update_search();
                } else if has_grep
                    && popup
                        .grep_is_header
                        .get(popup.grep_selected)
                        .copied()
                        .unwrap_or(false)
                {
                    popup.grep_expanded.remove(&popup.grep_selected);
                    app.popups.search = Some(popup);
                } else {
                    popup.focus = crate::popups::SearchFocus::Input;
                    if !apply_text_shortcuts(&app.keybinds, &mut popup.input, key) {
                        popup.input.input(Input::from(key));
                    }
                    app.popups.search = Some(popup);
                    app.update_search();
                }
            }
            _ => {
                popup.focus = crate::popups::SearchFocus::Input;
                if !apply_text_shortcuts(&app.keybinds, &mut popup.input, key) {
                    popup.input.input(Input::from(key));
                }
                app.popups.search = Some(popup);
                app.update_search();
            }
        }
        return false;
    }

    if app.popups.confirm.is_some() {
        if key.code == KeyCode::Left || key.code == KeyCode::Char('h') {
            app.confirm_popup_select_confirm();
        } else if key.code == KeyCode::Right || key.code == KeyCode::Char('l') {
            app.confirm_popup_select_cancel();
        } else if key.code == KeyCode::Tab {
            app.confirm_popup_toggle_button();
        } else if key.code == KeyCode::Enter {
            app.confirm_popup_activate();
        } else if key.code == KeyCode::Esc {
            app.cancel_confirm();
        } else if app.keybinds.matches_list(ListAction::Confirm, &key) {
            app.confirm_action();
        } else if app.keybinds.matches_list(ListAction::Cancel, &key) {
            app.cancel_confirm();
        }
        return false;
    }

    if let Some(ref mut trash) = app.popups.trash_view {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                trash.selected = trash.selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if trash.selected + 1 < trash.items.len() {
                    trash.selected += 1;
                }
            }
            KeyCode::Char('r') | KeyCode::Enter => {
                app.restore_from_trash();
            }
            KeyCode::Char('d') | KeyCode::Delete => {
                app.begin_delete_from_trash();
            }
            KeyCode::Char('E') => {
                app.begin_empty_trash();
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                app.close_trash_view();
            }
            _ => {}
        }
        return false;
    }

    if let Some(mut picker) = app.popups.folder_picker.take() {
        match key.code {
            KeyCode::Tab => {
                picker.focus = match picker.focus {
                    crate::app::FolderPickerFocus::Search => crate::app::FolderPickerFocus::Results,
                    crate::app::FolderPickerFocus::Results => crate::app::FolderPickerFocus::Search,
                };
                app.popups.folder_picker = Some(picker);
            }
            KeyCode::Esc => {
                app.popups.folder_picker = None;
            }
            _ => match picker.focus {
                crate::app::FolderPickerFocus::Results => match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        picker.selected = picker.selected.saturating_sub(1);
                        app.popups.folder_picker = Some(picker);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if picker.selected + 1 < picker.filtered_folders.len() {
                            picker.selected += 1;
                        }
                        app.popups.folder_picker = Some(picker);
                    }
                    KeyCode::Enter | KeyCode::Char('l') => {
                        app.popups.folder_picker = Some(picker);
                        app.confirm_move();
                    }
                    _ => {
                        app.popups.folder_picker = Some(picker);
                    }
                },
                crate::app::FolderPickerFocus::Search => {
                    let old_query = picker.input.lines().join("");
                    if !apply_text_shortcuts(&app.keybinds, &mut picker.input, key) {
                        picker.input.input(Input::from(key));
                    }
                    let new_query = picker.input.lines().join("");
                    if old_query != new_query {
                        app.popups.folder_picker = Some(picker);
                        app.update_folder_picker_filter();
                    } else if key.code == KeyCode::Enter {
                        picker.focus = crate::app::FolderPickerFocus::Results;
                        app.popups.folder_picker = Some(picker);
                    } else {
                        app.popups.folder_picker = Some(picker);
                    }
                }
            },
        }
        return false;
    }

    if let Some(mut popup) = app.popups.template.take() {
        match key.code {
            KeyCode::Tab | KeyCode::BackTab => {
                popup.focus = match popup.focus {
                    crate::popups::TemplatePopupFocus::Search => {
                        crate::popups::TemplatePopupFocus::Results
                    }
                    crate::popups::TemplatePopupFocus::Results => {
                        crate::popups::TemplatePopupFocus::Search
                    }
                };
                app.popups.template = Some(popup);
            }
            KeyCode::Char('?') => {
                if popup.focus == crate::popups::TemplatePopupFocus::Results {
                    app.popups.template = Some(popup);
                    app.open_help_page_with_tab(HelpTab::Templates);
                } else {
                    if !apply_text_shortcuts(&app.keybinds, &mut popup.input, key) {
                        popup.input.input(Input::from(key));
                    }
                    app.popups.template = Some(popup);
                    app.update_template_popup_filter();
                }
            }
            KeyCode::Char('n') => {
                if popup.focus == crate::popups::TemplatePopupFocus::Results {
                    app.popups.template = Some(popup);
                    app.create_template_from_popup();
                } else {
                    if !apply_text_shortcuts(&app.keybinds, &mut popup.input, key) {
                        popup.input.input(Input::from(key));
                    }
                    app.popups.template = Some(popup);
                    app.update_template_popup_filter();
                }
            }
            _ if app.keybinds.matches_list(ListAction::Cancel, &key) => {
                app.close_template_popup();
            }
            _ => match popup.focus {
                crate::popups::TemplatePopupFocus::Results => match key.code {
                    _ if app.keybinds.matches_list(ListAction::MoveUp, &key) => {
                        popup.selected = popup.selected.saturating_sub(1);
                        app.popups.template = Some(popup);
                    }
                    _ if app.keybinds.matches_list(ListAction::MoveDown, &key) => {
                        if popup.selected + 1 < popup.filtered_templates.len() {
                            popup.selected += 1;
                        }
                        app.popups.template = Some(popup);
                    }
                    _ if app.keybinds.matches_list(ListAction::Confirm, &key)
                        || app.keybinds.matches_list(ListAction::Open, &key) =>
                    {
                        app.popups.template = Some(popup);
                        app.select_template();
                    }
                    KeyCode::Char(' ') => {
                        app.popups.template = Some(popup);
                        app.edit_selected_template_from_popup();
                    }
                    KeyCode::Char('d') => {
                        app.popups.template = Some(popup);
                        app.begin_delete_selected_template_from_popup();
                    }
                    KeyCode::Char('h') => {
                        app.close_template_popup();
                    }
                    _ => {
                        app.popups.template = Some(popup);
                    }
                },
                crate::popups::TemplatePopupFocus::Search => match key.code {
                    _ if key.code == KeyCode::Enter => {
                        popup.focus = crate::popups::TemplatePopupFocus::Results;
                        app.popups.template = Some(popup);
                    }
                    _ => {
                        if !apply_text_shortcuts(&app.keybinds, &mut popup.input, key) {
                            popup.input.input(Input::from(key));
                        }
                        app.popups.template = Some(popup);
                        app.update_template_popup_filter();
                    }
                },
            },
        }
        return false;
    }

    if let Some(mut popup) = app.popups.theme.take() {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                match popup.focus {
                    crate::app::ThemePopupFocus::ThemeList => {
                        popup.selected = popup.selected.saturating_sub(1);
                        app.popups.theme = Some(popup);
                        app.select_theme();
                        return false;
                    }
                    crate::app::ThemePopupFocus::GeneralBg => {
                        popup.focus = crate::app::ThemePopupFocus::ThemeList;
                        popup.selected = popup.themes.len().saturating_sub(1);
                    }
                    crate::app::ThemePopupFocus::GraphBg => {
                        popup.focus = crate::app::ThemePopupFocus::GeneralBg;
                    }
                }
                app.popups.theme = Some(popup);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                match popup.focus {
                    crate::app::ThemePopupFocus::ThemeList => {
                        if popup.selected + 1 < popup.themes.len() {
                            popup.selected += 1;
                            app.popups.theme = Some(popup);
                            app.select_theme();
                            return false;
                        } else {
                            popup.focus = crate::app::ThemePopupFocus::GeneralBg;
                        }
                    }
                    crate::app::ThemePopupFocus::GeneralBg => {
                        popup.focus = crate::app::ThemePopupFocus::GraphBg;
                    }
                    crate::app::ThemePopupFocus::GraphBg => {
                        popup.focus = crate::app::ThemePopupFocus::ThemeList;
                        popup.selected = 0;
                    }
                }
                app.popups.theme = Some(popup);
            }
            KeyCode::Tab => {
                match popup.focus {
                    crate::app::ThemePopupFocus::ThemeList => {
                        popup.focus = crate::app::ThemePopupFocus::GeneralBg
                    }
                    crate::app::ThemePopupFocus::GeneralBg => {
                        popup.focus = crate::app::ThemePopupFocus::GraphBg
                    }
                    crate::app::ThemePopupFocus::GraphBg => {
                        popup.focus = crate::app::ThemePopupFocus::ThemeList
                    }
                }
                app.popups.theme = Some(popup);
            }
            _ if app.keybinds.matches_list(ListAction::Confirm, &key) => {
                let is_list = matches!(popup.focus, crate::app::ThemePopupFocus::ThemeList);
                app.popups.theme = Some(popup);
                app.select_theme();
                if is_list {
                    app.close_theme_popup();
                }
            }
            KeyCode::Char('l') | KeyCode::Char(' ') => {
                app.popups.theme = Some(popup);
                app.select_theme();
            }
            _ if app.keybinds.matches_list(ListAction::Cancel, &key) => {
                app.close_theme_popup();
            }
            _ => {
                app.popups.theme = Some(popup);
            }
        }
        return false;
    }

    if let Some(mut popup) = app.popups.sort.take() {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                popup.selected = popup.selected.saturating_sub(1);
                app.popups.sort = Some(popup);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if popup.selected < 3 {
                    popup.selected += 1;
                }
                app.popups.sort = Some(popup);
            }
            _ if app.keybinds.matches_list(ListAction::Confirm, &key) => {
                app.popups.sort = Some(popup);
                app.select_sort();
            }
            _ if app.keybinds.matches_list(ListAction::Cancel, &key) => {
                app.close_sort_popup();
            }
            _ => {
                app.popups.sort = Some(popup);
            }
        }
        return false;
    }

    if let Some(mut popup) = app.popups.create_format.take() {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                popup.selected = popup.selected.saturating_sub(1);
                app.popups.create_format = Some(popup);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if popup.selected < 3 {
                    popup.selected += 1;
                }
                app.popups.create_format = Some(popup);
            }
            _ if app.keybinds.matches_list(ListAction::Confirm, &key) => {
                app.popups.create_format = Some(popup);
                app.confirm_create_format();
            }
            _ if app.keybinds.matches_list(ListAction::Cancel, &key)
                || key.code == KeyCode::Esc =>
            {
                app.close_create_format_popup();
            }
            _ => {
                app.popups.create_format = Some(popup);
            }
        }
        return false;
    }

    if key.code == KeyCode::Esc {
        app.handle_esc_press();
    }

    if app.keybinds.matches_list(ListAction::CycleFocus, &key) {
        if app.list.notes_layout == crate::config::NotesLayout::Grid {
            app.cycle_grid_tab();
        }
        return false;
    }

    if app.keybinds.matches_list(ListAction::Quit, &key) && app.list.list_mode != ListMode::Select {
        app.initiate_quit();
        return false;
    }

    if app
        .keybinds
        .matches_list(ListAction::ToggleExternalEditor, &key)
    {
        app.toggle_external_editor_mode();
        return false;
    }

    if app
        .keybinds
        .matches_list(ListAction::ToggleSelectMode, &key)
    {
        if app.list.tag_to_assign.is_some() {
            return false;
        }
        app.list.list_mode = match app.list.list_mode {
            ListMode::Normal => {
                app.list.selected_indices.clear();
                app.list.selected_indices.insert(app.list.visual_index);
                ListMode::Select
            }
            ListMode::Select => {
                app.list.selected_indices.clear();
                ListMode::Normal
            }
        };
        return false;
    }

    if app.list.list_mode == ListMode::Select {
        if app
            .keybinds
            .matches_list(ListAction::ToggleSelectItem, &key)
        {
            if app.list.selected_indices.contains(&app.list.visual_index) {
                app.list.selected_indices.remove(&app.list.visual_index);
            } else {
                app.list.selected_indices.insert(app.list.visual_index);
            }
            return false;
        }
        if key.code == KeyCode::Esc || key.code == KeyCode::Char('q') {
            app.list.tag_to_assign = None;
            app.list.list_mode = ListMode::Normal;
            app.list.selected_indices.clear();
            return false;
        }
        if key.code == KeyCode::Enter {
            if let Some(tag) = app.list.tag_to_assign.take() {
                app.apply_tag_to_selected(tag);
            }
            return false;
        }
    }

    if key
        .modifiers
        .contains(crossterm::event::KeyModifiers::CONTROL)
    {
        match key.code {
            KeyCode::Char('h') => match &mut app.list.preview_content {
                Some(crate::list_view::PreviewContent::Markdown(renderer)) => {
                    renderer.prev_page();
                    return false;
                }
                Some(
                    crate::list_view::PreviewContent::CanvasGrid(_)
                    | crate::list_view::PreviewContent::DrawGrid(_),
                ) => {
                    app.list.snapshot_scroll_offset =
                        app.list.snapshot_scroll_offset.saturating_sub(3);
                    return false;
                }
                _ => {}
            },
            KeyCode::Char('l') => match &mut app.list.preview_content {
                Some(crate::list_view::PreviewContent::Markdown(renderer)) => {
                    renderer.next_page();
                    return false;
                }
                Some(
                    crate::list_view::PreviewContent::CanvasGrid(_)
                    | crate::list_view::PreviewContent::DrawGrid(_),
                ) => {
                    app.list.snapshot_scroll_offset =
                        app.list.snapshot_scroll_offset.saturating_add(3);
                    return false;
                }
                _ => {}
            },
            _ => {}
        }
    }

    if app.keybinds.matches_list(ListAction::Help, &key) {
        app.open_help_page();
        return false;
    }
    if app.keybinds.matches_list(ListAction::OpenLocation, &key) {
        app.open_selected_note_location();
        return false;
    }
    if app.keybinds.matches_list(ListAction::Delete, &key) {
        app.begin_delete_selected();
        return false;
    }
    let is_grid = app.list.notes_layout == crate::config::NotesLayout::Grid;
    let cols = if is_grid {
        app.list.grid_columns.max(1)
    } else {
        1
    };
    let len = app.list.visual_list.len();

    if app.keybinds.matches_list(ListAction::MoveLeft, &key) {
        if is_grid {
            app.list.visual_index = app.list.visual_index.saturating_sub(1);
            app.request_preview_update();
        } else {
            app.collapse_selected_folder();
        }
        return false;
    }
    if app.keybinds.matches_list(ListAction::MoveRight, &key) {
        if is_grid {
            if len > 0 {
                app.list.visual_index = (app.list.visual_index + 1).min(len - 1);
            }
            app.request_preview_update();
        } else {
            app.expand_selected_folder();
        }
        return false;
    }
    if app.keybinds.matches_list(ListAction::MoveDown, &key) {
        if is_grid {
            let next = app.list.visual_index + cols;
            if next < len {
                app.list.visual_index = next;
            } else if app.list.visual_index / cols < (len.saturating_sub(1)) / cols {
                app.list.visual_index = len.saturating_sub(1);
            }
            app.request_preview_update();
        } else if app.list.visual_index < len.saturating_sub(1) {
            app.list.visual_index += 1;
            app.request_preview_update();
        }
        if app.list.list_mode == crate::list_view::ListMode::Select {
            app.list.last_selection_change = Some(std::time::Instant::now());
        }
        return false;
    }
    if app.keybinds.matches_list(ListAction::MoveUp, &key) {
        if is_grid {
            if app.list.visual_index >= cols {
                app.list.visual_index -= cols;
            }
            app.request_preview_update();
        } else if app.list.visual_index > 0 {
            app.list.visual_index -= 1;
            app.request_preview_update();
        }
        if app.list.list_mode == crate::list_view::ListMode::Select {
            app.list.last_selection_change = Some(std::time::Instant::now());
        }
        return false;
    }
    if app.keybinds.matches_list(ListAction::CollapseFolder, &key) {
        app.collapse_selected_folder();
        return false;
    }
    if app.keybinds.matches_list(ListAction::ExpandFolder, &key) {
        app.expand_selected_folder();
        return false;
    }
    if app.keybinds.matches_list(ListAction::Open, &key) {
        app.open_selected();
        return false;
    }
    if app.keybinds.matches_list(ListAction::NewFromTemplate, &key) {
        app.open_template_popup();
        return false;
    }
    if app.keybinds.matches_list(ListAction::CreateFolder, &key) {
        app.begin_create_folder();
        return false;
    }
    if app.keybinds.matches_list(ListAction::CreateNote, &key) {
        app.begin_create_select_format();
        return false;
    }
    if app.keybinds.matches_list(ListAction::RenameFolder, &key)
        || app.keybinds.matches_list(ListAction::Rename, &key)
    {
        if let Some(item) = app.list.visual_list.get(app.list.visual_index) {
            match item {
                crate::app::VisualItem::Folder { .. } => app.begin_rename_folder(),
                crate::app::VisualItem::Note { .. } => app.begin_rename_note(),
                _ => app.set_temporary_status_static("Select a note or folder to rename"),
            }
        }
        return false;
    }
    if app.keybinds.matches_list(ListAction::MoveNote, &key) {
        app.begin_move();
        return false;
    }
    if app.keybinds.matches_list(ListAction::ManageTags, &key) {
        app.begin_manage_tags();
        return false;
    }
    if app
        .keybinds
        .matches_list(ListAction::OpenCommandPalette, &key)
    {
        if let Some(crate::app::VisualItem::Note { summary_idx, .. }) =
            app.list.visual_list.get(app.list.visual_index)
        {
            let id = app.notes[*summary_idx].id.clone();
            app.command_palette = Some(crate::palette::CommandPalette::new(Some(id), app));
        } else {
            app.command_palette = Some(crate::palette::CommandPalette::new(None, app));
        }
        return false;
    }

    if app.keybinds.matches_list(ListAction::Duplicate, &key) {
        app.duplicate_note();
        return false;
    }
    if app.keybinds.matches_list(ListAction::TogglePin, &key) {
        app.toggle_pin();
        return false;
    }
    if app.keybinds.matches_list(ListAction::CycleSort, &key) {
        app.cycle_sort();
        return false;
    }
    if app.keybinds.matches_list(ListAction::Search, &key) {
        app.begin_search();
        return false;
    }
    if app.keybinds.matches_list(ListAction::JumpToTop, &key) {
        app.jump_to_bottom();
        return false;
    }
    if app.keybinds.matches_list(ListAction::PageUp, &key) {
        app.page_up();
        return false;
    }
    if app.keybinds.matches_list(ListAction::PageDown, &key) {
        app.page_down();
        return false;
    }
    if app.keybinds.matches_list(ListAction::OpenTrash, &key) {
        app.open_trash_view();
        return false;
    }
    if app.keybinds.matches_list(ListAction::TogglePreview, &key) {
        app.toggle_preview();
        return false;
    }
    if app.keybinds.matches_list(ListAction::OpenGraph, &key) {
        app.open_graph_view();
        return false;
    }
    if app.keybinds.matches_list(ListAction::OpenCanvas, &key) {
        app.open_draw_view();
        return false;
    }

    if key.code == KeyCode::Char('g') && app.handle_g_press() {
        return false;
    }

    false
}

pub fn handle_help_keys(app: &mut App, key: KeyEvent) {
    // Ctrl+C → quit (interactive path, raw mode delivers Ctrl+C as key event)
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.initiate_quit();
        return;
    }

    if app.keybinds.matches_help(HelpAction::Close, &key) {
        app.close_help_page();
    } else if app.keybinds.matches_help(HelpAction::ScrollDown, &key) {
        app.help_scroll = app.help_scroll.saturating_add(1);
    } else if app.keybinds.matches_help(HelpAction::ScrollUp, &key) {
        app.help_scroll = app.help_scroll.saturating_sub(1);
    } else if app.keybinds.matches_help(HelpAction::NextTab, &key) {
        app.switch_help_tab(app.help_tab.next());
    } else if app.keybinds.matches_help(HelpAction::PrevTab, &key) {
        app.switch_help_tab(app.help_tab.prev());
    } else {
        match key.code {
            KeyCode::Char('1') => app.switch_help_tab(HelpTab::Notes),
            KeyCode::Char('2') => app.switch_help_tab(HelpTab::Editor),
            KeyCode::Char('3') => app.switch_help_tab(HelpTab::Graph),
            KeyCode::Char('4') => app.switch_help_tab(HelpTab::Draw),
            KeyCode::Char('5') => app.switch_help_tab(HelpTab::Canvas),
            KeyCode::Char('6') => app.switch_help_tab(HelpTab::Templates),
            _ => {}
        }
    }
}

pub fn handle_edit_keys(app: &mut App, key: KeyEvent, focus: &mut EditFocus) -> bool {
    // Ctrl+C → quit (interactive path, raw mode delivers Ctrl+C as key event)
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.initiate_quit();
        return false;
    }

    if let Some(mut menu) = app.popups.context_menu.take() {
        match key.code {
            KeyCode::Up => {
                menu.selected = menu.selected.saturating_sub(1);
                app.popups.context_menu = Some(menu);
            }
            KeyCode::Down => {
                if menu.selected < 3 {
                    menu.selected += 1;
                }
                app.popups.context_menu = Some(menu);
            }
            KeyCode::Enter => {
                app.handle_menu_action(menu.selected, focus);
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                app.popups.context_menu = None;
            }
            _ => {
                app.popups.context_menu = Some(menu);
            }
        }
        return false;
    }

    if key
        .modifiers
        .contains(crossterm::event::KeyModifiers::CONTROL)
    {
        match key.code {
            KeyCode::Char('h') => {
                if let Some(renderer) = &mut app.editor.md_preview_renderer {
                    renderer.prev_page();
                    return false;
                }
            }
            KeyCode::Char('l') => {
                if let Some(renderer) = &mut app.editor.md_preview_renderer {
                    renderer.next_page();
                    return false;
                }
            }
            _ => {}
        }
    }

    if app.keybinds.matches_edit(EditAction::CycleFocus, &key) {
        *focus = match *focus {
            EditFocus::Title => EditFocus::Body,
            EditFocus::Body => EditFocus::Title,
        };
        return false;
    }

    if app.keybinds.matches_edit(EditAction::Back, &key) {
        app.autosave();
        app.back_to_list();
        *focus = EditFocus::Body;
        return false;
    }

    if app
        .keybinds
        .matches_edit(EditAction::ToggleMarkdownPreview, &key)
    {
        app.toggle_markdown_preview();
        return false;
    }

    match *focus {
        EditFocus::Title => {
            if key.code == KeyCode::Enter {
                *focus = EditFocus::Body;
                return false;
            }
            if apply_text_shortcuts(&app.keybinds, &mut app.editor.title_editor, key) {
                app.request_editor_preview_update();
                return false;
            }

            if app.editor.title_editor.input(Input::from(key))
                && app.editor.title_editor.lines().len() > 1
            {
                let normalized =
                    get_title_text(&app.editor.title_editor).replace(['\r', '\n'], " ");
                app.editor.title_editor = make_title_editor(
                    &normalized,
                    app.app_theme.highlight_fg,
                    app.app_theme.highlight_bg,
                );
            }
            app.request_editor_preview_update();
        }
        EditFocus::Body => {
            if apply_text_shortcuts(&app.keybinds, &mut app.editor.editor, key) {
                app.request_editor_preview_update();
                return false;
            }
            if app.editor.editor.input(Input::from(key)) {
                app.request_editor_preview_update();
            }
        }
    }

    false
}

pub fn handle_list_mouse(app: &mut App, mouse_event: MouseEvent, terminal_area: Rect) {
    if app.popups.confirm.is_some() {
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
            let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Confirm, terminal_area);
            let click_x = mouse_event.column;
            let click_y = mouse_event.row;

            if click_x >= popup_area.x
                && click_x < popup_area.x + popup_area.width
                && click_y >= popup_area.y
                && click_y < popup_area.y + popup_area.height
            {
                let mid_x = popup_area.x + popup_area.width / 2;
                if click_x < mid_x {
                    app.confirm_action();
                } else {
                    app.cancel_confirm();
                }
            } else {
                app.cancel_confirm();
            }
        }
        return;
    }
    if let Some(mut popup) = app.popups.note_rename.take() {
        let area = crate::ui::centered_rect(crate::ui::PopupSize::Prompt, terminal_area);
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && !contains_cell(area, mouse_event.column, mouse_event.row)
        {
            return;
        }
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
            let inner = area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            });
            move_textarea_cursor_to_mouse(
                &mut popup.input,
                inner,
                mouse_event.column,
                mouse_event.row,
            );
        }
        app.popups.note_rename = Some(popup);
        return;
    }

    if let Some((mut popup, format)) = app.popups.create_note.take() {
        let area = crate::ui::centered_rect(crate::ui::PopupSize::Prompt, terminal_area);
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && !contains_cell(area, mouse_event.column, mouse_event.row)
        {
            return;
        }
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
            let inner = area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            });
            move_textarea_cursor_to_mouse(
                &mut popup.input,
                inner,
                mouse_event.column,
                mouse_event.row,
            );
        }
        app.popups.create_note = Some((popup, format));
        return;
    }

    if let Some(mut popup) = app.popups.import.take() {
        let area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && !contains_cell(area, mouse_event.column, mouse_event.row)
        {
            return;
        }
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
            let inner = area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            });
            move_textarea_cursor_to_mouse(
                &mut popup.input,
                inner,
                mouse_event.column,
                mouse_event.row,
            );
        }
        app.popups.import = Some(popup);
        return;
    }

    if let Some(mut popup) = app.popups.folder.take() {
        let area = crate::ui::centered_rect(crate::ui::PopupSize::Prompt, terminal_area);
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && !contains_cell(area, mouse_event.column, mouse_event.row)
        {
            return;
        }
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
            let inner = area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            });
            move_textarea_cursor_to_mouse(
                &mut popup.input,
                inner,
                mouse_event.column,
                mouse_event.row,
            );
        }
        app.popups.folder = Some(popup);
        return;
    }

    if let Some(mut popup) = app.popups.tag.take() {
        let suggestion_height = if popup.suggestions.is_empty() {
            0
        } else {
            (popup.suggestions.len() as u16).clamp(1, 5)
        };
        let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && !contains_cell(popup_area, mouse_event.column, mouse_event.row)
        {
            return;
        }
        let content = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(popup_area)[0];

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3 + suggestion_height),
                Constraint::Min(3),
            ])
            .split(content);

        let input_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(chunks[0]);

        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
            if contains_cell(chunks[1], mouse_event.column, mouse_event.row) {
                if !popup.all_tags.is_empty() {
                    let row = mouse_event
                        .row
                        .saturating_sub(chunks[1].y)
                        .saturating_sub(1) as usize;
                    popup.all_tags_selected = row.min(popup.all_tags.len() - 1);
                    popup.focus = crate::popups::TagPopupFocus::AllTagsList;
                }
            } else if !popup.suggestions.is_empty()
                && contains_cell(input_chunks[1], mouse_event.column, mouse_event.row)
            {
                let row = mouse_event.row.saturating_sub(input_chunks[1].y) as usize;
                popup.suggestion_index = row.min(popup.suggestions.len() - 1);
                app.popups.tag = Some(popup);
                app.accept_tag_suggestion();
                return;
            } else if contains_cell(input_chunks[0], mouse_event.column, mouse_event.row) {
                popup.focus = crate::popups::TagPopupFocus::Input;
                move_textarea_cursor_to_mouse(
                    &mut popup.input,
                    input_chunks[0],
                    mouse_event.column,
                    mouse_event.row,
                );
            }
        }
        app.popups.tag = Some(popup);
        return;
    }

    if let Some(mut popup) = app.popups.theme.take() {
        let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Medium, terminal_area);
        let content = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(popup_area)[0];

        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && !contains_cell(popup_area, mouse_event.column, mouse_event.row)
        {
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(3),
                Constraint::Length(3),
            ])
            .split(content);

        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
            if contains_cell(chunks[0], mouse_event.column, mouse_event.row) {
                let row = mouse_event
                    .row
                    .saturating_sub(chunks[0].y)
                    .saturating_sub(1) as usize;
                if !popup.themes.is_empty() {
                    let clicked = row.min(popup.themes.len() - 1);
                    let was_selected = popup.selected == clicked
                        && matches!(popup.focus, crate::app::ThemePopupFocus::ThemeList);
                    popup.selected = clicked;
                    popup.focus = crate::app::ThemePopupFocus::ThemeList;
                    app.popups.theme = Some(popup);
                    app.select_theme();
                    if was_selected {
                        app.close_theme_popup();
                    }
                    return;
                }
            } else if contains_cell(chunks[1], mouse_event.column, mouse_event.row) {
                popup.focus = crate::app::ThemePopupFocus::GeneralBg;
                app.popups.theme = Some(popup);
                app.select_theme();
                return;
            } else if contains_cell(chunks[2], mouse_event.column, mouse_event.row) {
                popup.focus = crate::app::ThemePopupFocus::GraphBg;
                app.popups.theme = Some(popup);
                app.select_theme();
                return;
            }
        }
        app.popups.theme = Some(popup);
        return;
    }

    if let Some(mut palette) = app.command_palette.take() {
        let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
        let content = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(popup_area)[0];

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // search input
                Constraint::Length(1), // tab bar
                Constraint::Min(0),    // results list
            ])
            .split(content);
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && !contains_cell(popup_area, mouse_event.column, mouse_event.row)
        {
            return;
        }
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
            if contains_cell(chunks[0], mouse_event.column, mouse_event.row) {
                let inner = chunks[0].inner(Margin {
                    vertical: 1,
                    horizontal: 1,
                });
                move_textarea_cursor_to_mouse(
                    &mut palette.input,
                    inner,
                    mouse_event.column,
                    mouse_event.row,
                );
            } else if mouse_event.row == chunks[1].y {
                let tabs: Vec<(&str, Option<&str>)> = crate::palette::PALETTE_TABS
                    .iter()
                    .map(|(l, g, _)| (*l, Some(*g)))
                    .collect();
                if let Some(i) = crate::ui::hit_test_tabs(
                    &tabs,
                    chunks[1].x,
                    chunks[1].width,
                    mouse_event.column,
                ) {
                    palette.active_tab = i;
                    palette.refresh_items(app);
                    palette.state.select(Some(0));
                }
            } else if contains_cell(chunks[2], mouse_event.column, mouse_event.row) {
                let row = mouse_event
                    .row
                    .saturating_sub(chunks[2].y)
                    .saturating_sub(1) as usize;
                // Palette items are 2 lines each (title + description).
                // Add the list's scroll offset so clicking after a scroll
                // hits the correct item.
                let scroll_offset = palette.state.offset();
                let clicked = scroll_offset + row / 2;
                if clicked < palette.items.len() {
                    if Some(clicked) == palette.state.selected() {
                        let item = &palette.items[clicked];
                        let action_id = item.id.clone();
                        let note_id = palette.context_note_id.clone();
                        if let Err(e) =
                            crate::actions::execute_action(&action_id, app, note_id.as_deref())
                        {
                            app.set_temporary_status(&format!("Action failed: {}", e));
                        }
                        app.command_palette = None;
                        return;
                    } else {
                        palette.state.select(Some(clicked));
                    }
                }
            }
        } else if mouse_event.kind == MouseEventKind::ScrollUp {
            if contains_cell(popup_area, mouse_event.column, mouse_event.row)
                && !palette.items.is_empty()
            {
                let current = palette.state.selected().unwrap_or(0);
                palette.state.select(Some(current.saturating_sub(1)));
            }
        } else if mouse_event.kind == MouseEventKind::ScrollDown
            && contains_cell(popup_area, mouse_event.column, mouse_event.row)
            && !palette.items.is_empty()
        {
            let current = palette.state.selected().unwrap_or(0);
            let next = (current + 1).min(palette.items.len().saturating_sub(1));
            palette.state.select(Some(next));
        }
        app.command_palette = Some(palette);
        return;
    }

    if app.popups.template.is_some() {
        let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && !contains_cell(popup_area, mouse_event.column, mouse_event.row)
        {
            app.popups.template = None;
            return;
        }
    }

    if let Some(popup) = &mut app.popups.template {
        let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(popup_area);

        let mut open_selected = false;
        let mut edit_selected = false;

        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && contains_cell(chunks[0], mouse_event.column, mouse_event.row)
        {
            popup.focus = crate::app::TemplatePopupFocus::Search;
        } else if contains_cell(chunks[1], mouse_event.column, mouse_event.row)
            && (mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
                || mouse_event.kind == MouseEventKind::Down(MouseButton::Right))
        {
            popup.focus = crate::app::TemplatePopupFocus::Results;
            if !popup.filtered_templates.is_empty() {
                let row = mouse_event
                    .row
                    .saturating_sub(chunks[1].y.saturating_add(1))
                    as usize;
                let clicked = row.min(popup.filtered_templates.len().saturating_sub(1));
                if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
                    && clicked == popup.selected
                {
                    open_selected = true;
                }
                popup.selected = clicked;
                if mouse_event.kind == MouseEventKind::Down(MouseButton::Right) {
                    edit_selected = true;
                }
            }
        }
        if edit_selected {
            app.edit_selected_template_from_popup();
            return;
        }
        if open_selected {
            app.select_template();
            return;
        }

        return;
    }

    if app.popups.folder_picker.is_some() {
        let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && !contains_cell(popup_area, mouse_event.column, mouse_event.row)
        {
            app.popups.folder_picker = None;
            return;
        }
    }

    if let Some(picker) = &mut app.popups.folder_picker {
        let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(popup_area);

        let mut confirm_selected = false;

        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && contains_cell(chunks[0], mouse_event.column, mouse_event.row)
        {
            picker.focus = crate::app::FolderPickerFocus::Search;
            let inner = chunks[0].inner(Margin {
                vertical: 1,
                horizontal: 1,
            });
            move_textarea_cursor_to_mouse(
                &mut picker.input,
                inner,
                mouse_event.column,
                mouse_event.row,
            );
        } else if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && contains_cell(chunks[1], mouse_event.column, mouse_event.row)
        {
            picker.focus = crate::app::FolderPickerFocus::Results;
            if !picker.filtered_folders.is_empty() {
                let row = mouse_event
                    .row
                    .saturating_sub(chunks[1].y.saturating_add(1))
                    as usize;
                let clicked = row.min(picker.filtered_folders.len().saturating_sub(1));
                if clicked == picker.selected {
                    confirm_selected = true;
                }
                picker.selected = clicked;
            }
        }

        if confirm_selected {
            app.confirm_move();
            return;
        }

        return;
    }

    if app.popups.search.is_some() {
        let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && !contains_cell(popup_area, mouse_event.column, mouse_event.row)
        {
            app.popups.search = None;
            return;
        }
    }

    if let Some(popup) = &mut app.popups.search {
        let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
        let has_filter = popup.focus != crate::popups::SearchFocus::Input
            || !popup.input.lines().join("").trim().is_empty();
        let constraints = if has_filter {
            vec![
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Min(3),
                Constraint::Length(1),
            ]
        } else {
            vec![
                Constraint::Length(3),
                Constraint::Min(3),
                Constraint::Length(1),
            ]
        };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(popup_area);

        let results_chunk_idx = if has_filter { 2 } else { 1 };
        let has_title = !popup.title_results.is_empty();
        let has_grep = !popup.grep_results.is_empty();
        let mut open_selected = false;

        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && contains_cell(chunks[0], mouse_event.column, mouse_event.row)
        {
            popup.focus = crate::popups::SearchFocus::Input;
        } else if contains_cell(
            chunks[results_chunk_idx],
            mouse_event.column,
            mouse_event.row,
        ) && (mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            || mouse_event.kind == MouseEventKind::Down(MouseButton::Right))
        {
            popup.focus = crate::popups::SearchFocus::Results;
            let row = mouse_event
                .row
                .saturating_sub(chunks[results_chunk_idx].y.saturating_add(1))
                as usize;
            if has_grep {
                let clicked = row.min(popup.grep_results.len().saturating_sub(1));
                if clicked == popup.grep_selected {
                    open_selected = true;
                }
                popup.grep_selected = clicked;
            } else if has_title {
                let clicked = row.min(popup.title_results.len().saturating_sub(1));
                if clicked == popup.title_selected {
                    open_selected = true;
                }
                popup.title_selected = clicked;
            }
        }

        if open_selected {
            app.jump_to_selected_result();
            app.confirm_search();
            return;
        }

        return;
    }

    if app.popups.trash_view.is_some() {
        let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && !contains_cell(popup_area, mouse_event.column, mouse_event.row)
        {
            app.popups.trash_view = None;
            return;
        }
    }

    if let Some(trash) = &mut app.popups.trash_view {
        let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, terminal_area);
        let mut restore_selected = false;
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left)
            && contains_cell(popup_area, mouse_event.column, mouse_event.row)
            && !trash.items.is_empty()
        {
            let row = mouse_event
                .row
                .saturating_sub(popup_area.y.saturating_add(1)) as usize;
            let clicked = row.min(trash.items.len().saturating_sub(1));
            if clicked == trash.selected {
                restore_selected = true;
            }
            trash.selected = clicked;
        }
        if restore_selected {
            app.restore_from_trash();
            return;
        }
        return;
    }

    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(terminal_area);

    let main_area = vertical_chunks[1];

    let (list_area, preview_area) = if app.list.preview_enabled {
        let (constraints, list_idx, p_idx) = match app.preview_position {
            crate::config::PreviewPosition::Left => (
                [
                    Constraint::Ratio(43, 100),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ],
                2,
                0,
            ),
            crate::config::PreviewPosition::Right => (
                [
                    Constraint::Min(0),
                    Constraint::Length(1),
                    Constraint::Ratio(43, 100),
                ],
                0,
                2,
            ),
        };
        let full_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(main_area);
        let list_area = full_cols[list_idx];
        let preview_area = Some(full_cols[p_idx]);
        (list_area, preview_area)
    } else {
        (
            Rect::new(
                terminal_area.x,
                terminal_area.y + 1,
                terminal_area.width,
                main_area.height,
            ),
            None,
        )
    };

    let inner_list_area = Rect::new(
        list_area.x.saturating_add(2),
        list_area.y.saturating_add(1),
        list_area.width.saturating_sub(4),
        list_area.height.saturating_sub(2),
    );

    if app.list.preview_enabled
        && let Some(p_area) = preview_area
        && contains_cell(p_area, mouse_event.column, mouse_event.row)
    {
        match &mut app.list.preview_content {
            Some(crate::list_view::PreviewContent::Markdown(renderer)) => {
                if mouse_event.kind == MouseEventKind::ScrollUp {
                    renderer.prev_page();
                    return;
                }
                if mouse_event.kind == MouseEventKind::ScrollDown {
                    renderer.next_page();
                    return;
                }
            }
            Some(
                crate::list_view::PreviewContent::CanvasGrid(_)
                | crate::list_view::PreviewContent::DrawGrid(_),
            ) => {
                if mouse_event.kind == MouseEventKind::ScrollUp {
                    app.list.snapshot_scroll_offset =
                        app.list.snapshot_scroll_offset.saturating_sub(3);
                    return;
                }
                if mouse_event.kind == MouseEventKind::ScrollDown {
                    app.list.snapshot_scroll_offset =
                        app.list.snapshot_scroll_offset.saturating_add(3);
                    return;
                }
            }
            None => {}
        }
    }

    if mouse_event.kind == MouseEventKind::ScrollUp {
        let current = app.list.list_state.selected().unwrap_or(0);
        app.list.list_state.select(Some(current.saturating_sub(1)));

        handle_list_keys(app, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        return;
    }

    if mouse_event.kind == MouseEventKind::ScrollDown {
        handle_list_keys(app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        return;
    }

    if app.list.notes_layout == crate::config::NotesLayout::Grid {
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
            // Vault/Pinned tabs
            if mouse_event.row == terminal_area.y {
                let tabs = [("Vault", Some("\u{f07b}")), ("Pinned", Some("\u{f4cc}"))];
                if let Some(i) = crate::ui::hit_test_tabs(
                    &tabs,
                    terminal_area.x,
                    terminal_area.width,
                    mouse_event.column,
                ) {
                    app.list.grid_folder = if i == 1 {
                        crate::app::VIRTUAL_PINNED_PATH.to_string()
                    } else {
                        String::new()
                    };
                    app.list.visual_index = 0;
                    app.refresh_visual_list();
                    return;
                }
            }

            // Breadcrumbs
            if mouse_event.row == list_area.y + 1
                && app.list.grid_folder != crate::app::VIRTUAL_PINNED_PATH
            {
                let mut offset = list_area.x;
                let vault_text = " \u{f07b} Vault";
                let vault_w = vault_text.chars().count() as u16;
                if mouse_event.column >= offset && mouse_event.column < offset + vault_w {
                    app.list.grid_folder = String::new();
                    app.list.visual_index = 0;
                    app.refresh_visual_list();
                    return;
                }
                offset += vault_w;
                if !app.list.grid_folder.is_empty() {
                    let parts: Vec<&str> = app.list.grid_folder.split('/').collect();
                    let mut current_path = String::new();
                    for part in parts {
                        // " / "
                        offset += 3;
                        let part_w = part.chars().count() as u16;
                        if !current_path.is_empty() {
                            current_path.push('/');
                        }
                        current_path.push_str(part);
                        if mouse_event.column >= offset && mouse_event.column < offset + part_w {
                            app.list.grid_folder = current_path;
                            app.list.visual_index = 0;
                            app.refresh_visual_list();
                            return;
                        }
                        offset += part_w;
                    }
                }
            }
        }

        if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
            for tile in &app.list.grid_tiles {
                if contains_cell(tile.rect, mouse_event.column, mouse_event.row) {
                    let clicked = tile.visual_index;
                    let is_select_mode = app.list.list_mode == crate::list_view::ListMode::Select
                        || app.list.tag_to_assign.is_some();
                    if is_select_mode {
                        app.list.visual_index = clicked;
                        if app.list.selected_indices.contains(&clicked) {
                            app.list.selected_indices.remove(&clicked);
                        } else {
                            app.list.selected_indices.insert(clicked);
                        }
                    } else if app.list.visual_index == clicked {
                        app.open_selected();
                    } else {
                        app.list.visual_index = clicked;
                        app.request_preview_update_immediate();
                    }
                    return;
                }
            }
        }
        if mouse_event.kind == MouseEventKind::Down(MouseButton::Right) {
            for tile in &app.list.grid_tiles {
                if contains_cell(tile.rect, mouse_event.column, mouse_event.row) {
                    app.list.visual_index = tile.visual_index;
                    app.request_preview_update_immediate();
                    break;
                }
            }
            if let Some(crate::app::VisualItem::Note { summary_idx, .. }) =
                app.list.visual_list.get(app.list.visual_index)
            {
                let id = app.notes[*summary_idx].id.clone();
                app.command_palette = Some(crate::palette::CommandPalette::new(Some(id), app));
            } else {
                app.command_palette = Some(crate::palette::CommandPalette::new(None, app));
            }
            return;
        }
        return; // grid never uses the row-based mapping below
    }

    if !contains_cell(inner_list_area, mouse_event.column, mouse_event.row) {
        return;
    }

    if mouse_event.kind == MouseEventKind::Down(MouseButton::Right) {
        let visual_row = mouse_event.row.saturating_sub(inner_list_area.y) as usize;
        let clicked_visual_index = app.list.list_state.offset().saturating_add(visual_row);
        if clicked_visual_index < app.list.visual_list.len() {
            app.list.visual_index = clicked_visual_index;
            app.request_preview_update_immediate();
        }
        if let Some(crate::app::VisualItem::Note { summary_idx, .. }) =
            app.list.visual_list.get(app.list.visual_index)
        {
            let id = app.notes[*summary_idx].id.clone();
            app.command_palette = Some(crate::palette::CommandPalette::new(Some(id), app));
        } else {
            app.command_palette = Some(crate::palette::CommandPalette::new(None, app));
        }
        return;
    }

    if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
        let visual_row = mouse_event.row.saturating_sub(inner_list_area.y) as usize;
        let clicked_visual_index = app.list.list_state.offset().saturating_add(visual_row);

        if clicked_visual_index < app.list.visual_list.len() {
            let is_select_mode = app.list.list_mode == crate::list_view::ListMode::Select
                || app.list.tag_to_assign.is_some();

            if is_select_mode {
                app.list.visual_index = clicked_visual_index;
                if app.list.selected_indices.contains(&clicked_visual_index) {
                    app.list.selected_indices.remove(&clicked_visual_index);
                } else {
                    app.list.selected_indices.insert(clicked_visual_index);
                }
            } else if app.list.visual_index == clicked_visual_index {
                app.open_selected();
            } else {
                app.list.visual_index = clicked_visual_index;
                app.request_preview_update_immediate();
            }
        }
    }
}

pub fn handle_edit_mouse(
    app: &mut App,
    mouse_event: MouseEvent,
    terminal_area: Rect,
    focus: &mut EditFocus,
    mouse_selecting: &mut bool,
    mouse_dragged: &mut bool,
) {
    if let Some(menu) = &app.popups.context_menu {
        let menu_rect = Rect::new(menu.x, menu.y, 14, 4);
        if contains_cell(menu_rect, mouse_event.column, mouse_event.row) {
            if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
                let clicked_idx = mouse_event.row.saturating_sub(menu.y) as usize;
                if clicked_idx < 4 {
                    app.handle_menu_action(clicked_idx, focus);
                }
                app.popups.context_menu = None;
            } else if mouse_event.kind == MouseEventKind::ScrollUp {
                let mut menu_copy = app
                    .popups
                    .context_menu
                    .take()
                    .expect("context_menu Some — guarded by enclosing if-let");
                menu_copy.selected = menu_copy.selected.saturating_sub(1);
                app.popups.context_menu = Some(menu_copy);
            } else if mouse_event.kind == MouseEventKind::ScrollDown {
                let mut menu_copy = app
                    .popups
                    .context_menu
                    .take()
                    .expect("context_menu Some — guarded by enclosing if-let");
                if menu_copy.selected < 3 {
                    menu_copy.selected += 1;
                }
                app.popups.context_menu = Some(menu_copy);
            }
            return;
        } else if matches!(mouse_event.kind, MouseEventKind::Down(_)) {
            app.popups.context_menu = None;
            if mouse_event.kind != MouseEventKind::Down(MouseButton::Right) {
                return;
            }
        } else {
            return;
        }
    }

    if mouse_event.kind == MouseEventKind::Down(MouseButton::Right) {
        let (title_inner, body_inner) = edit_view_input_areas(
            terminal_area,
            app.editor.editor_preview_enabled,
            app.editor.editor.lines().len(),
            app.editor.show_line_numbers,
        );

        if contains_cell(title_inner, mouse_event.column, mouse_event.row) {
            *focus = EditFocus::Title;
            move_textarea_cursor_to_mouse(
                &mut app.editor.title_editor,
                title_inner,
                mouse_event.column,
                mouse_event.row,
            );
        } else if contains_cell(body_inner, mouse_event.column, mouse_event.row) {
            *focus = EditFocus::Body;
            move_textarea_cursor_to_mouse(
                &mut app.editor.editor,
                body_inner,
                mouse_event.column,
                mouse_event.row,
            );
        }

        let max_x = terminal_area.width.saturating_sub(14);
        let max_y = terminal_area.height.saturating_sub(4);
        app.popups.context_menu = Some(ContextMenu {
            x: mouse_event.column.min(max_x),
            y: mouse_event.row.min(max_y),
            selected: 0,
        });
        return;
    }

    let (title_inner, body_inner) = edit_view_input_areas(
        terminal_area,
        app.editor.editor_preview_enabled,
        app.editor.editor.lines().len(),
        app.editor.show_line_numbers,
    );

    if app.editor.editor_preview_enabled
        && let Some(md_area) = edit_view_md_preview_area(terminal_area)
        && contains_cell(md_area, mouse_event.column, mouse_event.row)
    {
        match mouse_event.kind {
            MouseEventKind::ScrollUp => {
                if let Some(renderer) = &mut app.editor.md_preview_renderer {
                    renderer.prev_page();
                }
                return;
            }
            MouseEventKind::ScrollDown => {
                if let Some(renderer) = &mut app.editor.md_preview_renderer {
                    renderer.next_page();
                }
                return;
            }
            _ => {}
        }
    }

    match mouse_event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            *mouse_selecting = false;
            *mouse_dragged = false;
            if contains_cell(body_inner, mouse_event.column, mouse_event.row) {
                *focus = EditFocus::Body;
                move_textarea_cursor_to_mouse(
                    &mut app.editor.editor,
                    body_inner,
                    mouse_event.column,
                    mouse_event.row,
                );
                app.editor.editor.start_selection();
                *mouse_selecting = true;
            } else if contains_cell(title_inner, mouse_event.column, mouse_event.row) {
                *focus = EditFocus::Title;
                move_textarea_cursor_to_mouse(
                    &mut app.editor.title_editor,
                    title_inner,
                    mouse_event.column,
                    mouse_event.row,
                );
                app.editor.title_editor.start_selection();
                *mouse_selecting = true;
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if *mouse_selecting {
                *mouse_dragged = true;
                if *focus == EditFocus::Body {
                    move_textarea_cursor_to_mouse(
                        &mut app.editor.editor,
                        body_inner,
                        mouse_event.column,
                        mouse_event.row,
                    );
                } else {
                    move_textarea_cursor_to_mouse(
                        &mut app.editor.title_editor,
                        title_inner,
                        mouse_event.column,
                        mouse_event.row,
                    );
                }
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if *mouse_selecting && !*mouse_dragged {
                if *focus == EditFocus::Body {
                    app.editor.editor.cancel_selection();
                } else {
                    app.editor.title_editor.cancel_selection();
                }
            }
            *mouse_selecting = false;
            *mouse_dragged = false;
        }
        MouseEventKind::ScrollDown => {
            if *focus == EditFocus::Body {
                app.editor.editor.scroll((3, 0));
            }
        }
        MouseEventKind::ScrollUp if *focus == EditFocus::Body => {
            app.editor.editor.scroll((-3, 0));
        }
        _ => {}
    }
}

pub fn move_textarea_cursor_to_mouse(
    textarea: &mut TextArea,
    body_inner: Rect,
    mouse_col: u16,
    mouse_row: u16,
) {
    if textarea.lines().is_empty() || body_inner.width == 0 || body_inner.height == 0 {
        return;
    }

    let (scroll_row, scroll_col) = crate::ui::get_textarea_scroll(textarea);

    let row = mouse_row.saturating_sub(body_inner.y) as usize + scroll_row;
    let col = mouse_col.saturating_sub(body_inner.x) as usize + scroll_col;

    let max_row = textarea.lines().len().saturating_sub(1);
    let target_row = row.min(max_row);
    let max_col = textarea.lines()[target_row].chars().count();
    let target_col = col.min(max_col);

    textarea.move_cursor(CursorMove::Jump(target_row as u16, target_col as u16));
}

pub fn edit_view_input_areas(
    area: Rect,
    md_preview: bool,
    line_count: usize,
    show_line_numbers: bool,
) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(area);

    let title_inner = Rect::new(
        chunks[1].x + 2,
        chunks[1].y + 1,
        chunks[1].width.saturating_sub(4),
        chunks[1].height.saturating_sub(2),
    );

    let body_area = if md_preview {
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Length(1),
                Constraint::Percentage(50),
            ])
            .split(area);

        Rect::new(
            content_chunks[0].x,
            chunks[2].y,
            content_chunks[0].width,
            chunks[2].height,
        )
    } else {
        chunks[2]
    };

    let gutter_width = if show_line_numbers {
        (line_count.max(1).to_string().len() as u16) + 1
    } else {
        0
    };

    let body_inner = Rect::new(
        body_area.x + gutter_width,
        body_area.y,
        body_area.width.saturating_sub(gutter_width + 2),
        body_area.height,
    );

    (title_inner, body_inner)
}

pub fn edit_view_md_preview_area(area: Rect) -> Option<Rect> {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(area);

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Length(1),
            Constraint::Percentage(50),
        ])
        .split(area);

    let preview_area = Rect::new(
        content_chunks[2].x,
        chunks[2].y,
        content_chunks[2].width,
        chunks[2].height,
    );

    Some(Rect::new(
        preview_area.x + 2,
        preview_area.y + 1,
        preview_area.width.saturating_sub(4),
        preview_area.height.saturating_sub(2),
    ))
}

pub fn contains_cell(rect: Rect, col: u16, row: u16) -> bool {
    rect.width > 0
        && rect.height > 0
        && col >= rect.x
        && col < rect.x + rect.width
        && row >= rect.y
        && row < rect.y + rect.height
}

pub fn make_title_editor(
    initial: &str,
    highlight_fg: Color,
    highlight_bg: Color,
) -> TextArea<'static> {
    let mut title = if initial.is_empty() {
        TextArea::default()
    } else {
        TextArea::from([initial.to_string()])
    };
    title.set_cursor_style(Style::default().fg(highlight_fg).bg(highlight_bg));
    title
}

use std::borrow::Cow;

pub fn get_title_text<'a>(title_editor: &'a TextArea<'static>) -> Cow<'a, str> {
    let lines = title_editor.lines();

    if lines.len() == 1 {
        let line = lines[0].trim();
        if !line.contains(['\r', '\n']) {
            return Cow::Borrowed(line);
        }
    }

    Cow::Owned(
        lines
            .join(" ")
            .replace(['\r', '\n'], " ")
            .trim()
            .to_string(),
    )
}
