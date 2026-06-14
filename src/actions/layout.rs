use crate::actions::Action;
use crate::app::App;
use crate::config::NotesLayout;
use anyhow::Result;
use std::borrow::Cow;

pub struct ToggleLayoutAction;

impl Action for ToggleLayoutAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("toggle_notes_layout")
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Toggle Notes Layout")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Switch between Tree and Grid layout for the notes view")
    }
    fn category(&self) -> crate::actions::ActionCategory {
        crate::actions::ActionCategory::Settings
    }

    fn glyph(&self) -> &'static str {
        "\u{f0c9}"
    }

    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        app.list.notes_layout = match app.list.notes_layout {
            NotesLayout::Tree => NotesLayout::Grid,
            NotesLayout::Grid => NotesLayout::Tree,
        };
        // Ensure folder logic starts fresh when flipping layout
        app.list.visual_index = 0;
        app.list.grid_folder = app.get_current_folder_context();
        app.refresh_visual_list();
        Ok(())
    }

    fn name_dynamic(&self, app: &App) -> String {
        let state = match app.list.notes_layout {
            NotesLayout::Tree => "Tree",
            NotesLayout::Grid => "Grid",
        };
        format!("Toggle Notes Layout [{}]", state)
    }
}