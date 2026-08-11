//! Host-provided input source. crossterm `Event` is the app's input lingua
//! franca for both the TUI and GUI hosts.

pub trait EventSource {
    fn poll(&mut self, timeout: std::time::Duration) -> std::io::Result<bool>;
    fn read(&mut self) -> std::io::Result<crossterm::event::Event>;
}

/// TUI host: delegates to crossterm's queue.
pub struct CrosstermEventSource;
impl EventSource for CrosstermEventSource {
    fn poll(&mut self, timeout: std::time::Duration) -> std::io::Result<bool> {
        crossterm::event::poll(timeout)
    }
    fn read(&mut self) -> std::io::Result<crossterm::event::Event> {
        crossterm::event::read()
    }
}

/// GUI host: fed from an mpsc channel by the windowing thread.
pub struct ChannelEventSource {
    rx: std::sync::mpsc::Receiver<crossterm::event::Event>,
    stashed: std::collections::VecDeque<crossterm::event::Event>,
}
impl ChannelEventSource {
    pub fn new(rx: std::sync::mpsc::Receiver<crossterm::event::Event>) -> Self {
        Self {
            rx,
            stashed: std::collections::VecDeque::new(),
        }
    }
}
impl EventSource for ChannelEventSource {
    fn poll(&mut self, timeout: std::time::Duration) -> std::io::Result<bool> {
        while let Ok(ev) = self.rx.try_recv() {
            self.stashed.push_back(ev);
        }
        if !self.stashed.is_empty() {
            return Ok(true);
        }
        match self.rx.recv_timeout(timeout) {
            Ok(ev) => {
                self.stashed.push_back(ev);
                Ok(true)
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Ok(false),
        }
    }
    fn read(&mut self) -> std::io::Result<crossterm::event::Event> {
        while let Ok(ev) = self.rx.try_recv() {
            self.stashed.push_back(ev);
        }
        self.stashed.pop_front().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::WouldBlock, "ChannelEventSource empty")
        })
    }
}
