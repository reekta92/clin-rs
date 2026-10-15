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
    pub(crate) host_snapshot: Option<DocumentSnapshot>,
    pub(crate) public_revision: u64,
    pub(crate) inline_origin: Option<super::embedding::InlineOrigin>,
    pub document: BaseDocument,
    pub view: usize,
    pub columns: Vec<String>,
    pub(super) layout_options: Option<super::layout_options::LayoutOptions>,
    pub metadata: Arc<VaultMetadata>,
    pub result: Result<Arc<QueryResult>>,
    pub(super) row_numbers: Vec<usize>,
    pub(super) group_ids: Vec<super::eval::Identity>,
}
impl LoadedBase {
    pub(crate) fn configuration_source(
        &self,
        app: &mut crate::app::App,
        document: &BaseDocument,
    ) -> Result<String> {
        if self.inline_origin.is_some() {
            app.ensure_inline_host_write(&self.snapshot)?;
        }
        self.inline_origin.as_ref().map_or_else(
            || Ok(document.source().to_owned()),
            |origin| origin.replace(self.snapshot.source(), document.source()),
        )
    }
}

struct Request {
    embedded: Option<super::embedding::Request>,
    generation: u64,
    id: String,
    view: Option<String>,
    environment: Environment,
    search: String,
    invalidate: bool,
    paths: std::collections::BTreeSet<String>,
    cancelled: Arc<AtomicBool>,
}
struct Response {
    embedded: Option<u64>,
    generation: u64,
    loaded: Result<LoadedBase>,
    metadata_ready: bool,
}

#[derive(PartialEq, Eq)]
struct ReuseKey {
    revision: u64,
    host: Option<String>,
    host_source: Option<(Arc<str>, bool)>,
    query: String,
}
struct ReusedResult {
    key: ReuseKey,
    result: Arc<QueryResult>,
}

pub struct BasesWorker {
    sender: Option<SyncSender<Request>>,
    receiver: Receiver<Response>,
    pending: Option<Request>,
    generation: u64,
    metadata_dirty: bool,
    paths_dirty: std::collections::BTreeSet<String>,
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
                let mut public_revision = 0_u64;
                let mut reused: Option<ReusedResult> = None;
                let mut host_snapshot: Option<(String, Arc<str>, bool, Arc<VaultMetadata>)> = None;
                while let Ok(request) = requests.recv() {
                    if request.cancelled.load(Ordering::Relaxed) {
                        continue;
                    }
                    let mut metadata_ready = false;
                    let loaded = (|| {
                        if request.invalidate || metadata.is_none() {
                            let fresh = storage
                                .rebuild_vault_metadata(metadata.as_deref(), &request.cancelled)?;
                            if request.cancelled.load(Ordering::Relaxed) {
                                bail!("Bases query cancelled");
                            }
                            metadata = Some(Arc::new(fresh));
                            public_revision = public_revision.wrapping_add(1);
                            host_snapshot = None;
                            reused = None;
                        } else if !request.paths.is_empty() {
                            let fresh = storage.refresh_vault_metadata(
                                metadata.as_ref().expect("cached public metadata"),
                                &request.paths,
                                &request.cancelled,
                            )?;
                            metadata = Some(Arc::new(fresh));
                            public_revision = public_revision.wrapping_add(1);
                            host_snapshot = None;
                            reused = None;
                        }
                        metadata_ready = true;
                        let metadata = metadata
                            .as_ref()
                            .context("public metadata unavailable")?
                            .clone();
                        let metadata = if let Some(embedded) = &request.embedded
                            && let (Some(host), Some(source)) =
                                (&embedded.host, &embedded.host_source)
                        {
                            if !host_snapshot
                                .as_ref()
                                .is_some_and(|(id, old, body_only, _)| {
                                    id == host && old == source && *body_only == embedded.body_only
                                })
                            {
                                host_snapshot = Some((
                                    host.clone(),
                                    source.clone(),
                                    embedded.body_only,
                                    Arc::new(metadata.with_host_source(
                                        host,
                                        source,
                                        embedded.body_only,
                                    )?),
                                ));
                            }
                            host_snapshot
                                .as_ref()
                                .expect("current public host snapshot")
                                .3
                                .clone()
                        } else {
                            metadata
                        };
                        let host_snapshot = request
                            .embedded
                            .as_ref()
                            .and_then(|embedded| embedded.host.as_ref())
                            .filter(|host| {
                                metadata
                                    .file(host)
                                    .is_some_and(|file| file.extension.eq_ignore_ascii_case("md"))
                            })
                            .map(|host| storage.load_document(host))
                            .transpose()?;
                        let (snapshot, document, inline_origin) =
                            if let Some(embedded) = &request.embedded {
                                embedded.load(&storage, &metadata)?
                            } else {
                                let snapshot = storage.load_document(&request.id)?;
                                let document = BaseDocument::parse(snapshot.source())?;
                                (snapshot, document, None)
                            };
                        let (view, _) = document.view(request.view.as_deref())?;
                        let query = Query::compile(&document, view);
                        let columns = query
                            .as_ref()
                            .map_or_else(|_| Vec::new(), |query| query.columns.clone());
                        let layout_options = query
                            .as_ref()
                            .ok()
                            .map(|query| query.layout_options.clone());
                        let result = query.and_then(|mut query| {
                            query.set_search(&request.search)?;
                            let key = query.reuse_key().map(|query| ReuseKey {
                                revision: public_revision,
                                host: request.environment.host.clone(),
                                host_source: request.embedded.as_ref().and_then(|embedded| {
                                    embedded
                                        .host_source
                                        .as_ref()
                                        .map(|source| (source.clone(), embedded.body_only))
                                }),
                                query,
                            });
                            if let Some(key) = &key
                                && let Some(cached) = &reused
                                && cached.key == *key
                            {
                                return Ok(cached.result.clone());
                            }
                            let result = Arc::new(query.execute(
                                &metadata,
                                &request.environment,
                                &request.cancelled,
                            )?);
                            if let Some(key) = key {
                                reused = Some(ReusedResult {
                                    key,
                                    result: result.clone(),
                                });
                            }
                            Ok(result)
                        });
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
                        let mut row_numbers = result
                            .as_ref()
                            .map_or_else(|_| Vec::new(), |result| vec![0; result.rows.len()]);
                        if let Ok(result) = &result {
                            for group in &result.groups {
                                for (number, index) in group.rows.iter().enumerate() {
                                    row_numbers[*index] = number + 1;
                                }
                            }
                        }
                        Ok(LoadedBase {
                            snapshot,
                            host_snapshot,
                            public_revision,
                            inline_origin,
                            document,
                            view,
                            columns,
                            layout_options,
                            metadata,
                            result,
                            group_ids,
                            row_numbers,
                        })
                    })();
                    if request.cancelled.load(Ordering::Relaxed) {
                        continue;
                    }
                    let mut response = Response {
                        embedded: request.embedded.as_ref().map(|embedded| embedded.key),
                        generation: request.generation,
                        loaded,
                        metadata_ready,
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
            paths_dirty: std::collections::BTreeSet::new(),
            cancelled: Arc::new(AtomicBool::new(false)),
            thread: Some(thread),
        })
    }
    pub fn invalidate(&mut self) {
        self.cancel();
        self.generation += 1;
        self.pending = None;
        self.metadata_dirty = true;
        self.paths_dirty.clear();
    }
    pub(crate) fn invalidate_paths(&mut self, paths: impl Iterator<Item = String>) {
        self.cancel();
        self.generation += 1;
        self.paths_dirty.extend(paths);
        if self.paths_dirty.len() > 1024 {
            self.invalidate();
        }
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
            embedded: None,
            generation: self.generation,
            id: id.into(),
            view,
            environment,
            search,
            invalidate: self.metadata_dirty,
            paths: self.paths_dirty.clone(),
            cancelled: self.cancelled.clone(),
        });
        self.send_pending();
    }
    pub(crate) fn request_embed(
        &mut self,
        embedded: super::embedding::Request,
        mut environment: Environment,
        search: String,
        invalidate: bool,
    ) {
        environment.host.clone_from(&embedded.host);
        let view = embedded
            .view
            .clone()
            .or_else(|| match &embedded.definition {
                super::embedding::Definition::File { view, .. } => view.clone(),
                _ => None,
            });
        self.cancel();
        self.metadata_dirty |= invalidate;
        self.cancelled = Arc::new(AtomicBool::new(false));
        self.generation += 1;
        self.pending = Some(Request {
            embedded: Some(embedded),
            generation: self.generation,
            id: String::new(),
            view,
            environment,
            search,
            invalidate: self.metadata_dirty,
            paths: self.paths_dirty.clone(),
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
        self.poll_completed()
            .and_then(|(embedded, loaded)| embedded.is_none().then_some(loaded))
    }
    pub(crate) fn poll_completed(&mut self) -> Option<(Option<u64>, Result<LoadedBase>)> {
        self.send_pending();
        let mut result = None;
        while let Ok(response) = self.receiver.try_recv() {
            if response.generation == self.generation && !self.cancelled.load(Ordering::Relaxed) {
                if response.metadata_ready {
                    self.metadata_dirty = false;
                    self.paths_dirty.clear();
                }
                result = Some((response.embedded, response.loaded));
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
