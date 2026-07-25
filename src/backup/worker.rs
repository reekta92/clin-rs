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
use crate::app::messages::{MessageSeverity, OverlayMessage};

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
    tx_msg: Sender<OverlayMessage>,
) -> (Sender<BackupJob>, Receiver<()>) {
    let (tx, rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("clin-backup-worker".into())
        .spawn(move || {
            worker_loop(&rx, &git_lock, &status, &tx_msg, &done_tx);
        })
        .expect("failed to spawn backup worker");
    (tx, done_rx)
}

fn worker_loop(
    rx: &Receiver<BackupJob>,
    git_lock: &Arc<Mutex<()>>,
    status: &Arc<Mutex<Option<String>>>,
    tx_msg: &Sender<OverlayMessage>,
    done: &Sender<()>,
) {
    loop {
        // 1. Block for the next job. Err ⇒ all senders dropped ⇒ shutdown.
        let first = match rx.recv() {
            Ok(job) => job,
            Err(_) => {
                // No coalesced message pending at top-of-loop; just signal done.
                let _ = done.send(());
                return;
            }
        };

        match first {
            // 2. Flush: drain any other immediately-available jobs
            BackupJob::Flush(msg) => {
                while rx.try_recv().is_ok() {}
                run_backup(git_lock, status, tx_msg, &msg);
            }
            // 3. Auto: record the message and debounce.
            BackupJob::Auto(msg) => {
                let mut current = msg;
                let start = Instant::now();
                'debounce: loop {
                    let remaining = DEBOUNCE.saturating_sub(start.elapsed());
                    if remaining.is_zero() {
                        break;
                    }
                    match rx.recv_timeout(remaining) {
                        Ok(BackupJob::Auto(m)) => {
                            current = m;
                        }
                        Ok(BackupJob::Flush(m)) => {
                            current = m;
                            while rx.try_recv().is_ok() {}
                            break 'debounce;
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => break,
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            run_backup(git_lock, status, tx_msg, &current);
                            let _ = done.send(());
                            return;
                        }
                    }
                }
                run_backup(git_lock, status, tx_msg, &current);
            }
        }
    }
}

/// Worker's per-job helper: resolve the live config, then delegate to
/// `perform`. Config is read per job (not cached) so runtime changes made in
/// the Backup view's settings are picked up.
fn run_backup(
    git_lock: &Arc<Mutex<()>>,
    status: &Arc<Mutex<Option<String>>>,
    tx_msg: &Sender<OverlayMessage>,
    message: &str,
) {
    let config = match ClinConfig::load().0 {
        Ok(c) => c,
        Err(e) => {
            let err_msg = format!("Backup worker failed: config load failed: {e}");
            *status.lock() = Some(err_msg.clone());
            tx_msg.send(OverlayMessage {
                id: 0,
                text: err_msg,
                severity: MessageSeverity::Warning,
                timestamp: Instant::now(),
            }).ok();
            return;
        }
    };
    let vault_path = match config.effective_storage_path() {
        Ok(p) => p,
        Err(e) => {
            let err_msg = format!("Backup worker failed: vault path resolution failed: {e}");
            *status.lock() = Some(err_msg.clone());
            tx_msg.send(OverlayMessage {
                id: 0,
                text: err_msg,
                severity: MessageSeverity::Warning,
                timestamp: Instant::now(),
            }).ok();
            return;
        }
    };
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
        return;
    }
    let result = (|| -> anyhow::Result<String> {
        let _guard = git_lock.lock();
        let git_ops = GitOps::init(vault_path)?;
        match git_ops.has_changes() {
            Ok(true) => {}
            Ok(false) => return Ok(String::new()),
            Err(e) => return Err(anyhow::anyhow!("backup skipped: status check failed: {e}")),
        }
        git_ops.add_all()?;
        git_ops.commit(message)?;
        if backup.auto_push
            && let Some(remote) = &backup.remote_name
        {
            git_ops.push(remote)?;
        }
        Ok(message.to_string())
    })();
    match result {
        Ok(msg) if msg.is_empty() => {
            *status.lock() = None;
        }
        Ok(_) => {
            *status.lock() = None;
        }
        Err(e) => {
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
        let (tx_msg, _) = mpsc::channel();
        let (tx, done_rx) = spawn(git_lock, status, tx_msg);

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

    #[test]
    fn test_run_backup_corrupt_config() {
        let _guard = crate::config::ConfigTestGuard::lock();

        let tmp = tempdir().expect("tempdir");
        let corrupt_config = tmp.path().join("config.toml");
        fs::write(&corrupt_config, "invalid toml [[ [[]").expect("write");
        crate::config::set_config_path_override(corrupt_config);

        let (git_lock, status) = locks();
        let (tx_msg, _) = mpsc::channel();
        run_backup(&git_lock, &status, &tx_msg, "Test auto commit");

        let status_val = status.lock().clone();
        assert!(status_val.is_some());
        assert!(status_val.unwrap().starts_with("Backup worker failed: config load failed:"));
    }
}
