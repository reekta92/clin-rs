//! Terminal host behaviors — suspends/resumes the TUI around external commands.

pub struct TuiHost;

impl TuiHost {
    /// About to run an external command ($EDITOR) — leave alt screen/raw mode.
    pub fn suspend_for_external(&mut self) {
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
    /// External command finished — re-enter alt screen/raw mode + clear.
    pub fn resume_from_external(&mut self) {
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
