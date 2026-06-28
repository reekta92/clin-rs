use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, mpsc};

use crate::config::MarkdownRendererKind;
use crate::markdown::style::MarkdownTheme;
use crate::markdown::{RenderResult, execute_render};

pub(crate) struct Job {
    pub content: zeroize::Zeroizing<String>,
    pub cols: u16,
    pub estimated_rows: u16,
    pub theme: MarkdownTheme,
    pub wrap: bool,
    pub syntax_hl: bool,
    pub renderer: MarkdownRendererKind,
    pub cancel: Arc<AtomicBool>,
    pub result_tx: mpsc::Sender<Option<RenderResult>>,
}

pub(crate) struct Worker {
    tx: mpsc::Sender<Job>,
    _handle: std::thread::JoinHandle<()>,
}

pub(crate) static WORKER: LazyLock<Option<Worker>> = LazyLock::new(|| {
    let (tx, rx) = mpsc::channel::<Job>();
    let handle = std::thread::Builder::new()
        .name("markdown-render".to_string())
        .stack_size(4 * 1024 * 1024)
        .spawn(move || worker_loop(rx))
        .ok()?;
    Some(Worker {
        tx,
        _handle: handle,
    })
});

/// Submit a job. On failure (worker never started, or send error) returns the
/// job back so the caller can extract its fields for an inline fallback render.
pub(crate) fn submit(job: Job) -> Result<(), Job> {
    match &*WORKER {
        Some(w) => match w.tx.send(job) {
            Ok(()) => Ok(()),
            Err(std::sync::mpsc::SendError(j)) => Err(j),
        },
        None => Err(job),
    }
}

fn worker_loop(rx: mpsc::Receiver<Job>) {
    while let Ok(job) = rx.recv() {
        // Stale job (its renderer was already dropped/cycled): skip cheaply.
        if job.cancel.load(Ordering::Relaxed) {
            let _ = job.result_tx.send(None);
            continue;
        }
        let res = execute_render(
            &job.content,
            job.cols,
            job.estimated_rows,
            &job.theme,
            job.wrap,
            job.syntax_hl,
            job.renderer,
            Arc::clone(&job.cancel),
        );
        // result_tx may already be dropped (renderer cycled) -> send ignored.
        let _ = job.result_tx.send(res);
    }
}
