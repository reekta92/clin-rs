use super::Action;
use crate::app::App;
use anyhow::Result;
use std::borrow::Cow;

pub struct InsertDateAction;

impl Action for InsertDateAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("editor.insert_date")
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Insert Date")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Insert current date/time at the cursor position using the configured format")
    }

    fn category(&self) -> super::ActionCategory {
        super::ActionCategory::Notes
    }

    fn glyph(&self) -> (&'static str, &'static str) {
        ("\u{f133}", "\u{1f4c5}")
    }

    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        if app.mode == crate::app::ViewMode::Edit
            && (app.editor.properties.focused || app.editor.properties.dialog.is_some())
        {
            if let Some(crate::properties::PropertiesDialog::Edit(input)) =
                &mut app.editor.properties.dialog
                && input.control == 2
                && matches!(
                    input.kind,
                    crate::property_model::PropertyKind::String
                        | crate::property_model::PropertyKind::Yaml
                )
            {
                input.value.insert_str(
                    chrono::Local::now()
                        .format(&app.config.editor.date_format)
                        .to_string(),
                );
            }
            return Ok(());
        }
        let s = chrono::Local::now()
            .format(&app.config.editor.date_format)
            .to_string();
        app.editor.body.insert_str(&s);
        app.request_editor_preview_update();
        Ok(())
    }
}
