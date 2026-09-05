//! Unified overlay contract for sub-view states (Graph, Draw, Canvas, Backup,
//! Outline).
//!
//! Each sub-view state implements [`OverlayView`] and signals event outcomes
//! via the shared [`OverlayResult`] enum. This replaces the per-view `XResult`
//! enums whose variant names diverged (`Back` / `Finished` / `Quit` / `Normal`
//! all meant "leave the overlay") and standardizes the render signature.
//!
//! `overlay_render` takes an `app_status` hint used by views that draw their
//! own title bar internally (Graph, whose status text depends on the
//! graph-area layout computed from the body). Other overlays receive it
//! unused and draw their title bar at the App level. The trait signature is
//! uniform across all overlays either way.


use crate::app::ViewMode;

/// Compile-time registry of sub-view overlays. Adding a view = one enum
/// variant + one match arm here; event dispatch routes through
/// [`OverlayState`] instead of per-view plumbing at the call sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewKind {
    Graph,
    Canvas,
    Draw,
    Backup,
    Outline,
}

impl ViewKind {
    pub fn from_mode(mode: ViewMode) -> Option<Self> {
        match mode {
            ViewMode::Graph => Some(Self::Graph),
            ViewMode::Canvas => Some(Self::Canvas),
            ViewMode::Draw => Some(Self::Draw),
            ViewMode::Backup => Some(Self::Backup),
            ViewMode::Outline => Some(Self::Outline),
            _ => None,
        }
    }
}

/// Owns the live overlay state while it is dispatched (the overlay methods
/// take `&mut App`, so the state must be moved out of `App` first).
#[allow(clippy::large_enum_variant)] // Graph plugin dominates; moved per event, not stored in bulk
pub enum OverlayState {
    Graph(crate::graf_adapter::GrafPlugin),
    Canvas(crate::pinstar::state::PinstarState),
    Draw(crate::draw::app::DrawAppState),
    Backup(crate::backup::state::BackupState),
    Outline(crate::outline::state::OutlineState),
}

impl OverlayState {
    pub fn kind(&self) -> ViewKind {
        match self {
            Self::Graph(_) => ViewKind::Graph,
            Self::Canvas(_) => ViewKind::Canvas,
            Self::Draw(_) => ViewKind::Draw,
            Self::Backup(_) => ViewKind::Backup,
            Self::Outline(_) => ViewKind::Outline,
        }
    }

    /// Take the active overlay state out of the App, if the current mode has
    /// one.
    pub fn take(app: &mut crate::app::App) -> Option<Self> {
        match ViewKind::from_mode(app.mode)? {
            ViewKind::Graph => app.graph_plugin.take().map(Self::Graph),
            ViewKind::Canvas => app.canvas_state.take().map(Self::Canvas),
            ViewKind::Draw => app.draw_state.take().map(Self::Draw),
            ViewKind::Backup => app.backup_state.take().map(Self::Backup),
            ViewKind::Outline => app.outline_state.take().map(Self::Outline),
        }
    }

    pub fn put_back(self, app: &mut crate::app::App) {
        match self {
            Self::Graph(p) => app.graph_plugin = Some(p),
            Self::Canvas(s) => app.canvas_state = Some(s),
            Self::Draw(s) => app.draw_state = Some(s),
            Self::Backup(s) => app.backup_state = Some(s),
            Self::Outline(s) => app.outline_state = Some(s),
        }
    }
}

impl OverlayView for OverlayState {
    fn overlay_render(
        &mut self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        app: &mut crate::app::App,
    ) {
        match self {
            Self::Graph(p) => p.overlay_render(frame, area, app),
            Self::Canvas(s) => s.overlay_render(frame, area, app),
            Self::Draw(s) => s.overlay_render(frame, area, app),
            Self::Backup(s) => s.overlay_render(frame, area, app),
            Self::Outline(s) => s.overlay_render(frame, area, app),
        }
    }

    fn overlay_handle_event(
        &mut self,
        event: crossterm::event::Event,
        app: &mut crate::app::App,
        term_area: ratatui::layout::Rect,
    ) -> anyhow::Result<OverlayResult> {
        match self {
            Self::Graph(p) => p.overlay_handle_event(event, app, term_area),
            Self::Canvas(s) => s.overlay_handle_event(event, app, term_area),
            Self::Draw(s) => s.overlay_handle_event(event, app, term_area),
            Self::Backup(s) => s.overlay_handle_event(event, app, term_area),
            Self::Outline(s) => s.overlay_handle_event(event, app, term_area),
        }
    }
}
use crate::app::HelpTab;

/// Outcome of an overlay-handled event.
#[derive(Debug, Clone)]
pub enum OverlayResult {
    /// Event consumed (or no action); stay in the overlay.
    Continue,
    /// Leave the overlay and return to the previous view.
    Exit,
    /// Open the help page on the given tab.
    OpenHelp(HelpTab),
    /// A note was opened from within the overlay (Graph only).
    NoteOpened(String),
    /// Jump to a line in a note (Outline only).
    JumpToLine { note_id: String, line: usize },
    /// Note modified, needs refresh.
    NoteModified(String),
}

/// Uniform contract for sub-view overlays.
pub trait OverlayView {
    /// Render the overlay into `area`.
    fn overlay_render(
        &mut self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        app: &mut crate::app::App,
    );

    /// Handle one terminal event. Returns the outcome; never panics on
    /// unrecognized input (return [`OverlayResult::Continue`]).
    fn overlay_handle_event(
        &mut self,
        event: crossterm::event::Event,
        app: &mut crate::app::App,
        term_area: ratatui::layout::Rect,
    ) -> anyhow::Result<OverlayResult>;
}
