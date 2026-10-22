//! Application icons: single-cell font symbols and emoji alternatives.

use crate::config::IconMode;

#[derive(Clone, Copy, Debug)]
pub struct Icon(&'static str, &'static str, &'static str);

impl Icon {
    pub const fn pair(self) -> (&'static str, &'static str) {
        (self.0, self.1)
    }

    pub fn get(self, mode: IconMode) -> &'static str {
        match mode {
            IconMode::Nerd => self.0,
            IconMode::Unicode => self.1,
            IconMode::Emoji => self.2,
            IconMode::None => "",
        }
    }

    pub fn char(self, mode: IconMode) -> char {
        self.get(mode).chars().next().unwrap_or(' ')
    }
}

macro_rules! icons {
    ($($name:ident: $nerd:literal, $unicode:literal, $emoji:literal;)+) => {
        $(pub const $name: Icon = Icon($nerd, $unicode, $emoji);)+

        const ALL: &[(&str, Icon)] = &[$((stringify!($name), $name)),+];
    };
}

// Font Awesome's original BMP set avoids version-sensitive Nerd Font mappings.
// Unicode fallbacks avoid emoji presentation, combining marks and wide glyphs.
icons! {
    FILE: "\u{f15c}", "≡", "📄";
    MARKDOWN: "\u{f02d}", "¶", "📘";
    LOCK: "\u{f023}", "⊠", "🔒";
    DRAW: "\u{f1fc}", "✎", "🎨";
    CANVAS: "\u{f247}", "◇", "💠";
    BASES: "\u{f0ce}", "▦", "📊";
    PNG: "\u{f1c5}", "▧", "🌄";
    JPEG: "\u{f030}", "▣", "📷";
    GIF: "\u{f008}", "▷", "🎥";
    WEBP: "\u{f03e}", "◎", "🌐";
    UNKNOWN: "\u{f128}", "?", "❓";
    FOLDER: "\u{f07b}", "▱", "📁";
    OPEN_FOLDER: "\u{f07c}", "▰", "📂";
    PIN: "\u{f08d}", "♦", "📌";
    TAG: "\u{f02b}", "#", "🔖";
    SUBNOTES: "\u{f0c5}", "⊞", "📚";
    SETTINGS: "\u{f013}", "⚙", "🔧";
    EXPANDED: "\u{f078}", "▾", "🔽";
    COLLAPSED: "\u{f054}", "▸", "🔸";
    PARENT: "\u{f062}", "↑", "🔼";
    ADD: "\u{f067}", "+", "➕";
    SEARCH: "\u{f002}", "⌕", "🔍";
    TODAY: "\u{f133}", "□", "📅";
    CALENDAR: "\u{f073}", "▦", "📆";
    UNTAGGED: "\u{f02b}", "∅", "🔖";
    FILTER: "\u{f0b0}", "∇", "🔎";
    CLOCK: "\u{f017}", "◷", "🕒";
    LIST: "\u{f0ca}", "≡", "📋";
    IMPORT: "\u{f019}", "↓", "📥";
    VIEWS: "\u{f06e}", "◉", "👀";
    INFO: "\u{f05a}", "i", "💡";
    HELP: "\u{f059}", "?", "❔";
    EDIT: "\u{f040}", "✎", "📝";
    GRAPH: "\u{f1e0}", "○", "🔗";
    OUTLINE: "\u{f03a}", "≡", "📑";
    TEMPLATE: "\u{f0c5}", "▤", "📜";
    SELECT: "\u{f245}", "+", "🎯";
    SHAPE: "\u{f096}", "□", "🔷";
    TEXT: "\u{f031}", "T", "🔤";
    ERASE: "\u{f12d}", "×", "🧽";
    TRASH: "\u{f1f8}", "×", "🚮";
    IMAGE: "\u{f03e}", "▧", "🌅";
    OCR: "\u{f065}", "T", "🔠";
    CLIPBOARD: "\u{f0ea}", "▤", "📋";
    CODE: "\u{f121}", "λ", "💻";
    WEB: "\u{f0ac}", "◎", "🌍";
    LINK: "\u{f0c1}", "∞", "🔗";
    PLAY: "\u{f04b}", "▷", "🎬";
    MAIL: "\u{f0e0}", "✉", "📧";
    CHAT: "\u{f075}", "»", "💬";
    THEME: "\u{f042}", "◐", "🌗";
    KEYBOARD: "\u{f11c}", "K", "🎹";
    TERMINAL: "\u{f120}", ">", "💻";
    COLUMNS: "\u{f0db}", "║", "🏢";
    WRAP: "\u{f03b}", "↩", "🔁";
    ZEN: "\u{f186}", "☾", "🌙";
    NUMBER: "\u{f0cb}", "#", "🔢";
    QUIT: "\u{f08b}", "×", "🚪";
    SORT: "\u{f0dc}", "↕", "🔃";
    ICONS: "\u{f031}", "A", "🔣";
    GOAL: "\u{f140}", "⊙", "🎯";
    LAYOUT: "\u{f009}", "⊞", "🔲";
    SCROLL: "\u{f07d}", "↕", "🔃";
    ALIGN: "\u{f036}", "≡", "📐";
    MAP: "\u{f279}", "⊡", "🌎";
    UNLINK: "\u{f127}", "∅", "🔓";
    BACK: "\u{f060}", "←", "👈";
    FORWARD: "\u{f061}", "→", "👉";
    SAVE: "\u{f0c7}", "✓", "💾";
    SUCCESS: "\u{f00c}", "✓", "✅";
    CANCEL: "\u{f00d}", "✗", "❌";
}

/// Resolve built-in action icon pairs through the same catalog as view icons.
pub(crate) fn emoji_for_pair(nerd: &str, unicode: &'static str) -> &'static str {
    ALL.iter()
        .find(|(_, icon)| icon.0 == nerd && icon.1 == unicode)
        .map_or(unicode, |(_, icon)| icon.2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

    #[test]
    fn note_file_icons_are_distinct_and_case_insensitive() {
        let theme = crate::app_theme::AppThemeColors::default();
        for mode in [IconMode::Nerd, IconMode::Unicode, IconMode::Emoji] {
            let mut seen = std::collections::HashSet::new();
            for ext in [
                "md", "txt", "clin", "draw", "canvas", "base", "png", "jpg", "gif", "webp",
            ] {
                let (glyph, label, color) =
                    super::super::note_file_glyph(&format!("note.{ext}"), &theme, mode);
                assert!(seen.insert(glyph), "duplicate file icon: {ext}, {mode:?}");
                assert_eq!(
                    super::super::note_file_glyph(
                        &format!("note.{}", ext.to_ascii_uppercase()),
                        &theme,
                        mode
                    ),
                    (glyph, label, color),
                );
            }
            assert_eq!(
                super::super::note_file_glyph("unknown.bin", &theme, mode).0,
                UNKNOWN.char(mode)
            );
        }
    }

    #[test]
    fn emoji_icons_keep_the_full_glyph_in_char_and_action_paths() {
        for &(name, icon) in ALL {
            let glyph = icon.get(IconMode::Emoji);
            assert_eq!(glyph.chars().count(), 1, "{name}");
            assert_eq!(glyph.width(), 2, "{name}");
            assert_eq!(icon.char(IconMode::Emoji).to_string(), glyph, "{name}");
            assert_eq!(
                super::super::get_icon(icon.0, icon.1, IconMode::Emoji),
                glyph,
                "{name}"
            );
        }
    }

    #[test]
    fn every_icon_is_one_cell_and_respects_icon_mode() {
        for &(name, icon) in ALL {
            for mode in [IconMode::Nerd, IconMode::Unicode] {
                let glyph = icon.get(mode);
                assert_eq!(glyph.chars().count(), 1, "{name}: {mode:?}");
                assert_eq!(glyph.width(), 1, "{name}: {mode:?}");
                assert_eq!(icon.char(mode).width(), Some(1), "{name}: {mode:?}");
                if mode == IconMode::Unicode {
                    assert!(glyph.chars().all(|c| c <= '\u{ffff}'), "{name}");
                }
            }
            assert_eq!(icon.get(IconMode::None), "", "{name}");
            assert_eq!(icon.char(IconMode::None), ' ', "{name}");
        }
    }
}
