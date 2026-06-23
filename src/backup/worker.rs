//! Background backup worker.
//!
//! A single dedicated thread owns all auto-backup git work so the main/UI
//! thread never blocks on libgit2. Jobs are debounced (`Auto`) or run
//! immediately (`Flush`). The worker resolves the live config per job so
//! runtime changes made in the Backup view's settings are picked up.

use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::backup::git_ops::GitOps;
use crate::config::{BackupConfig, ClinConfig};

fn log_backup(level: &str, message: impl std::fmt::Display) {
    let ts = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f");
    let _ = std::eprintln!("[{ts}] [{level:<5}] [backup] {message}");
}

/// A backup job for the worker.
pub enum BackupJob {
    /// Debounced — coalesced with other pending `Auto` jobs (on-save, interval).
    Auto(String),
    /// Runs immediately, bypassing debounce (on-quit).
    Flush(String),
}

/// Debounce window for `Auto` jobs. Multiple saves inside this window collapse
/// into a single commit.
const DEBOUNCE: Duration = Duration::from_secs(2);

/// Upper bound on the quit-time join. If the worker is still busy after this,
/// we exit and let it finish (or be killed) in the background.
pub const FLUSH_BOUND: Duration = Duration::from_secs(15);

/// Spawns the worker thread. Returns the job sender (store on `App`) and a
/// `done` receiver used to bound the quit-time join.
///
/// `done` receives a single `()` once the worker has shut down (all senders
/// dropped) — after draining and running any coalesced message.
pub fn spawn(
    git_lock: Arc<Mutex<()>>,
    status: Arc<Mutex<Option<String>>>,
) -> (Sender<BackupJob>, Receiver<()>) {
    let (tx, rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("clin-backup-worker".into())
        .spawn(move || {
            worker_loop(&rx, &git_lock, &status, &done_tx);
        })
        .expect("failed to spawn backup worker");
    (tx, done_rx)
}

fn worker_loop(
    rx: &Receiver<BackupJob>,
    git_lock: &Arc<Mutex<()>>,
    status: &Arc<Mutex<Option<String>>>,
    done: &Sender<()>,
) {
    log_backup("INFO", "Backup worker started");
    loop {
        // 1. Block for the next job. Err ⇒ all senders dropped ⇒ shutdown.
        let first = match rx.recv() {
            Ok(job) => job,
            Err(_) => {
                // No coalesced message pending at top-of-loop; just signal done.
                log_backup("INFO", "Backup worker shutting down");
                let _ = done.send(());
                return;
            }
        };

        match first {
            // 2. Flush: drain any other immediately-available jobs
            BackupJob::Flush(msg) => {
                log_backup("INFO", &format!("Backup flush: {msg}"));
                while rx.try_recv().is_ok() {}
                run_backup(git_lock, status, &msg);
            }
            // 3. Auto: record the message and debounce.
            BackupJob::Auto(msg) => {
                let debounce_ms = DEBOUNCE.as_millis();
                log_backup("DEBUG", &format!("Backup auto: {msg} (debouncing {debounce_ms}ms)"));
                let mut current = msg;
                let start = Instant::now();
                'debounce: loop {
                    let remaining = DEBOUNCE.saturating_sub(start.elapsed());
                    if remaining.is_zero() {
                        break;
                    }
                    match rx.recv_timeout(remaining) {
                        Ok(BackupJob::Auto(m)) => {
                            log_backup("DEBUG", &format!("Backup auto: coalesced, reason={m}"));
                            current = m;
                        }
                        Ok(BackupJob::Flush(m)) => {
                            current = m;
                            while rx.try_recv().is_ok() {}
                            break 'debounce;
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => break,
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            run_backup(git_lock, status, &current);
                            log_backup("INFO", "Backup worker shutting down");
                            let _ = done.send(());
                            return;
                        }
                    }
                }
                log_backup("INFO", &format!("Backup auto: starting commit ({current})"));
                run_backup(git_lock, status, &current);
            }
        }
    }
}

/// Worker's per-job helper: resolve the live config, then delegate to
/// `perform`. Config is read per job (not cached) so runtime changes made in
/// the Backup view's settings are picked up.
fn run_backup(git_lock: &Arc<Mutex<()>>, status: &Arc<Mutex<Option<String>>>, message: &str) {
    let config = ClinConfig::load().unwrap_or_default();
    let vault_path = config
        .effective_storage_path()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    perform(git_lock, status, &vault_path, &config.backup, message);
}

/// Pure backup body (lifted from the old `try_auto_backup_raw`), parameterized
/// by vault path + backup config so it is unit-testable without touching the
/// global config. The shared git lock is acquired only when git work actually
/// happens, so disabled configs and clean repos do no locking.
pub(crate) fn perform(
    git_lock: &Arc<Mutex<()>>,
    status: &Arc<Mutex<Option<String>>>,
    vault_path: &Path,
    backup: &BackupConfig,
    message: &str,
) {
    if !backup.enabled {
        log_backup("DEBUG", "Backup: disabled in config, skipping");
        return;
    }
    log_backup("INFO", &format!("Backup commit starting: {message}"));
    let result = (|| -> anyhow::Result<String> {
        let _guard = git_lock.lock();
        let git_ops = GitOps::init(vault_path)?;
        if !git_ops.has_changes().unwrap_or(false) {
            return Ok(String::new());
        }
        git_ops.add_all()?;
        git_ops.commit(message)?;
        log_backup("INFO", &format!("Backup commit created: {message}"));
        if backup.auto_push
            && let Some(remote) = &backup.remote_name
        {
            log_backup("INFO", &format!("Backup push to {remote}"));
            if let Err(e) = git_ops.push(remote) {
                log_backup("WARN", &format!("Backup push failed: {e}"));
                return Err(e);
            }
            log_backup("INFO", "Backup push completed");
        }
        Ok(message.to_string())
    })();
    match result {
        Ok(msg) if msg.is_empty() => {
            log_backup("DEBUG", "Backup: no changes to commit");
            *status.lock() = None;
        }
        Ok(_) => {
            *status.lock() = None;
        }
        Err(e) => {
            log_backup("ERROR", &format!("Backup git error: {e}"));
            *status.lock() = Some(e.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn locks() -> (Arc<Mutex<()>>, Arc<Mutex<Option<String>>>) {
        (Arc::new(Mutex::new(())), Arc::new(Mutex::new(None)))
    }

    #[test]
    fn perform_creates_commit() {
        // Configure git user name/email in the local repo so
        // git2::Signature::default() succeeds in CI where there is
        // no global git config.
        let tmp = tempdir().expect("tempdir");
        let vault = tmp.path();
        GitOps::init(vault).expect("init");
        {
            let repo = git2::Repository::open(vault).expect("open repo");
            let mut cfg = repo.config().expect("config");
            cfg.set_str("user.name", "test").expect("set user.name");
            cfg.set_str("user.email", "test@test.com")
                .expect("set user.email");
        }
        fs::write(vault.join("note.md"), "hello").expect("write");

        let (git_lock, status) = locks();
        perform(
            &git_lock,
            &status,
            vault,
            &BackupConfig {
                enabled: true,
                ..Default::default()
            },
            "t",
        );

        assert!(status.lock().is_none(), "status should be clean");
        let git_ops = GitOps::init(vault).expect("init");
        assert!(
            !git_ops.log(1).unwrap_or_default().is_empty(),
            "expected a commit"
        );
    }

    #[test]
    fn perform_records_error_status() {
        // A path that is a regular file cannot host a git repo, so GitOps::init
        // errors and perform must surface the error in `status`.
        let tmp = tempdir().expect("tempdir");
        let file_path = tmp.path().join("not-a-dir");
        fs::write(&file_path, "x").expect("write");

        let (git_lock, status) = locks();
        perform(
            &git_lock,
            &status,
            &file_path,
            &BackupConfig {
                enabled: true,
                ..Default::default()
            },
            "t",
        );

        assert!(status.lock().is_some(), "expected an error status");
    }

    #[test]
    fn perform_disabled_is_noop() {
        let tmp = tempdir().expect("tempdir");
        let vault = tmp.path();
        GitOps::init(vault).expect("init");
        fs::write(vault.join("note.md"), "hello").expect("write");

        let (git_lock, status) = locks();
        perform(
            &git_lock,
            &status,
            vault,
            &BackupConfig {
                enabled: false,
                ..Default::default()
            },
            "t",
        );

        assert!(status.lock().is_none(), "disabled must not set status");
        let git_ops = GitOps::init(vault).expect("init");
        assert!(
            git_ops.log(1).unwrap_or_default().is_empty(),
            "disabled must not commit"
        );
    }

    #[test]
    fn worker_shuts_down_on_drop() {
        let (git_lock, status) = locks();
        let (tx, done_rx) = spawn(git_lock, status);

        tx.send(BackupJob::Auto("a".into())).expect("send 1");
        tx.send(BackupJob::Auto("b".into())).expect("send 2");
        drop(tx);

        // Worker drains coalesced message, runs it (a no-op with the default
        // disabled config), and signals done.
        let received = done_rx.recv_timeout(FLUSH_BOUND);
        assert!(
            received.is_ok(),
            "worker should signal shutdown: {received:?}"
        );
    }
}
