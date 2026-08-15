use crate::pinstar::data::CanvasData;
use crate::ui::CanvasSelection;
use anyhow::Result;
use ratatui_textarea::{TextArea, WrapMode};
use std::path::{Path, PathBuf};

pub struct PinstarState {
    pub path: PathBuf,
    pub data: CanvasData,
    pub viewport_x: f64,
    pub viewport_y: f64,
    pub zoom: f64,
    pub selection: CanvasSelection<String>,
    pub selected_edge_id: Option<String>,
    pub floating_editor: Option<TextArea<'static>>,
    pub raw_editor: TextArea<'static>,
    pub editor_focus: bool,
    pub last_mouse_pos: Option<(u16, u16)>,
    pub mouse_pos: Option<(u16, u16)>,
    pub last_click: Option<(u16, u16, std::time::Instant)>,
    pub context_menu: Option<crate::ui::CanvasContextMenu>,
    pub context_menu_pos: (f64, f64),
    pub menu_kind: Option<PinstarMenuType>,
    pub connection_source_id: Option<String>,
    pub resizing_node_id: Option<String>,
    pub is_dragging_resize_handle: bool,
    pub deleting_connection_source_id: Option<String>,
    pub show_editor_pane: bool,
    pub drag_start_pos: Option<(f64, f64)>,
    pub rename_popup: Option<TextArea<'static>>,
    pub last_mouse_canvas_pos: Option<(f64, f64)>,
    pub drag_captured_nodes: std::collections::HashSet<String>,
    pub(crate) mouse_selection: crate::text_edit::MouseTextSelection,
    pub(crate) text_selection_target: Option<PinstarTextField>,
    pub(crate) floating_editor_rect: Option<ratatui::layout::Rect>,
    pub edge_overlay_rect: Option<ratatui::layout::Rect>,
    pub grid: crate::ui::CanvasGridState,
    pub help_requested: bool,
    pub footer_hint: String,
    pub keybinds: crate::keybinds::Keybinds,
    pub seq_matcher: crate::keybinds::KeyMatcher,
    pub last_area: ratatui::layout::Rect,
    pub image_cache: crate::image_render::cache::ImageCache,
    pub image_picker: Option<ratatui_image::picker::Picker>,
    pub image_decode_tx: Option<std::sync::mpsc::Sender<crate::image_render::worker::ImageJob>>,
    pub is_panning: bool,
    pub undo_stack: Vec<PinstarSnapshot>,
    pub redo_stack: Vec<PinstarSnapshot>,
    pub marquee: crate::ui::MarqueeDragState,
    pub right_down_screen: Option<(u16, u16)>,
    pub last_zoom_at: Option<std::time::Instant>,
    pub orthogonal_connections: bool,
}

#[derive(Clone)]
pub struct PinstarSnapshot {
    pub data: CanvasData,
}

#[derive(Clone, Copy)]
pub struct EdgeSegments(pub [(f64, f64, f64, f64); 3], pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PinstarTextField {
    Raw,
    Floating,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PinstarMenuType {
    Canvas,
    ColorPicker,
    EdgeMenu,
    EdgeColorPicker,
    EdgeStylePicker,
}

/// Returns the single-letter keyboard shortcut for a context-menu item, if any.
/// Single source of truth shared by render (hint display) and input (key matching).
pub fn menu_item_shortcut_char(menu_type: PinstarMenuType, item: &str) -> Option<char> {
    match menu_type {
        PinstarMenuType::Canvas => match item {
            "Create Connection" => Some('c'),
            "Delete Connection" => Some('d'),
            "Rename Node" => Some('r'),
            "Resize Node" => Some('s'),
            "Set Color..." => Some('o'),
            "Delete All Connections" => Some('b'),
            "Delete Node" => Some('x'),
            "Add Text Node" => Some('t'),
            "Add Group" => Some('g'),
            "Add Image Node" => Some('m'),
            _ => None,
        },
        PinstarMenuType::EdgeMenu => match item {
            "Set Color..." => Some('o'),
            "Set Style..." => Some('s'),
            _ => None,
        },
        PinstarMenuType::ColorPicker | PinstarMenuType::EdgeColorPicker => match item {
            "Default" => Some('d'),
            "Red" => Some('r'),
            "Orange" => Some('o'),
            "Yellow" => Some('y'),
            "Green" => Some('g'),
            "Cyan" => Some('c'),
            "Purple" => Some('p'),
            "Blue" => Some('b'),
            "Magenta" => Some('m'),
            "White" => Some('w'),
            _ => None,
        },
        PinstarMenuType::EdgeStylePicker => match item {
            "Solid" => Some('s'),
            "Dashed" => Some('d'),
            "Dotted" => Some('t'),
            _ => None,
        },
    }
}

pub fn pinstar_menu_specs(
    kind: PinstarMenuType,
    selected_node: bool,
) -> Vec<crate::ui::CanvasMenuItemSpec> {
    let items = match kind {
        PinstarMenuType::Canvas => {
            if selected_node {
                vec![
                    "Create Connection",
                    "Delete Connection",
                    "Rename Node",
                    "Resize Node",
                    "Set Color...",
                    "Delete All Connections",
                    "Delete Node",
                ]
            } else {
                vec!["Add Text Node", "Add Group", "Add Image Node"]
            }
        }
        PinstarMenuType::EdgeMenu => vec!["Set Color...", "Set Style..."],
        PinstarMenuType::EdgeStylePicker => vec!["Solid", "Dashed", "Dotted"],
        PinstarMenuType::ColorPicker | PinstarMenuType::EdgeColorPicker => {
            let mut specs = vec![crate::ui::CanvasMenuItemSpec::new("Default").shortcut('d')];
            for (name, _, color) in crate::pinstar::COLOR_PICKER_PALETTE {
                let mut spec = crate::ui::CanvasMenuItemSpec::new(name).color(*color);
                if let Some(c) = menu_item_shortcut_char(kind, name) {
                    spec = spec.shortcut(c);
                }
                specs.push(spec);
            }
            return specs;
        }
    };

    let mut specs = Vec::new();
    for label in items {
        let mut spec = crate::ui::CanvasMenuItemSpec::new(label);
        if let Some(c) = menu_item_shortcut_char(kind, label) {
            spec = spec.shortcut(c);
        }
        specs.push(spec);
    }
    specs
}

impl PinstarState {
    pub fn load(
        path: &Path,
        keybinds: crate::keybinds::Keybinds,
        seq_matcher: crate::keybinds::KeyMatcher,
    ) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let data: CanvasData = serde_json::from_str(&content)?;
        Ok(Self {
            path: path.to_path_buf(),
            data,
            viewport_x: 0.0,
            viewport_y: 0.0,
            zoom: 0.1,
            selection: CanvasSelection::new(),
            selected_edge_id: None,
            floating_editor: None,
            raw_editor: TextArea::from(content.lines().map(String::from).collect::<Vec<_>>()),
            editor_focus: false,
            mouse_pos: None,
            last_mouse_pos: None,
            last_click: None,
            context_menu: None,
            context_menu_pos: (0.0, 0.0),
            menu_kind: None,
            connection_source_id: None,
            resizing_node_id: None,
            is_dragging_resize_handle: false,
            deleting_connection_source_id: None,
            show_editor_pane: false,
            drag_start_pos: None,
            rename_popup: None,
            last_mouse_canvas_pos: None,
            drag_captured_nodes: std::collections::HashSet::new(),
            grid: crate::ui::CanvasGridState::default(),
            mouse_selection: crate::text_edit::MouseTextSelection::default(),
            text_selection_target: None,
            floating_editor_rect: None,
            edge_overlay_rect: None,
            help_requested: false,
            footer_hint: String::new(),
            keybinds,
            seq_matcher,
            last_area: ratatui::layout::Rect::default(),
            image_cache: crate::image_render::cache::ImageCache::new(32),
            image_picker: None,
            image_decode_tx: None,
            is_panning: false,
            last_zoom_at: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            marquee: crate::ui::MarqueeDragState::new(3),
            right_down_screen: None,
            orthogonal_connections: false,
        })
    }

    /// Returns the header-bar status message for the active transient mode
    /// (connection / delete-connection / resize), or None when idle.
    pub fn active_mode_message(&self) -> Option<&'static str> {
        if self.connection_source_id.is_some() {
            Some("CONNECTION MODE: Select target node with mouse or Enter")
        } else if self.deleting_connection_source_id.is_some() {
            Some("DELETE CONNECTION MODE: Select target node to remove link")
        } else if self.resizing_node_id.is_some() {
            Some("RESIZE MODE: Drag mouse to resize, Left-click to confirm")
        } else {
            None
        }
    }

    pub fn record_undo_state(&mut self) {
        self.undo_stack.push(PinstarSnapshot {
            data: self.data.clone(),
        });
        if self.undo_stack.len() > 20 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }
    pub fn undo(&mut self) -> Result<()> {
        if let Some(snapshot) = self.undo_stack.pop() {
            self.redo_stack.push(PinstarSnapshot {
                data: self.data.clone(),
            });
            self.data = snapshot.data;
            self.selection.clear();
            self.selected_edge_id = None;
            self.save()?;
            self.sync_to_raw_editor();
        }
        Ok(())
    }
    pub fn redo(&mut self) -> Result<()> {
        if let Some(snapshot) = self.redo_stack.pop() {
            self.undo_stack.push(PinstarSnapshot {
                data: self.data.clone(),
            });
            self.data = snapshot.data;
            self.selection.clear();
            self.selected_edge_id = None;
            self.save()?;
            self.sync_to_raw_editor();
        }
        Ok(())
    }

    pub fn all_selected_node_ids(&self) -> std::collections::HashSet<String> {
        self.selection.all()
    }
    pub fn select_nodes_in_rect(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
        let (min_x, max_x) = if x1 < x2 { (x1, x2) } else { (x2, x1) };
        let (min_y, max_y) = if y1 < y2 { (y1, y2) } else { (y2, y1) };
        let ids: std::collections::HashSet<String> = self
            .data
            .nodes
            .iter()
            .filter(|n| {
                let (nx, ny) = n.pos();
                let (nw, nh) = n.size();
                nx + nw > min_x && nx < max_x && ny + nh > min_y && ny < max_y
            })
            .map(|n| n.id().to_string())
            .collect();
        let primary = ids.iter().next().cloned();
        self.selection.replace_set(ids, primary);
    }
    pub fn delete_selected_node(&mut self) {
        let ids = self.selection.all();
        if ids.is_empty() {
            return;
        }
        self.record_undo_state();
        self.data.nodes.retain(|n| !ids.contains(n.id()));
        self.data
            .edges
            .retain(|e| !ids.contains(&e.from_node) && !ids.contains(&e.to_node));
        self.selection.clear();
        let _ = self.save();
    }

    pub fn get_edge_segments(
        &self,
        edge: &crate::pinstar::data::CanvasEdge,
    ) -> Option<EdgeSegments> {
        let from_idx = self
            .data
            .nodes
            .iter()
            .position(|n| n.id() == edge.from_node)?;
        let to_idx = self
            .data
            .nodes
            .iter()
            .position(|n| n.id() == edge.to_node)?;
        let from = &self.data.nodes[from_idx];
        let to = &self.data.nodes[to_idx];
        let (fx, fy, fw, fh) = (from.pos().0, from.pos().1, from.size().0, from.size().1);
        let (tx, ty, tw, th) = (to.pos().0, to.pos().1, to.size().0, to.size().1);
        let scx = fx + fw / 2.0;
        let scy = fy + fh / 2.0;
        let tcx = tx + tw / 2.0;
        let tcy = ty + th / 2.0;
        let dx = tcx - scx;
        let dy = tcy - scy;

        let (ax, ay) = if dx.abs() > dy.abs() {
            if dx > 0.0 { (fx + fw, scy) } else { (fx, scy) }
        } else if dy > 0.0 {
            (scx, fy + fh)
        } else {
            (scx, fy)
        };
        let (bx, by) = if dx.abs() > dy.abs() {
            if dx > 0.0 { (tx, tcy) } else { (tx + tw, tcy) }
        } else if dy > 0.0 {
            (tcx, ty)
        } else {
            (tcx, ty + th)
        };

        let segs = if self.orthogonal_connections {
            let is_horiz = dx.abs() > dy.abs();
            if is_horiz {
                let mid_x = (ax + bx) / 2.0;
                [
                    (ax, ay, mid_x, ay),
                    (mid_x, ay, mid_x, by),
                    (mid_x, by, bx, by),
                ]
            } else {
                let mid_y = (ay + by) / 2.0;
                [
                    (ax, ay, ax, mid_y),
                    (ax, mid_y, bx, mid_y),
                    (bx, mid_y, bx, by),
                ]
            }
        } else {
            [(ax, ay, bx, by), (0.0, 0.0, 0.0, 0.0), (0.0, 0.0, 0.0, 0.0)]
        };
        let count = if self.orthogonal_connections { 3 } else { 1 };
        Some(EdgeSegments(segs, count))
    }

    pub fn select_edge_at(&mut self, cx: f64, cy: f64) -> Option<String> {
        let tolerance = 5.0 / self.zoom;
        let mut best: Option<(String, f64)> = None;
        for edge in &self.data.edges {
            let Some(seg) = self.get_edge_segments(edge) else {
                continue;
            };
            for &(sx, sy, ex, ey) in seg.0.iter().take(seg.1) {
                let dx = ex - sx;
                let dy = ey - sy;
                let len_sq = dx * dx + dy * dy;
                if len_sq == 0.0 {
                    let dist = ((cx - sx).powi(2) + (cy - sy).powi(2)).sqrt();
                    if dist < tolerance {
                        match &best {
                            Some((_, bd)) if dist >= *bd => {}
                            _ => best = Some((edge.id.clone(), dist)),
                        }
                    }
                    continue;
                }
                let t = ((cx - sx) * dx + (cy - sy) * dy) / len_sq;
                let t_clamped = t.clamp(0.0, 1.0);
                let px = sx + t_clamped * dx;
                let py = sy + t_clamped * dy;
                let dist = ((cx - px).powi(2) + (cy - py).powi(2)).sqrt();
                if dist < tolerance {
                    match &best {
                        Some((_, bd)) if dist >= *bd => {}
                        _ => best = Some((edge.id.clone(), dist)),
                    }
                }
            }
        }
        if let Some((id, _)) = best {
            self.selected_edge_id = Some(id.clone());
            self.selection.clear();
            Some(id)
        } else {
            self.selected_edge_id = None;
            None
        }
    }

    pub fn set_edge_color(&mut self, color: Option<String>) {
        if let Some(id) = &self.selected_edge_id {
            let id = id.clone();
            self.record_undo_state();
            for edge in &mut self.data.edges {
                if edge.id == id {
                    edge.color = color;
                    break;
                }
            }
            let _ = self.save();
            self.sync_to_raw_editor();
        }
    }

    pub fn set_edge_style(&mut self, style: crate::pinstar::data::EdgeStyle) {
        if let Some(id) = &self.selected_edge_id {
            let id = id.clone();
            self.record_undo_state();
            for edge in &mut self.data.edges {
                if edge.id == id {
                    edge.style = style;
                    break;
                }
            }
            let _ = self.save();
            self.sync_to_raw_editor();
        }
    }

    pub fn open_edge_context_menu(&mut self, x: u16, y: u16) {
        let specs = pinstar_menu_specs(PinstarMenuType::EdgeMenu, false);
        self.menu_kind = Some(PinstarMenuType::EdgeMenu);
        self.context_menu = Some(crate::ui::CanvasContextMenu::new(x, y, specs));
    }

    /// Opens the edge context menu centered in the given view area.
    pub fn open_edge_menu_centered(&mut self, area: ratatui::layout::Rect) {
        let menu_x = (area.width / 2).saturating_sub(12);
        let menu_y = area.height;
        self.open_edge_context_menu(menu_x, menu_y);
    }

    /// Edges connected to the currently selected node, in stable storage
    /// order. Used by the edge-list overlay and number-key selection.
    pub fn selected_node_edges(&self) -> Vec<&crate::pinstar::data::CanvasEdge> {
        let Some(node_id) = &self.selection.primary else {
            return Vec::new();
        };
        self.data
            .edges
            .iter()
            .filter(|e| e.from_node == *node_id || e.to_node == *node_id)
            .collect()
    }

    /// Selects the edge at the given 1-based index among the selected node's
    /// connected edges (deselecting the node). Returns its id, or None if out
    /// of range / no node selected.
    pub fn select_edge_of_selected_node(&mut self, index: usize) -> Option<String> {
        let edge_id = {
            let edges = self.selected_node_edges();
            if index >= 1 && index <= edges.len() {
                Some(edges[index - 1].id.clone())
            } else {
                None
            }
        };
        if let Some(edge_id) = edge_id {
            self.selected_edge_id = Some(edge_id.clone());
            self.selection.clear();
            Some(edge_id)
        } else {
            None
        }
    }

    pub fn save(&self) -> Result<()> {
        let content = serde_json::to_string_pretty(&self.data)?;
        crate::fsutil::atomic_write_str(&self.path, &content)?;
        Ok(())
    }

    /// Returns true while the view is undergoing continuous transforms
    /// (pan, zoom, node resize, connection drawing). During these states
    /// the pixel image render is suppressed to avoid churning the encode
    /// worker; cheap placeholder text is shown instead.
    pub fn is_view_transforming(&self) -> bool {
        use crate::image_render::TRANSFORM_SETTLE;
        self.resizing_node_id.is_some()
            || self.is_dragging_resize_handle
            || self.drag_start_pos.is_some()
            || self.is_panning
            || self.connection_source_id.is_some()
            || self.deleting_connection_source_id.is_some()
            || self
                .last_zoom_at
                .is_some_and(|t| t.elapsed() < TRANSFORM_SETTLE)
    }

    pub fn sync_from_raw_editor(&mut self) -> Result<()> {
        let content = self.raw_editor.lines().join("\n");
        if let Ok(data) = serde_json::from_str::<CanvasData>(&content) {
            self.record_undo_state();
            self.data = data;
            let _ = self.save();
            Ok(())
        } else {
            anyhow::bail!("Invalid JSON in editor")
        }
    }

    pub fn sync_to_raw_editor(&mut self) {
        if let Ok(content) = serde_json::to_string_pretty(&self.data) {
            self.raw_editor = TextArea::from(content.lines().map(String::from).collect::<Vec<_>>());
            self.raw_editor
                .set_cursor_line_style(ratatui::style::Style::default());
        }
    }
    pub fn pan(&mut self, dx: f64, dy: f64) {
        if let Some((nx, ny)) = crate::ui::camera::pan_centered(
            self.viewport_x,
            self.viewport_y,
            dx / self.zoom,
            dy / self.zoom,
        ) {
            self.viewport_x = nx;
            self.viewport_y = ny;
        }
    }

    pub fn zoom_in(&mut self) {
        if let Some(z) =
            crate::ui::camera::zoom_step(self.zoom, 1.1, crate::ui::camera::ZoomDir::In, 0.0)
        {
            self.zoom = z;
        }
        self.last_zoom_at = Some(std::time::Instant::now());
    }

    pub fn zoom_out(&mut self) {
        if let Some(z) = crate::ui::camera::zoom_step(
            self.zoom,
            1.1,
            crate::ui::camera::ZoomDir::Out,
            crate::ui::camera::CANVAS_ZOOM_MIN,
        ) {
            self.zoom = z;
        }
        self.last_zoom_at = Some(std::time::Instant::now());
    }

    pub fn center_on_selected(&mut self) {
        if let Some(id) = &self.selection.primary
            && let Some(node) = self.data.nodes.iter().find(|n| n.id() == id)
        {
            let (nx, ny) = node.pos();
            let (nw, nh) = node.size();
            self.viewport_x = nx + nw / 2.0;
            self.viewport_y = ny + nh / 2.0;
        }
    }

    pub fn screen_to_canvas(&self, sx: u16, sy: u16, area: ratatui::layout::Rect) -> (f64, f64) {
        let cx = ((sx as f64 + 0.5) - (area.x as f64 + area.width as f64 / 2.0)) / self.zoom
            + self.viewport_x;
        let cy = ((sy as f64 + 0.5) - (area.y as f64 + area.height as f64 / 2.0)) / self.zoom
            + self.viewport_y;
        (
            crate::ui::camera::clamp_world(cx),
            crate::ui::camera::clamp_world(cy),
        )
    }

    pub fn select_node_at(&mut self, x: f64, y: f64) -> Option<String> {
        let mut best_hit: Option<(String, f64, usize)> = None;

        for (idx, node) in self.data.nodes.iter().enumerate() {
            let (nx, ny) = node.pos();
            let (nw, nh) = node.size();
            if x >= nx && x <= nx + nw && y >= ny && y <= ny + nh {
                let area = nw * nh;
                let should_replace = match &best_hit {
                    None => true,
                    Some((_, best_area, _)) if area < *best_area => true,
                    Some((_, best_area, best_idx))
                        if (area - *best_area).abs() < 0.0001 && idx > *best_idx =>
                    {
                        true
                    }
                    _ => false,
                };
                if should_replace {
                    best_hit = Some((node.id().to_string(), area, idx));
                }
            }
        }

        if let Some((id, _, _)) = best_hit {
            self.selection.select_only(id.clone());
            Some(id)
        } else {
            self.selection.clear();
            None
        }
    }

    pub fn select_node_in_direction(&mut self, dx: f64, dy: f64) {
        let current_node = if let Some(id) = &self.selection.primary {
            self.data.nodes.iter().find(|n| n.id() == id)
        } else {
            None
        };

        let origin = if let Some(n) = current_node {
            let (nx, ny) = n.pos();
            let (nw, nh) = n.size();
            (nx + nw / 2.0, ny + nh / 2.0)
        } else {
            (self.viewport_x, self.viewport_y)
        };

        let mut ids: Vec<String> = Vec::new();
        let mut cands: Vec<(f64, f64)> = Vec::new();
        for node in &self.data.nodes {
            if let Some(id) = &self.selection.primary
                && node.id() == id
            {
                continue;
            }
            let (nx, ny) = node.pos();
            let (nw, nh) = node.size();
            ids.push(node.id().to_string());
            cands.push((nx + nw / 2.0, ny + nh / 2.0));
        }

        if let Some(i) =
            crate::ui::camera::nearest_in_dir(&cands, origin, (dx, dy), std::f64::consts::FRAC_PI_3)
        {
            self.selection.select_only(ids[i].clone());
        } else if self.selection.primary.is_none() && !self.data.nodes.is_empty() {
            self.selection
                .select_only(self.data.nodes[0].id().to_string());
        }
    }

    pub fn toggle_editor(&mut self) {
        let editor_text = if let Some(editor) = &self.floating_editor {
            Some(editor.lines().join("\n"))
        } else {
            None
        };
        if let Some(text) = editor_text {
            if self.selection.primary.is_some() {
                self.record_undo_state();
                let node_id = self
                    .selection
                    .primary
                    .as_ref()
                    .expect("checked is_some above");
                for node in &mut self.data.nodes {
                    if node.id() == node_id {
                        node.set_text(text);
                        break;
                    }
                }
                let _ = self.save();
            }
            self.floating_editor = None;
        } else if let Some(node_id) = &self.selection.primary
            && let Some(node) = self.data.nodes.iter().find(|n| n.id() == node_id)
        {
            let mut textarea =
                TextArea::from(node.text().lines().map(String::from).collect::<Vec<_>>());
            textarea.set_cursor_line_style(ratatui::style::Style::default());
            textarea.set_wrap_mode(WrapMode::Word);
            self.floating_editor = Some(textarea);
        }
    }

    pub fn open_context_menu(&mut self, x: u16, y: u16, canvas_x: f64, canvas_y: f64) {
        let specs = pinstar_menu_specs(PinstarMenuType::Canvas, self.selection.primary.is_some());
        self.menu_kind = Some(PinstarMenuType::Canvas);
        self.context_menu_pos = (canvas_x, canvas_y);
        self.context_menu = Some(crate::ui::CanvasContextMenu::new(x, y, specs));
    }

    pub fn start_resize(&mut self) {
        let id = self.selection.primary.clone();
        if let Some(id) = id {
            self.record_undo_state();
            self.resizing_node_id = Some(id.clone());
            self.context_menu = None;
        }
    }

    pub fn start_delete_connection(&mut self) {
        if let Some(id) = &self.selection.primary {
            self.deleting_connection_source_id = Some(id.clone());
            self.context_menu = None;
        }
    }

    pub fn rename_node(&mut self, new_title: String) {
        let node_id = self.selection.primary.clone();
        if let Some(node_id) = node_id {
            self.record_undo_state();
            let trimmed = new_title.trim().to_string();
            let title = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            };

            for node in &mut self.data.nodes {
                if node.id() == node_id {
                    node.set_title(title.clone());
                    break;
                }
            }

            let _ = self.save();
        }
    }

    pub fn delete_node_connections(&mut self) {
        let ids = self.selection.all();
        if ids.is_empty() {
            return;
        }
        self.record_undo_state();
        self.data
            .edges
            .retain(|e| !ids.contains(&e.from_node) && !ids.contains(&e.to_node));
        let _ = self.save();
    }

    pub fn set_node_color(&mut self, color: Option<String>) {
        let ids = self.selection.all();
        if ids.is_empty() {
            return;
        }
        self.record_undo_state();
        for node in &mut self.data.nodes {
            if ids.contains(node.id()) {
                match node {
                    crate::pinstar::data::CanvasNode::Text(n) => n.color = color.clone(),
                    crate::pinstar::data::CanvasNode::File(n) => n.color = color.clone(),
                    crate::pinstar::data::CanvasNode::Link(n) => n.color = color.clone(),
                    crate::pinstar::data::CanvasNode::Group(n) => n.color = color.clone(),
                }
            }
        }
        let _ = self.save();
    }

    pub fn add_text_node(&mut self, x: f64, y: f64) {
        self.record_undo_state();
        let id = format!("node_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        self.data.nodes.push(crate::pinstar::data::CanvasNode::Text(
            crate::pinstar::data::TextNode {
                id: id.clone(),
                x,
                y,
                width: 200.0,
                height: 100.0,
                text: "".to_string(),
                title: None,
                color: None,
            },
        ));
        self.selection.select_only(id.clone());
        self.resizing_node_id = Some(id);
        let _ = self.save();
    }

    pub fn add_group(&mut self, x: f64, y: f64) {
        self.record_undo_state();
        let id = format!("group_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        self.data.nodes.insert(
            0,
            crate::pinstar::data::CanvasNode::Group(crate::pinstar::data::GroupNode {
                id: id.clone(),
                x,
                y,
                width: 400.0,
                height: 300.0,
                label: Some("New Group".to_string()),
                color: None,
            }),
        );
        self.selection.select_only(id.clone());
        self.resizing_node_id = Some(id);
        let _ = self.save();
    }
    pub fn add_image_node(&mut self, x: f64, y: f64) {
        let path = match crate::ui::pick_file("Image", "png;jpg;jpeg;gif;webp;bmp") {
            Ok(Some(p)) => p,
            _ => return,
        };
        self.record_undo_state();
        let id = format!("node_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        self.data.nodes.push(crate::pinstar::data::CanvasNode::File(
            crate::pinstar::data::FileNode {
                id: id.clone(),
                x,
                y,
                width: 300.0,
                height: 200.0,
                file: path,
                subpath: None,
                title: None,
                color: None,
            },
        ));
        self.selection.select_only(id.clone());
        let _ = self.save();
    }

    pub fn start_connection(&mut self) {
        if let Some(id) = &self.selection.primary {
            self.connection_source_id = Some(id.clone());
            self.context_menu = None;
        }
    }

    pub fn finish_connection(&mut self, target_id: &str) {
        if let Some(source_id) = self.connection_source_id.take()
            && source_id != target_id
        {
            let edge_id = format!("edge_{source_id}_{target_id}");
            self.record_undo_state();
            if !self
                .data
                .edges
                .iter()
                .any(|e| e.from_node == source_id && e.to_node == target_id)
            {
                self.data.edges.push(crate::pinstar::data::CanvasEdge {
                    id: edge_id,
                    from_node: source_id,
                    from_side: Some("right".to_string()),
                    to_node: target_id.to_string(),
                    to_side: Some("left".to_string()),
                    label: None,
                    color: None,
                    style: crate::pinstar::data::EdgeStyle::Solid,
                });
                let _ = self.save();
            }
        }
    }

    pub fn finish_delete_connection(&mut self, target_id: &str) {
        if let Some(source_id) = self.deleting_connection_source_id.take()
            && source_id != target_id
        {
            self.record_undo_state();
            self.data.edges.retain(|e| {
                !((e.from_node == source_id && e.to_node == target_id)
                    || (e.from_node == target_id && e.to_node == source_id))
            });
            let _ = self.save();
        }
    }

    pub fn resize_selected_node(&mut self, dw: f64, dh: f64) {
        if let Some(id) = &self.resizing_node_id {
            for node in &mut self.data.nodes {
                if node.id() == id {
                    match node {
                        crate::pinstar::data::CanvasNode::Text(n) => {
                            n.width = (n.width + dw).max(10.0);
                            n.height = (n.height + dh).max(10.0);
                        }
                        crate::pinstar::data::CanvasNode::File(n) => {
                            n.width = (n.width + dw).max(10.0);
                            n.height = (n.height + dh).max(10.0);
                        }
                        crate::pinstar::data::CanvasNode::Link(n) => {
                            n.width = (n.width + dw).max(10.0);
                            n.height = (n.height + dh).max(10.0);
                        }
                        crate::pinstar::data::CanvasNode::Group(n) => {
                            n.width = (n.width + dw).max(10.0);
                            n.height = (n.height + dh).max(10.0);
                        }
                    }
                    break;
                }
            }
        }
    }

    pub fn capture_drag_nodes(&mut self) {
        self.drag_captured_nodes.clear();
        if let Some(id) = &self.selection.primary {
            let mut group_bounds = None;
            for node in &self.data.nodes {
                if node.id() == id {
                    if let crate::pinstar::data::CanvasNode::Group(n) = node {
                        group_bounds = Some((n.x, n.y, n.width, n.height));
                    }
                    break;
                }
            }

            if let Some((gx, gy, gw, gh)) = group_bounds {
                for node in &self.data.nodes {
                    let nid = node.id();
                    if nid != id {
                        let (nx, ny) = node.pos();
                        let (nw, nh) = node.size();
                        if nx >= gx && ny >= gy && (nx + nw) <= (gx + gw) && (ny + nh) <= (gy + gh)
                        {
                            self.drag_captured_nodes.insert(nid.to_string());
                        }
                    }
                }
            }
            // Also capture multi-selected nodes
            for nid in &self.selection.extra {
                self.drag_captured_nodes.insert(nid.clone());
            }
        } else {
            // When no primary node but multi-selected nodes exist, capture all of them
            for nid in &self.selection.extra {
                self.drag_captured_nodes.insert(nid.clone());
            }
        }
    }
    pub fn move_selected_node(&mut self, dx: f64, dy: f64) {
        let primary = self.selection.primary.clone();
        let captured = self.drag_captured_nodes.clone();
        if primary.is_none() && captured.is_empty() {
            return;
        }
        for node in &mut self.data.nodes {
            let nid = node.id();
            if primary.as_deref() == Some(nid) || captured.contains(nid) {
                match node {
                    crate::pinstar::data::CanvasNode::Text(n) => {
                        n.x += dx;
                        n.y += dy;
                    }
                    crate::pinstar::data::CanvasNode::File(n) => {
                        n.x += dx;
                        n.y += dy;
                    }
                    crate::pinstar::data::CanvasNode::Link(n) => {
                        n.x += dx;
                        n.y += dy;
                    }
                    crate::pinstar::data::CanvasNode::Group(n) => {
                        n.x += dx;
                        n.y += dy;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_context_menu_remains_available() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("canvas.json");
        std::fs::write(&path, r#"{"nodes":[],"edges":[]}"#).unwrap();
        let mut state = PinstarState::load(
            &path,
            crate::keybinds::Keybinds::default(),
            crate::keybinds::KeyMatcher::new(),
        )
        .unwrap();

        state.open_context_menu(4, 5, 0.0, 0.0);

        assert!(matches!(state.menu_kind, Some(PinstarMenuType::Canvas)));
    }

    #[test]
    fn connection_flow_and_delete_both_ways() {
        use crate::pinstar::data::{CanvasData, CanvasNode, TextNode};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.canvas");
        let data = CanvasData {
            nodes: vec![
                CanvasNode::Text(TextNode {
                    id: "a".into(),
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 50.0,
                    text: "".into(),
                    title: None,
                    color: None,
                }),
                CanvasNode::Text(TextNode {
                    id: "b".into(),
                    x: 200.0,
                    y: 0.0,
                    width: 100.0,
                    height: 50.0,
                    text: "".into(),
                    title: None,
                    color: None,
                }),
            ],
            edges: vec![],
        };
        std::fs::write(&path, serde_json::to_string(&data).unwrap()).unwrap();
        let mut s = PinstarState::load(
            &path,
            crate::keybinds::Keybinds::default(),
            crate::keybinds::KeyMatcher::new(),
        )
        .unwrap();
        s.selection.select_only("a".into());
        s.start_connection();
        s.select_node_in_direction(1.0, 0.0);
        assert_eq!(s.selection.primary.as_deref(), Some("b"));
        s.finish_connection("b");
        assert_eq!(s.data.edges.len(), 1);
        // delete both ways: source=b, target=a should remove a->b
        s.selection.select_only("b".into());
        s.start_delete_connection();
        s.finish_delete_connection("a");
        assert_eq!(s.data.edges.len(), 0);
    }

    #[test]
    fn edge_overlay_lists_and_selects_connected_edges() {
        use crate::pinstar::data::{CanvasData, CanvasEdge, CanvasNode, EdgeStyle, TextNode};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.canvas");
        let data = CanvasData {
            nodes: vec![
                CanvasNode::Text(TextNode {
                    id: "a".into(),
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 50.0,
                    text: "".into(),
                    title: None,
                    color: None,
                }),
                CanvasNode::Text(TextNode {
                    id: "b".into(),
                    x: 200.0,
                    y: 0.0,
                    width: 100.0,
                    height: 50.0,
                    text: "".into(),
                    title: None,
                    color: None,
                }),
                CanvasNode::Text(TextNode {
                    id: "c".into(),
                    x: 0.0,
                    y: 200.0,
                    width: 100.0,
                    height: 50.0,
                    text: "".into(),
                    title: None,
                    color: None,
                }),
            ],
            edges: vec![
                CanvasEdge {
                    id: "e1".into(),
                    from_node: "a".into(),
                    from_side: None,
                    to_node: "b".into(),
                    to_side: None,
                    label: None,
                    color: None,
                    style: EdgeStyle::Solid,
                },
                CanvasEdge {
                    id: "e2".into(),
                    from_node: "a".into(),
                    from_side: None,
                    to_node: "c".into(),
                    to_side: None,
                    label: None,
                    color: None,
                    style: EdgeStyle::Solid,
                },
            ],
        };
        std::fs::write(&path, serde_json::to_string(&data).unwrap()).unwrap();
        let mut s = PinstarState::load(
            &path,
            crate::keybinds::Keybinds::default(),
            crate::keybinds::KeyMatcher::new(),
        )
        .unwrap();
        s.selection.select_only("a".into());
        let edges = s.selected_node_edges();
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].id, "e1");
        assert_eq!(edges[1].id, "e2");
        assert_eq!(s.select_edge_of_selected_node(2).as_deref(), Some("e2"));
        assert_eq!(s.selected_edge_id.as_deref(), Some("e2"));
        assert_eq!(s.selection.primary, None);
        // out of range -> None
        s.selection.select_only("a".into());
        assert_eq!(s.select_edge_of_selected_node(3), None);
    }
}
