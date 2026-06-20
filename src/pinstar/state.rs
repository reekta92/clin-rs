use crate::pinstar::data::CanvasData;
use anyhow::Result;
use ratatui_textarea::{TextArea, WrapMode};
use std::path::{Path, PathBuf};

pub struct PinstarState {
    pub path: PathBuf,
    pub data: CanvasData,
    pub viewport_x: f64,
    pub viewport_y: f64,
    pub zoom: f64,
    pub selected_node_id: Option<String>,
    pub floating_editor: Option<TextArea<'static>>,
    pub raw_editor: TextArea<'static>,
    pub editor_focus: bool,
    pub last_mouse_pos: Option<(u16, u16)>,
    pub last_click: Option<(u16, u16, std::time::Instant)>,
    pub context_menu: Option<PinstarContextMenu>,
    pub context_menu_pos: (f64, f64),
    pub connection_source_id: Option<String>,
    pub resizing_node_id: Option<String>,
    pub is_dragging_resize_handle: bool,
    pub deleting_connection_source_id: Option<String>,
    pub show_editor_pane: bool,
    pub drag_start_pos: Option<(f64, f64)>,
    pub rename_popup: Option<TextArea<'static>>,
    pub last_mouse_canvas_pos: Option<(f64, f64)>,
    pub drag_captured_nodes: std::collections::HashSet<String>,
    pub show_grid: bool,
    pub mouse_selecting: bool,
    pub mouse_dragged: bool,
    pub help_requested: bool,
    pub footer_hint: String,
    pub keybinds: crate::keybinds::Keybinds,
    pub seq_matcher: crate::keybinds::KeyMatcher,
    pub last_area: ratatui::layout::Rect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PinstarMenuType {
    Canvas,
    Editor,
    ColorPicker,
}

pub struct PinstarContextMenu {
    pub x: u16,
    pub y: u16,
    pub selected: usize,
    pub items: Vec<String>,
    pub menu_type: PinstarMenuType,
}

impl PinstarState {
    pub fn load(path: &Path, keybinds: crate::keybinds::Keybinds, seq_matcher: crate::keybinds::KeyMatcher) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let data: CanvasData = serde_json::from_str(&content)?;
        let mut raw_editor = TextArea::from(content.lines().map(String::from).collect::<Vec<_>>());
        raw_editor.set_cursor_line_style(ratatui::style::Style::default());

        Ok(Self {
            path: path.to_path_buf(),
            data,
            viewport_x: 0.0,
            viewport_y: 0.0,
            zoom: 0.1,
            selected_node_id: None,
            floating_editor: None,
            raw_editor,
            editor_focus: false,
            last_mouse_pos: None,
            last_click: None,
            context_menu: None,
            context_menu_pos: (0.0, 0.0),
            connection_source_id: None,
            resizing_node_id: None,
            is_dragging_resize_handle: false,
            deleting_connection_source_id: None,
            show_editor_pane: false,
            drag_start_pos: None,
            rename_popup: None,
            last_mouse_canvas_pos: None,
            drag_captured_nodes: std::collections::HashSet::new(),
            show_grid: true,
            mouse_selecting: false,
            mouse_dragged: false,
            help_requested: false,
            footer_hint: String::new(),
            keybinds,
            seq_matcher,
            last_area: ratatui::layout::Rect::default(),
        })
    }

    pub fn save(&self) -> Result<()> {
        let content = serde_json::to_string_pretty(&self.data)?;
        std::fs::write(&self.path, content)?;
        Ok(())
    }

    pub fn sync_from_raw_editor(&mut self) -> Result<()> {
        let content = self.raw_editor.lines().join("\n");
        if let Ok(data) = serde_json::from_str::<CanvasData>(&content) {
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
        self.viewport_x += dx / self.zoom;
        self.viewport_y += dy / self.zoom;
    }

    pub fn center_on_selected(&mut self) {
        if let Some(id) = &self.selected_node_id
            && let Some(node) = self.data.nodes.iter().find(|n| n.id() == id)
        {
            let (nx, ny) = node.pos();
            let (nw, nh) = node.size();
            self.viewport_x = nx + nw / 2.0;
            self.viewport_y = ny + nh / 2.0;
        }
    }

    pub fn zoom_in(&mut self) {
        self.zoom *= 1.1;
    }

    pub fn zoom_out(&mut self) {
        self.zoom /= 1.1;
    }

    pub fn screen_to_canvas(&self, sx: u16, sy: u16, area: ratatui::layout::Rect) -> (f64, f64) {
        let cx = ((sx as f64 + 0.5) - (area.x as f64 + area.width as f64 / 2.0)) / self.zoom
            + self.viewport_x;
        let cy = ((sy as f64 + 0.5) - (area.y as f64 + area.height as f64 / 2.0)) / self.zoom
            + self.viewport_y;
        (cx, cy)
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
            self.selected_node_id = Some(id.clone());
            Some(id)
        } else {
            self.selected_node_id = None;
            None
        }
    }

    pub fn select_node_in_direction(&mut self, dx: f64, dy: f64) {
        let current_node = if let Some(id) = &self.selected_node_id {
            self.data.nodes.iter().find(|n| n.id() == id)
        } else {
            None
        };

        let (cur_x, cur_y) = if let Some(n) = current_node {
            let (nx, ny) = n.pos();
            let (nw, nh) = n.size();
            (nx + nw / 2.0, ny + nh / 2.0)
        } else {
            (self.viewport_x, self.viewport_y)
        };

        let mut best_node = None;
        let mut min_dist = f64::MAX;

        for node in &self.data.nodes {
            if let Some(id) = &self.selected_node_id
                && node.id() == id
            {
                continue;
            }

            let (nx, ny) = node.pos();
            let (nw, nh) = node.size();
            let (tx, ty) = (nx + nw / 2.0, ny + nh / 2.0);

            let v_x = tx - cur_x;
            let v_y = ty - cur_y;

            let dot = v_x * dx + v_y * dy;
            if dot <= 0.0 {
                continue;
            }

            let dist_sq = v_x * v_x + v_y * v_y;
            let ortho_dist = (v_x * -dy + v_y * dx).abs();
            let score = dist_sq + ortho_dist * ortho_dist * 2.0;

            if score < min_dist {
                min_dist = score;
                best_node = Some(node.id().to_string());
            }
        }

        if let Some(id) = best_node {
            self.selected_node_id = Some(id);
        } else if self.selected_node_id.is_none() && !self.data.nodes.is_empty() {
            self.selected_node_id = Some(self.data.nodes[0].id().to_string());
        }
    }

    pub fn toggle_editor(&mut self) {
        if let Some(editor) = &self.floating_editor {
            if let Some(node_id) = &self.selected_node_id {
                let text = editor.lines().join("\n");
                for node in &mut self.data.nodes {
                    if node.id() == node_id {
                        node.set_text(text);
                        break;
                    }
                }
                let _ = self.save();
            }
            self.floating_editor = None;
        } else if let Some(node_id) = &self.selected_node_id
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
        let items = if self.selected_node_id.is_some() {
            vec![
                "Create Connection".to_string(),
                "Delete Connection".to_string(),
                "Rename Node".to_string(),
                "Resize Node".to_string(),
                "Set Color...".to_string(),
                "Delete All Connections".to_string(),
                "Delete Node".to_string(),
            ]
        } else {
            vec!["Add Text Node".to_string(), "Add Group".to_string()]
        };

        self.context_menu_pos = (canvas_x, canvas_y);
        self.context_menu = Some(PinstarContextMenu {
            x,
            y,
            selected: 0,
            items,
            menu_type: PinstarMenuType::Canvas,
        });
    }

    pub fn open_editor_context_menu(&mut self, x: u16, y: u16) {
        let items = vec![
            "Copy".to_string(),
            "Cut".to_string(),
            "Paste".to_string(),
            "Select All".to_string(),
        ];

        self.context_menu = Some(PinstarContextMenu {
            x,
            y,
            selected: 0,
            items,
            menu_type: PinstarMenuType::Editor,
        });
    }

    pub fn start_resize(&mut self) {
        if let Some(id) = &self.selected_node_id {
            self.resizing_node_id = Some(id.clone());
            self.context_menu = None;
        }
    }

    pub fn start_delete_connection(&mut self) {
        if let Some(id) = &self.selected_node_id {
            self.deleting_connection_source_id = Some(id.clone());
            self.context_menu = None;
        }
    }

    pub fn rename_node(&mut self, new_id: String) {
        if let Some(old_id) = self.selected_node_id.take() {
            if old_id == new_id {
                self.selected_node_id = Some(old_id);
                return;
            }
            let final_id = if new_id.is_empty() {
                uuid::Uuid::new_v4().to_string()
            } else {
                new_id
            };
            let new_id = final_id;

            for node in &mut self.data.nodes {
                match node {
                    crate::pinstar::data::CanvasNode::Text(n) if n.id == old_id => {
                        n.id = new_id.clone()
                    }
                    crate::pinstar::data::CanvasNode::File(n) if n.id == old_id => {
                        n.id = new_id.clone()
                    }
                    crate::pinstar::data::CanvasNode::Link(n) if n.id == old_id => {
                        n.id = new_id.clone()
                    }
                    crate::pinstar::data::CanvasNode::Group(n) if n.id == old_id => {
                        n.id = new_id.clone()
                    }
                    _ => {}
                }
            }

            for edge in &mut self.data.edges {
                if edge.from_node == old_id {
                    edge.from_node = new_id.clone();
                }
                if edge.to_node == old_id {
                    edge.to_node = new_id.clone();
                }
            }

            self.selected_node_id = Some(new_id);
            let _ = self.save();
            self.sync_to_raw_editor();
        }
    }

    pub fn delete_node_connections(&mut self) {
        if let Some(id) = &self.selected_node_id {
            let id_clone = id.clone();
            self.data
                .edges
                .retain(|e| e.from_node != id_clone && e.to_node != id_clone);
            let _ = self.save();
            self.sync_to_raw_editor();
        }
    }

    pub fn set_node_color(&mut self, color: Option<String>) {
        if let Some(id) = &self.selected_node_id {
            for node in &mut self.data.nodes {
                if node.id() == id {
                    match node {
                        crate::pinstar::data::CanvasNode::Text(n) => n.color = color.clone(),
                        crate::pinstar::data::CanvasNode::File(n) => n.color = color.clone(),
                        crate::pinstar::data::CanvasNode::Link(n) => n.color = color.clone(),
                        crate::pinstar::data::CanvasNode::Group(n) => n.color = color.clone(),
                    }
                    break;
                }
            }
            let _ = self.save();
            self.sync_to_raw_editor();
        }
    }

    pub fn add_text_node(&mut self, x: f64, y: f64) {
        let id = format!("node_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        self.data.nodes.push(crate::pinstar::data::CanvasNode::Text(
            crate::pinstar::data::TextNode {
                id: id.clone(),
                x,
                y,
                width: 200.0,
                height: 100.0,
                text: "".to_string(),
                color: None,
            },
        ));
        self.selected_node_id = Some(id.clone());
        self.resizing_node_id = Some(id);
        let _ = self.save();
        self.sync_to_raw_editor();
    }

    pub fn add_group(&mut self, x: f64, y: f64) {
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
        self.selected_node_id = Some(id.clone());
        self.resizing_node_id = Some(id);
        let _ = self.save();
        self.sync_to_raw_editor();
    }

    pub fn start_connection(&mut self) {
        if let Some(id) = &self.selected_node_id {
            self.connection_source_id = Some(id.clone());
            self.context_menu = None;
        }
    }

    pub fn finish_connection(&mut self, target_id: &str) {
        if let Some(source_id) = self.connection_source_id.take()
            && source_id != target_id
        {
            let edge_id = format!("edge_{source_id}_{target_id}");
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
                });
                let _ = self.save();
                self.sync_to_raw_editor();
            }
        }
    }

    pub fn finish_delete_connection(&mut self, target_id: &str) {
        if let Some(source_id) = self.deleting_connection_source_id.take()
            && source_id != target_id
        {
            self.data
                .edges
                .retain(|e| !(e.from_node == source_id && e.to_node == target_id));
            let _ = self.save();
            self.sync_to_raw_editor();
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
            self.sync_to_raw_editor();
        }
    }

    pub fn capture_drag_nodes(&mut self) {
        self.drag_captured_nodes.clear();
        if let Some(id) = &self.selected_node_id {
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
        }
    }

    pub fn move_selected_node(&mut self, dx: f64, dy: f64) {
        if let Some(id) = &self.selected_node_id {
            for node in &mut self.data.nodes {
                let nid = node.id();
                if nid == id || self.drag_captured_nodes.contains(nid) {
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
            self.sync_to_raw_editor();
        }
    }
}
