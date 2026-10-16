//! Native live regions share one bounded query worker and public snapshots.
#[path = "embed_source.rs"]
mod source;
use crate::{
    app::App,
    overlay::{OverlayResult, OverlayView},
};
use anyhow::{Context, Result};
use crossterm::event::{Event, KeyCode, MouseButton, MouseEventKind};
use ratatui::{Frame, layout::Rect, widgets::Paragraph};
pub(crate) use source::{Definition, InlineOrigin, Request, SLOT_HEIGHT, Slot};
use std::ops::Range;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
};

#[derive(Default)]
pub(crate) struct Embeds {
    entries: RefCell<HashMap<u64, super::state::BasesState>>,
    visible: RefCell<HashSet<u64>>,
    regions: RefCell<Vec<(Rect, u64)>>,
    active: Cell<Option<u64>>,
    running: Option<u64>,
    return_mode: Option<crate::app::ViewMode>,
    suspended_base: Option<Box<super::state::BasesState>>,
    public_revision: u64,
}
impl Embeds {
    pub(crate) fn clear_palette_matchers(&mut self) {
        for state in self.entries.get_mut().values_mut() {
            state.matcher.clear();
        }
        if let Some(state) = &mut self.suspended_base {
            state.matcher.clear();
        }
    }
    pub(crate) fn focused(&self) -> bool {
        self.active
            .get()
            .is_some_and(|key| self.visible.borrow().contains(&key))
    }
    pub(crate) fn has_writes(&self) -> bool {
        self.entries
            .borrow()
            .values()
            .any(|state| state.is_writing())
            || self
                .suspended_base
                .as_ref()
                .is_some_and(|state| state.is_writing())
    }
    pub(crate) fn is_loading(&self) -> bool {
        self.entries
            .borrow()
            .iter()
            .any(|(key, state)| self.visible.borrow().contains(key) && state.loading)
    }
    pub fn has_visible(&self) -> bool {
        !self.visible.borrow().is_empty()
    }
    fn is_writing(&self) -> bool {
        self.active
            .get()
            .and_then(|key| {
                self.entries
                    .borrow()
                    .get(&key)
                    .map(|state| state.write_task.as_ref().is_some_and(|task| task.writing))
            })
            .unwrap_or(false)
    }
    pub fn has_popup(&self) -> bool {
        self.active
            .get()
            .and_then(|key| {
                self.entries
                    .borrow()
                    .get(&key)
                    .map(super::state::BasesState::has_popup)
            })
            .unwrap_or(false)
    }
    pub fn draw_popup(&self, frame: &mut Frame, app: &App) {
        if let Some(key) = self.active.get()
            && let Some(state) = self.entries.borrow_mut().get_mut(&key)
        {
            state.render_search(frame, frame.area(), app);
            if let Some(dialog) = &mut state.dialog {
                dialog.render(frame, frame.area(), app);
            }
        }
    }
    pub fn begin_frame(&self) {
        self.visible.borrow_mut().clear();
        self.regions.borrow_mut().clear();
    }
    pub fn invalidate(&mut self) {
        self.running = None;
        for state in self.entries.get_mut().values_mut() {
            state.loading = true;
            state.media.invalidate();
        }
    }
    pub(crate) fn observe_revision(&mut self, revision: u64) {
        if self.public_revision == revision {
            return;
        }
        self.public_revision = revision;
        for state in self.entries.get_mut().values_mut() {
            if state
                .loaded
                .as_ref()
                .is_some_and(|loaded| loaded.public_revision != revision)
            {
                state.loading = true;
                state.media.invalidate();
            }
        }
    }
    pub fn install(&mut self, key: u64, loaded: Result<super::worker::LoadedBase>) {
        if let Ok(loaded) = &loaded {
            self.observe_revision(loaded.public_revision);
        }
        if self.running == Some(key) {
            self.running = None;
        }
        if let Some(state) = self.entries.get_mut().get_mut(&key) {
            if let Ok(loaded) = &loaded {
                state.id = loaded.snapshot.id().to_owned();
            }
            state.install(loaded);
        }
    }
    pub fn draw(
        &self,
        frame: &mut Frame,
        area: Rect,
        renderer: &crate::markdown::MarkdownRenderer,
        range: Range<usize>,
        app: &App,
    ) {
        let Some(document) = renderer.document() else {
            return;
        };
        let Some((revision, source)) = renderer.base_content() else {
            return;
        };
        let (host_revision, host_source) = renderer
            .base_host_source
            .clone()
            .unwrap_or((revision, source));
        for (slot_index, slot) in document.base_slots.iter().enumerate() {
            let start = slot.rows.start.max(range.start);
            let end = slot.rows.end.min(range.end);
            if start >= end {
                continue;
            }
            let rect = Rect::new(
                area.x,
                area.y.saturating_add((start - range.start) as u16),
                area.width,
                (end - start) as u16,
            )
            .intersection(area);
            if rect.is_empty() {
                continue;
            }
            if !app.config.features.bases_view.is_enabled() {
                frame.render_widget(Paragraph::new("Bases disabled"), rect);
                continue;
            }
            if renderer
                .base_host
                .as_ref()
                .is_some_and(|host| host.ends_with(".clin") || App::is_virtual_path(host))
            {
                frame.render_widget(
                    Paragraph::new("Bases requires a public containing file"),
                    rect,
                );
                continue;
            }
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            (
                renderer.base_session,
                revision,
                host_revision,
                &renderer.base_host,
                slot.source_line,
            )
                .hash(&mut hash);
            let key = hash.finish();
            let mut entries = self.entries.borrow_mut();
            if !entries.contains_key(&key) && entries.len() >= 32 {
                let unused = entries.iter().find_map(|(key, state)| {
                    (!self.visible.borrow().contains(key)
                        && state.write_task.is_none()
                        && state.dialog.is_none()
                        && Some(*key) != self.active.get())
                    .then_some(*key)
                });
                if let Some(unused) = unused {
                    entries.remove(&unused);
                } else {
                    frame.render_widget(
                        Paragraph::new("Bases: 32 live regions already active"),
                        rect,
                    );
                    continue;
                }
            }
            if !entries.contains_key(&key) {
                let previous = entries.iter().find_map(|(old_key, state)| {
                    state
                        .embedded_request
                        .as_ref()
                        .filter(|request| {
                            request.session == renderer.base_session
                                && request.anchor == slot_index
                                && request.host == renderer.base_host
                        })
                        .map(|_| *old_key)
                });
                if let Some(previous) = previous
                    && let Some(mut state) = entries.remove(&previous)
                {
                    state.loading = true;
                    state.media.invalidate();
                    state.embedded_request = Some(Request {
                        key,
                        session: renderer.base_session,
                        anchor: slot_index,
                        definition: slot.definition.clone(),
                        host: renderer.base_host.clone(),
                        view: None,
                        body_only: renderer.base_body_only,
                        canvas_node: renderer.base_canvas_node.clone(),
                        host_source: renderer.base_host.as_ref().map(|_| host_source.clone()),
                    });
                    if self.active.get() == Some(previous) {
                        self.active.set(Some(key));
                    }
                    entries.insert(key, state);
                }
            }
            let state = entries.entry(key).or_insert_with(|| {
                let mut state = super::state::BasesState::new(String::new(), None);
                state.previous_mode = app.mode;
                state.embedded_request = Some(Request {
                    key,
                    session: renderer.base_session,
                    anchor: slot_index,
                    definition: slot.definition.clone(),
                    host: renderer.base_host.clone(),
                    view: None,
                    body_only: renderer.base_body_only,
                    canvas_node: renderer.base_canvas_node.clone(),
                    host_source: renderer.base_host.as_ref().map(|_| host_source.clone()),
                });
                state
            });
            self.visible.borrow_mut().insert(key);
            self.regions.borrow_mut().push((rect, key));
            frame.render_widget(ratatui::widgets::Clear, rect);
            frame.render_widget(
                ratatui::widgets::Block::default().style(app.app_theme.bg_style()),
                rect,
            );
            state.render_native(frame, rect, app);
        }
    }
    fn poll(&mut self, app: &mut App) -> bool {
        let mut dirty = false;
        for (&key, state) in self.entries.get_mut().iter_mut() {
            if state.write_task.is_some() {
                dirty |= state.poll_write(app);
                if state.loading {
                    self.running = Some(key);
                }
            }
        }
        if app.mode == crate::app::ViewMode::Bases {
            self.running = None;
            return dirty;
        }
        if self
            .running
            .is_some_and(|key| !self.visible.borrow().contains(&key))
        {
            if let Some(worker) = &mut app.bases_worker {
                worker.cancel();
            }
            self.running = None;
        }
        if self.running.is_none()
            && let Some((&key, state)) = self.entries.get_mut().iter_mut().find(|(key, state)| {
                self.visible.borrow().contains(key) && state.loading && state.write_task.is_none()
            })
            && app.ensure_bases_worker()
        {
            state.refresh(app, false);
            self.running = Some(key);
        }
        dirty
    }
    pub(crate) fn restore(
        &mut self,
        state: super::state::BasesState,
    ) -> (
        Option<crate::app::ViewMode>,
        Option<super::state::BasesState>,
    ) {
        if let Some(request) = &state.embedded_request {
            let key = request.key;
            self.entries.get_mut().insert(key, state);
            self.active.set(Some(key));
        }
        self.running = None;
        (
            self.return_mode.take(),
            self.suspended_base.take().map(|state| *state),
        )
    }
    fn expand(&mut self, key: u64, app: &mut App) {
        if !app.palette_input_available()
            || !self
                .entries
                .borrow()
                .get(&key)
                .is_some_and(|state| state.palette_input_available())
        {
            return;
        }
        if self
            .entries
            .get_mut()
            .get(&key)
            .is_some_and(|state| state.write_task.is_some())
        {
            app.set_temporary_status_static(
                "Wait for the Bases operation before opening the full view",
            );
            return;
        }
        if let Some(snapshot) = self
            .entries
            .get_mut()
            .get(&key)
            .and_then(|state| state.loaded.as_ref())
            .and_then(|loaded| loaded.host_snapshot.as_ref())
            .cloned()
            && let Err(error) = app.guard_embed_host(&snapshot)
        {
            app.set_temporary_status(&format!("Bases: {error:#}"));
            return;
        }
        if let Some(mut state) = self.entries.get_mut().remove(&key) {
            state.matcher.clear();
            self.return_mode = app.return_mode;
            self.suspended_base = app.bases_state.take().map(Box::new);
            state.previous_mode = app.mode;
            app.mode = crate::app::ViewMode::Bases;
            state.refresh(app, false);
            app.bases_state = Some(state);
            app.open_command_palette(None);
            self.active.set(None);
            self.running = None;
        }
    }
    fn event(&mut self, event: &Event, app: &mut App, area: Rect) -> bool {
        if app.popups.active.is_some()
            || app.command_palette.is_some()
            || app.popups.confirm.is_some()
        {
            return false;
        }
        if let Event::Mouse(mouse) = event
            && mouse.kind == MouseEventKind::Down(MouseButton::Left)
            && !self.has_popup()
            && !self.is_writing()
        {
            self.active.set(
                self.regions
                    .borrow()
                    .iter()
                    .find(|(rect, _)| rect.contains((mouse.column, mouse.row).into()))
                    .map(|(_, key)| *key),
            );
        }
        let Some(key) = self.active.get() else {
            return false;
        };
        if !self.visible.borrow().contains(&key) {
            self.active.set(None);
            return false;
        }
        if let Event::Key(key_event) = event
            && key_event.code == KeyCode::F(6)
            && key_event.modifiers.is_empty()
            && app.palette_fallback_available(crate::app::ViewMode::Bases)
        {
            if key_event.kind == crossterm::event::KeyEventKind::Press
                && self
                    .entries
                    .borrow()
                    .get(&key)
                    .is_some_and(|state| state.palette_input_available())
            {
                self.expand(key, app);
            }
            return true;
        }
        if let Event::Key(key_event) = event
            && key_event
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
            && key_event.code == KeyCode::Enter
            && !self.has_popup()
        {
            self.expand(key, app);
            return true;
        }
        if let Event::Key(key) = event
            && key.code == KeyCode::Esc
            && self.is_writing()
        {
            app.set_temporary_status_static(
                "Write in progress; wait before leaving the Bases region",
            );
            return true;
        }
        let Some(state) = self.entries.get_mut().get_mut(&key) else {
            self.active.set(None);
            return false;
        };
        if !matches!(event, Event::Key(key) if key.code == KeyCode::Esc)
            && let Some(snapshot) = state
                .loaded
                .as_ref()
                .and_then(|loaded| loaded.host_snapshot.clone())
            && let Err(error) = app.guard_embed_host(&snapshot)
        {
            app.set_temporary_status(&format!("Bases: {error:#}"));
            return true;
        }
        if let Event::Key(key) = event
            && key.code == KeyCode::Esc
            && !state.has_popup()
        {
            self.active.set(None);
            return true;
        }
        match state.overlay_handle_event(event.clone(), app, area) {
            Ok(OverlayResult::Exit) => self.active.set(None),
            Ok(OverlayResult::OpenHelp(tab)) => app.open_help_page_with_tab(tab),
            Ok(_) => {}
            Err(error) => app.set_temporary_status(&format!("Bases: {error:#}")),
        }
        if state.loading {
            self.running = Some(key);
        }
        let expand = std::mem::take(&mut state.expand_requested);
        if expand {
            self.expand(key, app);
        }
        true
    }
}
impl App {
    pub(crate) fn guard_embed_host(
        &mut self,
        snapshot: &crate::storage::documents::DocumentSnapshot,
    ) -> anyhow::Result<()> {
        if self.editor.editing_id.as_deref() != Some(snapshot.id())
            || self.editor.public_document.is_some()
        {
            return Ok(());
        }
        let baseline = self
            .editor
            .public_opening_snapshot
            .clone()
            .context("reopen the containing public note before editing Bases")?;
        let parsed = crate::frontmatter::properties::PropertyDocument::parse(baseline.source())?;
        let title = self
            .notes
            .iter()
            .find(|note| note.id == baseline.id())
            .map(|note| note.title.clone())
            .unwrap_or_else(|| {
                crate::events::get_title_text(&self.editor.title_editor).into_owned()
            });
        let current = self.editor.body.text();
        if self.editor.autosave_status != crate::editor::AutosaveStatus::Unsaved
            && current == parsed.body.replace("\r\n", "\n").trim_end_matches('\n')
        {
            let cursor = self.editor.body.cursor();
            self.editor.body.select_all();
            self.editor
                .body
                .insert_str(parsed.body.replace("\r\n", "\n"));
            self.editor
                .body
                .move_cursor(ratatui_textarea::CursorMove::Jump(
                    cursor.row as u16,
                    cursor.col as u16,
                ));
        }
        self.editor.public_opening_snapshot = None;
        self.editor.public_document = Some(baseline);
        self.editor.public_document_body_only = true;
        self.editor.public_document_title = Some(title);
        if self.editor.autosave_status == crate::editor::AutosaveStatus::Unsaved {
            self.write_draft();
        }
        Ok(())
    }
    pub(crate) fn ensure_inline_host_write(
        &mut self,
        baseline: &crate::storage::documents::DocumentSnapshot,
    ) -> anyhow::Result<()> {
        self.guard_embed_host(baseline)?;
        for editor in std::iter::once(&self.editor).chain(self.embedded_host_editor.as_deref()) {
            if editor.editing_id.as_deref() == Some(baseline.id())
                && editor.public_document_source().as_deref() != Some(baseline.source())
            {
                anyhow::bail!(
                    "save the containing public document before editing inline base configuration"
                );
            }
        }
        if let Some(canvas) = &self.canvas_state
            && canvas.state.path == self.storage.notes_dir.join(baseline.id())
            && canvas.base_host_has_pending_changes()
        {
            anyhow::bail!(
                "finish and save the Canvas edit before editing inline base configuration"
            );
        }
        Ok(())
    }
    pub(crate) fn synchronize_public_change(
        &mut self,
        before: &crate::storage::documents::DocumentSnapshot,
        after: &crate::storage::documents::DocumentSnapshot,
    ) {
        let mut failure = None;
        for editor in
            std::iter::once(&mut self.editor).chain(self.embedded_host_editor.as_deref_mut())
        {
            let old = editor.public_document.clone();
            if editor.synchronize_public_snapshot(before, after) {
                let source = editor
                    .public_document_source()
                    .expect("guarded public source");
                let pending = source != after.source();
                if !pending || self.storage.write_document_draft(after, &source).is_ok() {
                    if let Some(old) = old
                        && (!pending || old.recovery_id() != after.recovery_id())
                        && let Err(error) = self.storage.clear_document_draft(&old)
                    {
                        failure = Some(format!("Host updated; previous draft retained: {error:#}"));
                    }
                } else {
                    failure = Some("Host updated; previous recovery draft retained because the new draft could not be saved".into());
                }
                if pending {
                    editor.autosave_status = crate::editor::AutosaveStatus::Unsaved;
                }
            }
        }
        if let Some(change) = self.editor.body.take_change() {
            crate::editor_session::synchronize_source_highlight(self, change);
        }
        self.update_editor_markdown_preview();
        if let Some(canvas) = &mut self.canvas_state
            && canvas.state.path == self.storage.notes_dir.join(after.id())
            && let Err(error) = canvas.reload_base_host()
        {
            failure = Some(format!("Canvas source changed; reload failed: {error:#}"));
        }
        if let Some(failure) = failure {
            self.messages
                .push(failure, crate::app::messages::MessageSeverity::Warning);
        }
    }
    pub(crate) fn poll_base_embeds(&mut self) -> bool {
        let mut embeds = std::mem::take(&mut self.base_embeds);
        let dirty = embeds.poll(self);
        self.base_embeds = embeds;
        dirty
    }
    pub(crate) fn base_embed_event(&mut self, event: &Event, area: Rect) -> bool {
        let mut embeds = std::mem::take(&mut self.base_embeds);
        let handled = embeds.event(event, self, area);
        self.base_embeds = embeds;
        handled
    }
}
