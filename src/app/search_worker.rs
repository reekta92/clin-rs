use crate::popups::{SearchLineHit, SearchNoteHit};
use crate::storage::Storage;
use rayon::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::time::Duration;

pub(crate) struct SearchRequest {
    pub generation: u64,
    pub query: String,
    pub candidate_ids: Box<[Arc<str>]>,
}

pub(crate) enum SearchEvent {
    Batch {
        generation: u64,
        hits: Vec<SearchNoteHit>,
        finished: bool,
        errors: usize,
        globally_truncated: bool,
    },
}

pub(crate) struct SearchWorker {
    pub req_tx: SyncSender<SearchRequest>,
    pub event_rx: Receiver<SearchEvent>,
}

impl SearchWorker {
    pub fn spawn(storage: Storage, pool: Arc<rayon::ThreadPool>) -> Self {
        let (req_tx, req_rx) = sync_channel::<SearchRequest>(1);
        let (event_tx, event_rx) = sync_channel::<SearchEvent>(2);
        let generation = Arc::new(AtomicU64::new(1));
        let worker_gen = generation.clone();

        std::thread::Builder::new()
            .name("search-worker".to_string())
            .spawn(move || {
                while let Ok(req) = req_rx.recv() {
                    if worker_gen.load(Ordering::SeqCst) != req.generation {
                        continue;
                    }

                    run_search(&storage, &pool, &req, &worker_gen, &event_tx);
                }
            })
            .expect("failed spawning search worker");

        SearchWorker { req_tx, event_rx }
    }
}

fn send_event(
    event_tx: &SyncSender<SearchEvent>,
    event: SearchEvent,
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

fn run_search(
    storage: &Storage,
    pool: &rayon::ThreadPool,
    req: &SearchRequest,
    gen_atomic: &Arc<AtomicU64>,
    event_tx: &SyncSender<SearchEvent>,
) {
    let current_gen = req.generation;
    let query = &req.query;
    let is_query_ascii = query.is_ascii();

    let mut global_line_count = 0usize;
    let mut cumulative_errors = 0usize;
    let mut globally_truncated = false;

    let chunks = req.candidate_ids.chunks(128);

    for chunk in chunks {
        if gen_atomic.load(Ordering::SeqCst) != current_gen {
            return;
        }

        let chunk_results: Vec<_> = pool.install(|| {
            chunk
                .par_iter()
                .map(|id_arc| {
                    let res = storage.load_note(id_arc);
                    (id_arc.clone(), res)
                })
                .collect()
        });

        let mut hits = Vec::new();

        for (id_arc, res) in chunk_results {
            if gen_atomic.load(Ordering::SeqCst) != current_gen {
                return;
            }

            if global_line_count >= 20_000 {
                globally_truncated = true;
                break;
            }

            let note = match res {
                Ok(n) => n,
                Err(_) => {
                    cumulative_errors += 1;
                    continue;
                }
            };

            let mut match_count = 0usize;
            let mut lines = Vec::new();
            let mut note_truncated = false;

            for (line_idx, line) in note.content.lines().enumerate() {
                let trimmed = line.trim();
                let is_hit = if is_query_ascii && trimmed.is_ascii() {
                    trimmed.to_ascii_lowercase().contains(query)
                } else {
                    trimmed.to_lowercase().contains(query)
                };

                if is_hit {
                    match_count += 1;
                    if lines.len() < 200 && global_line_count < 20_000 {
                        let snippet = crate::fsutil::truncate_ellipsis(trimmed, 56);
                        lines.push(SearchLineHit {
                            line_number: line_idx + 1,
                            snippet,
                        });
                        global_line_count += 1;
                    } else if lines.len() >= 200 {
                        note_truncated = true;
                    }
                }
            }

            if !lines.is_empty() {
                hits.push(SearchNoteHit {
                    note_id: id_arc,
                    match_count,
                    lines,
                    truncated: note_truncated,
                });
            }
        }

        if !hits.is_empty()
            && !send_event(
                event_tx,
                SearchEvent::Batch {
                    generation: current_gen,
                    hits,
                    finished: false,
                    errors: cumulative_errors,
                    globally_truncated,
                },
                gen_atomic,
                current_gen,
            )
        {
            return;
        }

        if globally_truncated {
            break;
        }
    }

    send_event(
        event_tx,
        SearchEvent::Batch {
            generation: current_gen,
            hits: Vec::new(),
            finished: true,
            errors: cumulative_errors,
            globally_truncated,
        },
        gen_atomic,
        current_gen,
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{Note, Storage};
    use tempfile::TempDir;

    fn make_test_storage(dir: &std::path::Path) -> Storage {
        Storage {
            data_dir: dir.to_path_buf(),
            config_dir: dir.to_path_buf(),
            notes_dir: dir.to_path_buf(),
            templates_dir: dir.to_path_buf(),
            key: [1u8; 32],
            skip_dir_patterns: vec![],
        }
    }

    #[test]
    fn grep_search_caps_and_reports_errors() {
        let tmp = TempDir::new().unwrap();
        let mut storage = make_test_storage(tmp.path());

        let mut body = String::new();
        for i in 1..=300 {
            body.push_str(&format!("line {i} target_keyword\n"));
        }
        let note = Note {
            title: "Test Caps".to_string(),
            content: body,
            updated_at: 1000,
            tags: vec![],
        };
        let saved_id = storage.save_note("cap_test.md", &note).unwrap();

        let pool = Arc::new(
            rayon::ThreadPoolBuilder::new()
                .num_threads(2)
                .build()
                .unwrap(),
        );
        let req = SearchRequest {
            generation: 1,
            query: "target_keyword".to_string(),
            candidate_ids: vec![Arc::from(saved_id.as_str())].into_boxed_slice(),
        };
        let (tx, rx) = sync_channel(10);
        let gen_atomic = Arc::new(AtomicU64::new(1));

        run_search(&storage, &pool, &req, &gen_atomic, &tx);

        let mut hits = Vec::new();
        while let Ok(evt) = rx.try_recv() {
            let SearchEvent::Batch {
                hits: batch_hits, ..
            } = evt;
            hits.extend(batch_hits);
        }

        assert_eq!(hits.len(), 1);
        let note_hit = &hits[0];
        assert_eq!(note_hit.match_count, 300);
        assert_eq!(note_hit.lines.len(), 200);
        assert!(note_hit.truncated);
    }
}
