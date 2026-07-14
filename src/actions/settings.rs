use crate::actions::{Action, ActionCategory};
use crate::toggle_action;

toggle_action!(
    TogglePreviewPaneAction,
    "settings.preview_pane",
    "Toggle Preview Pane",
    "Show or hide the preview pane in the notes list",
    ActionCategory::Settings,
    "\u{f0db}",
    "\u{1f4cb}",
    toggle_preview,
    app,
    if app.list.preview_enabled {
        "On"
    } else {
        "Off"
    }
);

toggle_action!(
    TogglePreviewWrapAction,
    "settings.preview_wrap",
    "Toggle Preview Word Wrap",
    "Wrap long preview lines to the pane width",
    ActionCategory::Settings,
    "\u{f036}",
    "\u{1f4c4}",
    toggle_preview_wrap,
    app,
    if app.preview_wrap { "On" } else { "Off" }
);

toggle_action!(
    ToggleCalendarAction,
    "settings.calendar",
    "Toggle Calendar",
    "Show or hide the month calendar in the notes list",
    ActionCategory::Settings,
    "\u{f073}",
    "\u{1f4c5}",
    toggle_calendar,
    app,
    if app.list.calendar_enabled {
        "On"
    } else {
        "Off"
    }
);

toggle_action!(
    ToggleLineNumbersAction,
    "settings.line_numbers",
    "Toggle Line Numbers",
    "Show or hide line numbers in the note editor",
    ActionCategory::Settings,
    "\u{f03a}",
    "\u{0023}\u{20e3}",
    toggle_show_line_numbers,
    app,
    if app.editor.show_line_numbers {
        "On"
    } else {
        "Off"
    }
);

toggle_action!(
    ToggleConfirmDeleteAction,
    "settings.confirm_delete",
    "Toggle Delete Confirmation",
    "Ask for confirmation before moving notes to trash",
    ActionCategory::Settings,
    "\u{f1f8}",
    "\u{1f5d1}",
    toggle_confirm_on_delete,
    app,
    if app.confirm_on_delete { "On" } else { "Off" }
);

toggle_action!(
    TogglePinnedOnTopAction,
    "settings.pinned_on_top",
    "Toggle Pinned on Top",
    "Keep pinned notes above others in the list",
    ActionCategory::Settings,
    "\u{f08d}",
    "\u{1f4cc}",
    toggle_pinned_on_top,
    app,
    if app.pinned_on_top { "On" } else { "Off" }
);

toggle_action!(
    ToggleConfirmQuitAction,
    "settings.confirm_quit",
    "Toggle Quit Confirmation",
    "Ask for confirmation before quitting clin",
    ActionCategory::Settings,
    "\u{f08b}",
    "\u{1f6aa}",
    toggle_confirm_on_quit,
    app,
    if app.confirm_on_quit { "On" } else { "Off" }
);

toggle_action!(
    ToggleInlineInfoAction,
    "settings.inline_info",
    "Toggle Inline Info",
    "Show or hide inline metadata (tags, dates, sizes, folder counts) in the notes list",
    ActionCategory::Settings,
    "\u{f0b0}",
    "\u{2699}",
    toggle_inline_info,
    app,
    if app.list.inline_info { "On" } else { "Off" }
);

toggle_action!(
    TogglePreviewEncryptionAction,
    "settings.preview_encryption",
    "Toggle Encrypted Note Preview",
    "Show or hide previews of encrypted (.clin) notes",
    ActionCategory::Settings,
    "\u{f06e}",
    "\u{1f441}",
    toggle_preview_encryption,
    app,
    if app.preview_encryption { "On" } else { "Off" }
);

toggle_action!(
    CycleSortAction,
    "settings.cycle_sort",
    "Select Sort Order",
    "Select the notes sort field and order from a list",
    ActionCategory::Settings,
    "\u{f0dc}",
    "\u{1f4cb}",
    begin_sort_selection,
    app,
    {
        use crate::list_view::{SortField, SortOrder};
        match (app.list.sort_field, app.list.sort_order) {
            (SortField::Modified, SortOrder::Descending) => "Modified (newest)",
            (SortField::Modified, SortOrder::Ascending) => "Modified (oldest)",
            (SortField::Title, SortOrder::Ascending) => "Title (A-Z)",
            (SortField::Title, SortOrder::Descending) => "Title (Z-A)",
        }
    }
);

toggle_action!(
    ToggleShowHiddenFilesAction,
    "settings.show_hidden_files",
    "Show Hidden Files",
    "Show files and directories starting with '.' in the notes list",
    ActionCategory::Settings,
    "\u{f06e}",
    "\u{1f441}",
    toggle_show_hidden_files,
    app,
    if app.list.show_hidden_files {
        "On"
    } else {
        "Off"
    }
);

toggle_action!(
    ToggleShowAllFilesAction,
    "settings.show_all_files",
    "Show All Files",
    "Show every file in the vault, not just notes (.md/.txt/.clin/.draw/.canvas)",
    ActionCategory::Settings,
    "\u{f07c}",
    "\u{1f4c2}",
    toggle_show_all_files,
    app,
    if app.list.show_all_files { "On" } else { "Off" }
);

toggle_action!(
    ToggleTabIconsOnlyAction,
    "settings.tab_icons_only",
    "Tab Icons Only",
    "Show only Nerd Font icons (no text) on tab bars",
    ActionCategory::Settings,
    "\u{f26c}",
    "\u{1f5a5}",
    toggle_tab_icons_only,
    app,
    if app.config.ui.tab_icons_only {
        "On"
    } else {
        "Off"
    }
);

toggle_action!(
    SetWordGoalAction,
    "settings.word_goal",
    "Set Daily Word Goal",
    "Set target number of words to write daily",
    ActionCategory::Settings,
    "\u{f044}",
    "\u{1f3af}",
    begin_set_word_goal,
    app,
    app.config.goals.word_goal
);

toggle_action!(
    SetNoteGoalAction,
    "settings.note_goal",
    "Set Daily Note Goal",
    "Set target number of notes to edit daily",
    ActionCategory::Settings,
    "\u{f044}",
    "\u{1f3af}",
    begin_set_note_goal,
    app,
    app.config.goals.note_goal
);

toggle_action!(
    ToggleLayoutEditModeAction,
    "settings.layout_edit",
    "Toggle Layout Edit Mode",
    "Resize and reposition notes-view panes with the mouse (Hyprland-style)",
    ActionCategory::Settings,
    "\u{f0b0}",
    "\u{1f39b}",
    toggle_layout_edit,
    app,
    if app.layout_edit { "On" } else { "Off" }
);

toggle_action!(
    CycleIconModeAction,
    "settings.icon_mode",
    "Select Icon Mode",
    "Choose between Nerd Font, Unicode, or no icons",
    ActionCategory::Settings,
    "\u{f013}",
    "\u{1f5a8}",
    begin_icon_mode_selection,
    app,
    match app.config.ui.icon_mode {
        crate::config::IconMode::Nerd => "Nerd Font",
        crate::config::IconMode::Unicode => "Unicode",
        crate::config::IconMode::None => "None",
    }
);

toggle_action!(
    CycleHintBarStyleAction,
    "settings.hint_bar_style",
    "Select Hint Bar Style",
    "Choose how hint bars and popup footers are styled",
    ActionCategory::Settings,
    "\u{f0db}",
    "\u{1f4cb}",
    begin_hint_bar_style_selection,
    app,
    match app.config.ui.hint_bar_style {
        crate::config::HintBarStyle::Classic => "Classic",
        crate::config::HintBarStyle::Sharp => "Sharp",
        crate::config::HintBarStyle::Rounded => "Rounded",
        crate::config::HintBarStyle::Slanted => "Slanted",
    }
);

toggle_action!(
    ToggleFoldersFirstAction,
    "settings.folders_first",
    "Toggle Folders First",
    "Show subfolders before files in the notes list (Tree and Grid layouts)",
    ActionCategory::Settings,
    "\u{f07c}",
    "\u{1f4c1}",
    toggle_folders_first,
    app,
    if app.list.folders_first { "On" } else { "Off" }
);

toggle_action!(
    ToggleSmartFoldersAction,
    "settings.smart_folders",
    "Toggle Smart Folders",
    "Show or hide smart virtual folders in the notes list",
    ActionCategory::Settings,
    "\u{f0e7}",
    "\u{26a1}",
    toggle_smart_folders,
    app,
    if app.config.list.smart_folders_enabled {
        "On"
    } else {
        "Off"
    }
);

pub struct ConfigureSmartFoldersAction;
impl Action for ConfigureSmartFoldersAction {
    fn id(&self) -> std::borrow::Cow<'static, str> {
        "settings.configure_smart_folders".into()
    }
    fn name(&self) -> std::borrow::Cow<'static, str> {
        "Configure Smart Folders".into()
    }
    fn description(&self) -> std::borrow::Cow<'static, str> {
        "Open config.toml to edit custom smart folders".into()
    }
    fn category(&self) -> ActionCategory {
        ActionCategory::Settings
    }
    fn glyph(&self) -> (&'static str, &'static str) {
        ("\u{f0db}", "\u{1f4cb}")
    }
    fn execute(&self, app: &mut crate::app::App, _ctx: Option<&str>) -> anyhow::Result<()> {
        let path = crate::config::ClinConfig::config_path()?;
        app.open_path_in_external_editor(&path);
        app.reload_config();
        Ok(())
    }
}
