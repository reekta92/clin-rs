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
