use super::App;
use crate::storage::{documents::DocumentSnapshot, drafts::DraftWorker};

impl App {
    pub(crate) fn queue_draft(&mut self, baseline: DocumentSnapshot, source: String) {
        let result = (|| {
            if self.drafts.is_none() {
                self.drafts = Some(DraftWorker::spawn(self.storage.clone())?);
            }
            self.drafts
                .as_ref()
                .expect("draft worker initialized")
                .submit(baseline, source)
        })();
        if let Err(error) = result {
            self.set_temporary_status(&format!("Recovery draft unavailable: {error:#}"));
        }
    }

    pub(crate) fn queue_editor_draft(&mut self) -> anyhow::Result<()> {
        self.editor.ensure_frontmatter()?;
        let baseline = self
            .editor
            .frontmatter_baseline()
            .ok_or_else(|| anyhow::anyhow!("missing note baseline"))?
            .clone();
        let state = self
            .editor
            .frontmatter
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("missing frontmatter state"))?;
        if let Ok(source) = self.prepare_editor_note_source() {
            let note = crate::storage::Note {
                baseline: Some(baseline.clone()),
                title: crate::events::get_title_text(&self.editor.title_editor).into_owned(),
                content: self.editor.body.text(),
                tags: Vec::new(),
                updated_at: crate::ui::now_unix_secs(),
            };
            let target = self.storage.note_target_id(baseline.id(), &note)?;
            self.queue_draft(baseline.for_destination(target), source);
            return Ok(());
        }
        let editor = crate::storage::document_recovery::EditorRecovery {
            id: baseline.id().to_owned(),
            before: baseline.exists().then(|| baseline.source().to_owned()),
            title: crate::events::get_title_text(&self.editor.title_editor).into_owned(),
            loaded_title: state.loaded_title.clone(),
            yaml: state.yaml(),
            body: self.editor.body.text(),
            input: state.input.as_ref().map(|input| {
                (
                    input.name.lines().join(""),
                    input.value.lines().join("\n"),
                    input.value_focus,
                    input.original.clone(),
                )
            }),
        };
        if self.drafts.is_none() {
            self.drafts = Some(DraftWorker::spawn(self.storage.clone())?);
        }
        self.drafts
            .as_ref()
            .expect("draft worker")
            .submit_editor(baseline, editor)
    }
    /// Restore one owner at a time; other sessions stay on disk until this one is saved.
    pub(crate) fn restore_editor_draft(&mut self) {
        let result = (|| -> anyhow::Result<()> {
            let drafts = self.storage.editor_drafts()?;
            let Some((baseline, draft)) = drafts.into_iter().last() else {
                return Ok(());
            };
            self.editor.frontmatter = None;
            self.editor.initial_word_count = if baseline.exists() {
                crate::frontmatter::properties::PropertyFrame::parse(baseline.source())
                    .map(|frame| crate::goals::count_words(frame.body))
                    .unwrap_or(0)
            } else {
                0
            };
            self.editor.editing_id = Some(draft.id);
            self.editor.note_baseline = Some(baseline);
            self.editor.title_editor = crate::events::make_title_editor(
                &draft.title,
                self.app_theme.highlight_fg,
                self.app_theme.highlight_bg,
            );
            self.editor.body = crate::editor_document::EditorDocument::from_source(&draft.body);
            self.apply_editor_prefs();
            let state = self
                .editor
                .frontmatter
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("cannot restore note metadata"))?;
            state.raw = ratatui_textarea::TextArea::from(draft.yaml.split('\n'));
            state.loaded_title = draft.loaded_title;
            state.visible = true;
            state.input = draft.input.map(|(name, value, value_focus, original)| {
                crate::editor::frontmatter::PropertyInput {
                    original,
                    name: ratatui_textarea::TextArea::from([name]),
                    value: ratatui_textarea::TextArea::from(value.split('\n')),
                    value_focus,
                }
            });
            self.editor.autosave_status = crate::editor::AutosaveStatus::Unsaved;
            self.editor.autosave_timer = None;
            self.mode = crate::app::ViewMode::Edit;
            self.set_temporary_status_static("Recovered editor input. Repair frontmatter if needed, then save; the vault file is unchanged.");
            Ok(())
        })();
        if let Err(error) = result {
            self.set_temporary_status(&format!("Editor recovery retained: {error:#}"));
        }
    }

    pub(crate) fn flush_drafts(&self) -> anyhow::Result<()> {
        if let Some(worker) = &self.drafts {
            worker.flush()?;
        }
        Ok(())
    }

    pub(crate) fn clear_saved_draft(&self, snapshot: &DocumentSnapshot) -> anyhow::Result<()> {
        if let Some(worker) = &self.drafts {
            worker.forget(snapshot.recovery_id())?;
        }
        self.storage.clear_document_draft(snapshot)
    }

    pub(crate) fn poll_draft_errors(&mut self) -> bool {
        if let Some(error) = self.drafts.as_ref().and_then(DraftWorker::take_error) {
            self.set_temporary_status(&error);
            self.messages
                .push(error, crate::app::messages::MessageSeverity::Warning);
            return true;
        }
        false
    }
}
