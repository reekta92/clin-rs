use crate::storage::{FileStamp, NoteFileEntry, NoteSummary, Storage};
use anyhow::Result;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};
use std::time::{Duration, Instant};

const NOTE_CACHE_VERSION: u16 = 5;

#[derive(serde::Deserialize, serde::Serialize)]
struct PersistedNoteCache {
    version: u16,
    vault_digest: u64,
    show_hidden: bool,
    show_all: bool,
    folders: Vec<String>,
    entries: Vec<(String, FileStamp, NoteSummary)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PathChange {
    Upsert(String),
    Remove(String),
}

pub enum CatalogCommand {
    Reconcile {
        generation: u64,
        show_hidden: bool,
        show_all: bool,
    },
    Paths {
        generation: u64,
        changes: Vec<PathChange>,
    },
    PutKnown {
        generation: u64,
        summary: NoteSummary,
        stamp: FileStamp,
        old_id: Option<String>,
    },
    RemoveKnown {
        generation: u64,
        id: String,
    },
    Flush {
        ack: SyncSender<()>,
    },
    Shutdown,
}

pub enum CatalogEvent {
    Started {
        generation: u64,
        total: usize,
    },
    Delta {
        generation: u64,
        upserts: Vec<(NoteSummary, FileStamp)>,
        removed: Vec<String>,
        folders: Option<Vec<String>>,
        processed: usize,
        total: usize,
    },
    Finished {
        generation: u64,
        complete: bool,
        warnings: Vec<String>,
        snapshot: Vec<(NoteSummary, FileStamp)>,
    },
    Failed {
        generation: u64,
        message: String,
    },
}

pub(crate) struct BlockingCatalogLoad {
    pub summaries: Vec<NoteSummary>,
    pub map: HashMap<String, (FileStamp, NoteSummary)>,
    pub folders: Vec<String>,
    pub complete: bool,
}

pub(crate) fn load_notes_blocking(
    storage: &Storage,
    pool: &rayon::ThreadPool,
    show_hidden: bool,
    show_all: bool,
) -> Result<BlockingCatalogLoad> {
    let scan = storage.scan_vault(show_hidden, show_all)?;
    let parsed: Vec<_> = pool.install(|| {
        scan.files
            .par_iter()
            .map(|entry| {
                let res = storage.load_note_summary_from_entry(entry);
                (entry.id.clone(), entry.stamp, res)
            })
            .collect()
    });

    let mut summaries = Vec::with_capacity(parsed.len());
    let mut map = HashMap::with_capacity(parsed.len());

    for (id, stamp, res) in parsed {
        if let Ok(summary) = res {
            summaries.push(summary.clone());
            map.insert(id, (stamp, summary));
        }
    }

    summaries.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(BlockingCatalogLoad {
        summaries,
        map,
        folders: scan.folders,
        complete: scan.complete,
    })
}

#[allow(clippy::type_complexity)]
pub(crate) fn load_persisted_note_cache(
    _storage: &Storage,
    cache_path: &Path,
    vault_digest: u64,
    show_hidden: bool,
    show_all: bool,
) -> (
    Vec<NoteSummary>,
    HashMap<String, (FileStamp, NoteSummary)>,
    Vec<String>,
) {
    let raw = match std::fs::read(cache_path) {
        Ok(r) => r,
        Err(_) => return (Vec::new(), HashMap::new(), Vec::new()),
    };
    let cache: PersistedNoteCache = match bincode::serde::decode_from_slice(
        &raw,
        bincode::config::standard().with_limit::<{ 64 * 1024 * 1024 }>(),
    ) {
        Ok((c, _)) => c,
        Err(_) => return (Vec::new(), HashMap::new(), Vec::new()),
    };

    if cache.version != NOTE_CACHE_VERSION || cache.vault_digest != vault_digest {
        return (Vec::new(), HashMap::new(), Vec::new());
    }

    let mut summaries = Vec::with_capacity(cache.entries.len());
    let mut map = HashMap::with_capacity(cache.entries.len());

    for (id, stamp, summary) in cache.entries {
        if crate::templates::is_folder_template_config(Path::new(&id)) {
            continue;
        }
        if !show_hidden {
            let has_hidden = id.split('/').any(|s| s.starts_with('.'));
            if has_hidden {
                continue;
            }
        }
        if !show_all {
            let ext = Path::new(&id)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            let accepted = matches!(ext, "md" | "txt" | "draw" | "canvas" | "base")
                || crate::storage::is_image_ext(ext);
            if !accepted {
                continue;
            }
        }
        summaries.push(summary.clone());
        map.insert(id, (stamp, summary));
    }

    let folders = if !show_hidden {
        cache
            .folders
            .into_iter()
            .filter(|f| !f.split('/').any(|s| s.starts_with('.')))
            .collect()
    } else {
        cache.folders
    };

    (summaries, map, folders)
}

fn save_persisted_note_cache(
    _storage: &Storage,
    cache_path: &Path,
    vault_digest: u64,
    show_hidden: bool,
    show_all: bool,
    folders: &[String],
    map: &HashMap<String, (FileStamp, NoteSummary)>,
) -> Result<()> {
    let mut sorted_entries: Vec<_> = map
        .iter()
        .map(|(id, (stamp, summary))| (id.clone(), *stamp, summary.clone()))
        .collect();
    sorted_entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut sorted_folders = folders.to_vec();
    sorted_folders.sort();

    let cache_obj = PersistedNoteCache {
        version: NOTE_CACHE_VERSION,
        vault_digest,
        show_hidden,
        show_all,
        folders: sorted_folders,
        entries: sorted_entries,
    };

    let encoded = bincode::serde::encode_to_vec(&cache_obj, bincode::config::standard())?;
    let content = encoded;

    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    #[cfg(unix)]
    crate::fsutil::atomic_write_with_mode(cache_path, &content, 0o600)?;
    #[cfg(not(unix))]
    crate::fsutil::atomic_write(cache_path, &content)?;

    Ok(())
}

fn send_event(
    event_tx: &SyncSender<CatalogEvent>,
    event: CatalogEvent,
    gen_atomic: &Arc<AtomicU64>,
    current_gen: u64,
) -> bool {
    let mut payload = event;
    loop {
        if gen_atomic.load(Ordering::SeqCst) != current_gen {
            return false;
        }
        match event_tx.try_send(payload) {
            Ok(()) => return true,
            Err(TrySendError::Full(evt)) => {
                std::thread::sleep(Duration::from_millis(1));
                payload = evt;
            }
            Err(TrySendError::Disconnected(_)) => return false,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_catalog_worker(
    storage: Storage,
    pool: Arc<rayon::ThreadPool>,
    cache_path: PathBuf,
    vault_digest: u64,
    show_hidden: bool,
    show_all: bool,
    initial_map: HashMap<String, (FileStamp, NoteSummary)>,
    initial_folders: Vec<String>,
    initial_complete: bool,
    generation: Arc<AtomicU64>,
    cmd_rx: Receiver<CatalogCommand>,
    event_tx: SyncSender<CatalogEvent>,
) {
    std::thread::Builder::new()
        .name("catalog-worker".to_string())
        .spawn(move || {
            let mut map = initial_map;
            let mut folders = initial_folders;
            let mut baseline_complete = initial_complete;
            let mut dirty = false;
            let mut last_dirty_at: Option<Instant> = None;

            loop {
                if dirty
                    && baseline_complete
                    && last_dirty_at.is_some_and(|t| t.elapsed() >= Duration::from_secs(1))
                {
                    let _ = save_persisted_note_cache(
                        &storage,
                        &cache_path,
                        vault_digest,
                        show_hidden,
                        show_all,
                        &folders,
                        &map,
                    );
                    dirty = false;
                }

                let timeout = if dirty && baseline_complete {
                    let elapsed = last_dirty_at.map(|t| t.elapsed()).unwrap_or_default();
                    Duration::from_secs(1)
                        .saturating_sub(elapsed)
                        .max(Duration::from_millis(10))
                } else {
                    Duration::from_millis(100)
                };

                let cmd = match cmd_rx.recv_timeout(timeout) {
                    Ok(c) => c,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                };

                match cmd {
                    CatalogCommand::Shutdown => break,
                    CatalogCommand::Flush { ack } => {
                        if baseline_complete {
                            let _ = save_persisted_note_cache(
                                &storage,
                                &cache_path,
                                vault_digest,
                                show_hidden,
                                show_all,
                                &folders,
                                &map,
                            );
                            dirty = false;
                        }
                        let _ = ack.send(());
                    }
                    CatalogCommand::Reconcile {
                        generation: cmd_gen,
                        show_hidden: sh,
                        show_all: sa,
                    } => {
                        if generation.load(Ordering::SeqCst) != cmd_gen {
                            continue;
                        }

                        let scan = match storage.scan_vault(sh, sa) {
                            Ok(s) => s,
                            Err(err) => {
                                send_event(
                                    &event_tx,
                                    CatalogEvent::Failed {
                                        generation: cmd_gen,
                                        message: err.to_string(),
                                    },
                                    &generation,
                                    cmd_gen,
                                );
                                continue;
                            }
                        };

                        if !send_event(
                            &event_tx,
                            CatalogEvent::Started {
                                generation: cmd_gen,
                                total: scan.files.len(),
                            },
                            &generation,
                            cmd_gen,
                        ) {
                            continue;
                        }

                        if scan.complete {
                            folders = scan.folders.clone();
                        } else {
                            for f in &scan.folders {
                                if !folders.contains(f) {
                                    folders.push(f.clone());
                                }
                            }
                            folders.sort();
                        }

                        let scan_file_map: HashMap<String, FileStamp> =
                            scan.files.iter().map(|f| (f.id.clone(), f.stamp)).collect();

                        let mut to_parse = Vec::new();
                        for file in &scan.files {
                            let needs_reload = match map.get(&file.id) {
                                Some((existing_stamp, _)) => {
                                    existing_stamp.len != file.stamp.len
                                        || existing_stamp.modified_nanos
                                            != file.stamp.modified_nanos
                                        || file.stamp.modified_nanos.is_none()
                                }
                                None => true,
                            };
                            if needs_reload {
                                to_parse.push(file);
                            }
                        }

                        let total = scan.files.len();
                        let mut processed = total.saturating_sub(to_parse.len());
                        let mut is_first_chunk = true;
                        let mut aborted = false;

                        if to_parse.is_empty() {
                            let _ = send_event(
                                &event_tx,
                                CatalogEvent::Delta {
                                    generation: cmd_gen,
                                    upserts: Vec::new(),
                                    removed: Vec::new(),
                                    folders: Some(folders.clone()),
                                    processed: total,
                                    total,
                                },
                                &generation,
                                cmd_gen,
                            );
                        } else {
                            let mut idx = 0;
                            while idx < to_parse.len() {
                                if generation.load(Ordering::SeqCst) != cmd_gen {
                                    aborted = true;
                                    break;
                                }

                                let chunk_size = if idx == 0 { 128 } else { 512 };
                                let end = (idx + chunk_size).min(to_parse.len());
                                let chunk = &to_parse[idx..end];

                                let results: Vec<_> = pool.install(|| {
                                    chunk
                                        .par_iter()
                                        .map(|entry| {
                                            let summary_res =
                                                storage.load_note_summary_from_entry(entry);
                                            (entry.id.clone(), entry.stamp, summary_res)
                                        })
                                        .collect()
                                });

                                let mut upserts = Vec::new();
                                for (id, stamp, res) in results {
                                    if let Ok(summary) = res {
                                        map.insert(id, (stamp, summary.clone()));
                                        upserts.push((summary, stamp));
                                    }
                                }

                                processed += chunk.len();

                                let folders_opt = if is_first_chunk {
                                    is_first_chunk = false;
                                    Some(folders.clone())
                                } else {
                                    None
                                };

                                if !send_event(
                                    &event_tx,
                                    CatalogEvent::Delta {
                                        generation: cmd_gen,
                                        upserts,
                                        removed: Vec::new(),
                                        folders: folders_opt,
                                        processed,
                                        total,
                                    },
                                    &generation,
                                    cmd_gen,
                                ) {
                                    aborted = true;
                                    break;
                                }

                                idx = end;
                            }
                        }

                        if aborted {
                            continue;
                        }

                        if scan.complete {
                            let absent: Vec<String> = map
                                .keys()
                                .filter(|id| !scan_file_map.contains_key(*id))
                                .cloned()
                                .collect();

                            for id in &absent {
                                map.remove(id);
                            }

                            if !absent.is_empty() {
                                send_event(
                                    &event_tx,
                                    CatalogEvent::Delta {
                                        generation: cmd_gen,
                                        upserts: Vec::new(),
                                        removed: absent,
                                        folders: None,
                                        processed: total,
                                        total,
                                    },
                                    &generation,
                                    cmd_gen,
                                );
                            }

                            baseline_complete = true;
                            dirty = true;
                            last_dirty_at = Some(Instant::now());
                        }

                        send_event(
                            &event_tx,
                            CatalogEvent::Finished {
                                generation: cmd_gen,
                                complete: scan.complete,
                                warnings: scan.warnings,
                                snapshot: if scan.complete {
                                    map.values()
                                        .map(|(stamp, summary)| (summary.clone(), *stamp))
                                        .collect()
                                } else {
                                    Vec::new()
                                },
                            },
                            &generation,
                            cmd_gen,
                        );
                    }
                    CatalogCommand::Paths {
                        generation: cmd_gen,
                        changes,
                    } => {
                        if generation.load(Ordering::SeqCst) != cmd_gen {
                            continue;
                        }
                        for change in changes {
                            match change {
                                PathChange::Upsert(id) => {
                                    let path = storage.note_path(&id);
                                    if let Ok(meta) = std::fs::metadata(&path) {
                                        let modified_nanos = meta.modified().ok().and_then(|t| {
                                            t.duration_since(std::time::UNIX_EPOCH)
                                                .ok()
                                                .map(|d| d.as_nanos())
                                        });
                                        let stamp = FileStamp {
                                            modified_nanos,
                                            len: meta.len(),
                                        };
                                        let entry = NoteFileEntry {
                                            id: id.clone(),
                                            stamp,
                                        };
                                        if let Ok(summary) =
                                            storage.load_note_summary_from_entry(&entry)
                                        {
                                            map.insert(id.clone(), (stamp, summary.clone()));
                                            if baseline_complete {
                                                dirty = true;
                                                last_dirty_at = Some(Instant::now());
                                            }
                                            send_event(
                                                &event_tx,
                                                CatalogEvent::Delta {
                                                    generation: cmd_gen,
                                                    upserts: vec![(summary, stamp)],
                                                    removed: Vec::new(),
                                                    folders: None,
                                                    processed: 1,
                                                    total: 1,
                                                },
                                                &generation,
                                                cmd_gen,
                                            );
                                        }
                                    }
                                }
                                PathChange::Remove(id) => {
                                    map.remove(&id);
                                    if baseline_complete {
                                        dirty = true;
                                        last_dirty_at = Some(Instant::now());
                                    }
                                    send_event(
                                        &event_tx,
                                        CatalogEvent::Delta {
                                            generation: cmd_gen,
                                            upserts: Vec::new(),
                                            removed: vec![id],
                                            folders: None,
                                            processed: 1,
                                            total: 1,
                                        },
                                        &generation,
                                        cmd_gen,
                                    );
                                }
                            }
                        }
                    }
                    CatalogCommand::PutKnown {
                        generation: cmd_gen,
                        summary,
                        stamp,
                        old_id,
                    } => {
                        if generation.load(Ordering::SeqCst) != cmd_gen {
                            continue;
                        }
                        if let Some(ref old) = old_id
                            && old != &summary.id
                        {
                            map.remove(old);
                        }
                        let id = summary.id.clone();
                        map.insert(id.clone(), (stamp, summary.clone()));
                        if baseline_complete {
                            dirty = true;
                            last_dirty_at = Some(Instant::now());
                        }
                        let removed = if let Some(old) = old_id {
                            if old != id { vec![old] } else { Vec::new() }
                        } else {
                            Vec::new()
                        };
                        send_event(
                            &event_tx,
                            CatalogEvent::Delta {
                                generation: cmd_gen,
                                upserts: vec![(summary, stamp)],
                                removed,
                                folders: None,
                                processed: 1,
                                total: 1,
                            },
                            &generation,
                            cmd_gen,
                        );
                    }
                    CatalogCommand::RemoveKnown {
                        generation: cmd_gen,
                        id,
                    } => {
                        if generation.load(Ordering::SeqCst) != cmd_gen {
                            continue;
                        }
                        map.remove(&id);
                        if baseline_complete {
                            dirty = true;
                            last_dirty_at = Some(Instant::now());
                        }
                        send_event(
                            &event_tx,
                            CatalogEvent::Delta {
                                generation: cmd_gen,
                                upserts: Vec::new(),
                                removed: vec![id],
                                folders: None,
                                processed: 1,
                                total: 1,
                            },
                            &generation,
                            cmd_gen,
                        );
                    }
                }
            }
        })
        .expect("failed spawning catalog worker");
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{FileStamp, NoteSummary, Storage};
    use std::sync::mpsc::sync_channel;
    use tempfile::TempDir;

    fn make_test_storage(dir: &Path) -> Storage {
        Storage {
            data_dir: dir.to_path_buf(),
            config_dir: dir.to_path_buf(),
            notes_dir: dir.to_path_buf(),
            templates_dir: dir.to_path_buf(),
            skip_dir_patterns: vec![],
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        }
    }

    #[test]
    fn plaintext_cache_is_private_and_roundtrips() {
        let tmp = TempDir::new().unwrap();
        let storage = make_test_storage(tmp.path());
        let cache_path = tmp.path().join("cache/note_cache.bin");
        let legacy_path = tmp.path().join("cache/legacy.bin");
        std::fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        std::fs::write(&legacy_path, b"opaque legacy cache").unwrap();
        let digest = 42u64;

        let summary = NoteSummary {
            id: "folder/secret_note.md".to_string(),
            title: "Super Secret Title".to_string(),
            updated_at: 1_234_567_890,
            folder: "folder".to_string(),
            tags: vec!["secret".to_string()],
            pinned: true,
            links: vec![],
            size_bytes: 100,
        };
        let stamp = FileStamp {
            modified_nanos: Some(1_234_567_890_000_000_000),
            len: 100,
        };

        let mut map = HashMap::new();
        map.insert(
            "folder/secret_note.md".to_string(),
            (stamp, summary.clone()),
        );
        let folders = vec!["folder".to_string()];

        save_persisted_note_cache(&storage, &cache_path, digest, false, false, &folders, &map)
            .unwrap();

        assert_eq!(std::fs::read(&legacy_path).unwrap(), b"opaque legacy cache");
        let raw = std::fs::read(&cache_path).unwrap();
        let raw_str = String::from_utf8_lossy(&raw);
        assert!(raw_str.contains("Super Secret Title"));
        assert!(raw_str.contains("secret_note.md"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&cache_path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }

        let (reloaded_summaries, reloaded_map, reloaded_folders) =
            load_persisted_note_cache(&storage, &cache_path, digest, false, false);

        assert_eq!(reloaded_folders, folders);
        assert_eq!(reloaded_summaries.len(), 1);
        assert_eq!(reloaded_summaries[0].title, "Super Secret Title");
        assert_eq!(reloaded_map.get("folder/secret_note.md").unwrap().0, stamp);
    }

    #[test]
    fn catalog_incomplete_scan_does_not_prune() {
        let tmp = TempDir::new().unwrap();
        let storage = make_test_storage(tmp.path());
        let cache_path = tmp.path().join("cache/note_cache.bin");
        let digest = 1u64;

        let (summaries, _, _) =
            load_persisted_note_cache(&storage, &cache_path, digest, false, false);
        assert_eq!(summaries.len(), 0);
    }

    #[test]
    fn cancelled_batches_are_reconciled_even_after_the_worker_cached_them() -> Result<()> {
        let _guard = crate::config::ConfigTestGuard::lock();
        let root = TempDir::new()?;
        crate::config::set_config_path_override(root.path().join("config.toml"));
        let storage = make_test_storage(root.path());
        storage.create_document("removed.md", "before")?;
        let mut app = crate::app::App::new(storage.clone())?;
        std::fs::remove_file(root.path().join("removed.md"))?;
        storage.create_document("new.md", "new body")?;
        app.request_notes_reconcile();
        let mut queued = Vec::new();
        let mut saw_add = false;
        let mut saw_remove = false;
        while !saw_add || !saw_remove {
            let event = app.catalog_event_rx.recv_timeout(Duration::from_secs(5))?;
            if let CatalogEvent::Delta {
                upserts, removed, ..
            } = &event
            {
                saw_add |= upserts.iter().any(|(note, _)| note.id == "new.md");
                saw_remove |= removed.iter().any(|id| id == "removed.md");
            }
            queued.push(event);
        }
        app.request_notes_reconcile();
        for event in queued {
            app.handle_catalog_event(event);
        }
        for _ in 0..2 {
            loop {
                let event = app.catalog_event_rx.recv_timeout(Duration::from_secs(5))?;
                let finished = matches!(&event, CatalogEvent::Finished { generation, .. }
                    if *generation == app.catalog_generation.load(Ordering::SeqCst));
                app.handle_catalog_event(event);
                if finished {
                    break;
                }
            }
            assert_eq!(
                app.notes
                    .iter()
                    .map(|note| note.id.as_str())
                    .collect::<Vec<_>>(),
                ["new.md"]
            );
            app.request_notes_reconcile();
        }
        Ok(())
    }

    #[test]
    fn a_full_command_queue_retries_the_latest_reconciliation() -> Result<()> {
        let _guard = crate::config::ConfigTestGuard::lock();
        let root = TempDir::new()?;
        let mut app = crate::app::App::new(make_test_storage(root.path()))?;
        let (sender, receiver) = sync_channel(1);
        app.catalog_cmd_tx = sender;
        app.request_notes_reconcile();
        app.list.show_hidden_files = true;
        app.request_notes_reconcile();
        assert!(app.catalog_reconcile_pending);
        let _ = receiver.recv()?;
        app.retry_catalog_reconcile();
        assert!(!app.catalog_reconcile_pending);
        let CatalogCommand::Reconcile {
            generation,
            show_hidden,
            ..
        } = receiver.recv()?
        else {
            panic!("expected reconciliation");
        };
        assert_eq!(generation, app.catalog_generation.load(Ordering::SeqCst));
        assert!(show_hidden);
        Ok(())
    }

    #[test]
    fn catalog_generation_discards_old_batches() {
        let (tx, rx) = sync_channel(4);
        let gen_atomic = Arc::new(AtomicU64::new(2));

        let sent = send_event(
            &tx,
            CatalogEvent::Started {
                generation: 1,
                total: 10,
            },
            &gen_atomic,
            1,
        );
        assert!(!sent);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn load_notes_blocking_includes_pdf_when_show_all() {
        let tmp = TempDir::new().unwrap();
        let storage = make_test_storage(tmp.path());

        // Write a markdown note and a binary PDF file
        std::fs::write(tmp.path().join("a.md"), "# A").unwrap();
        std::fs::write(tmp.path().join("doc.pdf"), b"%PDF-1.4\n\x80\x81\x82").unwrap();

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap();

        // With show_all=true, both files should appear
        let load = load_notes_blocking(&storage, &pool, false, true).unwrap();
        assert!(
            load.summaries.iter().any(|s| s.id == "a.md"),
            "md file should appear with show_all=true"
        );
        assert!(
            load.summaries.iter().any(|s| s.id == "doc.pdf"),
            "PDF should appear with show_all=true"
        );
        assert_eq!(load.summaries.len(), 2);

        // With show_all=false, only the md file should appear
        let load_filtered = load_notes_blocking(&storage, &pool, false, false).unwrap();
        assert!(
            load_filtered.summaries.iter().any(|s| s.id == "a.md"),
            "md file should appear with show_all=false"
        );
        assert!(
            !load_filtered.summaries.iter().any(|s| s.id == "doc.pdf"),
            "PDF should NOT appear with show_all=false"
        );
    }
}
