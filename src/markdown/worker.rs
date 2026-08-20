use super::builtin::PendingCodeBlock;
use super::cache::{HighlightKey, RenderKey};
use super::style::{RenderLine, RenderedDocument};
use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, LazyLock, Once};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RenderViewport {
    pub start: usize,
    pub height: usize,
}

pub(crate) fn pack_viewport(start: usize, height: usize) -> u64 {
    let s = (start as u32) as u64;
    let h = (height as u32) as u64;
    (s << 32) | h
}

pub(crate) fn unpack_viewport(packed: u64) -> (usize, usize) {
    let start = (packed >> 32) as usize;
    let height = (packed & 0xFFFF_FFFF) as usize;
    (start, height)
}

pub(crate) enum RenderEvent {
    LayoutReady {
        generation: u64,
        document: RenderedDocument,
    },
    CodeBlockReady {
        generation: u64,
        line_range: Range<usize>,
        lines: Vec<RenderLine>,
    },
    Complete {
        generation: u64,
    },
}

pub(crate) struct RenderJob {
    pub generation: u64,
    pub key: RenderKey,
    pub viewport: Arc<AtomicU64>,
    pub cancel: Arc<AtomicBool>,
    pub tx: mpsc::SyncSender<RenderEvent>,
}

pub(crate) static POOL: LazyLock<rayon::ThreadPool> = LazyLock::new(|| {
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .clamp(1, 4);
    rayon::ThreadPoolBuilder::new()
        .thread_name(|i| format!("clin-markdown-{}", i))
        .num_threads(threads)
        .build()
        .expect("failed to build markdown ThreadPool")
});

static PREWARM: Once = Once::new();

pub(crate) fn prewarm_syntax_assets(code_theme: Arc<str>) {
    PREWARM.call_once(|| {
        POOL.spawn(move || {
            super::builtin::load_syntax_assets(&code_theme);
        });
    });
}

fn block_distance(
    block_start: usize,
    block_end: usize,
    viewport_start: usize,
    viewport_height: usize,
) -> usize {
    let viewport_end = viewport_start.saturating_add(viewport_height);
    if block_start < viewport_end && block_end > viewport_start {
        0
    } else if block_end <= viewport_start {
        viewport_start.saturating_sub(block_end)
    } else {
        block_start.saturating_sub(viewport_end)
    }
}

pub(crate) fn submit(job: RenderJob) {
    POOL.spawn(move || {
        execute_job(job);
    });
}

fn execute_job(job: RenderJob) {
    let cancel = &job.cancel;
    if cancel.load(Ordering::Relaxed) {
        return;
    }

    let layout_res = match super::builtin::render_layout(
        &job.key.content,
        job.key.cols,
        &job.key.theme,
        &job.key.opts,
        cancel,
    ) {
        Some(res) => res,
        None => return,
    };

    if cancel.load(Ordering::Relaxed) {
        return;
    }

    if job
        .tx
        .send(RenderEvent::LayoutReady {
            generation: job.generation,
            document: layout_res.document,
        })
        .is_err()
    {
        return;
    }

    if cancel.load(Ordering::Relaxed) {
        return;
    }

    use std::collections::BTreeMap;
    let mut blocks_map = BTreeMap::new();
    for block in layout_res.code_blocks {
        blocks_map.insert((block.line_range.start, block.id), block);
    }

    if blocks_map.is_empty() {
        let _ = job.tx.send(RenderEvent::Complete {
            generation: job.generation,
        });
        return;
    }

    let shared_blocks = Arc::new(parking_lot::Mutex::new(blocks_map));

    let first_block = {
        let packed = job.viewport.load(Ordering::Relaxed);
        let (vp_start, vp_height) = unpack_viewport(packed);
        let mut map = shared_blocks.lock();

        let mut best_key = None;
        for (&key, block) in map.iter() {
            let dist = block_distance(
                block.line_range.start,
                block.line_range.end,
                vp_start,
                vp_height,
            );
            if dist == 0 {
                best_key = Some(key);
                break;
            }
        }

        if let Some(key) = best_key {
            map.remove(&key)
        } else {
            None
        }
    };

    if let Some(block) = first_block
        && process_single_block(&block, &job, cancel)
    {
        return;
    }

    let num_threads = POOL.current_num_threads();
    POOL.scope(|s| {
        for _ in 0..num_threads {
            let shared_blocks = Arc::clone(&shared_blocks);
            let job = &job;
            let cancel = &job.cancel;

            s.spawn(move |_| {
                loop {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }

                    let next_block = {
                        let packed = job.viewport.load(Ordering::Relaxed);
                        let (vp_start, vp_height) = unpack_viewport(packed);
                        let mut map = shared_blocks.lock();
                        if map.is_empty() {
                            break;
                        }

                        let mut best_key = None;
                        let mut min_dist = usize::MAX;

                        for (&key, block) in map.iter() {
                            let dist = block_distance(
                                block.line_range.start,
                                block.line_range.end,
                                vp_start,
                                vp_height,
                            );
                            if dist < min_dist {
                                min_dist = dist;
                                best_key = Some(key);
                            }
                        }

                        if let Some(key) = best_key {
                            map.remove(&key)
                        } else {
                            None
                        }
                    };

                    if let Some(block) = next_block {
                        if process_single_block(&block, job, cancel) {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            });
        }
    });

    if cancel.load(Ordering::Relaxed) {
        return;
    }

    let _ = job.tx.send(RenderEvent::Complete {
        generation: job.generation,
    });
}

fn process_single_block(block: &PendingCodeBlock, job: &RenderJob, cancel: &AtomicBool) -> bool {
    if cancel.load(Ordering::Relaxed) {
        return true;
    }

    let highlight_key = HighlightKey::new(
        Arc::clone(&block.language),
        Arc::from(job.key.opts.code_theme.as_str()),
        Arc::clone(&block.literal),
        block.literal_fingerprint,
    );

    let highlighted = if let Some(cached) = super::cache::get_highlight(&highlight_key) {
        cached
    } else if let Some(hl) = super::builtin::highlight_code_block(
        &block.language,
        &block.literal,
        &job.key.opts.code_theme,
        cancel,
    ) {
        super::cache::insert_highlight(highlight_key, Arc::clone(&hl));
        hl
    } else {
        if cancel.load(Ordering::Relaxed) {
            return true;
        }
        let line_count = super::builtin::code_lines(&block.literal).count();
        Arc::new(vec![None; line_count])
    };

    if cancel.load(Ordering::Relaxed) {
        return true;
    }

    if let Some(patch_lines) = super::builtin::render_code_patch(
        block,
        &highlighted,
        job.key.cols,
        &job.key.theme,
        &job.key.opts,
        cancel,
    ) {
        if cancel.load(Ordering::Relaxed) {
            return true;
        }

        if job
            .tx
            .send(RenderEvent::CodeBlockReady {
                generation: job.generation,
                line_range: block.line_range.clone(),
                lines: patch_lines,
            })
            .is_err()
        {
            return true;
        }
    }

    false
}
