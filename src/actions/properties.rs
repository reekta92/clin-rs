use super::{Action, ActionCategory};
use crate::app::App;
use crate::property_management::PropertyManagerMode;
use anyhow::Result;
use std::borrow::Cow;

pub struct ManagePropertyDefinitionsAction;

impl Action for ManagePropertyDefinitionsAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("properties.definitions")
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Manage Property Definitions")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Manage vault property definitions and schema types")
    }

    fn category(&self) -> ActionCategory {
        ActionCategory::Notes
    }

    fn glyph(&self) -> (&'static str, &'static str) {
        ("", "")
    }

    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        app.open_property_manager(PropertyManagerMode::Definitions, None);
        Ok(())
    }
}

pub struct BulkEditPropertiesAction;

impl Action for BulkEditPropertiesAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("properties.bulk")
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Bulk Edit Properties")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Add, update, or remove properties on selected notes")
    }

    fn category(&self) -> ActionCategory {
        ActionCategory::Notes
    }

    fn glyph(&self) -> (&'static str, &'static str) {
        ("", "")
    }

    fn execute(&self, app: &mut App, context_note_id: Option<&str>) -> Result<()> {
        app.open_property_manager(PropertyManagerMode::Bulk, context_note_id);
        Ok(())
    }
}

pub struct RenamePropertyAction;

impl Action for RenamePropertyAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("properties.rename")
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Rename Property")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Rename a property key in the active note")
    }

    fn category(&self) -> ActionCategory {
        ActionCategory::Notes
    }

    fn glyph(&self) -> (&'static str, &'static str) {
        ("", "")
    }

    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        app.open_property_manager(PropertyManagerMode::Rename { global: false }, None);
        Ok(())
    }
}

pub struct RenamePropertyAcrossVaultAction;

impl Action for RenamePropertyAcrossVaultAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("properties.rename_vault")
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Rename Property Across Vault")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Rename a property key across all notes in the vault")
    }

    fn category(&self) -> ActionCategory {
        ActionCategory::Notes
    }

    fn glyph(&self) -> (&'static str, &'static str) {
        ("", "")
    }

    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        app.open_property_manager(PropertyManagerMode::Rename { global: true }, None);
        Ok(())
    }
}

pub struct ResumePropertyBatchAction;

impl Action for ResumePropertyBatchAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("properties.resume")
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Resume Property Batch")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Resume an interrupted property batch operation")
    }

    fn category(&self) -> ActionCategory {
        ActionCategory::Notes
    }

    fn glyph(&self) -> (&'static str, &'static str) {
        ("", "")
    }

    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        app.open_property_manager(PropertyManagerMode::Resume, None);
        Ok(())
    }
}

pub struct ApplyPropertyDefaultsAction;

impl Action for ApplyPropertyDefaultsAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("properties.defaults")
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Apply Property Defaults")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Apply defined default property values to the active note")
    }

    fn category(&self) -> ActionCategory {
        ActionCategory::Notes
    }

    fn glyph(&self) -> (&'static str, &'static str) {
        ("", "")
    }

    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        app.apply_property_defaults()?;
        Ok(())
    }
}
