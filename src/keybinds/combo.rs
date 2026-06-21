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
    pub fn matches_event(&self, event: &KeyEvent) -> bool {
        if self.code != event.code {
            return false;
        }
        if self.code == KeyCode::BackTab {
            let self_mods = self.modifiers & !KeyModifiers::SHIFT;
            let event_mods = event.modifiers & !KeyModifiers::SHIFT;
            self_mods == event_mods
        } else {
            self.modifiers == event.modifiers
        }
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

        let code = parse_key_code(key_part)?;
        Some(KeyStroke { code, modifiers })
    }

    /// Parse a key-combo string, possibly a multi-key sequence.
    /// Whitespace separates keys: `"g g"`, `"Space f"`, `"Ctrl+x Ctrl+s"`.
    pub fn parse(s: &str) -> Option<Self> {
        if s.is_empty() {
            return None;
        }
        let tokens: Vec<&str> = s.split_ascii_whitespace().collect();
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
    fn stroke_to_string(s: &KeyStroke) -> String {
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
            Some(KeyCode::Char(c.to_ascii_lowercase()))
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
