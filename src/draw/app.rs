use crate::draw::input::handle_event;
use crate::draw::render::draw_canvas;
use crate::draw::state::{DrawData, DrawItemId, Viewport};
use crate::keybinds::Keybinds;

pub enum DrawEventAction {
    Quit,
    OpenHelp,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DrawInteraction {
    Move {
        id: DrawItemId,
        start_world: (f64, f64),
        original_translation: (f64, f64),
        preview_translation: (f64, f64),
    },
}

use ratatui::layout::Rect;
use ratatui_textarea::TextArea;

pub struct DrawAppState {
    pub data: DrawData,
    pub viewport: Viewport,
    pub storage: crate::storage::Storage,
    pub current_file: Option<String>,
    pub running: bool,
    pub active_tool: crate::draw::state::DrawTool,
    pub selection: crate::ui::CanvasSelection<DrawItemId>,
    pub hovered: Option<DrawItemId>,
    pub interaction: Option<DrawInteraction>,
    pub current_stroke: Option<crate::draw::state::Stroke>,
    pub last_area: Rect,
    pub last_mouse_pos: Option<(u16, u16)>,
    pub mouse_pos: Option<(u16, u16)>,
    pub last_click: Option<(u16, u16, std::time::Instant)>,
    pub right_mouse: crate::ui::MarqueeDragState,
    pub right_mouse_screen: Option<(u16, u16)>,
    pub right_mouse_target: Option<DrawItemId>,
    pub text_editor: Option<(DrawItemId, TextArea<'static>)>,
    pub text_editor_rect: Option<Rect>,
    pub(crate) mouse_selection: crate::text_edit::MouseTextSelection,
    pub theme: crate::app_theme::AppThemeColors,
    pub active_shape_type: crate::draw::state::DrawShapeType,
    pub show_shape_selector: bool,
    pub creation_origin: Option<(f64, f64)>,
    pub preview_element: Option<crate::draw::state::DrawElement>,
    pub(crate) erase_start_data: Option<DrawData>,
    pub keybinds: Keybinds,
    pub show_grid: bool,
    pub seq_matcher: crate::keybinds::KeyMatcher,
    pub is_panning: bool,
    pub undo_stack: Vec<DrawData>,
    pub redo_stack: Vec<DrawData>,
}

impl DrawAppState {
    pub fn new(
        storage: crate::storage::Storage,
        file_id: Option<String>,
        theme: crate::app_theme::AppThemeColors,
        keybinds: Keybinds,
        seq_matcher: crate::keybinds::KeyMatcher,
    ) -> Self {
        let mut data = DrawData::default();
        if let Some(id) = &file_id {
            let path = storage.note_path(id);
            if path.exists()
                && let Ok(content) = std::fs::read_to_string(path)
                && let Ok(loaded_data) = serde_json::from_str(&content)
            {
                data = loaded_data;
            }
        }

        Self {
            data,
            viewport: Viewport::default(),
            storage,
            current_file: file_id,
            running: true,
            active_tool: crate::draw::state::DrawTool::Cursor,
            selection: crate::ui::CanvasSelection::new(),
            hovered: None,
            interaction: None,
            current_stroke: None,
            last_area: Rect::default(),
            mouse_pos: None,
            last_mouse_pos: None,
            last_click: None,
            right_mouse: crate::ui::MarqueeDragState::new(3),
            right_mouse_screen: None,
            right_mouse_target: None,
            text_editor: None,
            text_editor_rect: None,
            mouse_selection: crate::text_edit::MouseTextSelection::default(),
            theme,
            active_shape_type: crate::draw::state::DrawShapeType::Rect,
            show_shape_selector: false,
            creation_origin: None,
            preview_element: None,
            erase_start_data: None,
            keybinds,
            show_grid: true,
            seq_matcher,
            is_panning: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn set_active_tool(&mut self, tool: crate::draw::state::DrawTool) {
        self.clear_transient_interaction();
        self.active_tool = tool;
    }

    #[must_use]
    pub fn topmost_hit(&self, world_point: (f64, f64)) -> Option<DrawItemId> {
        let hit = |item: &crate::draw::state::DrawItem| {
            crate::draw::geometry::hit_test_item(item, world_point, 5.0, &self.viewport)
        };
        self.data
            .elements
            .iter()
            .rev()
            .find(|item| {
                matches!(&item.element, crate::draw::state::DrawElement::Text(_)) && hit(item)
            })
            .or_else(|| {
                self.data.elements.iter().rev().find(|item| {
                    !matches!(&item.element, crate::draw::state::DrawElement::Text(_)) && hit(item)
                })
            })
            .map(|item| item.id.clone())
    }

    pub fn begin_text_editor(&mut self, id: DrawItemId) -> bool {
        let Some(crate::draw::state::DrawElement::Text(text)) =
            self.data.item(&id).map(|item| &item.element)
        else {
            return false;
        };
        self.text_editor = Some((id, TextArea::new(vec![text.content.clone()])));
        true
    }

    pub fn record_undo_state(&mut self, previous: DrawData) {
        Self::push_history(&mut self.undo_stack, previous);
        self.redo_stack.clear();
    }

    pub fn commit_data_change(&mut self, previous: DrawData) -> anyhow::Result<bool> {
        if self.data == previous {
            return Ok(false);
        }
        self.record_undo_state(previous);
        self.save_draw()?;
        Ok(true)
    }

    pub fn undo(&mut self) -> anyhow::Result<bool> {
        let Some(previous) = self.undo_stack.pop() else {
            return Ok(false);
        };
        let current = std::mem::replace(&mut self.data, previous);
        Self::push_history(&mut self.redo_stack, current);
        self.clear_transient_interaction();
        self.save_draw()?;
        Ok(true)
    }

    pub fn redo(&mut self) -> anyhow::Result<bool> {
        let Some(next) = self.redo_stack.pop() else {
            return Ok(false);
        };
        let current = std::mem::replace(&mut self.data, next);
        Self::push_history(&mut self.undo_stack, current);
        self.clear_transient_interaction();
        self.save_draw()?;
        Ok(true)
    }

    pub fn clear_transient_interaction(&mut self) {
        self.current_stroke = None;
        self.creation_origin = None;
        self.preview_element = None;
        self.erase_start_data = None;
        self.text_editor = None;
        self.text_editor_rect = None;
        self.mouse_selection = crate::text_edit::MouseTextSelection::default();
        self.show_shape_selector = false;
        self.is_panning = false;
        self.last_mouse_pos = None;
        self.hovered = None;
        self.selection.clear();
        self.interaction = None;
        self.last_click = None;
        self.right_mouse.clear();
        self.right_mouse_screen = None;
        self.right_mouse_target = None;
    }

    fn push_history(stack: &mut Vec<DrawData>, data: DrawData) {
        const MAX_HISTORY: usize = 20;
        if stack.len() == MAX_HISTORY {
            stack.remove(0);
        }
        stack.push(data);
    }

    pub fn save_draw(&self) -> anyhow::Result<()> {
        if let Some(id) = &self.current_file {
            let path = self.storage.note_path(id);
            let content = serde_json::to_string(&self.data)?;
            crate::fsutil::atomic_write_str(&path, &content)?;
        }
        Ok(())
    }
}

impl crate::overlay::OverlayView for DrawAppState {
    fn overlay_render(
        &mut self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        app: &mut crate::app::App,
    ) {
        self.last_area = area;
        draw_canvas(frame, self, area, &app.config, self.mouse_pos);
    }

    fn overlay_handle_event(
        &mut self,
        event: crossterm::event::Event,
        app: &mut crate::app::App,
        _term_area: ratatui::layout::Rect,
    ) -> anyhow::Result<crate::overlay::OverlayResult> {
        let keybinds = self.keybinds.clone();
        if let Some(action) = handle_event(event, self, &keybinds, &app.config)? {
            match action {
                DrawEventAction::Quit => {
                    self.running = false;
                    self.save_draw()?;
                    return Ok(crate::overlay::OverlayResult::Exit);
                }
                DrawEventAction::OpenHelp => {
                    return Ok(crate::overlay::OverlayResult::OpenHelp(
                        crate::app::HelpTab::Draw,
                    ));
                }
            }
        }
        Ok(crate::overlay::OverlayResult::Continue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> (tempfile::TempDir, DrawAppState) {
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path().join("data");
        let config_dir = temp.path().join("config");
        let notes_dir = temp.path().join("notes");
        let templates_dir = temp.path().join("templates");
        std::fs::create_dir_all(&notes_dir).unwrap();
        let storage = crate::storage::Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0; 32],
            skip_dir_patterns: Vec::new(),
        };
        (
            temp,
            DrawAppState::new(
                storage,
                Some("history.draw".to_string()),
                crate::app_theme::AppThemeColors::default(),
                Keybinds::default(),
                crate::keybinds::KeyMatcher::new(),
            ),
        )
    }

    #[test]
    fn history_caps_invalidates_redo_and_saves_restores() {
        let (_temp, mut state) = test_state();
        let unchanged = state.data.clone();
        assert!(!state.commit_data_change(unchanged).unwrap());
        assert!(state.undo_stack.is_empty());

        for index in 1..=21 {
            let previous = state.data.clone();
            state.data.width = 1000.0 + index as f64;
            assert!(state.commit_data_change(previous).unwrap());
        }
        assert_eq!(state.undo_stack.len(), 20);

        let selected = DrawItemId::new();
        state.selection.select_only(selected.clone());
        state.hovered = Some(selected);
        state.current_stroke = Some(crate::draw::state::Stroke {
            points: vec![(0.0, 0.0)],
            color: (0, 0, 0),
        });
        assert!(state.undo().unwrap());
        assert_eq!(state.redo_stack.len(), 1);
        assert!(state.selection.is_empty());
        assert!(state.hovered.is_none());
        assert!(state.current_stroke.is_none());

        let previous = state.data.clone();
        state.data.height = 777.0;
        assert!(state.commit_data_change(previous).unwrap());
        assert!(state.redo_stack.is_empty());

        let saved = std::fs::read_to_string(state.storage.note_path("history.draw")).unwrap();
        assert_eq!(
            serde_json::from_str::<DrawData>(&saved).unwrap(),
            state.data
        );
    }

    #[test]
    fn topmost_hit_prefers_text_then_latest_vector() {
        let (_temp, mut state) = test_state();
        let text = crate::draw::state::DrawItem::new(crate::draw::state::DrawElement::Text(
            crate::draw::state::Text {
                content: "text".to_string(),
                x: 0.0,
                y: 0.0,
                color: (255, 255, 255),
            },
        ));
        let text_id = text.id.clone();
        state.data.elements.push(text);
        state.data.elements.push(crate::draw::state::DrawItem::new(
            crate::draw::state::DrawElement::Shape(crate::draw::state::Shape::Line {
                x1: 0.0,
                y1: 1.0,
                x2: 10.0,
                y2: 1.0,
                color: (255, 255, 255),
            }),
        ));
        assert_eq!(state.topmost_hit((1.0, 1.0)), Some(text_id));

        state.data.elements.remove(0);
        let vector = crate::draw::state::DrawItem::new(crate::draw::state::DrawElement::Shape(
            crate::draw::state::Shape::Line {
                x1: 0.0,
                y1: 1.0,
                x2: 10.0,
                y2: 1.0,
                color: (255, 255, 255),
            },
        ));
        let vector_id = vector.id.clone();
        state.data.elements.push(vector);
        assert_eq!(state.topmost_hit((1.0, 1.0)), Some(vector_id));
    }
}
