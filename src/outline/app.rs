use crossterm::event::Event;

use crate::outline::state::OutlineState;
use crate::outline::{input, render};

/// Shared mapping from input::OutlineInput to Option<OverlayResult>,
/// eliminating the identical match in the Key and Mouse dispatch arms.
fn map_input_result(
    state: &OutlineState,
    res: input::OutlineInput,
) -> Option<crate::overlay::OverlayResult> {
    use crate::overlay::OverlayResult;
    match res {
        input::OutlineInput::Back => Some(OverlayResult::Exit),
        input::OutlineInput::Help => Some(OverlayResult::OpenHelp(crate::app::HelpTab::Notes)),
        input::OutlineInput::Open => {
            if !state.load_error && state.selected < state.nodes.len() {
                let line = state.nodes[state.selected].line;
                Some(OverlayResult::JumpToLine {
                    note_id: state.note_id.clone(),
                    line,
                })
            } else {
                Some(OverlayResult::Exit)
            }
        }
        input::OutlineInput::None => None,
    }
}

impl crate::overlay::OverlayView for OutlineState {
    fn overlay_render(
        &mut self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        app: &mut crate::app::App,
    ) {
        self.last_area = area;
        let keybinds = self.keybinds.clone();
        let app_status = app.status.as_ref();
        render::draw_outline(
            frame,
            area,
            self,
            &app.app_theme,
            &keybinds,
            &app.config,
            Some(app_status),
        );
    }

    fn overlay_handle_event(
        &mut self,
        event: crossterm::event::Event,
        app: &mut crate::app::App,
        _terminal: &ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    ) -> anyhow::Result<crate::overlay::OverlayResult> {
        let keybinds = self.keybinds.clone();
        match event {
            Event::Key(key) => {
                if key.kind == crossterm::event::KeyEventKind::Release {
                    return Ok(crate::overlay::OverlayResult::Continue);
                }
                let r = input::handle_input(self, key, &keybinds, &app.config);
                if let Some(result) = map_input_result(self, r) {
                    return Ok(result);
                }
            }
            Event::Mouse(mouse) => {
                let term_area = self.last_area;
                let res =
                    input::handle_outline_mouse(self, mouse, term_area, app.config.ui.scrollbars);
                if let Some(result) = map_input_result(self, res) {
                    return Ok(result);
                }
            }
            _ => {}
        }
        Ok(crate::overlay::OverlayResult::Continue)
    }
}
