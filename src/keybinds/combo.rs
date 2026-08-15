use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::borrow::Cow;

/// A single keystroke — one key code with optional modifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyStroke {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

/// A key combination, possibly a multi-key sequence like `"g g"` or `"Ctrl+x Ctrl+s"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    pub keys: Vec<KeyStroke>,
}

impl KeyStroke {
    /// Returns true if this single keystroke matches the given event.
    ///
    /// Per crossterm conventions, uppercase `Char` keys can encode Shift in
    /// the character itself. Normalize both the binding and the event to
    /// a standard representation to compare them correctly.
    pub fn matches_event(&self, event: &KeyEvent) -> bool {
        // Canonicalize terminal-variant Ctrl+Backspace encodings.
        let event_code = if event.modifiers.contains(KeyModifiers::CONTROL) {
            match event.code {
                KeyCode::Char('\x08') | KeyCode::Char('\x7f') => KeyCode::Backspace,
                c => c,
            }
        } else {
            event.code
        };

        // Helper to normalize an alphabetic Char + modifiers into (lowercase_char, effective_modifiers).
        // It moves the "uppercase" nature of the char into the SHIFT modifier,
        // and makes the char lowercase for comparison.
        fn normalize(code: KeyCode, mut mods: KeyModifiers) -> (KeyCode, KeyModifiers) {
            if let KeyCode::Char(c) = code {
                if c.is_ascii_alphabetic() {
                    if c.is_uppercase() {
                        mods.insert(KeyModifiers::SHIFT);
                    }
                    return (KeyCode::Char(c.to_ascii_lowercase()), mods);
                }
            }
            // BackTab is essentially Shift+Tab
            if code == KeyCode::BackTab {
                mods.insert(KeyModifiers::SHIFT);
                return (KeyCode::Tab, mods);
            }
            (code, mods)
        }

        let (self_code, self_mods) = normalize(self.code, self.modifiers);
        let (ev_code, ev_mods) = normalize(event_code, event.modifiers);

        self_code == ev_code && self_mods == ev_mods
    }
}

impl KeyCombo {
    /// Build a single-key combo with no modifiers.
    pub fn simple(code: KeyCode) -> Self {
        Self {
            keys: vec![KeyStroke {
                code,
                modifiers: KeyModifiers::NONE,
            }],
        }
    }

    /// Build a single-key combo with CONTROL modifier.
    pub fn ctrl(code: KeyCode) -> Self {
        Self {
            keys: vec![KeyStroke {
                code,
                modifiers: KeyModifiers::CONTROL,
            }],
        }
    }

    /// Build a single-key combo with SHIFT modifier.
    pub fn shift(code: KeyCode) -> Self {
        Self {
            keys: vec![KeyStroke {
                code,
                modifiers: KeyModifiers::SHIFT,
            }],
        }
    }

    /// Build a single-key combo with CONTROL|SHIFT modifiers.
    pub fn ctrl_shift(code: KeyCode) -> Self {
        Self {
            keys: vec![KeyStroke {
                code,
                modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            }],
        }
    }

    /// Parse a single keystroke token (e.g. `"Ctrl+q"`, `"g"`, `"Enter"`).
    fn parse_single_stroke(s: &str) -> Option<KeyStroke> {
        if s.is_empty() {
            return None;
        }
        let (modifiers_str, key_part) = if s == "+" {
            ("", "+")
        } else if let Some(stripped) = s.strip_suffix("++") {
            (stripped, "+")
        } else {
            match s.rfind('+') {
                Some(idx) => (&s[..idx], &s[idx + 1..]),
                None => ("", s),
            }
        };

        let mut modifiers = KeyModifiers::NONE;
        if !modifiers_str.is_empty() {
            let parts: Vec<&str> = modifiers_str.split('+').collect();
            for part in parts {
                let part_lower = part.to_lowercase();
                match part_lower.as_str() {
                    "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
                    "shift" => modifiers |= KeyModifiers::SHIFT,
                    "alt" => modifiers |= KeyModifiers::ALT,
                    "super" | "meta" | "cmd" => modifiers |= KeyModifiers::SUPER,
                    _ => return None,
                }
            }
        }
        let mut code = parse_key_code(key_part)?;
        // crossterm delivers Shift+<letter> as an uppercase Char whose SHIFT
        // modifier is stripped by `matches_event` (capitalization signals
        // shift). Canonicalize lowercase Char + SHIFT (sole modifier) to
        // uppercase so a parsed "Shift+j" binding is byte-identical to the
        // `KeyCombo::shift(KeyCode::Char('J'))` helper used by defaults and
        // matches real terminal input. CTRL+SHIFT and other combos are left
        // untouched to preserve existing `ctrl_shift(...)` helper semantics.
        if modifiers == KeyModifiers::SHIFT
            && let KeyCode::Char(c) = code
            && c.is_ascii_lowercase()
        {
            code = KeyCode::Char(c.to_ascii_uppercase());
        }
        Some(KeyStroke { code, modifiers })
    }

    /// Parse a key-combo string, possibly a multi-key sequence.
    /// Whitespace separates keys: `"g g"`, `"Space f"`, `"Ctrl+x Ctrl+s"`.
    pub fn parse(s: &str) -> Option<Self> {
        if s.is_empty() {
            return None;
        }
        // Trim whitespace around '+' within a single stroke so "Shift + j"
        // collapses to "Shift+j" before whitespace is used to split multi-key
        // sequences ("g g", "Ctrl+x Ctrl+s").
        let collapsed: String = s.split('+').map(|p| p.trim()).collect::<Vec<_>>().join("+");
        let tokens: Vec<&str> = collapsed.split_ascii_whitespace().collect();
        if tokens.is_empty() {
            return None;
        }
        let mut keys = Vec::with_capacity(tokens.len());
        for token in tokens {
            keys.push(Self::parse_single_stroke(token)?);
        }
        Some(Self { keys })
    }

    /// Format a single keystroke for display (e.g. `"Ctrl+q"`, `"g"`, `"Enter"`).
    pub(crate) fn stroke_to_string(s: &KeyStroke) -> String {
        let key = key_code_to_string(&s.code);
        let mut result = String::with_capacity(24);

        let mut need_sep = false;
        if s.modifiers.contains(KeyModifiers::CONTROL) {
            result.push_str("Ctrl");
            need_sep = true;
        }
        if s.modifiers.contains(KeyModifiers::SHIFT) {
            if need_sep {
                result.push('+');
            }
            result.push_str("Shift");
            need_sep = true;
        }
        if s.modifiers.contains(KeyModifiers::ALT) {
            if need_sep {
                result.push('+');
            }
            result.push_str("Alt");
            need_sep = true;
        }
        if s.modifiers.contains(KeyModifiers::SUPER) {
            if need_sep {
                result.push('+');
            }
            result.push_str("Super");
            need_sep = true;
        }
        if need_sep {
            result.push('+');
        }
        result.push_str(&key);

        result
    }

    /// Convert a crossterm `KeyEvent` to a display string.
    pub(crate) fn keyevent_to_string(ev: &crossterm::event::KeyEvent) -> String {
        Self::stroke_to_string(&KeyStroke {
            code: ev.code,
            modifiers: ev.modifiers,
        })
    }

    /// Display string for this combo.
    /// Joins with no separator when every key is a single ASCII letter/digit (`gg`, `dd`),
    /// otherwise with a space (`g G`, `Space f`, `Ctrl+x Ctrl+s`).
    pub fn to_display_string(&self) -> String {
        let parts: Vec<String> = self.keys.iter().map(Self::stroke_to_string).collect();

        let all_simple = self.keys.iter().all(|s| {
            s.modifiers == KeyModifiers::NONE
                && if let KeyCode::Char(c) = s.code {
                    c.is_ascii_alphanumeric()
                } else {
                    false
                }
        });

        if all_simple {
            parts.join("")
        } else {
            parts.join(" ")
        }
    }

    /// Persistence form: every sequence stroke is separated by one ASCII space.
    pub(crate) fn to_config_string(&self) -> String {
        self.keys
            .iter()
            .map(Self::stroke_to_string)
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Legacy single-key fast-path match.
    /// Returns `true` if this is a length-1 combo whose key matches the event.
    /// Multi-key combos always return `false` here; use `KeyMatcher::resolve` for sequences.
    pub fn matches(&self, event: &KeyEvent) -> bool {
        self.keys.len() == 1 && self.keys[0].matches_event(event)
    }
}

fn parse_key_code(s: &str) -> Option<KeyCode> {
    let s_lower = s.to_lowercase();
    match s_lower.as_str() {
        "enter" | "return" => Some(KeyCode::Enter),
        "esc" | "escape" => Some(KeyCode::Esc),
        "backspace" | "bs" => Some(KeyCode::Backspace),
        "tab" => Some(KeyCode::Tab),
        "backtab" => Some(KeyCode::BackTab),
        "space" | " " => Some(KeyCode::Char(' ')),
        "delete" | "del" => Some(KeyCode::Delete),
        "insert" | "ins" => Some(KeyCode::Insert),
        "home" => Some(KeyCode::Home),
        "end" => Some(KeyCode::End),
        "pageup" | "pgup" => Some(KeyCode::PageUp),
        "pagedown" | "pgdn" => Some(KeyCode::PageDown),
        "up" => Some(KeyCode::Up),
        "down" => Some(KeyCode::Down),
        "left" => Some(KeyCode::Left),
        "right" => Some(KeyCode::Right),

        "f1" => Some(KeyCode::F(1)),
        "f2" => Some(KeyCode::F(2)),
        "f3" => Some(KeyCode::F(3)),
        "f4" => Some(KeyCode::F(4)),
        "f5" => Some(KeyCode::F(5)),
        "f6" => Some(KeyCode::F(6)),
        "f7" => Some(KeyCode::F(7)),
        "f8" => Some(KeyCode::F(8)),
        "f9" => Some(KeyCode::F(9)),
        "f10" => Some(KeyCode::F(10)),
        "f11" => Some(KeyCode::F(11)),
        "f12" => Some(KeyCode::F(12)),

        _ if s.len() == 1 => {
            let c = s.chars().next()?;
            Some(KeyCode::Char(c))
        }

        _ => None,
    }
}

fn key_code_to_string(code: &KeyCode) -> Cow<'static, str> {
    match code {
        KeyCode::Enter => Cow::Borrowed("Enter"),
        KeyCode::Esc => Cow::Borrowed("Esc"),
        KeyCode::Backspace => Cow::Borrowed("Backspace"),
        KeyCode::Tab => Cow::Borrowed("Tab"),
        KeyCode::BackTab => Cow::Borrowed("BackTab"),
        KeyCode::Delete => Cow::Borrowed("Delete"),
        KeyCode::Insert => Cow::Borrowed("Insert"),
        KeyCode::Home => Cow::Borrowed("Home"),
        KeyCode::End => Cow::Borrowed("End"),
        KeyCode::PageUp => Cow::Borrowed("PageUp"),
        KeyCode::PageDown => Cow::Borrowed("PageDown"),
        KeyCode::Up => Cow::Borrowed("Up"),
        KeyCode::Down => Cow::Borrowed("Down"),
        KeyCode::Left => Cow::Borrowed("Left"),
        KeyCode::Right => Cow::Borrowed("Right"),
        KeyCode::F(n) => Cow::Owned(format!("F{n}")),
        KeyCode::Char(' ') => Cow::Borrowed("Space"),
        KeyCode::Char(c) => Cow::Owned(c.to_string()),
        _ => Cow::Borrowed("?"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    /// Ctrl+Backspace delivered as Char('\x08')+CONTROL should match Backspace+CONTROL.
    #[test]
    fn ctrl_backspace_via_0x08() {
        let stroke = KeyStroke {
            code: KeyCode::Backspace,
            modifiers: KeyModifiers::CONTROL,
        };
        let event = KeyEvent::new(KeyCode::Char('\x08'), KeyModifiers::CONTROL);
        assert!(stroke.matches_event(&event));
    }

    /// Ctrl+Backspace delivered as Char('\x7f')+CONTROL should match Backspace+CONTROL.
    #[test]
    fn ctrl_backspace_via_0x7f() {
        let stroke = KeyStroke {
            code: KeyCode::Backspace,
            modifiers: KeyModifiers::CONTROL,
        };
        let event = KeyEvent::new(KeyCode::Char('\x7f'), KeyModifiers::CONTROL);
        assert!(stroke.matches_event(&event));
    }

    /// Plain Char('\x08') without CONTROL must NOT match — normal typing of that char works.
    #[test]
    fn plain_0x08_does_not_match_backspace() {
        let stroke = KeyStroke {
            code: KeyCode::Backspace,
            modifiers: KeyModifiers::CONTROL,
        };
        let event = KeyEvent::new(KeyCode::Char('\x08'), KeyModifiers::NONE);
        assert!(!stroke.matches_event(&event));
    }

    /// Plain Backspace (delete char) still works when no CONTROL modifier.
    #[test]
    fn plain_backspace_still_matches() {
        let stroke = KeyStroke {
            code: KeyCode::Backspace,
            modifiers: KeyModifiers::NONE,
        };
        let event = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
        assert!(stroke.matches_event(&event));
    }
    #[test]
    fn l_and_shift_l_conflict() {
        let l_stroke = KeyStroke {
            code: KeyCode::Char('l'),
            modifiers: KeyModifiers::NONE,
        };
        let shift_l_event = KeyEvent::new(KeyCode::Char('L'), KeyModifiers::SHIFT);
        // Does a lowercase 'l' match 'Shift+L'?
        assert!(!l_stroke.matches_event(&shift_l_event), "Lowercase binding should NOT match Shift+L");
    }

    #[test]
    fn config_string_round_trips_sequences() {
        for value in ["g g", "g e", "d d", "g G", "Ctrl+x Ctrl+s"] {
            let combo = KeyCombo::parse(value).unwrap();
            assert_eq!(combo.to_config_string(), value);
            assert_eq!(KeyCombo::parse(&combo.to_config_string()), Some(combo));
        }
    }
}
