use crate::draw::input::handle_event;
use crate::draw::render::draw_canvas;
use crate::draw::state::{DrawData, Viewport};
use crate::keybinds::Keybinds;
use crate::overlay::OverlayView;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::Stdout;

pub enum DrawEventAction {
    Quit,
    Save,
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
    pub text_editor: Option<(usize, TextArea<'static>)>,
    pub theme: crate::app_theme::AppThemeColors,
    pub active_shape_type: crate::draw::state::DrawShapeType,
    pub show_shape_selector: bool,
    pub creation_origin: Option<(f64, f64)>,
    pub preview_element: Option<crate::draw::state::DrawElement>,
    pub keybinds: Keybinds,
    pub seq_matcher: crate::keybinds::KeyMatcher,
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
            last_mouse_pos: None,
            text_editor: None,
            theme,
            active_shape_type: crate::draw::state::DrawShapeType::Rect,
            show_shape_selector: false,
            creation_origin: None,
            preview_element: None,
            keybinds,
            seq_matcher,
        }
    }

    pub fn save_draw(&self) -> anyhow::Result<()> {
        if let Some(id) = &self.current_file {
            let path = self.storage.note_path(id);
            let content = serde_json::to_string(&self.data)?;
            std::fs::write(path, content)?;
        }
        Ok(())
    }
}

impl OverlayView<()> for DrawAppState {
    fn render(
        &mut self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        _theme: &crate::app_theme::AppThemeColors,
        _config: &crate::config::ClinConfig,
    ) {
        self.last_area = area;
        draw_canvas(frame, self, area);
    }

    fn handle_event(
        &mut self,
        event: crossterm::event::Event,
        _terminal: &ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
        config: &mut crate::config::ClinConfig,
    ) -> anyhow::Result<Option<()>> {
        let keybinds = self.keybinds.clone();
        if let Some(action) = handle_event(event, self, &keybinds, config)? {
            match action {
                DrawEventAction::Quit => {
                    self.running = false;
                    self.save_draw()?;
                    return Ok(Some(()));
                }
                DrawEventAction::Save => {
                    self.save_draw()?;
                }
            }
        }
        Ok(None)
    }

    fn title(&self) -> String {
        "Draw".to_string()
    }
}

pub fn run_draw_view(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    storage: crate::storage::Storage,
    keybinds: &Keybinds,
    file_id: Option<String>,
    theme: crate::app_theme::AppThemeColors,
    seq_matcher: &mut crate::keybinds::KeyMatcher,
) -> anyhow::Result<Option<String>> {
    let mut app_state = DrawAppState::new(storage, file_id, theme, keybinds.clone(), seq_matcher.clone());
    let mut config = crate::config::ClinConfig::default();
    let theme_clone = app_state.theme.clone();
    crate::overlay::run_overlay(
        terminal,
        &mut app_state,
        &mut config,
        &theme_clone,
        std::time::Duration::from_millis(16),
    )?;
    Ok(None)
}
