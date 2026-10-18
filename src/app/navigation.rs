use super::{App, HelpTab, ViewMode};
use crate::{
    keybinds::KeyMatcher,
    overlay::{OverlayState, ViewKind},
};
use anyhow::{Context, Result, ensure};
use std::path::PathBuf;

#[derive(Clone)]
enum Place {
    Notes,
    Note(String),
    PublicSource(String),
    Template(PathBuf),
    Graph,
    Draw(Option<String>),
    Canvas(PathBuf),
    Bases {
        id: String,
        view: Option<String>,
        request: Option<crate::bases::embedding::Request>,
    },
    Outline(String),
    Help(HelpTab),
    Setup,
}

impl Place {
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Notes, Self::Notes)
            | (Self::Graph, Self::Graph)
            | (Self::Help(_), Self::Help(_))
            | (Self::Setup, Self::Setup) => true,
            (Self::Note(a), Self::Note(b))
            | (Self::PublicSource(a), Self::PublicSource(b))
            | (Self::Outline(a), Self::Outline(b)) => a == b,
            (Self::Template(a), Self::Template(b)) | (Self::Canvas(a), Self::Canvas(b)) => a == b,
            (Self::Draw(a), Self::Draw(b)) => a == b,
            (
                Self::Bases {
                    id: a, request: ar, ..
                },
                Self::Bases {
                    id: b, request: br, ..
                },
            ) => a == b && ar.as_ref().map(|r| r.key) == br.as_ref().map(|r| r.key),
            _ => false,
        }
    }
}

#[derive(Clone)]
struct Location {
    place: Place,
    parent: Option<Box<Self>>,
}

impl Location {
    fn capture(
        app: &App,
        mode: ViewMode,
        return_mode: Option<ViewMode>,
        following: Option<ViewMode>,
        host_editor: bool,
    ) -> Option<Self> {
        Self::capture_route(
            app,
            mode,
            return_mode,
            following,
            host_editor,
            &mut Vec::new(),
        )
    }

    fn capture_route(
        app: &App,
        mode: ViewMode,
        return_mode: Option<ViewMode>,
        following: Option<ViewMode>,
        host_editor: bool,
        visited: &mut Vec<(ViewMode, bool)>,
    ) -> Option<Self> {
        if visited.contains(&(mode, host_editor)) {
            return None;
        }
        visited.push((mode, host_editor));
        let editor = if host_editor {
            app.embedded_host_editor.as_deref().unwrap_or(&app.editor)
        } else {
            &app.editor
        };
        let place = match mode {
            ViewMode::List => Place::Notes,
            ViewMode::Edit => {
                if let Some(path) = &editor.template_edit_path {
                    Place::Template(path.clone())
                } else {
                    let id = editor.editing_id.clone()?;
                    if editor.public_document.is_some() && !editor.public_document_body_only {
                        Place::PublicSource(id)
                    } else {
                        Place::Note(id)
                    }
                }
            }
            ViewMode::Graph => {
                app.graph_plugin.as_ref()?;
                Place::Graph
            }
            ViewMode::Draw => Place::Draw(Some(app.draw_state.as_ref()?.current_file.clone())),
            ViewMode::Canvas => Place::Canvas(app.canvas_state.as_ref()?.state.path.clone()),
            ViewMode::Bases => {
                let state = app.bases_state.as_ref()?;
                Place::Bases {
                    id: state.id.clone(),
                    view: state.named_view.clone(),
                    request: state.embedded_request.clone(),
                }
            }
            ViewMode::Outline => Place::Outline(app.outline_state.as_ref()?.note_id.clone()),
            ViewMode::Help => Place::Help(app.help_tab),
            ViewMode::Setup => Place::Setup,
        };
        let (parent, next, host_editor) = match mode {
            ViewMode::List => (None, None, false),
            ViewMode::Help | ViewMode::Setup => (return_mode, following, false),
            ViewMode::Bases => (
                app.bases_state.as_ref().map(|s| s.previous_mode),
                app.base_embeds.return_mode(),
                true,
            ),
            _ => (return_mode, None, false),
        };
        let parent = parent.filter(|parent| *parent != mode).and_then(|parent| {
            Self::capture_route(app, parent, next, None, host_editor, visited).map(Box::new)
        });
        Some(Self { place, parent })
    }

    fn prepare(&self, app: &App, into: &mut Vec<Prepared>) -> Result<()> {
        if let Some(parent) = &self.parent {
            parent.prepare(app, into)?;
        }
        let prepared = match &self.place {
            Place::Notes => Prepared::Notes,
            Place::Note(id) => Prepared::Note(id.clone(), app.storage.load_note(id)?),
            Place::PublicSource(id) => Prepared::Public(app.storage.load_document(id)?),
            Place::Template(path) => {
                if crate::templates::is_folder_template_config(path) {
                    app.storage.check_folder_template_config_path(path)?;
                }
                Prepared::Template(
                    path.clone(),
                    std::fs::read_to_string(path).context("failed to reopen template")?,
                )
            }
            Place::Graph => {
                ensure!(
                    app.config.features.graph_view.is_enabled(),
                    "Graph disabled"
                );
                Prepared::overlay(OverlayState::Graph(crate::graf_adapter::GrafPlugin::new(
                    &app.config,
                    app.storage.clone(),
                    app.notes.clone(),
                    app.config_errors.clone(),
                    app.keybinds.clone(),
                    KeyMatcher::default(),
                )?))
            }
            Place::Draw(id) => {
                ensure!(app.config.features.draw_view.is_enabled(), "Draw disabled");
                let id = id
                    .as_deref()
                    .context("Drawing has no save target; create a drawing first")?;
                let state = crate::draw_adapter::DrawPlugin::load(
                    &app.storage,
                    id,
                    app.keybinds.clone(),
                    KeyMatcher::default(),
                )?;
                Prepared::overlay(OverlayState::Draw(state))
            }

            Place::Canvas(path) => {
                ensure!(
                    app.config.features.canvas_view.is_enabled(),
                    "Canvas disabled"
                );
                let id = path
                    .strip_prefix(&app.storage.notes_dir)
                    .context("Canvas is outside the current vault")?
                    .to_str()
                    .context("invalid Canvas path")?;
                let snapshot = app.storage.load_document(id)?;
                let _: serde_json::Value =
                    serde_json::from_str(snapshot.source()).context("failed to reopen Canvas")?;
                Prepared::overlay(OverlayState::Canvas(
                    crate::pinstar_adapter::PinstarPlugin::new(
                        path,
                        &app.config,
                        app.keybinds.clone(),
                        KeyMatcher::default(),
                        app.image_picker.clone(),
                        &app.storage.data_dir,
                    )?,
                ))
            }
            Place::Bases { id, view, request } => {
                ensure!(
                    app.config.features.bases_view.is_enabled(),
                    "Bases disabled"
                );
                let mut request = request.clone();
                let source = if let Some(request) = &mut request {
                    if let Some(host) = &request.host {
                        *request = request.after_save(app.storage.load_document(host)?.source())?;
                    }
                    match &request.definition {
                        crate::bases::embedding::Definition::Inline { source, .. } => {
                            source.to_string()
                        }
                        crate::bases::embedding::Definition::File { target, .. } => {
                            app.storage.load_document(target)?.source().to_owned()
                        }
                    }
                } else {
                    app.storage.load_document(id)?.source().to_owned()
                };
                crate::bases::format::BaseDocument::parse(&source)?;
                let mut state = crate::bases::state::BasesState::new(id.clone(), view.clone());
                state.embedded_request = request;
                Prepared::overlay(OverlayState::Bases(state))
            }
            Place::Outline(id) => {
                ensure!(
                    app.config.features.outline_view.is_enabled(),
                    "Outline disabled"
                );
                let note = app.storage.load_note(id)?;
                Prepared::overlay(OverlayState::Outline(
                    crate::outline::state::OutlineState::new(
                        id.clone(),
                        &note.title,
                        &note.content,
                        app.keybinds.clone(),
                        KeyMatcher::default(),
                    ),
                ))
            }
            Place::Help(tab) => {
                ensure!(app.config.features.help_view.is_enabled(), "Help disabled");
                Prepared::Help(*tab)
            }
            Place::Setup => Prepared::Setup,
        };
        into.push(prepared);
        Ok(())
    }
}

enum Prepared {
    Notes,
    Note(String, crate::storage::Note),
    Public(crate::storage::documents::DocumentSnapshot),
    Template(PathBuf, String),
    Overlay(Box<OverlayState>),
    Help(HelpTab),
    Setup,
}

impl Prepared {
    fn overlay(state: OverlayState) -> Self {
        Self::Overlay(Box::new(state))
    }
    fn install(self, app: &mut App) {
        let parent = app.mode;
        match self {
            Self::Notes => app.mode = ViewMode::List,
            Self::Note(id, note) => {
                app.open_loaded_note(&id, note, None);
                app.return_mode = (parent != ViewMode::List).then_some(parent);
            }
            Self::Public(snapshot) => {
                if parent == ViewMode::Bases
                    && app
                        .bases_state
                        .as_ref()
                        .is_some_and(|state| state.previous_mode == ViewMode::Edit)
                {
                    app.embedded_host_editor = Some(Box::new(std::mem::take(&mut app.editor)));
                }
                app.open_prepared_public_document(snapshot);
            }
            Self::Template(path, source) => app.open_template_source(&path, source),
            Self::Overlay(mut view) => {
                app.mode = match view.kind() {
                    ViewKind::Graph => ViewMode::Graph,
                    ViewKind::Draw => ViewMode::Draw,
                    ViewKind::Canvas => ViewMode::Canvas,
                    ViewKind::Bases => ViewMode::Bases,
                    ViewKind::Outline => ViewMode::Outline,
                };
                if let OverlayState::Bases(state) = &mut *view {
                    state.previous_mode = parent;
                    if state.embedded_request.is_some() {
                        let suspended = app.bases_state.take().map(Box::new);
                        app.base_embeds
                            .retain_history_host(app.return_mode, suspended);
                    } else {
                        app.return_mode = Some(parent);
                    }
                    state.refresh(app, false);
                } else {
                    app.return_mode = Some(parent);
                }
                view.put_back(app);
            }
            Self::Help(tab) => app.open_help_page_with_tab(tab),
            Self::Setup => app.open_setup_view(),
        }
    }
}

pub(crate) struct History {
    entries: Vec<Location>,
    cursor: usize,
}

impl Default for History {
    fn default() -> Self {
        Self {
            entries: vec![Location {
                place: Place::Notes,
                parent: None,
            }],
            cursor: 0,
        }
    }
}

impl App {
    pub(crate) fn observe_navigation(&mut self) {
        if self.should_quit {
            return;
        }
        let Some(location) = Location::capture(
            self,
            self.mode,
            self.return_mode,
            self.visit_return_mode,
            false,
        ) else {
            return;
        };
        let history = &mut self.navigation;
        if history.entries[history.cursor].place.same(&location.place) {
            history.entries[history.cursor] = location;
        } else {
            history.entries.truncate(history.cursor + 1);
            history.entries.push(location);
            if history.entries.len() > 64 {
                history.entries.remove(0);
            }
            history.cursor = history.entries.len() - 1;
        }
    }

    pub(crate) fn can_navigate_history(&self, forward: bool) -> bool {
        if forward {
            self.navigation.cursor + 1 < self.navigation.entries.len()
        } else {
            self.navigation.cursor > 0
        }
    }

    pub(crate) fn navigate_history(&mut self, forward: bool) -> Result<()> {
        ensure!(self.mode != ViewMode::Setup, "Finish or cancel Setup first");
        self.observe_navigation();
        ensure!(
            self.can_navigate_history(forward),
            "No view in that history direction"
        );
        let cursor = if forward {
            self.navigation.cursor + 1
        } else {
            self.navigation.cursor - 1
        };
        let location = self.navigation.entries[cursor].clone();
        self.save_before_session_replace()?;
        let mut prepared = Vec::new();
        location.prepare(self, &mut prepared)?;
        let worker = if prepared.iter().any(
            |p| matches!(p, Prepared::Overlay(view) if matches!(**view, OverlayState::Bases(_))),
        ) && self.bases_worker.is_none()
        {
            Some(crate::bases::worker::BasesWorker::new(
                self.storage.clone(),
            )?)
        } else {
            None
        };
        self.release_navigation_owners();
        if let Some(worker) = worker {
            self.bases_worker = Some(worker);
        }
        for destination in prepared {
            destination.install(self);
        }
        self.navigation.cursor = cursor;
        self.clear_palette_matchers();
        self.sync_retained_view_settings();
        self.set_default_status();
        Ok(())
    }

    pub(crate) fn save_active_view(&mut self) -> Result<()> {
        if self.mode == ViewMode::Edit {
            self.save_editor_before_navigation()?;
        } else {
            self.save_before_session_replace()?;
        }
        self.set_temporary_status_static("Saved");
        Ok(())
    }

    pub(crate) fn quit_from_palette(&mut self) -> Result<()> {
        self.save_before_session_replace()?;
        self.initiate_quit();
        Ok(())
    }
}
