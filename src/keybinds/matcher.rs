use super::KeyCombo;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;

/// The result of trying to match a key event against a set of bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchOutcome<A> {
    /// The event (possibly combined with previous buffered events) matched an action.
    /// The second element is an optional count prefix (None = no prefix, Some(n) with n >= 1).
    Matched(A, Option<u32>),
    /// The event started a multi-key sequence but hasn't completed one yet; the event was consumed.
    Pending,
    /// No binding matched the event; fall through to hardcoded handling.
    NoMatch,
}

/// Per-view key-sequence matcher with timeout.
/// Buffers recent events and checks them against multi-key combos.
#[derive(Debug, Clone)]
pub struct KeyMatcher {
    pub(crate) pending: Vec<KeyEvent>,
    pub(crate) last_event_at: Option<std::time::Instant>,
    pub(crate) timeout: std::time::Duration,
    pub(crate) count: Option<u32>, // accumulated leading-digit count prefix
}

impl Default for KeyMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyMatcher {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            last_event_at: None,
            timeout: std::time::Duration::from_millis(500),
            count: None,
        }
    }
    pub fn clear(&mut self) {
        self.pending.clear();
        self.last_event_at = None;
        self.count = None;
    }

    pub fn pending_display(&self) -> Option<String> {
        let count_str = self.count.map(|n| n.to_string());
        let pending_str = if self.pending.is_empty() {
            None
        } else {
            Some(
                self.pending
                    .iter()
                    .map(|ev| crate::keybinds::KeyCombo::keyevent_to_string(ev))
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        };
        match (count_str, pending_str) {
            (None, None) => None,
            (Some(c), None) => Some(c),
            (None, Some(p)) => Some(p),
            (Some(c), Some(p)) => Some(format!("{} {}", c, p)),
        }
    }
    /// Resolve a key event against a binding map.
    ///
    /// When `sequences_enabled` is false, this is a simple length-1 match against all bindings.
    /// When true, it buffers events and tries to match multi-key sequences with a 500 ms timeout.
    pub fn resolve<A: Copy + Eq + std::hash::Hash>(
        &mut self,
        event: KeyEvent,
        bindings: &HashMap<A, Vec<KeyCombo>>,
        sequences_enabled: bool,
        counts_enabled: bool,
    ) -> MatchOutcome<A> {
        // Digit capture for count prefix (before any other logic, so digits are consumed
        // even when sequences are disabled).
        if counts_enabled && event.modifiers == KeyModifiers::NONE {
            if let KeyCode::Char(c) = event.code {
                if c.is_ascii_digit() {
                    if c == '0' && self.count.is_none() {
                        // bare '0' is not a count digit; fall through to normal matching
                    } else {
                        let d = (c as u8 - b'0') as u32;
                        self.count = Some((self.count.unwrap_or(0) * 10 + d).min(9999));
                        self.last_event_at = Some(std::time::Instant::now());
                        return MatchOutcome::Pending;
                    }
                }
            }
        }

        if !sequences_enabled {
            for (action, combos) in bindings {
                for combo in combos {
                    if combo.matches(&event) {
                        return MatchOutcome::Matched(*action, self.count.take());
                    }
                }
            }
            self.count = None;
            return MatchOutcome::NoMatch;
        }

        // Check timeout: if too long since last event, clear pending
        if let Some(last) = self.last_event_at
            && last.elapsed() > self.timeout
        {
            self.pending.clear();
            self.count = None;
        }

        // Push current event
        self.pending.push(event);
        self.last_event_at = Some(std::time::Instant::now());

        let mut pending_prefix = false;
        let mut full_match: Option<A> = None;

        for (action, combos) in bindings {
            for combo in combos {
                let keys = &combo.keys;
                let pending_len = self.pending.len();
                if keys.len() < pending_len {
                    continue;
                }
                let pending_slice = &self.pending[..pending_len.min(keys.len())];

                // Check if pending[..] matches keys[..pending_len]
                let all_match = keys
                    .iter()
                    .zip(pending_slice.iter())
                    .all(|(k, ev)| k.matches_event(ev));

                if all_match {
                    if keys.len() == self.pending.len() {
                        // Exact match — keep checking for longer prefixes
                        full_match = Some(*action);
                    } else {
                        // Strict prefix match (longer combo starts with these keys)
                        pending_prefix = true;
                    }
                }
            }
        }

        // If any multi-key sequence is a prefix of the pending buffer, prefer Pending
        // over an immediate single-key match. This allows Space-leader sequences like
        // "Space d" to work even when Space is also bound as a single key.
        if pending_prefix && self.pending.len() == 1 {
            return MatchOutcome::Pending;
        }

        if let Some(action) = full_match {
            self.pending.clear();
            self.last_event_at = None;
            return MatchOutcome::Matched(action, self.count.take());
        }

        if pending_prefix {
            return MatchOutcome::Pending;
        }

        // No prefix match — pop the last event and re-check as a single key
        self.pending.pop(); // remove last pushed event
        let last_event = self.pending.pop().unwrap_or(event); // use last buffered or the original
        self.pending.clear();
        self.last_event_at = None;

        // Re-check as length-1
        for (action, combos) in bindings {
            for combo in combos {
                if combo.matches(&last_event) {
                    return MatchOutcome::Matched(*action, self.count.take());
                }
            }
        }

        self.count = None;
        MatchOutcome::NoMatch
    }
}
