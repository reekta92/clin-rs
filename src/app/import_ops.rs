use super::*;
use crate::popups::*;

impl App {
    pub fn begin_import(
        &mut self,
        source: ImportSource,
        target: ImportTarget,
        folder: String,
        note_id: Option<String>,
    ) {
        if target == ImportTarget::NewNote && Self::is_virtual_path(&folder) {
            self.set_temporary_status_static("Cannot create note inside a virtual folder");
            return;
        }

        let prompt = match source {
            ImportSource::File => "File path - Esc cancel, Enter import",
            ImportSource::Csv => "CSV/TSV file path - Esc cancel, Enter import",
            ImportSource::Json => "JSON file path - Esc cancel, Enter import",
            ImportSource::Url => "URL - Esc cancel, Enter import",
            ImportSource::Clipboard => "Clipboard - Esc cancel, Enter import",
        };

        let mut input = crate::ui::make_popup_textarea(&self.app_theme, "");
        input.set_block(
            ratatui::widgets::Block::default()
                .style(self.app_theme.bg_style())
                .borders(ratatui::widgets::Borders::ALL)
                .title(prompt),
        );
        self.present_action_popup(crate::popups::ActivePopup::Import(ImportPopup {
            source,
            target,
            note_id,
            input,
        }));
    }

    pub fn confirm_import(&mut self) {
        let Some(crate::popups::ActivePopup::Import(popup)) = self.popups.active.take() else {
            return;
        };
        if let Err(error) = self.apply_import(
            popup.source,
            popup.target,
            popup.note_id.as_deref(),
            &popup.input.lines().join(""),
        ) {
            self.set_temporary_status(&format!("Import failed: {error:#}"));
        }
    }

    pub(crate) fn apply_import(
        &mut self,
        source: ImportSource,
        target: ImportTarget,
        note_id: Option<&str>,
        input: &str,
    ) -> anyhow::Result<()> {
        use crate::actions::import::*;
        let input = input.trim();
        anyhow::ensure!(!input.is_empty(), "No path/URL entered");
        let (title, md) = match source {
            ImportSource::File => convert_file(input),
            ImportSource::Csv => convert_csv(input),
            ImportSource::Json => convert_json(input),
            ImportSource::Url => convert_url(input),
            ImportSource::Clipboard => anyhow::bail!("Clipboard imports do not use a prompt"),
        }?;
        self.insert_content(target, note_id, title, md)
    }
}
