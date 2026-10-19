//! Session lifecycle: signal registration, background worker spawn,
//! and orderly recovery flush on exit. Separated from the terminal event
//! loop so GUI hosts can reuse startup/shutdown without a terminal.

use crate::app::App;
use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct SessionGuard {
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
    // Explicit title lookup needs a complete catalog before entering the event loop.
    let mut app = if open_title.is_some() {
        App::new(storage)?
    } else {
        App::new_deferred(storage)?
    };
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
/// → SHOULD_EXIT/FORCE_QUIT atomics), recovery flushing, image decode worker spawn,
/// fs watcher init. Sets app.image_decode_tx/rx, app.fs_event_rx, app.fs_overflow.
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

    // Recovery writes are started lazily when an editor first queues a draft.

    // Spawn the background image decode worker (only when the feature is enabled).
    if app.config.features.images.is_enabled() {
        let (decode_tx, decode_rx) = crate::image_render::worker::spawn();
        app.image_decode_tx = Some(decode_tx);
        app.image_decode_rx = Some(decode_rx);
    }

    // Initialize the optional file system watcher.
    let watcher = if app.config.core.auto_refresh {
        use notify::{EventKind, RecursiveMode, Watcher};
        let (tx, rx) = std::sync::mpsc::sync_channel::<crate::app::WatchedFsEvent>(1024);
        let overflow = Arc::new(AtomicBool::new(false));
        app.fs_event_rx = Some(rx);
        app.fs_overflow = overflow.clone();

        let notes_path = app.storage.notes_dir.clone();
        let ignored_root = notes_path.clone();
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
                        p.strip_prefix(&ignored_root).is_ok_and(|relative| {
                            relative.components().next().is_some_and(|component| {
                                matches!(component.as_os_str().to_str(), Some(".clin" | ".git"))
                            })
                        }) || path_str.contains("/.git/")
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

    SessionGuard { watcher }
}

/// Stop session resources; editor persistence is flushed before returning.
pub fn finish_session(app: &mut App, _guard: SessionGuard) -> Result<()> {
    app.flush_drafts()?;
    Ok(())
}

pub fn finish_session_for_rebootstrap(app: &mut App, guard: SessionGuard) -> Result<()> {
    finish_session(app, guard)
}
