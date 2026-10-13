//! One cancelable query worker and one public metadata snapshot per vault host.

use super::{
    eval::Environment,
    format::BaseDocument,
    query::{Query, QueryResult},
};
use crate::storage::{Storage, documents::DocumentSnapshot, metadata::VaultMetadata};
use anyhow::{Context as _, Result, bail};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, SyncSender, TrySendError, sync_channel},
};
use std::thread::JoinHandle;

pub struct LoadedBase {
    pub snapshot: DocumentSnapshot,
    pub document: BaseDocument,
    pub view: usize,
    pub columns: Vec<String>,
    pub metadata: Arc<VaultMetadata>,
    pub result: Result<Arc<QueryResult>>,
    pub(super) group_ids: Vec<super::eval::Identity>,
}
struct Request {
    generation: u64,
    id: String,
    view: Option<String>,
    environment: Environment,
    search: String,
    invalidate: bool,
    cancelled: Arc<AtomicBool>,
}
struct Response {
    generation: u64,
    loaded: Result<LoadedBase>,
}

pub struct BasesWorker {
    sender: Option<SyncSender<Request>>,
    receiver: Receiver<Response>,
    pending: Option<Request>,
    generation: u64,
    metadata_dirty: bool,
    cancelled: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl BasesWorker {
    pub fn new(storage: Storage) -> Result<Self> {
        let (sender, requests) = sync_channel::<Request>(1);
        let (responses, receiver) = sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("bases-query".into())
            .spawn(move || {
                let mut metadata: Option<Arc<VaultMetadata>> = None;
                while let Ok(request) = requests.recv() {
                    if request.cancelled.load(Ordering::Relaxed) {
                        continue;
                    }
                    let loaded = (|| {
                        let snapshot = storage.load_document(&request.id)?;
                        let document = BaseDocument::parse(snapshot.source())?;
                        let (view, _) = document.view(request.view.as_deref())?;
                        if request.invalidate || metadata.is_none() {
                            let fresh = storage.load_vault_metadata(&request.cancelled)?;
                            if request.cancelled.load(Ordering::Relaxed) {
                                bail!("Bases query cancelled");
                            }
                            metadata = Some(Arc::new(fresh));
                        }
                        let metadata = metadata
                            .as_ref()
                            .context("public metadata unavailable")?
                            .clone();
                        let query = Query::compile(&document, view);
                        let columns = query
                            .as_ref()
                            .map_or_else(|_| Vec::new(), |query| query.columns.clone());
                        let result = query
                            .and_then(|mut query| {
                                query.set_search(&request.search)?;
                                query.execute(&metadata, &request.environment, &request.cancelled)
                            })
                            .map(Arc::new);
                        let group_ids = if let Ok(result) = &result {
                            let formulas = super::eval::Formulas::default();
                            let mut budget = super::eval::Budget::new();
                            let mut evaluator = super::eval::Evaluator::new(
                                &metadata,
                                &formulas,
                                "",
                                &request.environment,
                                &request.cancelled,
                                &mut budget,
                            );
                            result
                                .groups
                                .iter()
                                .map(|group| {
                                    evaluator.step()?;
                                    Ok(evaluator.identity(&group.value))
                                })
                                .collect::<Result<_>>()?
                        } else {
                            Vec::new()
                        };
                        Ok(LoadedBase {
                            snapshot,
                            document,
                            view,
                            columns,
                            metadata,
                            result,
                            group_ids,
                        })
                    })();
                    if request.cancelled.load(Ordering::Relaxed) {
                        continue;
                    }
                    let mut response = Response {
                        generation: request.generation,
                        loaded,
                    };
                    loop {
                        match responses.try_send(response) {
                            Ok(()) => break,
                            Err(TrySendError::Disconnected(_)) => return,
                            Err(TrySendError::Full(value)) => {
                                if request.cancelled.load(Ordering::Relaxed) {
                                    break;
                                }
                                response = value;
                                std::thread::sleep(std::time::Duration::from_millis(1));
                            }
                        }
                    }
                }
            })?;
        Ok(Self {
            sender: Some(sender),
            receiver,
            pending: None,
            generation: 0,
            metadata_dirty: true,
            cancelled: Arc::new(AtomicBool::new(false)),
            thread: Some(thread),
        })
    }
    pub fn invalidate(&mut self) {
        self.cancel();
        self.generation += 1;
        self.pending = None;
        self.metadata_dirty = true;
    }
    pub fn request(
        &mut self,
        id: &str,
        view: Option<String>,
        environment: Environment,
        search: String,
        invalidate: bool,
    ) {
        self.cancel();
        self.metadata_dirty |= invalidate;
        self.cancelled = Arc::new(AtomicBool::new(false));
        self.generation += 1;
        self.pending = Some(Request {
            generation: self.generation,
            id: id.into(),
            view,
            environment,
            search,
            invalidate: self.metadata_dirty,
            cancelled: self.cancelled.clone(),
        });
        self.send_pending();
    }
    fn send_pending(&mut self) {
        if let Some(request) = self.pending.take() {
            match self.sender.as_ref().expect("live worker").try_send(request) {
                Ok(()) => {}
                Err(TrySendError::Full(request)) => self.pending = Some(request),
                Err(TrySendError::Disconnected(_)) => {}
            }
        }
    }
    pub fn poll(&mut self) -> Option<Result<LoadedBase>> {
        self.send_pending();
        let mut result = None;
        while let Ok(response) = self.receiver.try_recv() {
            if response.generation == self.generation && !self.cancelled.load(Ordering::Relaxed) {
                if response.loaded.is_ok() {
                    self.metadata_dirty = false;
                }
                result = Some(response.loaded);
            }
        }
        result
    }
    pub fn cancel(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
        self.pending = None;
    }
}
impl Drop for BasesWorker {
    fn drop(&mut self) {
        self.cancel();
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn newest_generation_wins_and_private_files_never_enter_worker_snapshot() {
        let (vault, storage) = super::super::tests::vault();
        std::fs::write(vault.path().join("public.md"), "---\nscore: 2\n---\nBody").unwrap();
        std::fs::write(
            vault.path().join("secret.clin"),
            "not even valid ciphertext",
        )
        .unwrap();
        std::fs::write(vault.path().join("test.base"),"filters: file.ext == 'md'\nviews:\n  - {type: table, name: First, order: [file.name, score]}\n  - {type: list, name: Latest, order: [file.name]}\n").unwrap();
        let mut worker = BasesWorker::new(storage).unwrap();
        worker.request(
            "test.base",
            Some("First".into()),
            Environment::default(),
            "no matches".into(),
            false,
        );
        worker.request(
            "test.base",
            Some("Latest".into()),
            Environment::default(),
            "public".into(),
            false,
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let loaded = loop {
            if let Some(result) = worker.poll() {
                break result.unwrap();
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(1));
        };
        assert_eq!(loaded.view, 1);
        assert!(loaded.metadata.file("secret.clin").is_none());
        assert_eq!(loaded.result.unwrap().rows[0].id, "public.md");
    }
}
