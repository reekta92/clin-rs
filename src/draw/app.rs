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
    Rotate {
        id: DrawItemId,
        pivot_world: (f64, f64),
        original_degrees: f64,
        preview_degrees: f64,
        start_angle: Option<f64>,
    },
    Scale {
        id: DrawItemId,
        pivot_world: (f64, f64),
        original_scale: f64,
        preview_scale: f64,
        start_distance: Option<f64>,
    },
    Paste {
        item: crate::draw::state::DrawItem,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum DrawMenuTarget {
    NonText(DrawItemId),
    Text(DrawItemId),
    Empty { x: f64, y: f64 },
}

impl DrawMenuTarget {
    #[must_use]
    pub fn item_id(&self) -> Option<&DrawItemId> {
        match self {
            Self::NonText(id) | Self::Text(id) => Some(id),
            Self::Empty { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawMenuKind {
    Actions,
    Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawMenuItem {
    Rotate,
    Scale,
    Color,
    Copy,
    Erase,
    EditText,
    Paste,
}

impl DrawMenuItem {
    const fn label(self) -> &'static str {
        match self {
            Self::Rotate => "Rotate",
            Self::Scale => "Scale",
            Self::Color => "Color...",
            Self::Copy => "Copy",
            Self::Erase => "Erase",
            Self::EditText => "Edit Text",
            Self::Paste => "Paste",
        }
    }

    const fn shortcut(self) -> char {
        match self {
            Self::Rotate => 'r',
            Self::Scale => 's',
            Self::Color => 'o',
            Self::Copy => 'c',
            Self::Erase => 'e',
            Self::EditText => 't',
            Self::Paste => 'v',
        }
    }
}

#[must_use]
pub fn draw_menu_items(
    target: &DrawMenuTarget,
    clipboard_available: bool,
) -> &'static [DrawMenuItem] {
    const NON_TEXT: &[DrawMenuItem] = &[
        DrawMenuItem::Rotate,
        DrawMenuItem::Scale,
        DrawMenuItem::Color,
        DrawMenuItem::Copy,
        DrawMenuItem::Erase,
    ];
    const TEXT: &[DrawMenuItem] = &[
        DrawMenuItem::EditText,
        DrawMenuItem::Color,
        DrawMenuItem::Copy,
        DrawMenuItem::Erase,
    ];
    const PASTE: &[DrawMenuItem] = &[DrawMenuItem::Paste];

    match target {
        DrawMenuTarget::NonText(_) => NON_TEXT,
        DrawMenuTarget::Text(_) => TEXT,
        DrawMenuTarget::Empty { .. } if clipboard_available => PASTE,
        DrawMenuTarget::Empty { .. } => &[],
    }
}

pub(crate) fn draw_menu_shortcut_index(
    target: &DrawMenuTarget,
    clipboard_available: bool,
    shortcut: char,
) -> Option<usize> {
    let shortcut = shortcut.to_ascii_lowercase();
    draw_menu_items(target, clipboard_available)
        .iter()
        .position(|item| item.shortcut().to_ascii_lowercase() == shortcut)
}

#[must_use]
pub fn draw_menu_specs(
    target: &DrawMenuTarget,
    clipboard_available: bool,
) -> Vec<crate::ui::CanvasMenuItemSpec> {
    draw_menu_items(target, clipboard_available)
        .iter()
        .map(|item| crate::ui::CanvasMenuItemSpec::new(item.label()).shortcut(item.shortcut()))
        .collect()
}

#[must_use]
pub fn draw_color_menu_specs() -> Vec<crate::ui::CanvasMenuItemSpec> {
    crate::pinstar::COLOR_PICKER_PALETTE
        .iter()
        .map(|(label, _, color)| {
            let shortcut = match *label {
                "Red" => 'r',
                "Orange" => 'o',
                "Yellow" => 'y',
                "Green" => 'g',
                "Cyan" => 'c',
                "Purple" => 'p',
                "Blue" => 'b',
                "Magenta" => 'm',
                "White" => 'w',
                _ => unreachable!("fixed color palette has known names"),
            };
            crate::ui::CanvasMenuItemSpec::new(label)
                .shortcut(shortcut)
                .color(*color)
        })
        .collect()
}

use ratatui::layout::Rect;
use ratatui_textarea::TextArea;

pub struct DrawAppState {
    pub data: DrawData,
    pub viewport: Viewport,
    pub storage: crate::storage::Storage,
    pub current_file: Option<String>,

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
    pub context_menu: Option<crate::ui::CanvasContextMenu>,
    pub menu_target: Option<DrawMenuTarget>,
    pub menu_kind: Option<DrawMenuKind>,
    pub text_editor: Option<(DrawItemId, TextArea<'static>)>,
    pub text_editor_rect: Option<Rect>,
    pub(crate) mouse_selection: crate::text_edit::MouseTextSelection,
    pub theme: crate::app_theme::AppThemeColors,
    pub active_shape_type: crate::draw::state::DrawShapeType,
    pub show_shape_selector: bool,
    pub show_color_selector: bool,
    pub active_color: (u8, u8, u8),
    pub creation_origin: Option<(f64, f64)>,
    pub preview_element: Option<crate::draw::state::DrawElement>,
    pub(crate) erase_start_data: Option<DrawData>,
    pub keybinds: Keybinds,
    pub grid: bool,
    pub seq_matcher: crate::keybinds::KeyMatcher,
    pub is_panning: bool,
    status_notice: Option<&'static str>,
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
            context_menu: None,
            menu_target: None,
            menu_kind: None,
            text_editor: None,
            text_editor_rect: None,
            mouse_selection: crate::text_edit::MouseTextSelection::default(),
            theme,
            active_shape_type: crate::draw::state::DrawShapeType::Rect,
            show_shape_selector: false,
            show_color_selector: false,
            active_color: (255, 255, 255),
            creation_origin: None,
            preview_element: None,
            erase_start_data: None,
            keybinds,
            grid: true,
            seq_matcher,
            is_panning: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            status_notice: None,
        }
    }

    pub fn set_active_tool(&mut self, tool: crate::draw::state::DrawTool) {
        self.clear_transient_interaction();
        self.active_tool = tool;
    }
    const TRANSIENT_MODE_MESSAGES: [&str; 3] = [
        "ROTATE MODE: Drag pointer to rotate, Left-click to begin",
        "SCALE MODE: Drag pointer to scale, Left-click to begin",
        "PASTE MODE: Move pointer, Left-click to place",
    ];

    #[must_use]
    pub fn active_mode_message(&self) -> Option<&'static str> {
        match self.interaction.as_ref() {
            Some(DrawInteraction::Rotate { .. }) => Some(Self::TRANSIENT_MODE_MESSAGES[0]),
            Some(DrawInteraction::Scale { .. }) => Some(Self::TRANSIENT_MODE_MESSAGES[1]),
            Some(DrawInteraction::Paste { .. }) => Some(Self::TRANSIENT_MODE_MESSAGES[2]),
            Some(DrawInteraction::Move { .. }) | None => None,
        }
    }

    pub fn notify(&mut self, message: &'static str) {
        self.status_notice = Some(message);
    }

    pub fn sync_header_status(&mut self, app: &mut crate::app::App) {
        if let Some(message) = self.status_notice.take() {
            app.set_temporary_status_static(message);
        } else if app.status_until.is_none() {
            if let Some(message) = self.active_mode_message() {
                app.status = std::borrow::Cow::Borrowed(message);
            } else if Self::TRANSIENT_MODE_MESSAGES.contains(&app.status.as_ref()) {
                app.set_default_status();
            }
        }
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

    pub fn open_context_menu(
        &mut self,
        x: u16,
        y: u16,
        target: DrawMenuTarget,
        clipboard_available: bool,
    ) {
        let specs = draw_menu_specs(&target, clipboard_available);
        if specs.is_empty() {
            return;
        }
        self.context_menu = Some(crate::ui::CanvasContextMenu::new(x, y, specs));
        self.menu_target = Some(target);
        self.menu_kind = Some(DrawMenuKind::Actions);
    }

    pub fn open_color_menu(&mut self, x: u16, y: u16) {
        self.context_menu = Some(crate::ui::CanvasContextMenu::new(
            x,
            y,
            draw_color_menu_specs(),
        ));
        self.menu_kind = Some(DrawMenuKind::Color);
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
        self.show_color_selector = false;
        self.is_panning = false;
        self.last_mouse_pos = None;
        self.hovered = None;
        self.selection.clear();
        self.interaction = None;
        self.last_click = None;
        self.right_mouse.clear();
        self.right_mouse_screen = None;
        self.right_mouse_target = None;
        self.context_menu = None;
        self.menu_target = None;
        self.menu_kind = None;
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
        let config = &app.config;
        let clipboard = &mut app.draw_clipboard;
        let action = handle_event(event, self, &keybinds, config, clipboard)?;
        self.sync_header_status(app);
        if let Some(action) = action {
            match action {
                DrawEventAction::Quit => {
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

    #[test]
    fn active_mode_message_only_tracks_transient_interactions() {
        let (_temp, mut state) = test_state();
        assert_eq!(state.active_mode_message(), None);

        state.set_active_tool(crate::draw::state::DrawTool::Text);
        assert_eq!(state.active_mode_message(), None);

        state.interaction = Some(DrawInteraction::Paste {
            item: crate::draw::state::DrawItem::new(crate::draw::state::DrawElement::Stroke(
                crate::draw::state::Stroke {
                    points: vec![(0.0, 0.0)],
                    color: (255, 255, 255),
                },
            )),
        });
        assert_eq!(
            state.active_mode_message(),
            Some("PASTE MODE: Move pointer, Left-click to place")
        );
    }

    #[test]
    fn context_menu_specs_match_draw_item_scope() {
        let id = DrawItemId::new();
        let labels = |target: DrawMenuTarget, clipboard_available| {
            draw_menu_specs(&target, clipboard_available)
                .into_iter()
                .map(|spec| (spec.label, spec.shortcut))
                .collect::<Vec<_>>()
        };

        assert_eq!(
            labels(DrawMenuTarget::NonText(id.clone()), false),
            vec![
                ("Rotate", Some('r')),
                ("Scale", Some('s')),
                ("Color...", Some('o')),
                ("Copy", Some('c')),
                ("Erase", Some('e')),
            ]
        );
        assert_eq!(
            labels(DrawMenuTarget::Text(id), false),
            vec![
                ("Edit Text", Some('t')),
                ("Color...", Some('o')),
                ("Copy", Some('c')),
                ("Erase", Some('e')),
            ]
        );
        assert!(draw_menu_specs(&DrawMenuTarget::Empty { x: 0.0, y: 0.0 }, false).is_empty());
        assert_eq!(
            labels(DrawMenuTarget::Empty { x: 0.0, y: 0.0 }, true),
            vec![("Paste", Some('v'))]
        );

        let id = DrawItemId::new();
        assert_eq!(
            draw_menu_shortcut_index(&DrawMenuTarget::NonText(id.clone()), false, 'E'),
            Some(4)
        );
        assert_eq!(
            draw_menu_shortcut_index(&DrawMenuTarget::Text(id), false, 'T'),
            Some(0)
        );
    }
}
