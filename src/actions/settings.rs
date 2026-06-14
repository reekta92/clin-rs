use crate::actions::{Action, ActionCategory};
use crate::app::App;
use anyhow::Result;
use std::borrow::Cow;

pub struct TogglePreviewPaneAction;

impl Action for TogglePreviewPaneAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("settings.preview_pane")
    }
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Toggle Preview Pane")
    }
    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Show or hide the preview pane in the notes list")
    }
    fn category(&self) -> ActionCategory {
        ActionCategory::Settings
    }
    fn glyph(&self) -> &'static str {
        "\u{f0db}"
    }
    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        app.toggle_preview();
        Ok(())
    }

    fn name_dynamic(&self, app: &App) -> String {
        let state = if app.list.preview_enabled {
            "On"
        } else {
            "Off"
        };
        format!("Toggle Preview Pane [{}]", state)
    }
}

pub struct ToggleLineNumbersAction;

impl Action for ToggleLineNumbersAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("settings.line_numbers")
    }
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Toggle Line Numbers")
    }
    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Show or hide line numbers in the note editor")
    }
    fn category(&self) -> ActionCategory {
        ActionCategory::Settings
    }
    fn glyph(&self) -> &'static str {
        "\u{f03a}"
    }
    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        app.toggle_show_line_numbers();
        Ok(())
    }

    fn name_dynamic(&self, app: &App) -> String {
        let state = if app.editor.show_line_numbers {
            "On"
        } else {
            "Off"
        };
        format!("Toggle Line Numbers [{}]", state)
    }
}

pub struct ToggleConfirmDeleteAction;

impl Action for ToggleConfirmDeleteAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("settings.confirm_delete")
    }
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Toggle Delete Confirmation")
    }
    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Ask for confirmation before moving notes to trash")
    }
    fn category(&self) -> ActionCategory {
        ActionCategory::Settings
    }
    fn glyph(&self) -> &'static str {
        "\u{f3ed}"
    }
    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        app.toggle_confirm_on_delete();
        Ok(())
    }

    fn name_dynamic(&self, app: &App) -> String {
        let state = if app.confirm_on_delete { "On" } else { "Off" };
        format!("Toggle Delete Confirmation [{}]", state)
    }
}

pub struct TogglePinnedOnTopAction;

impl Action for TogglePinnedOnTopAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("settings.pinned_on_top")
    }
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Toggle Pinned on Top")
    }
    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Keep pinned notes above others in the list")
    }
    fn category(&self) -> ActionCategory {
        ActionCategory::Settings
    }
    fn glyph(&self) -> &'static str {
        "\u{f08d}"
    }
    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        app.toggle_pinned_on_top();
        Ok(())
    }

    fn name_dynamic(&self, app: &App) -> String {
        let state = if app.pinned_on_top { "On" } else { "Off" };
        format!("Toggle Pinned on Top [{}]", state)
    }
}

pub struct ToggleConfirmQuitAction;

impl Action for ToggleConfirmQuitAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("settings.confirm_quit")
    }
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Toggle Quit Confirmation")
    }
    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Ask for confirmation before quitting clin")
    }
    fn category(&self) -> ActionCategory {
        ActionCategory::Settings
    }
    fn glyph(&self) -> &'static str {
        "\u{f08b}"
    }
    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        app.toggle_confirm_on_quit();
        Ok(())
    }

    fn name_dynamic(&self, app: &App) -> String {
        let state = if app.confirm_on_quit { "On" } else { "Off" };
        format!("Toggle Quit Confirmation [{}]", state)
    }
}

pub struct TogglePreviewEncryptionAction;

impl Action for TogglePreviewEncryptionAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("settings.preview_encryption")
    }
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Toggle Encrypted Note Preview")
    }
    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Show or hide previews of encrypted (.clin) notes")
    }
    fn category(&self) -> ActionCategory {
        ActionCategory::Settings
    }
    fn glyph(&self) -> &'static str {
        "\u{f06e}"
    }
    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        app.toggle_preview_encryption();
        Ok(())
    }

    fn name_dynamic(&self, app: &App) -> String {
        let state = if app.preview_encryption { "On" } else { "Off" };
        format!("Toggle Encrypted Note Preview [{}]", state)
    }
}

pub struct CycleSortAction;

impl Action for CycleSortAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("settings.cycle_sort")
    }
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Select Sort Order")
    }
    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Select the notes sort field and order from a list")
    }
    fn category(&self) -> ActionCategory {
        ActionCategory::Settings
    }
    fn glyph(&self) -> &'static str {
        "\u{f0dc}"
    }
    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        app.begin_sort_selection();
        Ok(())
    }

    fn name_dynamic(&self, app: &App) -> String {
        use crate::list_view::{SortField, SortOrder};
        let sort_desc = match (app.list.sort_field, app.list.sort_order) {
            (SortField::Modified, SortOrder::Descending) => "Modified (newest)",
            (SortField::Modified, SortOrder::Ascending) => "Modified (oldest)",
            (SortField::Title, SortOrder::Ascending) => "Title (A-Z)",
            (SortField::Title, SortOrder::Descending) => "Title (Z-A)",
        };
        format!("Sort Order: {}", sort_desc)
    }
}
