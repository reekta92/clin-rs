use anyhow::{Context, Result};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use ratatui::layout::Rect;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::app::{App, EditFocus, ViewMode};
use crate::text_edit::{MouseTextSelection, TextEditTarget, copy_mouse_selection};

/// Run Edit mode without generic application queue draining or unconditional
/// redraws. The session remains in-process and mutates the same `App`.
pub(crate) fn run_editor_session<B: ratatui::backend::Backend>(
    terminal: &mut ratatui::Terminal<B>,
    app: &mut App,
    events: &mut crate::event_source::EventSource,
    pre_draw_hook: &mut dyn FnMut(&mut App) -> bool,
) -> Result<()>
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    let mut focus = EditFocus::Body;
    let mut mouse_selection = MouseTextSelection::default();
    let mut dirty = true;
    if let Some(change) = app.editor.body.take_change() {
        synchronize_source_highlight(app, change);
    }

    while !app.should_quit && app.mode == ViewMode::Edit {
        if crate::SHOULD_EXIT.load(Ordering::Acquire) {
            app.should_quit = true;
            break;
        }

        let revision_before_reload = app.notes_revision;
        app.check_and_reload_config();
        dirty |= app.notes_revision != revision_before_reload;
        dirty |= app.messages.tick_expirations();
        dirty |= app.tick_status();
        dirty |= app.poll_editor_renderers();
        dirty |= app.poll_editor_image_results();
        dirty |= app.tick_autosave();
        if let Some(requested) = app.editor.properties.focus_request.take() {
            focus = requested;
            dirty = true;
        }
        if focus == EditFocus::Properties
            && app.editor.sidebar != crate::editor::EditSidebar::Properties
        {
            focus = if app.editor.sidebar == crate::editor::EditSidebar::None {
                EditFocus::Body
            } else {
                EditFocus::Sidebar
            };
            dirty = true;
        }
        if app.preview_fullscreen && focus == EditFocus::Properties {
            focus = EditFocus::Body;
            dirty = true;
        }

        if dirty {
            if !(pre_draw_hook)(app) {
                terminal
                    .draw(|frame| crate::ui::draw_ui(frame, app, focus))
                    .context("editor frame draw failed")?;
            }
            dirty = false;
        }

        let editor_pending = app
            .editor
            .md_preview_renderer
            .as_ref()
            .is_some_and(crate::markdown::MarkdownRenderer::is_pending)
            || app.editor.pending_editor_preview_update;
        let timeout = if editor_pending {
            std::time::Duration::from_millis(16)
        } else if let Some(timer) = app.editor.autosave_timer {
            timer
                .saturating_duration_since(std::time::Instant::now())
                .min(std::time::Duration::from_millis(200))
        } else {
            std::time::Duration::from_millis(200)
        };
        if !events.poll(timeout).context("editor event poll failed")? {
            continue;
        }

        let mut pending = Vec::with_capacity(64);
        pending.push(events.read().context("editor event read failed")?);
        while pending.len() < 64 && events.poll(Duration::ZERO)? {
            pending.push(events.read()?);
        }
        for event in coalesce_editor_events(pending) {
            let body_rev_before = app.editor.body.revision();
            let title_before = crate::events::get_title_text(&app.editor.title_editor).into_owned();
            let properties_before = app.editor.properties.revision;

            dirty |= dispatch_editor_event(terminal, app, event, &mut focus, &mut mouse_selection)?;

            let body_rev_after = app.editor.body.revision();
            let title_after = crate::events::get_title_text(&app.editor.title_editor).into_owned();

            if body_rev_before != body_rev_after
                || title_before != title_after
                || properties_before != app.editor.properties.revision
            {
                if body_rev_before != body_rev_after
                    && let Some(change) = app.editor.body.take_change()
                {
                    synchronize_source_highlight(app, change);
                }
                if body_rev_before != body_rev_after
                    || properties_before != app.editor.properties.revision
                {
                    app.editor.links = app.compute_links();
                }
                app.editor.autosave_status = crate::editor::AutosaveStatus::Unsaved;
                app.editor.autosave_timer =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
                dirty = true;
            }
            if app.mode != ViewMode::Edit || app.should_quit {
                break;
            }
        }
    }
    Ok(())
}

fn coalesce_editor_events(events: Vec<Event>) -> Vec<Event> {
    let mut batch = Vec::with_capacity(events.len());

    for event in events {
        let same_run = match (batch.last(), &event) {
            (Some(Event::Mouse(previous)), Event::Mouse(next)) => {
                previous.kind == MouseEventKind::Moved && next.kind == MouseEventKind::Moved
            }
            (Some(Event::Resize(_, _)), Event::Resize(_, _)) => true,
            _ => false,
        };
        if same_run {
            *batch.last_mut().expect("batch contains prior event") = event;
        } else {
            batch.push(event);
        }
    }
    batch
}
fn synchronize_source_highlight(app: &mut App, change: crate::editor_document::DocumentChange) {
    let theme = app.app_theme.clone();
    let ghost_syntax = app.config.editor.ghost_syntax;
    let extended_features = app.config.editor.extended_markdown_features;
    let highlighter = app.editor.source_highlighter.get_or_insert_with(|| {
        crate::markdown::SourceHighlighter::new(&theme, ghost_syntax, extended_features)
    });
    highlighter.apply_change(&app.editor.body, change);
}

fn dispatch_editor_event<B: ratatui::backend::Backend>(
    terminal: &mut ratatui::Terminal<B>,
    app: &mut App,
    event: Event,
    focus: &mut EditFocus,
    mouse_selection: &mut MouseTextSelection,
) -> Result<bool>
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    if app.handle_property_manager_event(event.clone()) {
        return Ok(true);
    }
    let size = terminal.size().context("editor terminal size failed")?;
    let area = Rect::new(0, 0, size.width, size.height);
    if *focus == EditFocus::Properties
        && app.editor.sidebar != crate::editor::EditSidebar::Properties
    {
        *focus = if app.editor.sidebar == crate::editor::EditSidebar::None {
            EditFocus::Body
        } else {
            EditFocus::Sidebar
        };
    }
    app.editor.properties.focused = *focus == EditFocus::Properties;
    match event {
        // All-keys keyboard mode reports bare modifier presses and text-less
        // IME events (key code 0); drop them before any handler sees them.
        Event::Key(key)
            if key.kind == KeyEventKind::Press
                && (matches!(key.code, KeyCode::Modifier(_))
                    || key.code == KeyCode::Char('\0')) =>
        {
            Ok(true)
        }
        Event::Key(key)
            if key.kind == KeyEventKind::Press
                && key.code == KeyCode::Char('c')
                && key.modifiers == KeyModifiers::CONTROL =>
        {
            if crate::events::handle_global_popups_and_palette(app, Event::Key(key), area)
                || app.handle_properties_dialog_key(key)
            {
                return Ok(true);
            }
            // Ctrl+C copies when a text selection is active; otherwise it
            // force-quits (terminals always deliver the plain key).
            let has_selection = match *focus {
                EditFocus::Title => app.editor.title_editor.has_selection(),
                EditFocus::Body => app.editor.body.has_selection(),
                EditFocus::Sidebar | EditFocus::Properties => false,
            };
            if has_selection {
                let notice = if *focus == EditFocus::Title {
                    copy_mouse_selection(&mut app.editor.title_editor)
                } else {
                    copy_mouse_selection(&mut app.editor.body)
                };
                if let Some(notice) = notice {
                    app.set_temporary_status(notice);
                }
                Ok(true)
            } else {
                if app.autosave().is_err() {
                    return Ok(true);
                }
                crate::force_quit()
            }
        }
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            // Route through the global dispatcher first (mirrors lib.rs and
            // the mouse path below): F1 help, F2 quick-keybinds, F3 message
            // overlay, F4 vault switcher, and popup/palette input would
            // otherwise be unreachable inside the nested editor session.
            if crate::events::handle_global_popups_and_palette(app, Event::Key(key), area) {
                return Ok(true);
            }
            crate::handle_edit_keys(app, key, focus);
            if let Some(message) = crate::text_edit::take_clipboard_notice() {
                app.set_temporary_status(message);
            }
            Ok(true)
        }
        Event::Mouse(mouse) => {
            app.mouse_pos = Some((mouse.column, mouse.row));
            if crate::events::handle_global_popups_and_palette(app, Event::Mouse(mouse), area)
                || crate::events::handle_global_popup_mouse(app, &mouse, area)
            {
                return Ok(true);
            }
            crate::handle_edit_mouse(app, mouse, area, focus, mouse_selection);
            Ok(true)
        }
        Event::Paste(data) => {
            let handled = crate::events::handle_bracketed_paste(app, data, focus);
            if handled {
                app.set_temporary_status("Pasted from clipboard");
            }
            Ok(handled)
        }
        Event::Resize(_, _) => Ok(true),
        _ => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use tempfile::tempdir;

    #[test]
    fn editor_session_draws_once_while_idle_and_exits() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let dir = tempdir().expect("tempdir");
        let storage = crate::storage::Storage {
            data_dir: dir.path().join("data"),
            config_dir: dir.path().join("config"),
            notes_dir: dir.path().join("notes"),
            templates_dir: dir.path().join("templates"),
            key: <[u8; 32]>::default(),
            skip_dir_patterns: Vec::new(),
            rename_on_title_change: true,
        };
        for path in [
            &storage.data_dir,
            &storage.config_dir,
            &storage.notes_dir,
            &storage.templates_dir,
        ] {
            std::fs::create_dir_all(path).expect("create storage directory");
        }
        let mut app = App::new(storage).expect("app");
        app.start_blank_note_with_title(String::new(), "session".to_string());
        let (sender, receiver) = std::sync::mpsc::channel();
        sender
            .send(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)))
            .expect("send exit");
        let mut events = crate::event_source::EventSource::channel(receiver);
        let mut terminal = ratatui::Terminal::new(TestBackend::new(100, 30)).expect("terminal");
        let mut draws = 0;
        run_editor_session(&mut terminal, &mut app, &mut events, &mut |_| {
            draws += 1;
            false
        })
        .expect("session");
        assert_eq!(app.mode, ViewMode::List);
        assert_eq!(draws, 1);
    }

    #[test]
    fn editor_session_properties_commit_cancel_autosave_and_failure() {
        let _guard = crate::config::ConfigTestGuard::lock();
        let dir = tempdir().unwrap();
        crate::config::set_config_path_override(dir.path().join("config.toml"));
        let storage = crate::storage::Storage {
            data_dir: dir.path().into(),
            config_dir: dir.path().into(),
            notes_dir: dir.path().into(),
            templates_dir: dir.path().join("templates"),
            key: <[u8; 32]>::default(),
            skip_dir_patterns: vec![],
            rename_on_title_change: false,
        };
        std::fs::write(
            dir.path().join("note.md"),
            "---\nstatus: open # keep\n---\nbody",
        )
        .unwrap();
        let mut app = App::new(storage).unwrap();
        app.load_and_open_note("note.md", None);
        app.editor.sidebar = crate::editor::EditSidebar::Properties;
        app.editor.properties.focus_request = Some(EditFocus::Properties);
        let (sender, receiver) = std::sync::mpsc::channel();
        let send = move |code, modifiers| {
            sender
                .send(Event::Key(KeyEvent::new(code, modifiers)))
                .unwrap()
        };
        // Cancel existing value, then commit property-only change; reject reserved name.
        // Text entry uses actual key path, not direct state mutation.
        let input = std::thread::spawn(move || {
            send(KeyCode::Enter, KeyModifiers::NONE);
            send(KeyCode::Char('a'), KeyModifiers::CONTROL);
            for c in "cancelled".chars() {
                send(KeyCode::Char(c), KeyModifiers::NONE);
            }
            send(KeyCode::Esc, KeyModifiers::NONE);
            send(KeyCode::Enter, KeyModifiers::NONE);
            send(KeyCode::Char('a'), KeyModifiers::CONTROL);
            for c in "answered".chars() {
                send(KeyCode::Char(c), KeyModifiers::NONE);
            }
            send(KeyCode::Enter, KeyModifiers::CONTROL);
            send(KeyCode::Char('a'), KeyModifiers::NONE);
            for c in "title".chars() {
                send(KeyCode::Char(c), KeyModifiers::NONE);
            }
            send(KeyCode::Enter, KeyModifiers::CONTROL);
            // Let invalid dialog render while autosave runs independently.
            std::thread::sleep(Duration::from_millis(2300));
            send(KeyCode::Esc, KeyModifiers::NONE);
            send(KeyCode::Esc, KeyModifiers::NONE);
            send(KeyCode::Esc, KeyModifiers::NONE);
        });
        let mut events = crate::event_source::EventSource::channel(receiver);
        let mut terminal = ratatui::Terminal::new(TestBackend::new(100, 30)).unwrap();
        let mut saw_dirty = false;
        let mut saw_saved = false;
        let mut rejected_reserved = false;
        run_editor_session(&mut terminal, &mut app, &mut events, &mut |app| {
            saw_dirty |= app.editor.autosave_status == crate::editor::AutosaveStatus::Unsaved;
            saw_saved |= app.editor.autosave_status == crate::editor::AutosaveStatus::RecentlySaved;
            if let Some(crate::properties::PropertiesDialog::Edit(dialog)) =
                &app.editor.properties.dialog
            {
                rejected_reserved |= dialog
                    .error
                    .as_deref()
                    .is_some_and(|error| error.contains("Managed by Clin"));
            }
            false
        })
        .unwrap();
        input.join().unwrap();
        assert!(saw_dirty && saw_saved && rejected_reserved);
        assert_eq!(app.mode, ViewMode::List);
        assert_eq!(app.storage.load_note("note.md").unwrap().content, "body");
        let header = app.storage.load_frontmatter("note.md").unwrap().unwrap();
        assert_eq!(
            crate::frontmatter::checked_parse(&header).unwrap().extra["status"].as_str(),
            Some("answered")
        );
        assert!(header.contains("status: answered # keep"));
        app.load_and_open_note("note.md", None);
        app.editor
            .properties
            .commit(
                crate::frontmatter::FrontmatterEdit {
                    key_yaml: "status".into(),
                    value_yaml: Some("failed".into()),
                    rename_from: None,
                },
                false,
            )
            .unwrap();
        app.write_draft();
        std::fs::write(dir.path().join("note.md"), "---\nx: 1\nx: 2\n---\nbody").unwrap();
        app.editor.autosave_status = crate::editor::AutosaveStatus::Unsaved;
        app.editor.autosave_timer = Some(std::time::Instant::now());
        assert!(app.tick_autosave());
        assert_eq!(
            app.editor.autosave_status,
            crate::editor::AutosaveStatus::Unsaved
        );
        assert!(app.editor.autosave_timer.is_none());
        assert_ne!(app.editor.properties.pending.len(), 0);
        assert!(app.storage.editor_draft_path().exists());
    }
}
