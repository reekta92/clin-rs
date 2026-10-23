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

        dirty |= app.messages.tick_expirations();
        dirty |= app.tick_status();
        dirty |= app.poll_calendar();
        if app.base_embeds.has_visible() {
            crate::process_watcher_events(app);
            dirty |= app.poll_bases();
        }
        dirty |= app.poll_editor_renderers();
        dirty |= app.poll_editor_image_results();
        dirty |= app.tick_autosave();
        dirty |= app.poll_draft_errors();
        if app.poll_import() {
            dirty = true;
            if let Some(change) = app.editor.body.take_change() {
                synchronize_source_highlight(app, change);
            }
        }

        if dirty {
            if !(pre_draw_hook)(app) {
                terminal
                    .draw(|frame| crate::ui::draw_ui(frame, app, focus))
                    .context("editor frame draw failed")?;
            }
            dirty = false;
        }

        let editor_pending = app.import_task.is_some()
            || app
                .editor
                .md_preview_renderer
                .as_ref()
                .is_some_and(crate::markdown::MarkdownRenderer::is_pending)
            || app.editor.pending_editor_preview_update
            || app.base_embeds.is_loading();
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
            let frontmatter_visible = app
                .editor
                .frontmatter
                .as_ref()
                .is_some_and(|state| state.visible);
            let header_before = app.editor.frontmatter_mutation();
            let body_rev_before = app.editor.body.revision();
            let title_before = crate::events::get_title_text(&app.editor.title_editor).into_owned();

            dirty |= dispatch_editor_event(terminal, app, event, &mut focus, &mut mouse_selection)?;

            let frontmatter_now_visible = app
                .editor
                .frontmatter
                .as_ref()
                .is_some_and(|state| state.visible);
            if frontmatter_now_visible && !frontmatter_visible {
                focus = EditFocus::Frontmatter;
            } else if !frontmatter_now_visible && focus == EditFocus::Frontmatter {
                focus = EditFocus::Body;
            }
            let body_rev_after = app.editor.body.revision();
            let title_after = crate::events::get_title_text(&app.editor.title_editor).into_owned();

            if body_rev_before != body_rev_after
                || title_before != title_after
                || header_before != app.editor.frontmatter_mutation()
            {
                if body_rev_before != body_rev_after
                    && let Some(change) = app.editor.body.take_change()
                {
                    synchronize_source_highlight(app, change);
                }
                app.write_draft();
                app.editor.autosave_status = crate::editor::AutosaveStatus::Unsaved;
                app.editor.autosave_timer =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
                dirty = true;
            }
            if app.mode != ViewMode::Edit || app.should_quit {
                if finish_editor_transition(app) {
                    break;
                }
                dirty = true;
            }
        }
    }
    Ok(())
}

/// Global quit and view actions obey the same save contract as Escape.
fn finish_editor_transition(app: &mut App) -> bool {
    if (app.editor.editing_id.is_some()
        || app.editor.public_document.is_some()
        || app.editor.template_edit_path.is_some())
        && let Err(error) = app.autosave()
    {
        app.mode = ViewMode::Edit;
        app.should_quit = false;
        app.set_temporary_status(&error);
        return false;
    }
    true
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
pub(crate) fn synchronize_source_highlight(
    app: &mut App,
    change: crate::editor_document::DocumentChange,
) {
    crate::ui::apply_editor_visual_change(&mut app.editor, &change);
    let theme = app.app_theme.clone();
    let ghost_syntax = app.config.editor.ghost_syntax;
    let extended_features = app.config.editor.extended_markdown_features;
    let highlighter = app.editor.source_highlighter.get_or_insert_with(|| {
        crate::markdown::SourceHighlighter::new(&theme, ghost_syntax, extended_features)
    });
    highlighter.apply_change(&app.editor.body, change);
}

pub(crate) fn dispatch_editor_event<B: ratatui::backend::Backend>(
    terminal: &mut ratatui::Terminal<B>,
    app: &mut App,
    event: Event,
    focus: &mut EditFocus,
    mouse_selection: &mut MouseTextSelection,
) -> Result<bool>
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    app.observe_navigation();
    let result = dispatch_editor_event_inner(terminal, app, event, focus, mouse_selection);
    app.observe_navigation();
    result
}

fn dispatch_editor_event_inner<B: ratatui::backend::Backend>(
    terminal: &mut ratatui::Terminal<B>,
    app: &mut App,
    event: Event,
    focus: &mut EditFocus,
    mouse_selection: &mut MouseTextSelection,
) -> Result<bool>
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    if app.import_event(&event) {
        return Ok(true);
    }
    let size = terminal.size().context("editor terminal size failed")?;
    let area = Rect::new(0, 0, size.width, size.height);
    if crate::events::handle_open_palette_event(app, &event, area) {
        return Ok(true);
    }
    if app.base_embed_event(&event, area) {
        return Ok(true);
    }
    if let Event::Key(key) = &event
        && key.kind == KeyEventKind::Press
        && !mouse_selection.active
        && app.palette_input_available()
    {
        match app.resolve_palette_shortcut(*key, app.mode) {
            crate::keybinds::MatchOutcome::Matched(_, _) => {
                app.open_command_palette(Some(*focus));
                return Ok(true);
            }
            crate::keybinds::MatchOutcome::Pending => return Ok(true),
            crate::keybinds::MatchOutcome::NoMatch => {}
        }
    } else {
        app.palette_matcher.clear();
    }
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
            // Ctrl+C is copy in text inputs, never an implicit emergency exit.
            let has_selection = match *focus {
                EditFocus::Title => app.editor.title_editor.has_selection(),
                EditFocus::Body => app.editor.body.has_selection(),
                EditFocus::Frontmatter => app.editor.frontmatter.as_ref().is_some_and(|state| {
                    state.raw.has_selection()
                        || state.input.as_ref().is_some_and(|input| {
                            input.name.has_selection() || input.value.has_selection()
                        })
                }),
                EditFocus::Sidebar => false,
            };
            if has_selection {
                let notice = if *focus == EditFocus::Frontmatter {
                    let raw = app.config.editor.frontmatter_mode
                        == crate::config::FrontmatterMode::RawYaml;
                    app.editor
                        .frontmatter
                        .as_mut()
                        .and_then(|state| state.input_mut(raw))
                        .and_then(copy_mouse_selection)
                } else if *focus == EditFocus::Title {
                    copy_mouse_selection(&mut app.editor.title_editor)
                } else {
                    copy_mouse_selection(&mut app.editor.body)
                };
                if let Some(notice) = notice {
                    app.set_temporary_status(notice);
                }
                Ok(true)
            } else {
                app.set_temporary_status_static("Nothing selected to copy");
                Ok(true)
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
mod safety_tests;

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
            skip_dir_patterns: Vec::new(),
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
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
}
