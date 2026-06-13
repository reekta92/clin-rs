use crate::app_theme::AppThemeColors;
use crate::keybinds::{CanvasAction, Keybinds};
use crate::pinstar::input::{handle_pinstar_event, handle_pinstar_mouse};
use crate::pinstar::render::draw_pinstar_view;
use crate::pinstar::state::PinstarState;
use crate::storage::Storage;
use crossterm::event::{self, Event};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::Stdout;
use std::time::Duration;

pub enum PinstarResult {
    Normal,
    HelpRequested,
}

pub fn run_pinstar_view(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    storage: Storage,
    keybinds: &Keybinds,
    file_id: Option<String>,
    theme: AppThemeColors,
    ext_editor_enabled: bool,
    external_editor: Option<String>,
) -> anyhow::Result<PinstarResult> {
    let mut state = if let Some(id) = file_id {
        let path = storage.note_path(&id);
        let mut s = PinstarState::load(&path)?;
        s.ext_editor_enabled = ext_editor_enabled;
        s
    } else {
        anyhow::bail!("No file ID provided for Pinstar view");
    };

    state.footer_hint = format!(
        "{} switch focus · {} back · Arrows select · {} edit · {} save",
        keybinds.canvas_keys_display(CanvasAction::CycleFocus),
        keybinds.canvas_keys_display(CanvasAction::Quit),
        keybinds.canvas_keys_display(CanvasAction::EditOrConnect),
        keybinds.canvas_keys_display(CanvasAction::Save),
    );
    let mut running = true;

    while running {
        if state.trigger_ext_editor {
            state.trigger_ext_editor = false;
            if let Some(node_id) = &state.selected_node_id {
                let node_text = state
                    .data
                    .nodes
                    .iter()
                    .find(|n| n.id() == node_id)
                    .map(|n| n.text().to_string())
                    .unwrap_or_default();

                let temp_dir = std::env::temp_dir();
                let temp_id = uuid::Uuid::new_v4().to_string();
                let temp_file_path = temp_dir.join(format!("clin_pinstar_{}.md", temp_id));
                std::fs::write(&temp_file_path, &node_text)?;

                use crossterm::terminal::{
                    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
                };
                let _ = disable_raw_mode();
                let _ = crossterm::execute!(
                    std::io::stdout(),
                    LeaveAlternateScreen,
                    crossterm::event::DisableMouseCapture,
                );

                let editor = external_editor
                    .clone()
                    .or_else(|| std::env::var("VISUAL").ok())
                    .or_else(|| std::env::var("EDITOR").ok())
                    .unwrap_or_else(|| "vi".to_string());

                let parts: Vec<&str> = editor.split_whitespace().collect();
                let (program, editor_args) = parts
                    .split_first()
                    .map(|(p, a)| (*p, a.to_vec()))
                    .unwrap_or(("vi", vec![]));

                let mut command = std::process::Command::new(program);
                for arg in editor_args {
                    command.arg(arg);
                }
                command.arg(&temp_file_path);
                let _ = command.status();

                let _ = enable_raw_mode();
                let _ = crossterm::execute!(
                    std::io::stdout(),
                    EnterAlternateScreen,
                    crossterm::event::EnableMouseCapture,
                    crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
                );
                terminal.clear()?;

                if let Ok(new_text) = std::fs::read_to_string(&temp_file_path)
                    && new_text != node_text
                {
                    for node in &mut state.data.nodes {
                        if node.id() == node_id {
                            node.set_text(new_text);
                            break;
                        }
                    }
                    let _ = state.save();
                    state.sync_to_raw_editor();
                }
                let _ = std::fs::remove_file(&temp_file_path);
            }
        }

        terminal.draw(|frame| {
            let full = frame.area();
            let outer = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Length(1),
                    ratatui::layout::Constraint::Min(0),
                ])
                .split(full);
            crate::ui::draw_view_title_bar(frame, outer[0], "Canvas", &theme);
            draw_pinstar_view(frame, &mut state, &theme, outer[1]);
        })?;

        if event::poll(Duration::from_millis(100))? {
            let mut pending = true;
            while pending {
                let term_area: ratatui::layout::Rect = terminal.size()?.into();
                let outer = ratatui::layout::Layout::default()
                    .direction(ratatui::layout::Direction::Vertical)
                    .constraints([
                        ratatui::layout::Constraint::Length(1),
                        ratatui::layout::Constraint::Min(0),
                    ])
                    .split(term_area);
                let area = outer[1];
                match event::read()? {
                    Event::Key(key) => {
                        if !handle_pinstar_event(&mut state, key, &mut running, area, keybinds) {}
                    }
                    Event::Mouse(mouse) => {
                        handle_pinstar_mouse(&mut state, mouse, area);
                    }
                    _ => {}
                }
                pending = event::poll(Duration::ZERO)?;
            }
        }
    }

    if state.help_requested {
        Ok(PinstarResult::HelpRequested)
    } else {
        Ok(PinstarResult::Normal)
    }
}
