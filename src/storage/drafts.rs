//! Coalesced recovery writes; durable flushes happen at save and session boundaries.
use super::{Storage, documents::DocumentSnapshot, write_guard::CoordinationGuard};
use anyhow::{Result, anyhow};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

enum DraftContent {
    Source(String),
    Editor(super::document_recovery::EditorRecovery),
}
type Jobs = Arc<Mutex<HashMap<uuid::Uuid, (DocumentSnapshot, DraftContent)>>>;
enum Command {
    Wake,
    Flush(mpsc::Sender<Result<(), String>>),
    Forget(uuid::Uuid, mpsc::Sender<()>),
    Stop,
}

pub(crate) struct DraftWorker {
    _lease: CoordinationGuard,
    pending: Jobs,
    commands: mpsc::SyncSender<Command>,
    error: Arc<Mutex<Option<String>>>,
    thread: Option<JoinHandle<()>>,
}

fn persist(storage: &Storage, pending: &Jobs, error: &Mutex<Option<String>>) -> Result<(), String> {
    let jobs = std::mem::take(&mut *pending.lock().expect("draft queue poisoned"));
    let mut failure = None;
    for (id, (baseline, source)) in jobs {
        let result = match &source {
            DraftContent::Source(source) => storage.write_document_draft(&baseline, source),
            DraftContent::Editor(editor) => storage.write_editor_draft(&baseline, editor),
        };
        if let Err(err) = result {
            failure = Some(format!(
                "Recovery draft failed; pending content retained in memory: {err:#}"
            ));
            pending
                .lock()
                .expect("draft queue poisoned")
                .entry(id)
                .or_insert((baseline, source));
        }
    }
    *error.lock().expect("draft error poisoned") = failure.clone();
    failure.map_or(Ok(()), Err)
}

impl DraftWorker {
    pub(crate) fn spawn(storage: Storage) -> Result<Self> {
        let lease = storage.recovery_guard(false)?;
        let pending: Jobs = Arc::new(Mutex::new(HashMap::new()));
        let error = Arc::new(Mutex::new(None));
        let (commands, receiver) = mpsc::sync_channel(1);
        let jobs = Arc::clone(&pending);
        let errors = Arc::clone(&error);
        let thread = thread::Builder::new()
            .name("note-recovery".into())
            .spawn(move || {
                while let Ok(mut command) = receiver.recv() {
                    if matches!(command, Command::Wake) {
                        let deadline = Instant::now() + Duration::from_millis(150);
                        loop {
                            match receiver
                                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                            {
                                Ok(Command::Wake) => {}
                                Ok(next) => {
                                    command = next;
                                    break;
                                }
                                Err(_) => break,
                            }
                        }
                    }
                    if let Command::Forget(id, reply) = command {
                        let mut pending = jobs.lock().expect("draft queue poisoned");
                        pending.remove(&id);
                        if pending.is_empty() {
                            *errors.lock().expect("draft error poisoned") = None;
                        }
                        let _ = reply.send(());
                        continue;
                    }
                    let result = persist(&storage, &jobs, &errors);
                    match command {
                        Command::Flush(reply) => {
                            let _ = reply.send(result);
                        }
                        Command::Stop => break,
                        Command::Wake => {}
                        Command::Forget(..) => unreachable!("handled before persistence"),
                    }
                }
            })?;
        Ok(Self {
            _lease: lease,
            pending,
            commands,
            error,
            thread: Some(thread),
        })
    }

    pub(crate) fn submit(&self, baseline: DocumentSnapshot, source: String) -> Result<()> {
        self.submit_content(baseline, DraftContent::Source(source))
    }
    pub(crate) fn submit_editor(
        &self,
        baseline: DocumentSnapshot,
        editor: super::document_recovery::EditorRecovery,
    ) -> Result<()> {
        self.submit_content(baseline, DraftContent::Editor(editor))
    }
    fn submit_content(&self, baseline: DocumentSnapshot, source: DraftContent) -> Result<()> {
        self.pending
            .lock()
            .map_err(|_| anyhow!("draft queue poisoned"))?
            .insert(baseline.recovery_id(), (baseline, source));
        match self.commands.try_send(Command::Wake) {
            Ok(()) | Err(mpsc::TrySendError::Full(_)) => Ok(()),
            Err(mpsc::TrySendError::Disconnected(_)) => Err(anyhow!("recovery worker stopped")),
        }
    }

    pub(crate) fn flush(&self) -> Result<()> {
        let (reply, result) = mpsc::channel();
        self.commands
            .send(Command::Flush(reply))
            .map_err(|_| anyhow!("recovery worker stopped"))?;
        result
            .recv()
            .map_err(|_| anyhow!("recovery worker stopped"))?
            .map_err(anyhow::Error::msg)
    }

    /// A durable main-file save supersedes queued recovery for this editor identity.
    pub(crate) fn forget(&self, id: uuid::Uuid) -> Result<()> {
        // The acknowledgement also waits for any write already in flight.
        let (reply, result) = mpsc::channel();
        self.commands
            .send(Command::Forget(id, reply))
            .map_err(|_| anyhow!("recovery worker stopped"))?;
        result
            .recv()
            .map_err(|_| anyhow!("recovery worker stopped"))?;
        Ok(())
    }

    pub(crate) fn take_error(&self) -> Option<String> {
        self.error.lock().expect("draft error poisoned").take()
    }
}

#[cfg(test)]
mod tests;

impl Drop for DraftWorker {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
