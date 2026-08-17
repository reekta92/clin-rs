//! Session lifecycle: signal registration, background worker spawn,
//! and orderly backup flush on exit. Separated from the terminal event
//! loop so GUI hosts can reuse startup/shutdown without a terminal.

use crate::app::App;
use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct SessionGuard {
    backup_done_rx: std::sync::mpsc::Receiver<()>,
    pub watcher: Option<notify::RecommendedWatcher>,
}

/// = launch_tui body minus run_tui_session: first_run check, Storage::init,
/// App::new_deferred, init/startup warning messages, setup view, open_note_by_title.
pub fn bootstrap_app(open_title: Option<String>, force_setup: bool) -> Result<App> {
    let first_run = crate::config::ClinConfig::config_path()
        .map(|p| !p.exists())
        .unwrap_or(false);
    let (storage_res, init_warnings) = crate::storage::Storage::init();
    let (storage, startup_err) = match storage_res {
        Ok(s) => (s, None),
        Err(e) => (crate::storage::Storage::new_fallback(), Some(e.to_string())),
    };
    let mut app = App::new_deferred(storage)?;
    let _ = app.storage.recover_editor_draft();
    for w in init_warnings {
        app.messages
            .push(w, crate::app::messages::MessageSeverity::Warning);
    }
    if let Some(err) = startup_err {
        let msg =
            format!("Storage initialization failed: {err}. The app may not function correctly.");
        app.messages
            .push(msg, crate::app::messages::MessageSeverity::Fatal);
    }
    if first_run || force_setup {
        app.open_setup_view();
    }
    if let Some(title) = open_title
        && !app.open_note_by_title(&title)
    {
        eprintln!(
            "{}",
            crate::console::error(&format!("No note found with title: {title}"))
        );
        std::process::exit(1);
    }
    Ok(app)
}

/// = cleanup_orphaned_temp_files, signal registration (SIGINT/SIGTERM/SIGHUP/SIGQUIT
/// → SHOULD_EXIT/FORCE_QUIT atomics), backup worker spawn, image decode worker spawn,
/// fs watcher init. Sets app.backup_tx, app.image_decode_tx/rx, app.fs_event_rx, app.fs_overflow.
pub fn start_session(app: &mut App) -> SessionGuard {
    crate::fsutil::cleanup_orphaned_temp_files();

    let register_signal = |sig: std::os::raw::c_int| {
        let _ = unsafe {
            signal_hook::low_level::register(sig, || {
                crate::SHOULD_EXIT.store(true, Ordering::Release);
                if crate::SIGNAL_COUNT.fetch_add(1, Ordering::SeqCst) >= 1 {
                    crate::FORCE_QUIT.store(true, Ordering::Release);
                }
            })
        };
    };
    register_signal(signal_hook::consts::SIGINT);
    register_signal(signal_hook::consts::SIGTERM);
    #[cfg(unix)]
    {
        register_signal(signal_hook::consts::SIGHUP);
        register_signal(signal_hook::consts::SIGQUIT);
    }

    // Spawn the background backup worker.
    let (tx, done_rx) = crate::backup::worker::spawn(
        app.git_lock.clone(),
        app.backup_status.clone(),
        app.message_tx.clone(),
    );
    app.backup_tx = Some(tx);

    // Spawn the background image decode worker.
    let (decode_tx, decode_rx) = crate::image_render::worker::spawn();
    app.image_decode_tx = Some(decode_tx);
    app.image_decode_rx = Some(decode_rx);

    // Initialize the optional file system watcher.
    let watcher = if app.config.core.auto_refresh {
        use notify::{EventKind, RecursiveMode, Watcher};
        let (tx, rx) = std::sync::mpsc::sync_channel::<crate::app::WatchedFsEvent>(1024);
        let overflow = Arc::new(AtomicBool::new(false));
        app.fs_event_rx = Some(rx);
        app.fs_overflow = overflow.clone();

        let notes_path = app.storage.notes_dir.clone();
        let overflow_cb = overflow.clone();
        let mut watcher =
            match notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                let observed_at = std::time::Instant::now();
                let event = match res {
                    Ok(e) => e,
                    Err(_) => {
                        overflow_cb.store(true, Ordering::SeqCst);
                        return;
                    }
                };
                if event.need_rescan() {
                    overflow_cb.store(true, Ordering::SeqCst);
                    return;
                }
                if matches!(event.kind, EventKind::Access(_)) {
                    return;
                }
                if !event.paths.is_empty() {
                    let all_ignored = event.paths.iter().all(|p| {
                        let path_str = p.to_string_lossy();
                        path_str.contains("/.git/")
                            || path_str.contains("\\.git\\")
                            || path_str.ends_with(".tmp")
                            || path_str.ends_with(".lock")
                            || path_str.ends_with('~')
                    });
                    if all_ignored {
                        return;
                    }
                }

                if tx
                    .try_send(crate::app::WatchedFsEvent { observed_at, event })
                    .is_err()
                {
                    overflow_cb.store(true, Ordering::SeqCst);
                }
            }) {
                Ok(w) => Some(w),
                Err(e) => {
                    app.messages.push(
                        format!("File watcher failed to start; auto-refresh disabled: {e}"),
                        crate::app::messages::MessageSeverity::Warning,
                    );
                    None
                }
            };

        if let Some(w) = &mut watcher
            && let Err(e) = w.watch(&notes_path, RecursiveMode::Recursive)
        {
            app.messages.push(
                format!("File watcher cannot watch vault; auto-refresh disabled: {e}"),
                crate::app::messages::MessageSeverity::Warning,
            );
        }
        watcher
    } else {
        None
    };

    SessionGuard {
        backup_done_rx: done_rx,
        watcher,
    }
}

/// = the post-loop backup flush block verbatim (signal_exit / backup_on_quit /
/// done_rx deadline logic, incl. println!/eprintln! messages).
pub fn finish_session(app: &mut App, guard: SessionGuard) -> Result<()> {
    let signal_exit = crate::SHOULD_EXIT.load(Ordering::Acquire);

    if signal_exit {
        drop(app.backup_tx.take());
    } else if app.config.backup.enabled && app.config.backup.backup_on_quit {
        println!("Backing up…");
        let _ = app.backup_tx.as_ref().map(|tx| {
            tx.send(crate::backup::worker::BackupJob::Flush(
                "auto: backup on quit".into(),
            ))
        });
        drop(app.backup_tx.take());
        let deadline = std::time::Instant::now() + crate::backup::worker::FLUSH_BOUND;
        let timed_out = loop {
            if crate::FORCE_QUIT.load(Ordering::Acquire) {
                break true;
            }
            match guard
                .backup_done_rx
                .recv_timeout(std::time::Duration::from_millis(200))
            {
                Ok(()) => break false,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break true,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if std::time::Instant::now() >= deadline {
                        break true;
                    }
                }
            }
        };
        if timed_out {
            eprintln!("Backup still running in background; exiting.");
        } else {
            println!("Done.");
        }
        if let Some(msg) = app.backup_status.lock().take() {
            eprintln!("Backup warning: {msg}");
        }
    } else {
        drop(app.backup_tx.take());
        let deadline = std::time::Instant::now() + crate::backup::worker::FLUSH_BOUND;
        loop {
            if crate::FORCE_QUIT.load(Ordering::Acquire) {
                break;
            }
            match guard
                .backup_done_rx
                .recv_timeout(std::time::Duration::from_millis(200))
            {
                Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if std::time::Instant::now() >= deadline {
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Stop workers for an in-process vault rebootstrap without triggering backup-on-quit.
pub fn finish_session_for_rebootstrap(app: &mut App, guard: SessionGuard) -> Result<()> {
    drop(app.backup_tx.take());
    let deadline = std::time::Instant::now() + crate::backup::worker::FLUSH_BOUND;
    loop {
        match guard
            .backup_done_rx
            .recv_timeout(std::time::Duration::from_millis(200))
        {
            Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                if std::time::Instant::now() >= deadline =>
            {
                break;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
    Ok(())
}
