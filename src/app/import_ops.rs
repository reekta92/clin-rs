use super::*;
use crate::constants::*;
use crate::editor::*;
use crate::list_view::*;
use crate::popups::*;
use crate::keybinds::Keybinds;
use crate::storage::{Note, NoteSummary, Storage};
use crate::templates::Template;
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::borrow::Cow;
use std::time::{Duration, Instant};
use ratatui_textarea::TextArea;

impl App {


    pub fn enqueue_backup(&self, message: impl Into<String>) {
        if let Some(tx) = &self.backup_tx {
            let _ = tx.send(crate::backup::worker::BackupJob::Auto(message.into()));
        }
    }
    pub fn begin_import(
        &mut self,
        source: ImportSource,
        target: ImportTarget,
        folder: String,
        note_id: Option<String>,
    ) {
        if target == ImportTarget::NewNote && Self::is_virtual_pinned_path(&folder) {
            self.set_temporary_status_static("Cannot create note inside virtual Pinned");
            return;
        }

        let prompt = match source {
            ImportSource::File => "File path - Esc cancel, Enter import",
            ImportSource::Csv => "CSV/TSV file path - Esc cancel, Enter import",
            ImportSource::Json => "JSON file path - Esc cancel, Enter import",
            ImportSource::Url => "URL - Esc cancel, Enter import",
            ImportSource::Clipboard => "Clipboard - Esc cancel, Enter import",
        };

        let mut input = TextArea::default();
        input.set_cursor_line_style(ratatui::style::Style::default());
        input.set_style(self.app_theme.bg_style());
        input.set_block(
            ratatui::widgets::Block::default()
                .style(self.app_theme.bg_style())
                .borders(ratatui::widgets::Borders::ALL)
                .title(prompt),
        );

        self.popups.import = Some(ImportPopup {
            source,
            target,
            note_id,
            input,
        });
    }

    pub fn confirm_import(&mut self) {
        let Some(popup) = self.popups.import.take() else {
            return;
        };
        let input = popup.input.lines().join("").trim().to_string();
        if input.is_empty() {
            self.set_temporary_status_static("No path/URL entered");
            return;
        }

        use crate::actions::import::*;
        let result = match popup.source {
            ImportSource::File => convert_file(&input),
            ImportSource::Csv => convert_csv(&input),
            ImportSource::Json => convert_json(&input),
            ImportSource::Url => convert_url(&input),
            ImportSource::Clipboard => unreachable!(),
        };

        match result {
            Ok((title, md)) => {
                if let Err(e) =
                    self.insert_content(popup.target, popup.note_id.as_deref(), title, md)
                {
                    self.set_temporary_status(&format!("Import failed: {e:#}"));
                }
            }
            Err(e) => {
                self.set_temporary_status(&format!("Import failed: {e:#}"));
            }
        }
    }}
