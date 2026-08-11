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

#[derive(Debug, Default)]
pub struct MessageOverlay {
    pub messages: Vec<OverlayMessage>,
    pub scroll: usize,
    pub force_open: bool,
    next_id: usize,
    /// Cached active state from the previous tick, so tick_expirations can
    /// detect transitions (active → inactive or vice versa).
    prev_active: bool,
}

const MAX_MESSAGES: usize = 50;

const FRESH_SECS: u64 = 3;

impl MessageOverlay {
    pub fn push(&mut self, text: String, severity: MessageSeverity) {
        // Consecutive-dedupe: identical to the previous message re-freshens that
        // entry's timestamp so the occurrence re-appears in the dropdown, instead
        // of growing a duplicate line.
        if let Some(last) = self.messages.last_mut()
            && last.text == text
            && last.severity == severity
        {
            last.timestamp = Instant::now();
            return;
        }
        let msg = OverlayMessage {
            id: self.next_id,
            text,
            severity,
            timestamp: Instant::now(),
        };
        self.next_id = self.next_id.wrapping_add(1);
        self.messages.push(msg);
        self.retain_non_fatals_over_capacity();
    }

    fn retain_non_fatals_over_capacity(&mut self) {
        if self.messages.len() <= MAX_MESSAGES {
            return;
        }
        let mut to_remove = self.messages.len() - MAX_MESSAGES;
        self.messages.retain(|m| {
            if to_remove > 0 && m.severity != MessageSeverity::Fatal {
                to_remove -= 1;
                false
            } else {
                true
            }
        });
    }

    /// Compare current active state against the cached previous state.
    /// Returns `true` when the overlay should appear or disappear.
    pub fn tick_expirations(&mut self) -> bool {
        self.retain_non_fatals_over_capacity();
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
        self.force_open || self.messages.iter().any(Self::is_message_visible)
    }

    /// A message is visible if it's fatal or still fresh (< 3s).
    fn is_message_visible(m: &OverlayMessage) -> bool {
        m.severity == MessageSeverity::Fatal || m.timestamp.elapsed().as_secs() < FRESH_SECS
    }

    pub fn is_fresh(m: &OverlayMessage) -> bool {
        m.timestamp.elapsed().as_secs() < FRESH_SECS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_dedupes_consecutive_identical() {
        let mut overlay = MessageOverlay::default();
        overlay.push("error A".to_string(), MessageSeverity::Warning);
        overlay.push("error A".to_string(), MessageSeverity::Warning);
        assert_eq!(overlay.messages.len(), 1);
        assert_eq!(overlay.messages[0].text, "error A");

        // Age the entry: after the freshness window expires, the overlay
        // should be inactive.
        overlay.messages[0].timestamp = Instant::now()
            .checked_sub(std::time::Duration::from_secs(10))
            .unwrap();
        assert!(!overlay.is_active());

        // Re-push the same message: it must re-freshen the existing entry
        // instead of appending a duplicate.
        overlay.push("error A".to_string(), MessageSeverity::Warning);
        assert_eq!(overlay.messages.len(), 1);
        assert!(overlay.is_active());
    }

    #[test]
    fn push_allows_different_text() {
        let mut overlay = MessageOverlay::default();
        overlay.push("error A".to_string(), MessageSeverity::Warning);
        overlay.push("error B".to_string(), MessageSeverity::Warning);
        assert_eq!(overlay.messages.len(), 2);
    }

    #[test]
    fn push_allows_same_text_different_severity() {
        let mut overlay = MessageOverlay::default();
        overlay.push("error A".to_string(), MessageSeverity::Warning);
        overlay.push("error A".to_string(), MessageSeverity::Fatal);
        assert_eq!(overlay.messages.len(), 2);
        assert_eq!(overlay.messages[0].severity, MessageSeverity::Warning);
        assert_eq!(overlay.messages[1].severity, MessageSeverity::Fatal);
    }

    #[test]
    fn non_consecutive_duplicate_not_deduped() {
        let mut overlay = MessageOverlay::default();
        overlay.push("error A".to_string(), MessageSeverity::Warning);
        overlay.push("error B".to_string(), MessageSeverity::Warning);
        overlay.push("error A".to_string(), MessageSeverity::Warning);
        assert_eq!(overlay.messages.len(), 3);
    }

    #[test]
    fn drain_keeps_fatal_messages() {
        let mut overlay = MessageOverlay::default();
        overlay.push("fatal error".to_string(), MessageSeverity::Fatal);
        for i in 0..60 {
            overlay.push(format!("warning {i}"), MessageSeverity::Warning);
        }
        // Fatal must be retained even when capacity exceeded
        assert!(overlay.has_fatal());
        assert!(overlay.messages.len() <= 50);
        // The fatal should be the first message
        assert_eq!(overlay.messages[0].severity, MessageSeverity::Fatal);
    }
}
