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
        terminal: &ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    ) -> anyhow::Result<OverlayResult>;
}
