//! clin-side adapter for the upstream `pinstar` library.
//!
//! Owns host state the lib deliberately doesn't: clin keybinds (clin
//! keybinds win — keys resolve to `CanvasAction` here and map 1:1 onto
//! `pinstar::PinstarAction`), the sequence matcher, the statusline footer,
//! rename keybind hints, the image file dialog, the system clipboard and the
//! per-vault orthogonal-connections preference. Canvas engine state lives in
//! [`pinstar::PinstarState`].

use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;

use pinstar::image::DecodedImage;
use pinstar::{
    ActionCtx, PinstarAction, PinstarState, Settings, ThemeColors, apply_action, draw_pinstar_view,
    execute_menu_action, handle_pinstar_mouse,
};

use crate::app::{App, HelpTab};
use crate::keybinds::{CanvasAction, KeyMatcher, Keybinds, MatchOutcome};
use crate::overlay::{OverlayResult, OverlayView};

/// Status strings owned by the canvas modes; cleared when the mode ends so
/// temporary statuses from other sources survive.
const MODE_MESSAGES: &[&str] = &[
    "CONNECTION MODE: Select target node with mouse or Enter",
    "DELETE CONNECTION MODE: Select target node to remove link",
    "RESIZE MODE: Drag mouse to resize, Right-click to confirm",
];

fn sync_mode_status(app: &mut App, state: &PinstarState) {
    if let Some(msg) = state.active_mode_message() {
        app.status = std::borrow::Cow::Borrowed(msg);
        app.status_until = None;
    } else if MODE_MESSAGES.contains(&app.status.as_ref()) {
        // Clear a stale mode message only; leave temporary statuses alone.
        app.set_default_status();
    }
}

/// Map a clin `CanvasAction` onto the lib action enum (identical variant
/// names on the shared subset).
fn to_lib_action(action: CanvasAction) -> PinstarAction {
    match action {
        CanvasAction::Quit => PinstarAction::Quit,
        CanvasAction::Undo => PinstarAction::Undo,
        CanvasAction::Redo => PinstarAction::Redo,
        CanvasAction::Save => PinstarAction::Save,
        CanvasAction::ZoomFineIn => PinstarAction::ZoomFineIn,
        CanvasAction::ZoomFineOut => PinstarAction::ZoomFineOut,
        CanvasAction::ZoomIn => PinstarAction::ZoomIn,
        CanvasAction::ZoomOut => PinstarAction::ZoomOut,
        CanvasAction::MoveLeft => PinstarAction::MoveLeft,
        CanvasAction::MoveRight => PinstarAction::MoveRight,
        CanvasAction::MoveUp => PinstarAction::MoveUp,
        CanvasAction::MoveDown => PinstarAction::MoveDown,
        CanvasAction::EditOrConnect => PinstarAction::EditOrConnect,
        CanvasAction::OpenContextMenu => PinstarAction::OpenContextMenu,
        CanvasAction::MenuUp => PinstarAction::MenuUp,
        CanvasAction::MenuDown => PinstarAction::MenuDown,
        CanvasAction::MenuSelect => PinstarAction::MenuSelect,
        CanvasAction::MenuClose => PinstarAction::MenuClose,
        CanvasAction::CreateConnection => PinstarAction::CreateConnection,
        CanvasAction::DeleteConnection => PinstarAction::DeleteConnection,
        CanvasAction::RenameNode => PinstarAction::RenameNode,
        CanvasAction::ResizeMode => PinstarAction::ResizeMode,
        CanvasAction::SetColor => PinstarAction::SetColor,
        CanvasAction::DeleteNode => PinstarAction::DeleteNode,
        CanvasAction::DeleteAllConnections => PinstarAction::DeleteAllConnections,
        CanvasAction::AddTextNode => PinstarAction::AddTextNode,
        CanvasAction::AddGroup => PinstarAction::AddGroup,
        CanvasAction::AddImageNode => PinstarAction::AddImageNode,
        CanvasAction::ToggleGrid => PinstarAction::ToggleGrid,
        CanvasAction::ToggleOrthogonal => PinstarAction::ToggleOrthogonal,
        CanvasAction::ToggleEditorPane => PinstarAction::ToggleEditorPane,
        CanvasAction::CycleFocus => PinstarAction::CycleFocus,
        CanvasAction::Help => PinstarAction::Help,
        CanvasAction::RenameConfirm => PinstarAction::RenameConfirm,
        CanvasAction::RenameCancel => PinstarAction::RenameCancel,
        CanvasAction::ConfirmResize => PinstarAction::ConfirmResize,
        CanvasAction::CancelResize => PinstarAction::CancelResize,
        CanvasAction::EditorUnfocus => PinstarAction::EditorUnfocus,
        CanvasAction::CloseEditor => PinstarAction::CloseEditor,
        CanvasAction::CloseEditorAlt => PinstarAction::CloseEditorAlt,
    }
}

pub struct PinstarPlugin {
    pub state: PinstarState,
    pub(crate) current_file: String,
    pub keybinds: Keybinds,
    pub seq_matcher: KeyMatcher,
    image_rx: Option<std::sync::mpsc::Receiver<anyhow::Result<DecodedImage>>>,
    pub last_area: Rect,
    base_host_raw: String,
    next_host_check: std::time::Instant,
    base_renderers: std::collections::HashMap<String, crate::markdown::MarkdownRenderer>,
}

impl PinstarPlugin {
    pub(crate) fn palette_input_available(&self) -> bool {
        let s = &self.state;
        s.context_menu.is_none()
            && s.rename_popup.is_none()
            && s.floating_editor.is_none()
            && !s.editor_focus
            && s.connection_source_id.is_none()
            && s.deleting_connection_source_id.is_none()
            && s.resizing_node_id.is_none()
            && !s.is_dragging_resize_handle
            && s.drag_start_pos.is_none()
            && !s.is_panning
            && !s.mouse_selecting
            && s.select_rect_start.is_none()
            && s.right_down_screen.is_none()
    }

    pub(crate) fn save_for_navigation(&mut self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.palette_input_available(),
            "Finish the Canvas interaction before leaving"
        );
        self.state.save()?;
        self.base_host_raw = self.state.raw_editor.lines().join("\n");
        Ok(())
    }
    pub fn new(
        path: &std::path::Path,
        config: &crate::config::ClinConfig,
        keybinds: Keybinds,
        seq_matcher: KeyMatcher,
        picker: Option<ratatui_image::picker::Picker>,
        storage_data_dir: &std::path::Path,
    ) -> anyhow::Result<Self> {
        let storage = crate::storage::Storage {
            notes_dir: path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| std::path::Path::new("."))
                .to_owned(),
            data_dir: storage_data_dir.to_owned(),
            config_dir: std::path::PathBuf::new(),
            templates_dir: std::path::PathBuf::new(),
            skip_dir_patterns: Vec::new(),
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        };
        Self::new_in_storage(path, config, keybinds, seq_matcher, picker, &storage)
    }

    pub(crate) fn new_in_storage(
        path: &std::path::Path,
        config: &crate::config::ClinConfig,
        keybinds: Keybinds,
        seq_matcher: KeyMatcher,
        picker: Option<ratatui_image::picker::Picker>,
        storage: &crate::storage::Storage,
    ) -> anyhow::Result<Self> {
        let root = storage.notes_dir.canonicalize()?;
        let path = path.canonicalize()?;
        let target = path
            .strip_prefix(root)?
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("invalid Canvas path"))?
            .replace('\\', "/");
        let expected_path = storage.document_path(&target)?;
        let mut state = PinstarState::load(&expected_path)?;
        let current_file = target.clone();
        let writer = storage.clone();
        state.set_save_handler(move |path, before, after| {
            anyhow::ensure!(
                path == expected_path,
                "Canvas path changed; reopen before saving"
            );
            writer.replace_document(&target, before, after)
        });
        state.settings = Settings {
            enable_image_nodes: true,
            image_cache_size: config.image.cache_size,
            rename_uses_id: false,
            show_hints: false,
        };
        state.image_picker = picker;

        let image_rx = {
            let (tx, rx) = pinstar::image::spawn_worker();
            state.image_decode_tx = Some(tx);
            Some(rx)
        };

        // Load per-vault orthogonal preference.
        if let Ok(vault_id) = crate::local_state::vault_identity_path(&storage.data_dir) {
            let vault_key = vault_id.to_string_lossy().into_owned();
            if let Ok(paths) = crate::paths::AppPaths::discover(
                crate::config::ClinConfig::config_path().unwrap_or_default(),
            ) && let Ok(st) = crate::local_state::LocalState::load(&paths.state_path())
                && let Some(vs) = st.vaults.get(&vault_key)
            {
                state.orthogonal_connections = vs.canvas_orthogonal;
            }
        }

        Ok(Self {
            current_file,
            base_host_raw: state.raw_editor.lines().join("\n"),
            next_host_check: std::time::Instant::now(),
            state,
            keybinds,
            seq_matcher,
            image_rx,
            last_area: Rect::default(),
            base_renderers: std::collections::HashMap::new(),
        })
    }

    pub(crate) fn poll_images(&mut self) -> bool {
        let Some(rx) = &self.image_rx else {
            return false;
        };
        let mut changed = false;
        while let Ok(result) = rx.try_recv() {
            changed = true;
            if let (Ok(img), Some(picker)) = (result, self.state.image_picker.as_ref()) {
                self.state.image_cache.install_decoded(img, picker);
            }
        }
        changed
    }

    fn draw_base_embeds(&mut self, frame: &mut Frame, area: Rect, app: &App) {
        if self.state.context_menu.is_some()
            || self.state.rename_popup.is_some()
            || self.state.floating_editor.is_some()
        {
            return;
        }
        let area = if self.state.show_editor_pane {
            ratatui::layout::Layout::horizontal([
                ratatui::layout::Constraint::Percentage(30),
                ratatui::layout::Constraint::Percentage(70),
            ])
            .split(area)[1]
        } else {
            area
        };
        let host = Some(self.current_file.clone());
        let mut visible = std::collections::HashSet::new();
        for node in &self.state.data.nodes {
            let content = match node {
                pinstar::data::CanvasNode::Text(node)
                    if node.text.contains(".base]]")
                        || node.text.contains(".base#")
                        || node.text.contains("```base") =>
                {
                    node.text.clone()
                }
                pinstar::data::CanvasNode::File(node) if node.file.ends_with(".base") => format!(
                    "![[{}{}]]",
                    node.file,
                    node.subpath.as_deref().unwrap_or("")
                ),
                _ => continue,
            };
            let (x, y) = node.pos();
            let (width, height) = node.size();
            let z = self.state.zoom;
            let x =
                (x - self.state.viewport_x) * z + f64::from(area.x) + f64::from(area.width) / 2.0;
            let y =
                (y - self.state.viewport_y) * z + f64::from(area.y) + f64::from(area.height) / 2.0;
            let left = x.max(f64::from(area.x));
            let top = y.max(f64::from(area.y));
            let right = (x + width * z).min(f64::from(area.right()));
            let bottom = (y + height * z).min(f64::from(area.bottom().saturating_sub(1)));
            if right <= left + 2.0 || bottom <= top + 2.0 {
                continue;
            }
            if visible.len() >= 32 {
                frame.render_widget(
                    ratatui::widgets::Paragraph::new("Bases: at most 32 live Canvas regions"),
                    Rect::new(
                        left as u16 + 1,
                        top as u16 + 1,
                        (right - left) as u16 - 2,
                        1,
                    ),
                );
                continue;
            }
            visible.insert(node.id().to_owned());
            let rect = Rect::new(
                left as u16 + 1,
                top as u16 + 1,
                (right - left) as u16 - 2,
                (bottom - top) as u16 - 2,
            );
            let renderer = self.base_renderers.entry(node.id().to_owned()).or_default();
            renderer.base_host.clone_from(&host);
            renderer.base_canvas_node =
                matches!(node, pinstar::data::CanvasNode::Text(_)).then(|| node.id().to_owned());
            renderer.base_body_only = false;
            let opts = crate::markdown::MdRenderOpts::from_config(&app.config, host.as_deref());
            renderer.render_with(
                &content,
                rect.width,
                &app.app_theme,
                &opts,
                crate::markdown::RenderViewport {
                    start: 0,
                    height: usize::from(rect.height),
                },
            );
            renderer.poll();
            app.base_embeds
                .draw(frame, rect, renderer, 0..usize::from(rect.height), app);
        }
        self.base_renderers.retain(|id, _| visible.contains(id));
    }

    pub(crate) fn base_host_has_pending_changes(&self) -> bool {
        self.state.last_save_failed()
            || self.state.editor_focus
            || self.state.floating_editor.is_some()
            || self.state.rename_popup.is_some()
            || self.state.drag_start_pos.is_some()
            || self.state.resizing_node_id.is_some()
            || !self
                .state
                .raw_editor
                .lines()
                .iter()
                .map(String::as_str)
                .eq(self.base_host_raw.split('\n'))
    }
    pub(crate) fn reload_base_host(&mut self) -> anyhow::Result<()> {
        self.state.reload()?;
        self.base_host_raw = self.state.raw_editor.lines().join("\n");
        // Canvas snapshots predate the guarded host write; Bases owns its checked undo/redo.
        self.state.undo_stack.clear();
        self.state.redo_stack.clear();
        Ok(())
    }
    pub(crate) fn poll_base_renderers(&mut self) -> anyhow::Result<bool> {
        let mut dirty = self
            .base_renderers
            .values_mut()
            .fold(false, |dirty, renderer| renderer.poll() | dirty);
        let now = std::time::Instant::now();
        let check_host = now >= self.next_host_check;
        if check_host {
            self.next_host_check = now + std::time::Duration::from_millis(200);
        }
        if check_host
            && self.state.format == pinstar::formats::SupportedFormat::Canvas
            && !self.base_host_has_pending_changes()
            && std::fs::metadata(&self.state.path)
                .and_then(|metadata| metadata.modified())
                .is_ok_and(|modified| modified != self.state.last_modified)
            && serde_json::from_str::<serde_json::Value>(&self.state.raw_editor.lines().join("\n"))
                .is_ok()
        {
            self.reload_base_host()?;
            dirty = true;
        }
        Ok(dirty)
    }

    /// Persist the orthogonal-connections preference per vault.
    fn persist_orthogonal(&self, app: &App) {
        if let Ok(vault_id) = crate::local_state::vault_identity_path(&app.storage.data_dir) {
            let vault_key = vault_id.to_string_lossy().into_owned();
            if let Ok(paths) = crate::paths::AppPaths::discover(
                crate::config::ClinConfig::config_path().unwrap_or_default(),
            ) {
                let value = self.state.orthogonal_connections;
                let _ = crate::local_state::LocalState::update(&paths.state_path(), |s| {
                    s.vaults
                        .entry(vault_key.clone())
                        .or_default()
                        .canvas_orthogonal = value;
                    Ok(())
                });
            }
        }
    }

    /// Dispatch a resolved clin action through the lib. Returns the overlay
    /// outcome (Exit only for Quit).
    fn dispatch_action(
        &mut self,
        action: CanvasAction,
        count: usize,
        app: &mut App,
    ) -> OverlayResult {
        let area = self.last_area;
        let outcome = apply_action(
            &mut self.state,
            to_lib_action(action),
            &ActionCtx { area, count },
        );

        if let Some(notice) = outcome.notice {
            app.set_temporary_status(notice);
        }

        match outcome.host_action {
            Some(PinstarAction::Quit) => match self.save_for_navigation() {
                Ok(()) => OverlayResult::Exit,
                Err(error) => {
                    app.set_temporary_status(&format!("Canvas save failed: {error:#}"));
                    OverlayResult::Continue
                }
            },
            Some(PinstarAction::Save) => {
                if let Err(error) = self.state.save() {
                    app.set_temporary_status(&format!("Canvas save failed: {error:#}"));
                }
                OverlayResult::Continue
            }
            Some(PinstarAction::Help) => OverlayResult::OpenHelp(HelpTab::Canvas),
            Some(PinstarAction::AddImageNode) => {
                let (x, y) = (self.state.viewport_x, self.state.viewport_y);
                if let Ok(Some(path)) = crate::ui::pick_file("Image", "png;jpg;jpeg;gif;webp;bmp") {
                    self.state
                        .add_image_node_with(std::path::PathBuf::from(path), x, y);
                    self.state.sync_to_raw_editor();
                }
                OverlayResult::Continue
            }
            Some(PinstarAction::ToggleOrthogonal) => {
                self.persist_orthogonal(app);
                OverlayResult::Continue
            }
            _ => OverlayResult::Continue,
        }
    }
}

impl OverlayView for PinstarPlugin {
    fn overlay_render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        self.last_area = area;

        let theme = app.app_theme.clone();
        let mouse_pos = self.state.mouse_pos;
        draw_pinstar_view(
            frame,
            &mut self.state,
            &theme_colors(&theme),
            area,
            mouse_pos,
        );

        self.draw_base_embeds(frame, area, app);

        // ── rename popup keybind hints (below the lib-drawn popup) ──
        if self.state.rename_popup.is_some()
            && let Some(popup) = self.state.rename_popup_rect
        {
            let hints_area = Rect::new(
                popup.x.saturating_sub(1),
                (popup.bottom() + 1).min(area.bottom().saturating_sub(1)),
                popup.width.saturating_add(2).min(area.width),
                1,
            );
            if hints_area.y < area.bottom() {
                let hints_items = [
                    (
                        self.keybinds.display_canvas(CanvasAction::RenameConfirm),
                        "confirm",
                    ),
                    (
                        self.keybinds.display_canvas(CanvasAction::RenameCancel),
                        "cancel",
                    ),
                ];
                let hint = crate::ui::popup_footer_hints(
                    &theme,
                    crate::ui::PopupHints::Keybinds(&hints_items),
                );
                crate::ui::draw_popup_footer(frame, hints_area, &theme, &hint);
            }
        }
    }

    fn overlay_handle_event(
        &mut self,
        event: Event,
        app: &mut App,
        term_area: Rect,
    ) -> anyhow::Result<OverlayResult> {
        let area = if term_area == Rect::default() {
            self.last_area
        } else {
            crate::ui::chrome::rows(term_area)[1]
        };
        let modified = self.state.last_modified;
        let outcome = match event {
            Event::Key(key) => {
                let result = self.handle_key(key, app, area);
                sync_mode_status(app, &self.state);
                Ok(result)
            }
            Event::Mouse(mouse) => {
                self.state.mouse_pos = Some((mouse.column, mouse.row));
                let outcome = handle_pinstar_mouse(&mut self.state, mouse, area);
                if let Some(clipboard) = outcome.clipboard {
                    let notice = crate::text_edit::clipboard_notice(
                        crate::text_edit::write_system_clipboard(&clipboard),
                    );
                    app.set_temporary_status(notice);
                } else if let Some(notice) = outcome.notice {
                    app.set_temporary_status(notice);
                }
                if self.state.trigger_image_picker {
                    self.state.trigger_image_picker = false;
                    let (x, y) = (self.state.context_menu_pos.0, self.state.context_menu_pos.1);
                    if let Ok(Some(path)) =
                        crate::ui::pick_file("Image", "png;jpg;jpeg;gif;webp;bmp")
                    {
                        self.state
                            .add_image_node_with(std::path::PathBuf::from(path), x, y);
                        self.state.sync_to_raw_editor();
                    }
                }
                sync_mode_status(app, &self.state);
                Ok(OverlayResult::Continue)
            }
            _ => Ok(OverlayResult::Continue),
        };
        if self.state.last_modified != modified {
            self.base_host_raw = self.state.raw_editor.lines().join("\n");
        }
        if let Some(error) = self.state.take_save_error() {
            app.set_temporary_status(&format!("Canvas save failed; edits retained: {error}"));
        }
        outcome
    }
}

impl PinstarPlugin {
    fn handle_key(&mut self, key: KeyEvent, app: &mut App, area: Rect) -> OverlayResult {
        let keybinds = self.keybinds.clone();

        // Rename popup: confirm/cancel/feed keys.
        if self.state.rename_popup.is_some() {
            self.seq_matcher.clear();
            let Some(textarea) = self.state.rename_popup.as_mut() else {
                return OverlayResult::Continue;
            };
            if keybinds.matches_canvas(CanvasAction::RenameCancel, &key) {
                self.state.rename_popup = None;
            } else if keybinds.matches_canvas(CanvasAction::RenameConfirm, &key) {
                let new_title = textarea.lines().join("");
                self.state.rename_node_title(new_title);
                self.state.rename_popup = None;
            } else {
                crate::text_edit::feed_key(&keybinds, textarea, key);
            }
            return OverlayResult::Continue;
        }

        // Context menu: keys drive the menu exclusively.
        let mut menu_action: Option<(pinstar::PinstarMenuType, String, u16, u16)> = None;
        let mut close_menu = false;

        if let Some(menu) = self.state.context_menu.as_mut() {
            self.seq_matcher.clear();
            if keybinds.matches_canvas(CanvasAction::MenuClose, &key) {
                close_menu = true;
            } else if keybinds.matches_canvas(CanvasAction::MenuUp, &key) {
                menu.move_up();
            } else if keybinds.matches_canvas(CanvasAction::MenuDown, &key) {
                menu.move_down();
            } else if keybinds.matches_canvas(CanvasAction::MenuSelect, &key) {
                menu_action = menu
                    .label(menu.selected)
                    .map(|l| (menu.menu_type, l.to_string(), menu.x, menu.y));
                close_menu = true;
            } else if let KeyCode::Char(c) = key.code
                && let Some(idx) = menu.find_shortcut(c)
            {
                menu_action = menu
                    .label(idx)
                    .map(|l| (menu.menu_type, l.to_string(), menu.x, menu.y));
                close_menu = true;
            }
        }

        if close_menu {
            self.state.context_menu = None;
        }

        if let Some((menu_type, label, mx, my)) = menu_action {
            let outcome = execute_menu_action(&mut self.state, &label, menu_type, mx, my);
            if let Some(notice) = outcome.notice {
                app.set_temporary_status(notice);
            }
            if self.state.trigger_image_picker {
                self.state.trigger_image_picker = false;
                let (x, y) = (self.state.context_menu_pos.0, self.state.context_menu_pos.1);
                if let Ok(Some(path)) = crate::ui::pick_file("Image", "png;jpg;jpeg;gif;webp;bmp") {
                    self.state
                        .add_image_node_with(std::path::PathBuf::from(path), x, y);
                    self.state.sync_to_raw_editor();
                }
            }
            return OverlayResult::Continue;
        } else if close_menu {
            return OverlayResult::Continue;
        }

        if self.state.context_menu.is_some() {
            return OverlayResult::Continue;
        }

        // Floating editor.
        if self.state.floating_editor.is_some() {
            self.seq_matcher.clear();
            if keybinds.matches_canvas(CanvasAction::CloseEditor, &key)
                || keybinds.matches_canvas(CanvasAction::CloseEditorAlt, &key)
            {
                self.state.toggle_editor();
                self.state.sync_to_raw_editor();
            } else if let Some(editor) = self.state.floating_editor.as_mut() {
                crate::text_edit::feed_key(&keybinds, editor, key);
                if let Some(node_id) = self.state.selection.primary.clone() {
                    let text = editor.lines().join("\n");
                    for node in &mut self.state.data.nodes {
                        if node.id() == node_id {
                            node.set_text(text);
                            break;
                        }
                    }
                    let _ = self.state.save();
                }
            }
            return OverlayResult::Continue;
        }

        // Resize mode.
        if self.state.resizing_node_id.is_some() {
            self.seq_matcher.clear();
            if keybinds.matches_canvas(CanvasAction::ConfirmResize, &key)
                || keybinds.matches_canvas(CanvasAction::CancelResize, &key)
            {
                self.state.resizing_node_id = None;
                self.state.is_dragging_resize_handle = false;
                let _ = self.state.save();
                return OverlayResult::Continue;
            }
        }

        // Raw editor focus.
        if self.state.editor_focus {
            self.seq_matcher.clear();
            if keybinds.matches_canvas(CanvasAction::EditorUnfocus, &key) {
                self.state.editor_focus = false;
            } else {
                crate::text_edit::feed_key(&keybinds, &mut self.state.raw_editor, key);
                let _ = self.state.sync_from_raw_editor();
            }
            return OverlayResult::Continue;
        }

        // Edge-list overlay shortcuts: digits 1..n select the corresponding
        // edge connected to the selected node.
        if let KeyCode::Char(c) = key.code
            && c.is_ascii_digit()
            && self.state.connection_source_id.is_none()
            && self.state.deleting_connection_source_id.is_none()
            && self.state.resizing_node_id.is_none()
        {
            let idx = (c as u8 - b'0') as usize;
            if self.state.select_edge_of_selected_node(idx).is_some() {
                self.state.open_edge_menu_centered(area);
                return OverlayResult::Continue;
            }
        }

        // Connection / delete-connection mode: Enter/i completes the
        // operation on the selected node. Checked explicitly because Enter
        // also binds to RenameConfirm/MenuSelect/ConfirmResize, making
        // resolve_canvas non-deterministic for Enter.
        if keybinds.matches_canvas(CanvasAction::EditOrConnect, &key) {
            if self.state.connection_source_id.is_some() {
                if let Some(target_id) = self.state.selection.primary.clone() {
                    self.state.finish_connection(&target_id);
                }
                return OverlayResult::Continue;
            }
            if self.state.deleting_connection_source_id.is_some() {
                if let Some(target_id) = self.state.selection.primary.clone() {
                    self.state.finish_delete_connection(&target_id);
                }
                return OverlayResult::Continue;
            }
        }

        let config = app.config.clone();
        let seq = config.sequences_enabled();
        let counts = config.counts_enabled();
        match self
            .keybinds
            .resolve_canvas(&mut self.seq_matcher, key, seq, counts)
        {
            MatchOutcome::Matched(action, count) => {
                let n = count.unwrap_or(1).max(1) as usize;
                // Esc binds to Quit and several cancel actions; HashMap
                // iteration order is nondeterministic, so re-check Quit
                // explicitly (port of the original input.rs catch-all).
                let action = if keybinds.matches_canvas(CanvasAction::Quit, &key) {
                    CanvasAction::Quit
                } else {
                    action
                };
                if let Some(id) = action.palette_action_id() {
                    app.record_command_usage(id);
                }
                self.dispatch_action(action, n, app)
            }
            MatchOutcome::Pending => OverlayResult::Continue,
            MatchOutcome::NoMatch => OverlayResult::Continue,
        }
    }
}

/// Map the clin app theme onto the lib theme colors field-by-field.
pub fn theme_colors(theme: &crate::app_theme::AppThemeColors) -> ThemeColors {
    ThemeColors {
        accent: theme.accent,
        heading: theme.heading,
        success: theme.success,
        warning: theme.warning,
        destructive: theme.destructive,
        muted: theme.muted,
        text: theme.text,
        fg: theme.fg,
        bg: theme.bg.unwrap_or(ratatui::style::Color::Reset),
        border: theme.border,
        tag: theme.tag,
        folder: theme.folder,
        highlight_fg: theme.highlight_fg,
        highlight_bg: theme.highlight_bg,
        selection_indicator: Some(theme.selection_indicator),
    }
}
