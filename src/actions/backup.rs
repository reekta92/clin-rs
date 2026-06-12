use anyhow::Result;
use std::borrow::Cow;
use crate::app::App;
use crate::actions::Action;

pub struct OpenBackupAction;

impl Action for OpenBackupAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("backup.open")
    }
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Open Backup Dashboard")
    }
    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("View git backup status, commit history, and push to remote")
    }
    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        app.open_backup_view();
        Ok(())
    }
}
