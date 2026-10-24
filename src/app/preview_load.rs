//! One cancellable preview read in flight, one waiting request, and one retained source.
use crate::{draw::state::DrawData, storage::Storage};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender, TrySendError},
};

#[derive(Clone)]
pub(crate) enum Source {
    Markdown(Arc<str>),
    Draw(Arc<DrawData>),
    Canvas(Arc<pinstar::data::CanvasData>),
    Template(Arc<str>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Key {
    Note(String),
    Template(String),
}
struct Request {
    key: Key,
    cancel: Arc<AtomicBool>,
}
struct Response {
    key: Key,
    cancel: Arc<AtomicBool>,
    result: Result<Source, String>,
}
pub(crate) struct PreviewLoader {
    sender: SyncSender<Request>,
    receiver: Receiver<Response>,
    current: Option<Request>,
    waiting: Option<Request>,
    loaded: Option<Result<Source, String>>,
}
impl Drop for PreviewLoader {
    fn drop(&mut self) {
        self.invalidate();
    }
}
impl PreviewLoader {
    pub(crate) fn new(storage: Storage) -> Self {
        let (sender, requests) = mpsc::sync_channel::<Request>(1);
        let (responses, receiver) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("preview-loader".into())
            .spawn(move || {
                while let Ok(request) = requests.recv() {
                    if request.cancel.load(Ordering::Relaxed) {
                        continue;
                    }
                    let result = load(&storage, &request.key).map_err(|error| format!("{error:#}"));
                    let mut response = Response {
                        key: request.key,
                        cancel: request.cancel,
                        result,
                    };
                    loop {
                        if response.cancel.load(Ordering::Relaxed) {
                            break;
                        }
                        match responses.try_send(response) {
                            Ok(()) => break,
                            Err(TrySendError::Disconnected(_)) => return,
                            Err(TrySendError::Full(pending)) => {
                                response = pending;
                                std::thread::sleep(std::time::Duration::from_millis(2));
                            }
                        }
                    }
                }
            })
            .expect("failed to spawn preview loader");
        Self {
            sender,
            receiver,
            current: None,
            waiting: None,
            loaded: None,
        }
    }
    pub(crate) fn invalidate(&mut self) {
        if let Some(request) = self.current.take() {
            request.cancel.store(true, Ordering::Relaxed);
        }
        self.waiting = None;
        self.loaded = None;
    }
    pub(crate) fn cancel_pending(&mut self) {
        if self.is_pending() {
            self.invalidate();
        }
    }
    pub(crate) fn is_pending(&self) -> bool {
        self.current.is_some() && self.loaded.is_none()
    }
    pub(crate) fn source(&mut self, key: Key) -> Option<Result<Source, String>> {
        if self
            .current
            .as_ref()
            .is_none_or(|request| request.key != key)
        {
            self.invalidate();
            let cancel = Arc::new(AtomicBool::new(false));
            self.current = Some(Request {
                key: key.clone(),
                cancel: cancel.clone(),
            });
            self.waiting = Some(Request { key, cancel });
        }
        self.poll();
        self.loaded.clone()
    }
    pub(crate) fn poll(&mut self) -> bool {
        let mut changed = false;
        if let Some(request) = self.waiting.take() {
            match self.sender.try_send(request) {
                Ok(()) => {}
                Err(TrySendError::Full(request)) => self.waiting = Some(request),
                Err(TrySendError::Disconnected(_)) => {
                    self.loaded = Some(Err("Preview loader stopped".into()));
                    changed = true;
                }
            }
        }
        loop {
            let response = match self.receiver.try_recv() {
                Ok(response) => response,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    if self.is_pending() {
                        self.loaded = Some(Err("Preview loader stopped".into()));
                        changed = true;
                    }
                    break;
                }
            };
            if !response.cancel.load(Ordering::Relaxed)
                && self.current.as_ref().is_some_and(|request| {
                    request.key == response.key && Arc::ptr_eq(&request.cancel, &response.cancel)
                })
            {
                self.loaded = Some(response.result);
                changed = true;
            }
        }
        changed
    }
}
fn load(storage: &Storage, key: &Key) -> anyhow::Result<Source> {
    match key {
        Key::Template(folder) => Ok(Source::Template(
            std::fs::read_to_string(storage.folder_template_config_path(folder)?)?.into(),
        )),
        Key::Note(id) if id.ends_with(".draw") => Ok(Source::Draw(Arc::new(
            ::draw::DrawState::load(&storage.document_path(id)?)?.data,
        ))),
        Key::Note(id) if id.ends_with(".canvas") => Ok(Source::Canvas(Arc::new(
            serde_json::from_str(storage.load_document(id)?.source())?,
        ))),
        Key::Note(id) if id.ends_with(".base") => Ok(Source::Markdown(format!("![[{id}]]").into())),
        Key::Note(id) => Ok(Source::Markdown(storage.load_note(id)?.content.into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disconnected_loader_finishes_the_pending_request() {
        let (sender, requests) = mpsc::sync_channel(1);
        let (responses, receiver) = mpsc::sync_channel(1);
        let mut loader = PreviewLoader {
            sender,
            receiver,
            current: None,
            waiting: None,
            loaded: None,
        };
        assert!(loader.source(Key::Note("note.md".into())).is_none());
        drop(requests);
        drop(responses);
        assert!(loader.poll());
        assert!(!loader.is_pending());
        assert!(matches!(loader.loaded, Some(Err(_))));
        assert!(!loader.poll());
    }
    #[test]
    fn newest_request_wins_and_unchanged_sources_survive_resize() {
        let root = tempfile::tempdir().unwrap();
        let storage = Storage {
            data_dir: root.path().into(),
            notes_dir: root.path().into(),
            config_dir: root.path().join("config"),
            templates_dir: root.path().join("templates"),
            skip_dir_patterns: Vec::new(),
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        };
        std::fs::write(root.path().join("first.md"), "first").unwrap();
        std::fs::write(root.path().join("last.md"), "last").unwrap();
        let mut loader = PreviewLoader::new(storage);
        for _ in 0..100 {
            loader.source(Key::Note("first.md".into()));
            loader.source(Key::Note("last.md".into()));
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let content = loop {
            if let Some(result) = loader.source(Key::Note("last.md".into())) {
                break result.unwrap();
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(1));
        };
        assert!(matches!(content, Source::Markdown(ref text) if text.as_ref() == "last"));
        std::fs::remove_file(root.path().join("last.md")).unwrap();
        assert!(matches!(
            loader.source(Key::Note("last.md".into())),
            Some(Ok(Source::Markdown(_)))
        ));
        loader.invalidate();
        loop {
            if let Some(result) = loader.source(Key::Note("last.md".into())) {
                assert!(result.is_err());
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}
