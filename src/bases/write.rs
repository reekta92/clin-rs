//! Background preparation and writes keep terminal input independent of bulk disk I/O.

use super::edits::{self, Transaction};
use crate::storage::{Storage, documents::DocumentSnapshot, metadata::VaultMetadata};
use anyhow::{Result, bail};
use serde_yaml_ng::Value;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, TryRecvError, channel},
};

pub enum Operation {
    Prepare {
        metadata: Arc<VaultMetadata>,
        ids: Vec<String>,
        column: String,
    },
    Commit {
        snapshots: Vec<DocumentSnapshot>,
        key: String,
        value: Option<Value>,
    },
    AssignGroup {
        snapshots: Vec<DocumentSnapshot>,
        key: String,
        value: Option<Value>,
        metadata: Arc<VaultMetadata>,
        source: String,
    },
    Replay {
        transaction: Transaction,
        forward: bool,
    },
    Save {
        baseline: DocumentSnapshot,
        source: String,
    },
    CreateNote {
        id: String,
        group: Option<(String, Option<Value>)>,
        metadata: Arc<VaultMetadata>,
        source: String,
    },
    Export {
        id: String,
        columns: Vec<String>,
        result: Arc<super::query::QueryResult>,
        markdown: bool,
    },
    Open {
        id: String,
        metadata: Arc<VaultMetadata>,
    },
}
pub enum Outcome {
    Prepared {
        snapshots: Vec<DocumentSnapshot>,
        key: String,
    },
    Changed(Transaction),
    Replayed {
        transaction: Transaction,
        forward: bool,
    },
    Saved(super::edits::Change),
    Created(DocumentSnapshot),
    Opened(DocumentSnapshot),
    Image {
        id: String,
        image: Box<image::DynamicImage>,
    },
}
pub struct WriteTask {
    receiver: Receiver<Result<Outcome>>,
    cancelled: Arc<AtomicBool>,
    pub writing: bool,
}
impl WriteTask {
    pub fn spawn(storage: Storage, pool: &rayon::ThreadPool, operation: Operation) -> Self {
        let writing = !matches!(
            operation,
            Operation::Prepare { .. } | Operation::Open { .. }
        );
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let (sender, receiver) = channel();
        pool.spawn(move || {
            let result = (|| {
                if flag.load(Ordering::Relaxed) {
                    bail!("Bases operation cancelled");
                }
                match operation {
                    Operation::Prepare {
                        metadata,
                        ids,
                        column,
                    } => {
                        let key = edits::property_key(&column)?.to_owned();
                        let snapshots = edits::prepare(&storage, &metadata, &ids, &flag)?;
                        Ok(Outcome::Prepared { snapshots, key })
                    }
                    Operation::Commit {
                        snapshots,
                        key,
                        value,
                    } => edits::commit(&storage, snapshots, &[(key, value)], &flag)
                        .map(Outcome::Changed),
                    Operation::AssignGroup {
                        snapshots,
                        key,
                        value,
                        metadata,
                        source,
                    } => edits::assign_group(
                        &storage,
                        &metadata,
                        snapshots,
                        &key,
                        value.as_ref(),
                        &source,
                        &flag,
                    )
                    .map(Outcome::Changed),
                    Operation::Replay {
                        transaction,
                        forward,
                    } => {
                        transaction.replay(&storage, forward, &flag)?;
                        Ok(Outcome::Replayed {
                            transaction,
                            forward,
                        })
                    }
                    Operation::Save { baseline, source } => {
                        let after = storage.save_document(&baseline, &source)?;
                        Ok(Outcome::Saved(super::edits::Change {
                            before: baseline,
                            after,
                        }))
                    }
                    Operation::CreateNote {
                        id,
                        group,
                        metadata,
                        source: origin,
                    } => {
                        if !id.ends_with(".md") {
                            bail!("new note path must end in .md");
                        }
                        let mut source = String::new();
                        if let Some((property, value)) = group {
                            if property == "file.folder" {
                                let folder = match value {
                                    Some(Value::String(folder)) => folder,
                                    None => String::new(),
                                    _ => bail!("folder group must be text"),
                                };
                                let parent = std::path::Path::new(&id)
                                    .parent()
                                    .and_then(std::path::Path::to_str)
                                    .unwrap_or("");
                                if parent != folder.trim_matches('/') {
                                    bail!("new note must stay in selected group folder");
                                }
                            } else {
                                let key = edits::property_key(&property)?;
                                let value = value
                                    .as_ref()
                                    .map(|value| {
                                        edits::retarget_group_links(&metadata, value, &origin, &id)
                                    })
                                    .transpose()?;
                                if let Some(value) = &value {
                                    storage.load_property_types()?.validate(key, value)?;
                                }
                                source = crate::frontmatter::properties::edit_properties(
                                    "",
                                    &[(key.to_owned(), value)],
                                )?;
                            }
                        }
                        if flag.load(Ordering::Relaxed) {
                            bail!("Bases operation cancelled");
                        }
                        storage.create_document(&id, &source).map(Outcome::Created)
                    }
                    Operation::Export {
                        id,
                        columns,
                        result,
                        markdown,
                    } => {
                        let source = super::export::export(&columns, &result, markdown)?;
                        storage.create_document(&id, &source).map(Outcome::Created)
                    }
                    Operation::Open { id, metadata } => {
                        let file = metadata.file(&id).ok_or_else(|| {
                            anyhow::anyhow!("file is absent from public metadata")
                        })?;
                        if crate::storage::is_image_ext(&file.extension) {
                            let bytes = storage.load_public_attachment(&id)?;
                            let image =
                                crate::image_render::worker::decode_public_image(bytes, 512)?;
                            if flag.load(Ordering::Relaxed) {
                                bail!("Bases operation cancelled");
                            }
                            Ok(Outcome::Image {
                                id,
                                image: Box::new(image),
                            })
                        } else {
                            if !matches!(file.extension.as_str(), "md" | "base") {
                                bail!("native preview is unavailable for this attachment type");
                            }
                            storage.load_document(&id).map(Outcome::Opened)
                        }
                    }
                }
            })();
            let _ = sender.send(result);
        });
        Self {
            receiver,
            cancelled,
            writing,
        }
    }
    pub fn poll(&self) -> Option<Result<Outcome>> {
        match self.receiver.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                Some(Err(anyhow::anyhow!("Bases write worker disconnected")
                    .context("original files must be reloaded before retry")))
            }
        }
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}
impl Drop for WriteTask {
    fn drop(&mut self) {
        self.cancel();
    }
}
