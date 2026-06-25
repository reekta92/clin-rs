use std::collections::HashMap;
use crossterm::event::KeyEvent;
use super::KeyCombo;

/// The result of trying to match a key event against a set of bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchOutcome<A> {
    /// The event (possibly combined with previous buffered events) matched an action.
    Matched(A),
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
        }
    }

    pub fn clear(&mut self) {
        self.pending.clear();
        self.last_event_at = None;
    }

    /// Returns a display string for the currently buffered pending keys,
    /// or `None` if no sequence is in progress.
    pub fn pending_display(&self) -> Option<String> {
        if self.pending.is_empty() {
            return None;
        }
        Some(
            self.pending
                .iter()
                .map(|ev| crate::keybinds::KeyCombo::keyevent_to_string(ev))
                .collect::<Vec<_>>()
                .join(" "),
        )
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
    ) -> MatchOutcome<A> {
        if !sequences_enabled {
            for (action, combos) in bindings {
                for combo in combos {
                    if combo.matches(&event) {
                        return MatchOutcome::Matched(*action);
                    }
                }
            }
            return MatchOutcome::NoMatch;
        }

        // Check timeout: if too long since last event, clear pending
        if let Some(last) = self.last_event_at
            && last.elapsed() > self.timeout
        {
            self.pending.clear();
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
            return MatchOutcome::Matched(action);
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
                    return MatchOutcome::Matched(*action);
                }
            }
        }

        MatchOutcome::NoMatch
    }
}
