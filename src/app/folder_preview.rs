use crate::list_view::FolderGraphNode;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct FolderPreviewCatalogNode {
    pub id: Arc<str>,
    pub title: Arc<str>,
    pub tags: Vec<Arc<str>>,
    pub links: Vec<Arc<str>>,
    pub is_clin: bool,
    pub is_draw: bool,
    pub is_canvas: bool,
}

pub struct FolderPreviewCatalog {
    pub revision: u64,
    pub notes: Vec<FolderPreviewCatalogNode>,
    pub by_id: HashMap<Arc<str>, usize>,
    pub notes_by_folder: HashMap<String, Vec<usize>>,
    pub child_folders_by_parent: HashMap<String, Vec<String>>,
    pub pinned_indices: Vec<usize>,
}

impl FolderPreviewCatalog {
    pub fn build(
        revision: u64,
        notes: &[crate::storage::NoteSummary],
        catalog_folders: &[String],
        note_index: &crate::note_index::NoteIndex,
    ) -> Arc<Self> {
        let mut string_interner: HashMap<String, Arc<str>> = HashMap::new();
        let mut intern = |s: &str| -> Arc<str> {
            if let Some(existing) = string_interner.get(s) {
                existing.clone()
            } else {
                let arc: Arc<str> = Arc::from(s);
                string_interner.insert(s.to_string(), arc.clone());
                arc
            }
        };

        let cat_notes: Vec<FolderPreviewCatalogNode> = notes
            .iter()
            .map(|n| FolderPreviewCatalogNode {
                id: intern(&n.id),
                title: intern(&n.title),
                tags: n.tags.iter().map(|t| intern(t)).collect(),
                links: n.links.iter().map(|l| intern(l)).collect(),
                is_clin: n.id.ends_with(".clin"),
                is_draw: n.id.ends_with(".draw"),
                is_canvas: n.id.ends_with(".canvas"),
            })
            .collect();

        let mut by_id = HashMap::with_capacity(cat_notes.len());
        for (i, node) in cat_notes.iter().enumerate() {
            by_id.insert(node.id.clone(), i);
        }

        Arc::new(FolderPreviewCatalog {
            revision,
            notes: cat_notes,
            by_id,
            notes_by_folder: note_index.notes_by_folder.clone(),
            child_folders_by_parent: note_index.child_folders_by_parent.clone(),
            pinned_indices: note_index.pinned_indices.clone(),
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct GroupNode {
    pub path: String,
    pub label: String,
    pub items: Vec<FolderGraphNode>,
}

#[derive(Clone, Debug)]
pub(crate) struct FolderGraphModel {
    pub focused_path: String,
    pub focused_label: String,
    pub nodes: Vec<FolderGraphNode>,
    pub groups: Vec<GroupNode>,
}

pub struct FolderPreviewRequest {
    pub generation: u64,
    pub notes_revision: u64,
    pub focused_path: String,
    pub catalog: Arc<FolderPreviewCatalog>,
}

pub struct FolderPreviewResponse {
    pub generation: u64,
    pub notes_revision: u64,
    pub focused_path: String,
    pub model: Arc<FolderGraphModel>,
}

pub struct FolderPreviewService {
    pub generation: Arc<AtomicU64>,
    pub req_tx: SyncSender<FolderPreviewRequest>,
    pub res_rx: Receiver<FolderPreviewResponse>,
}

impl FolderPreviewService {
    pub fn spawn(pool: Arc<rayon::ThreadPool>) -> Self {
        let (req_tx, req_rx) = sync_channel::<FolderPreviewRequest>(1);
        let (res_tx, res_rx) = sync_channel::<FolderPreviewResponse>(1);
        let generation = Arc::new(AtomicU64::new(1));
        let worker_gen = generation.clone();

        std::thread::Builder::new()
            .name("folder-preview-worker".to_string())
            .spawn(move || {
                while let Ok(req) = req_rx.recv() {
                    if worker_gen.load(Ordering::SeqCst) != req.generation {
                        continue;
                    }

                    let model = build_preview_model(&req.catalog, &req.focused_path);
                    let response = FolderPreviewResponse {
                        generation: req.generation,
                        notes_revision: req.notes_revision,
                        focused_path: req.focused_path,
                        model: Arc::new(model),
                    };

                    let mut resp_payload = response;
                    loop {
                        if worker_gen.load(Ordering::SeqCst) != req.generation {
                            break;
                        }
                        match res_tx.try_send(resp_payload) {
                            Ok(()) => break,
                            Err(TrySendError::Full(payload)) => {
                                std::thread::sleep(Duration::from_millis(1));
                                resp_payload = payload;
                            }
                            Err(TrySendError::Disconnected(_)) => break,
                        }
                    }
                }
            })
            .expect("failed to spawn folder preview worker");

        FolderPreviewService {
            generation,
            req_tx,
            res_rx,
        }
    }
}

fn build_preview_model(
    catalog: &FolderPreviewCatalog,
    focused_path: &str,
) -> FolderGraphModel {
    let mut raw_nodes: Vec<FolderGraphNode> = Vec::new();

    if focused_path == crate::app::VIRTUAL_PINNED_PATH {
        for &idx in &catalog.pinned_indices {
            if let Some(n) = catalog.notes.get(idx) {
                raw_nodes.push(FolderGraphNode {
                    key: n.id.to_string(),
                    label: n.title.to_string(),
                    is_note: true,
                    x: 0.0,
                    y: 0.0,
                    links: n.links.iter().map(|l| l.to_string()).collect(),
                });
            }
        }
    } else {
        if let Some(subfolders) = catalog.child_folders_by_parent.get(focused_path) {
            for f in subfolders {
                let name = f.split('/').next_back().unwrap_or("").to_string();
                raw_nodes.push(FolderGraphNode {
                    key: f.clone(),
                    label: name,
                    is_note: false,
                    x: 0.0,
                    y: 0.0,
                    links: Vec::new(),
                });
            }
        }
        if let Some(note_indices) = catalog.notes_by_folder.get(focused_path) {
            for &idx in note_indices {
                if let Some(n) = catalog.notes.get(idx) {
                    raw_nodes.push(FolderGraphNode {
                        key: n.id.to_string(),
                        label: n.title.to_string(),
                        is_note: true,
                        x: 0.0,
                        y: 0.0,
                        links: n.links.iter().map(|l| l.to_string()).collect(),
                    });
                }
            }
        }
    }

    let focused_label = if focused_path.is_empty() {
        "Vault (Root)".to_string()
    } else {
        focused_path.rsplit('/').next().unwrap_or(focused_path).to_string()
    };

    if raw_nodes.len() <= 128 {
        return FolderGraphModel {
            focused_path: focused_path.to_string(),
            focused_label,
            nodes: raw_nodes,
            groups: Vec::new(),
        };
    }

    // Grouping for > 128 nodes
    let mut top_nodes: Vec<FolderGraphNode> = Vec::new();
    let mut groups: Vec<GroupNode> = Vec::new();
    let mut chunk_idx = 0;

    for chunk in raw_nodes.chunks(32) {
        chunk_idx += 1;
        let group_path = format!("__group__/{focused_path}/{chunk_idx}");
        let label = format!("Items · {}", chunk.len());
        let group = GroupNode {
            path: group_path.clone(),
            label: label.clone(),
            items: chunk.to_vec(),
        };
        groups.push(group);

        top_nodes.push(FolderGraphNode {
            key: group_path,
            label,
            is_note: false,
            x: 0.0,
            y: 0.0,
            links: Vec::new(),
        });
    }

    FolderGraphModel {
        focused_path: focused_path.to_string(),
        focused_label,
        nodes: top_nodes,
        groups,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::NoteSummary;

    #[test]
    fn folder_graph_preview_is_bounded() {
        let mut notes = Vec::new();
        for i in 0..1000 {
            notes.push(NoteSummary {
                id: format!("note_{i:04}.md"),
                title: format!("Note {i}"),
                updated_at: 1000,
                folder: String::new(),
                tags: vec![],
                pinned: false,
                links: vec![],
                size_bytes: 10,
            });
        }
        let note_index = crate::note_index::NoteIndex::build(1, &notes, &[], &[], 1000);
        let catalog = FolderPreviewCatalog::build(1, &notes, &[], &note_index);

        let model = build_preview_model(&catalog, "");
        assert!(model.nodes.len() <= 128);
        assert!(!model.groups.is_empty());
    }
}
