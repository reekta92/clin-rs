use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageSeverity {
    Warning,
    Fatal,
}

#[derive(Debug, Clone)]
pub struct OverlayMessage {
    pub id: usize,
    pub text: String,
    pub severity: MessageSeverity,
    pub timestamp: Instant,
}

#[derive(Debug)]
pub struct MessageOverlay {
    pub messages: Vec<OverlayMessage>,
    pub scroll: usize,
    pub force_open: bool,
    next_id: usize,
    /// Cached active state from the previous tick, so tick_expirations can
    /// detect transitions (active → inactive or vice versa).
    prev_active: bool,
}

impl Default for MessageOverlay {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            scroll: 0,
            force_open: false,
            next_id: 0,
            prev_active: false,
        }
    }
}

const MAX_MESSAGES: usize = 50;

impl MessageOverlay {
    pub fn push(&mut self, text: String, severity: MessageSeverity) {
        let msg = OverlayMessage {
            id: self.next_id,
            text,
            severity,
            timestamp: Instant::now(),
        };
        self.next_id = self.next_id.wrapping_add(1);
        self.messages.push(msg);
        if self.messages.len() > MAX_MESSAGES {
            let drain = self.messages.len() - MAX_MESSAGES;
            self.messages.drain(..drain);
        }
    }

    /// Compare current active state against the cached previous state.
    /// Returns `true` when the overlay should appear or disappear.
    pub fn tick_expirations(&mut self) -> bool {
        if self.messages.len() > MAX_MESSAGES {
            let drain = self.messages.len() - MAX_MESSAGES;
            self.messages.drain(..drain);
        }
        let now_active = self.is_active();
        let changed = now_active != self.prev_active;
        self.prev_active = now_active;
        changed
    }

    pub fn has_fatal(&self) -> bool {
        self.messages
            .iter()
            .any(|m| m.severity == MessageSeverity::Fatal)
    }

    /// Active when force_open is true, or when there are fresh/fatal messages.
    pub fn is_active(&self) -> bool {
        self.force_open || self.messages.iter().any(|m| Self::is_message_visible(m))
    }

    /// A message is visible if it's fatal or still fresh (< 5s).
    fn is_message_visible(m: &OverlayMessage) -> bool {
        m.severity == MessageSeverity::Fatal || m.timestamp.elapsed().as_secs() < 5
    }

    pub fn is_fresh(m: &OverlayMessage) -> bool {
        m.timestamp.elapsed().as_secs() < 5
    }
}
