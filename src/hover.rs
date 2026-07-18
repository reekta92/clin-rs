use ratatui::layout::{Constraint, Direction, Layout, Rect};
use crate::app::{App, VIRTUAL_PINNED_PATH, VIRTUAL_SMART_PATH, VIRTUAL_SUBNOTES_PATH, ViewMode};
use crate::backup::state::SettingsField;
use crate::backup::render::backup_tabs;
use crate::editor::EditSidebar;

/// Identifies the currently hovered UI element, if any.
/// Used by the render gate to skip redraws when the hover target hasn't changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoverKey {
    None,
    ListRow(usize),
    ListTab(usize),
    ListSmartTag { row: usize, tag: usize },
    ListSubnotes(usize),
    ListVault(usize),
    ListTagPart { row: usize, part: usize },
    ListTile { row: usize, col: usize },
    GraphNode(u32),
    BackupRow(usize),
    BackupHistoryRow(usize),
    BackupTab(usize),
    BackupField(SettingsField),
    BackupSaveButton,
    TreeRow(usize),
    EditSidebarRow(usize),
    HelpTab(usize),
    SetupRow(usize),
    SetupDoneButton,
    PopupListRow { idx: usize },
    QuickSearchRow(usize),
    PaletteCmd(usize),
    PaletteTab(usize),
}

/// Compute the hover key for the current mouse position.
/// Returns `HoverKey::None` when the cursor is outside any hoverable region,
/// when `mouse_pos` is `None`, or for continuous-render views (Graph/Draw/Canvas).
///
/// Dispatch order matches `draw_ui` paint order in `src/ui/mod.rs`.
pub fn compute_hover_key(app: &App, area: Rect) -> HoverKey {
    if app.mouse_pos.is_none() {
        return HoverKey::None;
    }

    if app.command_palette.is_some() {
        return compute_palette_hover_key(app, area);
    }

    if app.popups.active.is_some() {
        return compute_popup_hover_key(app, area);
    }

    match app.mode {
        ViewMode::List => compute_list_hover_key(app, area),
        ViewMode::Graph => compute_graph_hover_key(app, area),
        ViewMode::Draw | ViewMode::Canvas => HoverKey::None,
        ViewMode::Backup => compute_backup_hover_key(app, area),
        ViewMode::ContentTree => compute_tree_hover_key(app, area),
        ViewMode::Edit => compute_edit_hover_key(app, area),
        ViewMode::Help => compute_help_hover_key(app, area),
        ViewMode::Setup => compute_setup_hover_key(app, area),
    }
}

// ── List view ──────────────────────────────────────────────────────────────

fn compute_list_hover_key(app: &App, area: Rect) -> HoverKey {
    let Some((col, row)) = app.mouse_pos else {
        return HoverKey::None;
    };

    // Replicate the main area layout from draw_list_view
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title bar
            Constraint::Min(5),
            Constraint::Length(1), // status bar
        ])
        .split(area);

    // Determine if we're in grid or list layout
    let is_grid = app.list.notes_layout == crate::config::NotesLayout::Grid;

    if is_grid {
        // --- Title tab hover (only for grid layout) ---
        if row == chunks[0].y {
            let icon_mode = app.config.ui.icon_mode;
            let mut tabs: Vec<(&str, Option<&str>)> = vec![
                ("Vault", Some(crate::ui::get_icon("\u{f07b}", "\u{1f4c1}", icon_mode))),
                ("Pinned", Some(crate::ui::get_icon("\u{f4cc}", "\u{1f4cc}", icon_mode))),
            ];
            if app.config.list.smart_folders_enabled {
                tabs.push(("Smart", Some(crate::ui::get_icon("\u{f0e7}", "\u{26a1}", icon_mode))));
            }
            tabs.push(("Subnotes", Some(crate::ui::get_icon("\u{f02c}", "\u{1f3f7}", icon_mode))));

            // Use "List" as the title for tab region computation
            let region = crate::ui::title_bar_tabs_region(chunks[0], "List");
            if let Some(i) = crate::ui::hit_test_tabs(
                &tabs,
                chunks[0].x,
                chunks[0].width,
                region.x,
                col,
                app.config.ui.tab_icons_only,
                icon_mode,
            ) {
                return HoverKey::ListTab(i);
            }
        }

        // --- Get list_area same way as draw_list_view ---
        let preview_enabled = app.list.preview_enabled || app.preview_fullscreen;
        let (list_area, _, _) = crate::ui::list_view_layout(
            area,
            preview_enabled,
            app.preview_position,
            app.config.list.calendar_enabled,
            app.preview_fullscreen,
            app.list.preview_width_ratio,
            app.config.list.calendar_height,
            app.config.list.calendar_position,
        );

        // --- Breadcrumb row hover (smart tag, subnotes, vault, tag parts) ---
        if row == list_area.y + 1 {
            let is_smart = app.list.grid_folder == *VIRTUAL_SMART_PATH
                || app.list.grid_folder.starts_with('@');
            let is_subnotes = app.list.grid_folder == *VIRTUAL_SUBNOTES_PATH
                || crate::app::App::is_subnotes_parent_grid_path(&app.list.grid_folder);
            let is_pinned = app.list.grid_folder == *VIRTUAL_PINNED_PATH;

            if is_pinned {
                return HoverKey::None;
            }

            if is_smart {
                let icon_mode = app.config.ui.icon_mode;
                let smart_icon = crate::ui::get_icon("\u{f0e7}", "\u{26a1}", icon_mode);
                let smart_text = format!(" {smart_icon} Smart");
                let smart_w = smart_text.chars().count() as u16;
                if col >= list_area.x && col < list_area.x + smart_w
                    && app.list.grid_folder != *VIRTUAL_SMART_PATH
                {
                    return HoverKey::ListSmartTag { row: 0, tag: 0 };
                }
                return HoverKey::None;
            }

            if is_subnotes {
                let icon_mode = app.config.ui.icon_mode;
                let sub_icon = crate::ui::get_icon("\u{f02c}", "\u{1f3f7}", icon_mode);
                let sub_text = format!(" {sub_icon} Subnotes");
                let sub_w = sub_text.chars().count() as u16;
                if col >= list_area.x && col < list_area.x + sub_w
                    && app.list.grid_folder != *VIRTUAL_SUBNOTES_PATH
                {
                    return HoverKey::ListSubnotes(0);
                }
                return HoverKey::None;
            }

            // Vault breadcrumb
            let icon_mode = app.config.ui.icon_mode;
            let vault_icon = crate::ui::get_icon("\u{f07b}", "\u{1f4c1}", icon_mode);
            let vault_text = format!(" {vault_icon} Vault");
            let vault_w = vault_text.chars().count() as u16;
            if col >= list_area.x && col < list_area.x + vault_w
                && !app.list.grid_folder.is_empty()
            {
                return HoverKey::ListVault(0);
            }

            // Tag path parts hover
            if !app.list.grid_folder.is_empty() {
                let parts: Vec<&str> = app.list.grid_folder.split('/').collect();
                let mut offset = list_area.x + vault_w;
                for (part_idx, part) in parts.iter().enumerate() {
                    offset += 3; // " / " separator
                    let part_w = part.chars().count() as u16;
                    if part_idx < parts.len() - 1
                        && col >= offset && col < offset + part_w
                    {
                        return HoverKey::ListTagPart { row: 0, part: part_idx };
                    }
                    offset += part_w;
                }
            }
        }

        // --- Grid tile hover ---
        const GRID_LEFT_MARGIN: u16 = 1;
        const GRID_GAP: u16 = 1;
        const GRID_TILE_W: u16 = 20;
        const GRID_TILE_H: u16 = 3;
        const GRID_TOP_MARGIN: u16 = 1;

        let cols = ((list_area.width.saturating_sub(GRID_LEFT_MARGIN + GRID_GAP))
            / (GRID_TILE_W + GRID_GAP))
            .max(1) as usize;
        let rows = ((list_area.height.saturating_sub(GRID_TOP_MARGIN + GRID_GAP))
            / (GRID_TILE_H + GRID_GAP)) as usize;
        let len = app.list.visual_list.len();
        if cols > 0 && rows > 0 && len > 0 {
            let start = app.list.grid_scroll * cols;
            let count = (rows * cols).min(len.saturating_sub(start));
            for i in 0..count {
                let vi = start + i;
                if vi >= len { break; }
                let tile_row = i / cols;
                let tile_col = i % cols;
                let tile_rect = Rect::new(
                    list_area.x + GRID_LEFT_MARGIN + (tile_col as u16) * (GRID_TILE_W + GRID_GAP),
                    list_area.y + GRID_TOP_MARGIN + (tile_row as u16) * (GRID_TILE_H + GRID_GAP),
                    GRID_TILE_W,
                    GRID_TILE_H,
                );
                if crate::events::contains_cell(tile_rect, col, row) {
                    return HoverKey::ListTile { row: tile_row, col: tile_col };
                }
            }
        }
    } else {
        // --- List-style layout ---
        let preview_enabled = app.list.preview_enabled || app.preview_fullscreen;
        let (list_area, _, _) = crate::ui::list_view_layout(
            area,
            preview_enabled,
            app.preview_position,
            app.config.list.calendar_enabled,
            app.preview_fullscreen,
            app.list.preview_width_ratio,
            app.config.list.calendar_height,
            app.config.list.calendar_position,
        );

        // List-style row hit-test
        let inner_y = list_area.y + 1;
        let inner_h = list_area.height.saturating_sub(2);
        let inner_x = list_area.x + 2;
        let inner_w = list_area.width.saturating_sub(4);

        if col >= inner_x && col < inner_x + inner_w
            && row >= inner_y && row < inner_y + inner_h
            && let Some(idx) = crate::ui::list_index_at(
                row, inner_y, 1,
                app.list.list_state.offset(),
                app.list.display_items.len(),
            )
        {
            return HoverKey::ListRow(idx);
        }
    }

    HoverKey::None
}

// ── Graph view ─────────────────────────────────────────────────────────────

fn compute_graph_hover_key(app: &App, area: Rect) -> HoverKey {
    let Some((col, row)) = app.mouse_pos else {
        return HoverKey::None;
    };

    let Some(ref graf_app_state) = app.graph_state else {
        return HoverKey::None;
    };
    let Some(ref graph_state_arc) = graf_app_state.graph_state else {
        return HoverKey::None;
    };
    let guard = graph_state_arc.read();

    // canvas_area = area minus status bar
    let mut canvas_area = area;
    canvas_area.height = canvas_area.height.saturating_sub(1);

    let (wx, wy) = guard.viewport.screen_to_world(col, row, canvas_area);
    if let Some(idx) = guard.viewport.hit_test(wx, wy, &guard) {
        return HoverKey::GraphNode(idx.index() as u32);
    }

    HoverKey::None
}


// ── Backup view ────────────────────────────────────────────────────────────

fn compute_backup_hover_key(app: &App, area: Rect) -> HoverKey {
    let Some(ref state) = app.backup_state else {
        return HoverKey::None;
    };
    let Some((col, row)) = state.mouse_pos else {
        return HoverKey::None;
    };

    // Replicate layout from draw_dashboard
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    // Tab hover (title bar row)
    if row == outer[0].y {
        let icon_mode = app.config.ui.icon_mode;
        let tabs_array = backup_tabs(icon_mode);
        let tabs: Vec<(&str, Option<&str>)> = tabs_array
            .iter()
            .map(|&(l, g)| (l, Some(g)))
            .collect();
        let region = crate::ui::title_bar_tabs_region(outer[0], "Backup");
        if let Some(i) = crate::ui::hit_test_tabs(
            &tabs,
            outer[0].x,
            outer[0].width,
            region.x,
            col,
            state.tab_icons_only,
            icon_mode,
        ) {
            return HoverKey::BackupTab(i);
        }
    }

    // If settings popup is open, handle settings hover
    if state.settings_open {
        return compute_backup_settings_hover(app, area, state, col, row);
    }

    // Content area layout
    let content_area = outer[1];
    if state.selected_section == crate::backup::state::BackupSection::Status {
        // Status list hover
        let inner_y = content_area.y + 1;
        let inner_h = content_area.height.saturating_sub(2);
        let inner_x = content_area.x + 2;
        let inner_w = content_area.width.saturating_sub(4);
        if col >= inner_x && col < inner_x + inner_w
            && row >= inner_y && row < inner_y + inner_h
            && let Some(idx) = crate::ui::list_index_at(
                row, inner_y, 1,
                state.list_state.offset(),
                state.selectable_files.len(),
            )
            && state.file_index_at_rendered_line(idx).is_some()
        {
            return HoverKey::BackupRow(idx);
        }
    } else if state.selected_section == crate::backup::state::BackupSection::History {
        // History list hover
        let inner_y = content_area.y + 1;
        let inner_h = content_area.height.saturating_sub(2);
        let inner_x = content_area.x + 2;
        let inner_w = content_area.width.saturating_sub(4);
        if col >= inner_x && col < inner_x + inner_w
            && row >= inner_y && row < inner_y + inner_h
            && let Some(idx) = crate::ui::list_index_at(
                row, inner_y, 1,
                state.history_list_state.offset(),
                state.commits.len().saturating_add(1),
            )
            && idx > 0
            && !state.commits.is_empty()
        {
            return HoverKey::BackupHistoryRow(idx);
        }
    }

    HoverKey::None
}

fn compute_backup_settings_hover(
    _app: &App,
    area: Rect,
    _state: &crate::backup::state::BackupState,
    col: u16,
    row: u16,
) -> HoverKey {
    // Replicate the settings popup layout from draw_settings_popup
    let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Large, area);
    let inner_content = Rect {
        x: popup_area.x + 1,
        y: popup_area.y + 1,
        width: popup_area.width.saturating_sub(2),
        height: popup_area.height.saturating_sub(2),
    };

    let chunks = Layout::default()
        .constraints([
            Constraint::Length(3), // Enabled
            Constraint::Length(3), // Backup on Save
            Constraint::Length(3), // Backup on Quit
            Constraint::Length(3), // Auto Push
            Constraint::Length(3), // Remote URL
            Constraint::Length(3), // Remote Name
            Constraint::Length(1), // Spacer
            Constraint::Length(3), // Save button
            Constraint::Min(0),
        ])
        .split(inner_content);

    // Toggle fields
    let toggle_fields = [
        (chunks[0], SettingsField::Enabled),
        (chunks[1], SettingsField::BackupOnSave),
        (chunks[2], SettingsField::BackupOnQuit),
        (chunks[3], SettingsField::AutoPush),
    ];
    for (field_area, field) in toggle_fields {
        if crate::events::contains_cell(field_area, col, row) {
            return HoverKey::BackupField(field);
        }
    }

    // Text fields
    let text_fields = [
        (chunks[4], SettingsField::RemoteUrl),
        (chunks[5], SettingsField::RemoteName),
    ];
    for (field_area, field) in text_fields {
        if crate::events::contains_cell(field_area, col, row) {
            return HoverKey::BackupField(field);
        }
    }

    // Save button
    if crate::events::contains_cell(chunks[7], col, row) {
        return HoverKey::BackupSaveButton;
    }

    HoverKey::None
}

// ── ContentTree view ───────────────────────────────────────────────────────

fn compute_tree_hover_key(app: &App, area: Rect) -> HoverKey {
    let Some(ref state) = app.content_tree_state else {
        return HoverKey::None;
    };
    let Some((col, row)) = state.mouse_pos else {
        return HoverKey::None;
    };

    // Replicate layout from draw_content_tree
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);
    let main_area = chunks[0];

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(45, 100), // Left: Tree
            Constraint::Length(1),      // Separator
            Constraint::Min(0),         // Right: Full Content
        ])
        .split(main_area);

    let left_area = content_chunks[0];

    // Row hit-test via list_index_at
    let visible = state.visible_indices();
    let item_count = visible.len();
    if item_count == 0 || left_area.width == 0 || left_area.height == 0 {
        return HoverKey::None;
    }

    if col < left_area.x || col >= left_area.x + left_area.width {
        return HoverKey::None;
    }
    if row < left_area.y || row >= left_area.y + left_area.height {
        return HoverKey::None;
    }

    if let Some(idx) = crate::ui::list_index_at(
        row, left_area.y, 1,
        state.tree_scroll_offset,
        item_count,
    ) {
        return HoverKey::TreeRow(idx);
    }

    HoverKey::None
}

// ── Edit view ──────────────────────────────────────────────────────────────

fn compute_edit_hover_key(app: &App, _area: Rect) -> HoverKey {
    let Some((col, row)) = app.mouse_pos else {
        return HoverKey::None;
    };

    let list_area = app.editor.sidebar_list_rect;
    if list_area.width == 0 || list_area.height == 0 {
        return HoverKey::None;
    }

    if col < list_area.x || col >= list_area.x + list_area.width {
        return HoverKey::None;
    }
    if row < list_area.y || row >= list_area.y + list_area.height {
        return HoverKey::None;
    }

    // Compute item count based on sidebar mode
    let item_count = match app.editor.sidebar {
        EditSidebar::Outline => app.editor.outline_nodes.len(),
        EditSidebar::Links => app.editor.links.len(),
        EditSidebar::None => return HoverKey::None,
    };

    if item_count == 0 {
        return HoverKey::None;
    }

    if let Some(idx) = crate::ui::list_index_at(
        row, list_area.y, 1,
        app.editor.sidebar_scroll_offset,
        item_count,
    ) {
        return HoverKey::EditSidebarRow(idx);
    }

    HoverKey::None
}

// ── Help view ──────────────────────────────────────────────────────────────

fn compute_help_hover_key(app: &App, area: Rect) -> HoverKey {
    let Some((col, row)) = app.mouse_pos else {
        return HoverKey::None;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title bar
            Constraint::Min(8),
            Constraint::Length(1), // status bar
        ])
        .split(area);

    if row == chunks[0].y {
        let tabs = crate::ui::help_tabs(app.config.ui.icon_mode);
        let region = crate::ui::title_bar_tabs_region(chunks[0], "Help");
        if let Some(i) = crate::ui::hit_test_tabs(
            &tabs,
            chunks[0].x,
            chunks[0].width,
            region.x,
            col,
            app.config.ui.tab_icons_only,
            app.config.ui.icon_mode,
        ) {
            return HoverKey::HelpTab(i);
        }
    }

    HoverKey::None
}

// ── Setup view ─────────────────────────────────────────────────────────────

fn compute_setup_hover_key(app: &App, area: Rect) -> HoverKey {
    let Some((col, row)) = app.mouse_pos else {
        return HoverKey::None;
    };
    let Some(ref state) = app.setup_state else {
        return HoverKey::None;
    };

    // Replicate setup_layout
    let layout = crate::ui::setup::setup_layout(area);

    // Option rows
    if !state.is_done_selected()
        && col >= layout.options.x
        && col < layout.options.x + layout.options.width
        && row >= layout.options.y
        && row < layout.options.y + crate::setup::OPTION_ROWS as u16
    {
        let row_idx = (row - layout.options.y) as usize;
        return HoverKey::SetupRow(row_idx);
    }

    // Done button
    let btn_w = 14u16.min(layout.done.width);
    let btn_area = Rect::new(
        layout.done.x + (layout.done.width - btn_w) / 2,
        layout.done.y,
        btn_w,
        layout.done.height,
    );
    if !state.is_done_selected()
        && crate::events::contains_cell(btn_area, col, row)
    {
        return HoverKey::SetupDoneButton;
    }

    HoverKey::None
}

// ── Popup hover ────────────────────────────────────────────────────────────

fn compute_popup_hover_key(app: &App, area: Rect) -> HoverKey {
    use crate::popups::ActivePopup;

    let Some((col, row)) = app.mouse_pos else {
        return HoverKey::None;
    };

    let Some(ref active) = app.popups.active else {
        return HoverKey::None;
    };

    match active {
        // List-style popups with paint_list_hover — fixed option counts
        ActivePopup::Sort(_) => {
            popup_list_hover_key(app, area, crate::ui::PopupSize::Medium, 4, 0)
        }
        ActivePopup::IconMode(_) => {
            popup_list_hover_key(app, area, crate::ui::PopupSize::Medium, 3, 0)
        }
        ActivePopup::HintBarStyle(_) => {
            popup_list_hover_key(app, area, crate::ui::PopupSize::Medium, 4, 0)
        }
        ActivePopup::KeybindPreset(_) => {
            popup_list_hover_key(app, area, crate::ui::PopupSize::Medium, 4, 0)
        }
        ActivePopup::CreateFormat(_) => {
            popup_list_hover_key(app, area, crate::ui::PopupSize::Medium, 4, 0)
        }
        ActivePopup::Template(p) => {
            let content = crate::ui::centered_rect(crate::ui::PopupSize::Large, area);
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(0)])
                .split(content);
            popup_list_hover_in_rect(col, row, chunks[1], p.filtered_templates.len(), p.scroll_offset)
        }
        ActivePopup::FolderPicker(p) => {
            let content = crate::ui::centered_rect(crate::ui::PopupSize::Large, area);
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(1)])
                .split(content);
            popup_list_hover_in_rect(col, row, chunks[1], p.filtered_folders.len(), p.scroll_offset)
        }
        ActivePopup::Theme(p) => {
            let content = crate::ui::centered_rect(crate::ui::PopupSize::Medium, area);
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(3), Constraint::Length(3)])
                .split(content);
            popup_list_hover_in_rect(col, row, chunks[0], p.themes.len(), 0)
        }
        ActivePopup::TrashView(p) => {
            let content = crate::ui::centered_rect(crate::ui::PopupSize::Large, area);
            popup_list_hover_in_rect(col, row, content, p.items.len(), p.scroll_offset)
        }
        ActivePopup::ContextMenu(p) => {
            let w = p.items.iter().map(|l| l.len() as u16).max().unwrap_or(0);
            let h = p.items.len() as u16;
            let menu_rect = Rect::new(p.x, p.y, w, h);
            if crate::events::contains_cell(menu_rect, col, row)
                && let Some(idx) = crate::ui::list_index_at(row, menu_rect.y, 1, 0, p.items.len())
            {
                return HoverKey::PopupListRow { idx };
            }
            HoverKey::None
        }
        // Search/NotesGrep — complex layout, skip for now
        ActivePopup::Search(_) => HoverKey::None,
        // Popups without list hover
        ActivePopup::Folder(_)
        | ActivePopup::NoteRename(_)
        | ActivePopup::CreateNote(_, _)
        | ActivePopup::Import(_)
        | ActivePopup::Subnotes(_)
        | ActivePopup::Goals(_)
        | ActivePopup::Info(_)
        | ActivePopup::Tag(_) => HoverKey::None,
    }
}

fn popup_list_hover_key(app: &App, area: Rect, size: crate::ui::PopupSize, item_count: usize, scroll_offset: usize) -> HoverKey {
    let Some((col, row)) = app.mouse_pos else {
        return HoverKey::None;
    };
    let content = crate::ui::centered_rect(size, area);
    popup_list_hover_in_rect(col, row, content, item_count, scroll_offset)
}

fn popup_list_hover_in_rect(col: u16, row: u16, content: Rect, item_count: usize, scroll_offset: usize) -> HoverKey {
    let inner = Rect {
        x: content.x + 1,
        y: content.y + 1,
        width: content.width.saturating_sub(2),
        height: content.height.saturating_sub(2),
    };
    if inner.width == 0 || inner.height == 0 || item_count == 0 {
        return HoverKey::None;
    }
    if col < inner.x || col >= inner.x + inner.width {
        return HoverKey::None;
    }
    if row < inner.y || row >= inner.y + inner.height {
        return HoverKey::None;
    }
    if let Some(idx) = crate::ui::list_index_at(row, inner.y, 1, scroll_offset, item_count) {
        return HoverKey::PopupListRow { idx };
    }
    HoverKey::None
}

// ── Command palette hover ──────────────────────────────────────────────────

fn compute_palette_hover_key(app: &App, area: Rect) -> HoverKey {
    let Some(ref palette) = app.command_palette else {
        return HoverKey::None;
    };
    let Some((col, row)) = app.mouse_pos else {
        return HoverKey::None;
    };

    // Palette is rendered as a centered popup — replicate layout
    let content = crate::ui::centered_rect(crate::ui::PopupSize::Large, area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // search input
            Constraint::Length(1), // tab bar
            Constraint::Min(0),    // results list
        ])
        .split(content);

    // Tab hover
    if row == chunks[1].y {
        let tabs: Vec<(&str, Option<&str>)> = crate::palette::palette_tabs(app.config.ui.icon_mode)
            .iter()
            .map(|(l, g, _)| (*l, Some(*g)))
            .collect();
        if let Some(i) = crate::ui::hit_test_tabs(
            &tabs,
            chunks[1].x,
            chunks[1].width,
            chunks[1].x,
            col,
            app.config.ui.tab_icons_only,
            app.config.ui.icon_mode,
        ) {
            return HoverKey::PaletteTab(i);
        }
    }

    // Command list hover (pitch=2 for two-line entries)
    let inner_y = chunks[2].y + 1;
    if !palette.items.is_empty()
        && row >= inner_y
        && col > chunks[2].x
        && col < chunks[2].x + chunks[2].width - 1
        && let Some(idx) = crate::ui::list_index_at(
            row, inner_y, 2,
            palette.state.offset(),
            palette.items.len(),
        )
    {
        return HoverKey::PaletteCmd(idx);
    }

    HoverKey::None
}
