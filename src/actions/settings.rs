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
    ToggleWrapAction,
    "settings.wrap",
    "Toggle Word Wrap",
    "Toggle word wrap for both editor and preview",
    ActionCategory::Settings,
    "\u{f036}",
    "\u{1f4c4}",
    toggle_wrap,
    app,
    if match app.mode {
        crate::app::ViewMode::Edit => app.config.editor.soft_wrap,
        _ => app.preview_wrap,
    } {
        "On"
    } else {
        "Off"
    }
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
    app.config.ui.hint_bar_style.name()
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

toggle_action!(
    ToggleEditModeHighlightAction,
    "settings.edit_mode_highlight",
    "Toggle Edit Mode Highlight",
    "Show or hide edit mode highlighting in the note editor",
    ActionCategory::Settings,
    "\u{f044}",
    "\u{1f4dd}",
    toggle_edit_mode_highlight,
    app,
    if app.config.editor.edit_mode_highlight {
        "On"
    } else {
        "Off"
    }
);

toggle_action!(
    ToggleGhostSyntaxAction,
    "settings.ghost_syntax",
    "Toggle Ghost Syntax",
    "Show or hide ghost syntax highlighting in the note editor",
    ActionCategory::Settings,
    "\u{f0db}",
    "\u{1f4dd}",
    toggle_ghost_syntax,
    app,
    if app.config.editor.ghost_syntax {
        "On"
    } else {
        "Off"
    }
);

toggle_action!(
    ToggleExtendedMarkdownAction,
    "settings.extended_markdown",
    "Toggle Extended Markdown",
    "Enable or disable extended markdown features in the editor",
    ActionCategory::Settings,
    "\u{f15b}",
    "\u{1f4d6}",
    toggle_extended_markdown,
    app,
    if app.config.editor.extended_markdown_features {
        "On"
    } else {
        "Off"
    }
);

toggle_action!(
    ToggleScrollbarsAction,
    "settings.scrollbars",
    "Toggle Scrollbars",
    "Show or hide mouse-draggable scrollbars on scrollable regions",
    ActionCategory::Settings,
    "\u{f0db}",
    "\u{1f4cb}",
    toggle_scrollbars,
    app,
    if app.config.ui.scrollbars {
        "On"
    } else {
        "Off"
    }
);

toggle_action!(
    ToggleScrollbarPanModeAction,
    "settings.scrollbar_pan_mode",
    "Toggle Scrollbar Pan Mode",
    "Scrollbar pans the viewport instead of moving selection; any key snaps back",
    ActionCategory::Settings,
    "\u{f878}",
    "\u{1f4d7}",
    toggle_scrollbar_pan_mode,
    app,
    if app.config.ui.scrollbar_pan_mode {
        "On"
    } else {
        "Off"
    }
);

toggle_action!(
    ToggleSyntaxHighlightingAction,
    "settings.syntax_highlighting",
    "Toggle Syntax Highlighting",
    "Enable or disable syntax highlighting in code blocks",
    ActionCategory::Settings,
    "\u{f121}",
    "\u{1f4d6}",
    toggle_syntax_highlighting,
    app,
    if app.config.core.syntax_highlighting {
        "On"
    } else {
        "Off"
    }
);

toggle_action!(
    ToggleCodeLineNumbersAction,
    "settings.code_line_numbers",
    "Toggle Code Line Numbers",
    "Show or hide line numbers in code blocks",
    ActionCategory::Settings,
    "\u{f03a}",
    "\u{0023}\u{20e3}",
    toggle_code_line_numbers,
    app,
    if app.config.core.code_line_numbers {
        "On"
    } else {
        "Off"
    }
);

toggle_action!(
    ToggleShowFileSizeAction,
    "settings.show_file_size",
    "Toggle Show File Size",
    "Show or hide file sizes in the notes list",
    ActionCategory::Settings,
    "\u{f15c}",
    "\u{1f4c4}",
    toggle_show_file_size,
    app,
    if app.list.show_file_size { "On" } else { "Off" }
);

toggle_action!(
    CycleListDensityAction,
    "settings.cycle_list_density",
    "Cycle List Density",
    "Switch between compact and comfortable list spacing",
    ActionCategory::Settings,
    "\u{f00a}",
    "\u{1f4cb}",
    cycle_list_density,
    app,
    if app.list.list_density == crate::config::ListDensity::Compact {
        "Compact"
    } else {
        "Comfortable"
    }
);

toggle_action!(
    CycleWeekStartAction,
    "settings.cycle_week_start",
    "Cycle Week Start",
    "Switch between Sunday and Monday as the first day of the week",
    ActionCategory::Settings,
    "\u{f073}",
    "\u{1f4c5}",
    cycle_week_start,
    app,
    if app.list.week_start == crate::config::WeekStart::Sunday {
        "Sunday"
    } else {
        "Monday"
    }
);

toggle_action!(
    ToggleGoalsAction,
    "settings.goals",
    "Toggle Goals",
    "Show or hide daily goals in the notes list",
    ActionCategory::Settings,
    "\u{f1e5}",
    "\u{1f3af}",
    toggle_goals,
    app,
    if app.config.goals.enabled {
        "On"
    } else {
        "Off"
    }
);

toggle_action!(
    ToggleGraphPreviewAction,
    "settings.folder_graph_preview",
    "Toggle Folder Graph Preview",
    "Show or hide the graph preview in the notes list",
    ActionCategory::Settings,
    "\u{f0c0}",
    "\u{1f578}",
    toggle_folder_graph_preview,
    app,
    if app.config.list.folder_graph_preview {
        "On"
    } else {
        "Off"
    }
);

toggle_action!(
    ToggleGraphShowLegendAction,
    "settings.graph_show_legend",
    "Toggle Graph Legend",
    "Show or hide the legend in the graph overlay",
    ActionCategory::Settings,
    "\u{f02e}",
    "\u{1f4cb}",
    toggle_graph_show_legend,
    app,
    if app.config.graf.visual.show_legend {
        "On"
    } else {
        "Off"
    }
);

toggle_action!(
    ToggleGraphShowMinimapAction,
    "settings.graph_show_minimap",
    "Toggle Graph Minimap",
    "Show or hide the minimap in the graph overlay",
    ActionCategory::Settings,
    "\u{f279}",
    "\u{1f5fa}",
    toggle_graph_show_minimap,
    app,
    if app.config.graf.visual.show_minimap {
        "On"
    } else {
        "Off"
    }
);

toggle_action!(
    ToggleGraphShowOrphanAction,
    "settings.graph_show_orphan",
    "Toggle Orphan Nodes",
    "Show or hide orphan nodes in the graph overlay",
    ActionCategory::Settings,
    "\u{f0c1}",
    "\u{1f4e1}",
    toggle_graph_show_orphan,
    app,
    if app.config.graf.filter.show_orphan {
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
