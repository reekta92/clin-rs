use crate::canvas::state::{CanvasData, Viewport};
use crate::keybinds::Keybinds;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::Stdout;
use crate::canvas::render::draw_canvas;
use crate::canvas::input::handle_event;

pub enum EventAction {
    Quit,
    Save,
}

use ratatui::layout::Rect;

use tui_textarea::TextArea;
use std::time::Instant;

pub struct CanvasAppState {
    pub data: CanvasData,
    pub viewport: Viewport,
    pub storage: crate::storage::Storage,
    pub current_file: Option<String>,
    pub running: bool,
    pub active_tool: crate::canvas::state::CanvasTool,
    pub current_stroke: Option<crate::canvas::state::Stroke>,
    pub last_area: Rect,
    pub last_mouse_pos: Option<(u16, u16)>,
    pub text_editor: Option<(usize, TextArea<'static>)>, 
    pub last_click: Option<(u16, u16, Instant)>,
    pub theme: crate::app_theme::AppThemeColors,
    pub active_shape_type: crate::canvas::state::ShapeType,
    pub show_shape_selector: bool,
    pub creation_origin: Option<(f64, f64)>,
    pub preview_element: Option<crate::canvas::state::CanvasElement>,
}

impl CanvasAppState {
    pub fn new(
        storage: crate::storage::Storage,
        file_id: Option<String>,
        theme: crate::app_theme::AppThemeColors,
    ) -> Self {
        let mut data = CanvasData::default();
        if let Some(id) = &file_id {
            let path = storage.note_path(id);
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(path) {
                    if let Ok(loaded_data) = serde_json::from_str(&content) {
                        data = loaded_data;
                    }
                }
            }
        }

        Self {
            data,
            viewport: Viewport::default(),
            storage,
            current_file: file_id,
            running: true,
            active_tool: crate::canvas::state::CanvasTool::Draw,
            current_stroke: None,
            last_area: Rect::default(),
            last_mouse_pos: None,
            text_editor: None,
            last_click: None,
            theme,
            active_shape_type: crate::canvas::state::ShapeType::Rect,
            show_shape_selector: false,
            creation_origin: None,
            preview_element: None,
        }
    }

    pub fn save_canvas(&self) -> anyhow::Result<()> {
        if let Some(id) = &self.current_file {
            let path = self.storage.note_path(id);
            let content = serde_json::to_string(&self.data)?;
            std::fs::write(path, content)?;
        }
        Ok(())
    }
}

pub fn run_canvas_view(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    storage: crate::storage::Storage,
    keybinds: &Keybinds,
    file_id: Option<String>,
    theme: crate::app_theme::AppThemeColors,
) -> anyhow::Result<Option<String>> {
    let mut app_state = CanvasAppState::new(storage, file_id, theme);
    
    while app_state.running {
        terminal.draw(|frame| {
            app_state.last_area = frame.area();
            draw_canvas(frame, &app_state);
        })?;

        if crossterm::event::poll(std::time::Duration::from_millis(16))? {
            loop {
                let ev = crossterm::event::read()?;
                if let Some(action) = handle_event(ev, &mut app_state, keybinds)? {
                    match action {
                        EventAction::Quit => {
                            app_state.running = false;
                        }
                        EventAction::Save => {
                            app_state.save_canvas()?;
                        }
                    }
                }

                if !app_state.running || !crossterm::event::poll(std::time::Duration::from_millis(0))? {
                    break;
                }
            }
        }
    }
    
    Ok(None)
}
