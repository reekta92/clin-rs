use super::*;
use crate::popups::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, TryRecvError, channel},
};

pub(crate) struct ImportTask {
    result: Receiver<anyhow::Result<(String, String)>>,
    cancelled: Arc<AtomicBool>,
    target: ImportTarget,
    note_id: Option<String>,
    folder: String,
}

impl Drop for ImportTask {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

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
        let input = input.trim();
        anyhow::ensure!(!input.is_empty(), "No path/URL entered");
        anyhow::ensure!(
            source != ImportSource::Clipboard,
            "Clipboard imports do not use a prompt"
        );
        anyhow::ensure!(self.import_task.is_none(), "An import is already running");
        if target == ImportTarget::AppendCurrent {
            let id = note_id.ok_or_else(|| anyhow::anyhow!("No note selected"))?;
            anyhow::ensure!(
                matches!(
                    std::path::Path::new(id)
                        .extension()
                        .and_then(|ext| ext.to_str()),
                    Some("md" | "txt")
                ),
                "Only Markdown and text notes accept appended content"
            );
        }
        self.save_before_session_replace()?;
        let folder = self.get_current_folder_context();
        let input = input.to_owned();
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancelled);
        let (sender, result) = channel();
        self.notes_worker_pool.spawn(move || {
            let _ = sender.send(crate::actions::import::convert_source(
                source, &input, &flag,
            ));
        });
        self.import_task = Some(ImportTask {
            result,
            cancelled,
            target,
            note_id: note_id.map(str::to_owned),
            folder,
        });
        self.set_default_status();
        Ok(())
    }

    pub(crate) fn import_event(&mut self, event: &crossterm::event::Event) -> bool {
        use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
        if self.import_task.is_none() {
            return false;
        }
        match event {
            Event::Key(key)
                if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL =>
            {
                false
            }
            Event::Key(key) if key.code == KeyCode::Esc && key.kind == KeyEventKind::Press => {
                self.import_task = None;
                self.set_temporary_status_static("Import cancelled");
                true
            }
            Event::Key(_) | Event::Mouse(_) | Event::Paste(_) => true,
            _ => false,
        }
    }

    pub(crate) fn poll_import(&mut self) -> bool {
        let Some(task) = &self.import_task else {
            return false;
        };
        let result = match task.result.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => Err(anyhow::anyhow!("Import worker stopped")),
        };
        let task = self.import_task.take().expect("import task is present");
        let result = result.and_then(|(title, content)| match task.target {
            ImportTarget::NewNote => {
                let id = std::path::Path::new(&task.folder).join(self.storage.new_note_id());
                let id = id
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("Invalid import destination"))?;
                let note = crate::storage::Note {
                    baseline: None,
                    title,
                    content,
                    tags: Vec::new(),
                    updated_at: crate::ui::now_unix_secs(),
                };
                let saved = self.storage.save_note(id, &note)?;
                self.refresh_note_single(None, &saved);
                self.load_and_open_note(&saved, None);
                self.set_temporary_status_static("Imported note saved");
                Ok(())
            }
            ImportTarget::AppendCurrent => {
                self.insert_content(task.target, task.note_id.as_deref(), title, content)
            }
        });
        if let Err(error) = result {
            let message = format!("Import failed: {error:#}");
            self.set_temporary_status(&message);
            self.messages
                .push(message, crate::app::messages::MessageSeverity::Warning);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{Duration, Instant},
    };

    fn fixture(root: &std::path::Path) -> App {
        crate::config::set_config_path_override(root.join("config.toml"));
        let vault = root.join("vault");
        fs::create_dir_all(&vault).unwrap();
        App::new(crate::storage::Storage {
            data_dir: vault.clone(),
            notes_dir: vault.clone(),
            config_dir: root.join("config"),
            templates_dir: vault.join(".clin/templates"),
            skip_dir_patterns: Vec::new(),
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        })
        .unwrap()
    }

    #[test]
    fn imports_can_cancel_and_finish_in_the_original_destination() -> anyhow::Result<()> {
        let _guard = crate::config::ConfigTestGuard::lock();
        let root = tempfile::tempdir()?;
        let mut app = fixture(root.path());
        let input = root.path().join("input.csv");
        fs::write(&input, "a,b\n1,2\n")?;
        app.default_folder = Some("destination".into());
        app.storage.create_folder("destination")?;
        app.apply_import(
            ImportSource::Csv,
            ImportTarget::NewNote,
            None,
            input.to_str().unwrap(),
        )?;
        let escape = crossterm::event::Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert!(app.import_event(&escape));
        assert!(!app.poll_import());
        assert!(app.storage.list_note_ids(false, false)?.is_empty());
        app.apply_import(
            ImportSource::Csv,
            ImportTarget::NewNote,
            None,
            input.to_str().unwrap(),
        )?;
        app.default_folder = Some("other".into());
        let deadline = Instant::now() + Duration::from_secs(3);
        while app.import_task.is_some() && Instant::now() < deadline {
            app.poll_import();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(app.import_task.is_none());
        let ids = app.storage.list_note_ids(false, false)?;
        assert_eq!(ids.len(), 1);
        assert!(ids[0].starts_with("destination/"));
        assert!(
            app.storage
                .load_note(&ids[0])?
                .content
                .contains("| 1 | 2 |")
        );
        Ok(())
    }

    #[test]
    fn appending_preserves_live_editor_text_and_its_save_baseline() -> anyhow::Result<()> {
        let _guard = crate::config::ConfigTestGuard::lock();
        let root = tempfile::tempdir()?;
        let mut app = fixture(root.path());
        app.storage.create_document("note.md", "body")?;
        app.load_and_open_note("note.md", None);
        app.editor
            .body
            .move_cursor(ratatui_textarea::CursorMove::End);
        app.editor.body.insert_str(" pending");
        app.insert_content(
            ImportTarget::AppendCurrent,
            Some("note.md"),
            "unused".into(),
            "imported".into(),
        )?;
        let note = app.storage.load_note("note.md")?;
        assert_eq!(note.content, "body pending\n\nimported");
        app.autosave().map_err(anyhow::Error::msg)?;
        assert_eq!(app.storage.load_note("note.md")?.content, note.content);
        Ok(())
    }
}
