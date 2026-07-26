use crate::draw::input::handle_event;
use crate::draw::render::draw_canvas;
use crate::draw::state::{DrawData, Viewport};
use crate::keybinds::Keybinds;

pub enum DrawEventAction {
    Quit,
    Save,
    OpenHelp,
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
    pub current_stroke: Option<crate::draw::state::Stroke>,
    pub last_area: Rect,
    pub last_mouse_pos: Option<(u16, u16)>,
    pub mouse_pos: Option<(u16, u16)>,
    pub text_editor: Option<(usize, TextArea<'static>)>,
    pub theme: crate::app_theme::AppThemeColors,
    pub active_shape_type: crate::draw::state::DrawShapeType,
    pub show_shape_selector: bool,
    pub creation_origin: Option<(f64, f64)>,
    pub preview_element: Option<crate::draw::state::DrawElement>,
    pub keybinds: Keybinds,
    pub show_grid: bool,
    pub seq_matcher: crate::keybinds::KeyMatcher,
    pub is_panning: bool,
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
            active_tool: crate::draw::state::DrawTool::Draw,
            current_stroke: None,
            last_area: Rect::default(),
            mouse_pos: None,
            last_mouse_pos: None,
            text_editor: None,
            theme,
            active_shape_type: crate::draw::state::DrawShapeType::Rect,
            show_shape_selector: false,
            creation_origin: None,
            preview_element: None,
            keybinds,
            show_grid: true,
            seq_matcher,
            is_panning: false,
        }
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
                DrawEventAction::Save => {
                    self.save_draw()?;
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
