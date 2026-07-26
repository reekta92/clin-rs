//! Behaviors that differ between a real terminal and a GUI window host.

pub trait HostHooks: Send {
    /// About to run an external command ($EDITOR) — TUI: leave alt screen/raw mode.
    fn suspend_for_external(&mut self) {}
    /// External command finished — TUI: re-enter alt screen/raw mode + clear.
    fn resume_from_external(&mut self) {}
    /// Whether bare Ctrl+C force-quits (TUI yes, GUI no).
    fn ctrl_c_quits(&self) -> bool {
        true
    }
}

pub struct TuiHost;
impl HostHooks for TuiHost {
    fn suspend_for_external(&mut self) {
        if let Err(e) = crossterm::terminal::disable_raw_mode() {
            eprintln!("Failed to disable raw mode: {e}");
        }
        if let Err(e) = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture,
            crossterm::event::DisableBracketedPaste
        ) {
            eprintln!("Failed to reset terminal: {e}");
        }
    }
    fn resume_from_external(&mut self) {
        if let Err(e) = crossterm::terminal::enable_raw_mode() {
            eprintln!("Failed to enable raw mode: {e}");
        }
        if let Err(e) = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::EnterAlternateScreen,
            crossterm::event::EnableMouseCapture,
            crossterm::event::EnableBracketedPaste,
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
        ) {
            eprintln!("Failed to restore terminal: {e}");
        }
    }
}

pub struct GuiHost;
impl HostHooks for GuiHost {
    fn ctrl_c_quits(&self) -> bool {
        false
    }
}
