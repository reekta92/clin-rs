use super::{
    BackupAction, CanvasAction, ContentTreeAction, DrawAction, EditAction, GraphAction, HelpAction,
    KeyCombo, KeyMatcher, Keybinds, KeybindsToml, ListAction, MatchOutcome, SetupAction,
};
use anyhow::{Context, Result};
use crossterm::event::KeyEvent;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ── Generic keybind-section helpers ─────────────────────────────────────────

fn merge_section<A: std::hash::Hash + std::cmp::Eq + Clone>(
    into: &mut HashMap<A, Vec<KeyCombo>>,
    from: &HashMap<A, Vec<String>>,
) {
    for (action, strs) in from {
        let combos: Vec<KeyCombo> = strs.iter().filter_map(|s| KeyCombo::parse(s)).collect();
        if !combos.is_empty() {
            into.insert(action.clone(), combos);
        }
    }
}

fn section_to_toml<A: std::hash::Hash + std::cmp::Eq + Clone>(
    from: &HashMap<A, Vec<KeyCombo>>,
) -> HashMap<A, Vec<String>> {
    from.iter()
        .map(|(a, c)| {
            (
                a.clone(),
                c.iter().map(|k| k.to_display_string()).collect::<Vec<_>>(),
            )
        })
        .collect()
}

/// Emits the four parallel accessor families (`matches_*`, `*_keys_display`,
/// `bindings_for_*`, `display_*`) for one keybind scope from a single template.
/// Add a scope by adding one `keybind_scope!(...)` line below.
macro_rules! keybind_scope {
    ($field:ident, $Action:ty, $matches:ident, $kd:ident, $bindings:ident, $display:ident) => {
        impl Keybinds {
            pub fn $matches(&self, action: $Action, event: &KeyEvent) -> bool {
                self.$field
                    .get(&action)
                    .is_some_and(|c| c.iter().any(|x| x.matches(event)))
            }
            pub fn $kd(&self, action: $Action) -> String {
                self.$field
                    .get(&action)
                    .map(|c| {
                        c.iter()
                            .map(|k| k.to_display_string())
                            .collect::<Vec<_>>()
                            .join("/")
                    })
                    .unwrap_or_default()
            }
            pub fn $bindings(&self) -> &HashMap<$Action, Vec<KeyCombo>> {
                &self.$field
            }
            pub fn $display(&self, action: $Action) -> String {
                self.$field
                    .get(&action)
                    .map(|v| Self::pick_hint_key(v))
                    .unwrap_or_else(|| "?".to_string())
            }
        }
    };
}

impl Keybinds {
    pub fn load(path: &Path) -> Result<Self> {
        Self::load_layered(path, Self::default())
    }

    pub fn load_layered(path: &Path, base: Keybinds) -> Result<Keybinds> {
        let mut keybinds = base;

        if !path.exists() {
            return Ok(keybinds);
        }

        let content = fs::read_to_string(path).context("failed to read keybinds file")?;

        let toml: KeybindsToml =
            toml::from_str(&content).context("failed to parse keybinds file")?;

        merge_section(&mut keybinds.list, &toml.list);
        merge_section(&mut keybinds.edit, &toml.edit);
        merge_section(&mut keybinds.help, &toml.help);
        merge_section(&mut keybinds.graph, &toml.graph);
        merge_section(&mut keybinds.draw, &toml.draw);
        merge_section(&mut keybinds.canvas, &toml.canvas);
        merge_section(&mut keybinds.backup, &toml.backup);
        merge_section(&mut keybinds.content_tree, &toml.content_tree);
        merge_section(&mut keybinds.setup, &toml.setup);
        Ok(keybinds)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let toml = self.to_toml();
        let content = toml::to_string_pretty(&toml).context("failed to serialize keybinds")?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("failed to create keybinds directory")?;
        }

        crate::fsutil::atomic_write(path, content.as_bytes())?;

        Ok(())
    }

    pub fn to_toml(&self) -> KeybindsToml {
        KeybindsToml {
            list: section_to_toml(&self.list),
            edit: section_to_toml(&self.edit),
            help: section_to_toml(&self.help),
            graph: section_to_toml(&self.graph),
            draw: section_to_toml(&self.draw),
            canvas: section_to_toml(&self.canvas),
            backup: section_to_toml(&self.backup),
            content_tree: section_to_toml(&self.content_tree),
            setup: section_to_toml(&self.setup),
        }
    }

    // -- Resolve wrappers (delegate to KeyMatcher::resolve) --
    pub fn resolve_list(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
        counts: bool,
    ) -> MatchOutcome<ListAction> {
        let mut filtered = self.list.clone();
        filtered.remove(&ListAction::Confirm);
        filtered.remove(&ListAction::Cancel);
        m.resolve(event, &filtered, seq, counts)
    }
    pub fn resolve_edit(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
        _counts: bool,
    ) -> MatchOutcome<EditAction> {
        // Edit view never accepts count - digits are text input
        m.resolve(event, self.bindings_for_edit(), seq, false)
    }
    pub fn resolve_help(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
        _counts: bool,
    ) -> MatchOutcome<HelpAction> {
        // Help view never accepts count - digits are tab-switchers
        m.resolve(event, self.bindings_for_help(), seq, false)
    }
    pub fn resolve_graph(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
        counts: bool,
    ) -> MatchOutcome<GraphAction> {
        m.resolve(event, self.bindings_for_graph(), seq, counts)
    }
    pub fn resolve_draw(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
        counts: bool,
    ) -> MatchOutcome<DrawAction> {
        m.resolve(event, self.bindings_for_draw(), seq, counts)
    }
    pub fn resolve_canvas(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
        counts: bool,
    ) -> MatchOutcome<CanvasAction> {
        m.resolve(event, self.bindings_for_canvas(), seq, counts)
    }
    pub fn resolve_backup(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
        counts: bool,
    ) -> MatchOutcome<BackupAction> {
        m.resolve(event, self.bindings_for_backup(), seq, counts)
    }
    pub fn resolve_content_tree(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
        counts: bool,
    ) -> MatchOutcome<ContentTreeAction> {
        m.resolve(event, self.bindings_for_content_tree(), seq, counts)
    }

    pub fn resolve_setup(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
        _counts: bool,
    ) -> MatchOutcome<SetupAction> {
        m.resolve(event, self.bindings_for_setup(), seq, false)
    }

    /// Pick the best key combo to display in hint bars.
    /// Skips arrow keys, function keys, and page-nav keys to prefer
    /// letter keys (j/k) or conventional keys (Enter, Esc, Tab).
    fn pick_hint_key(combos: &[KeyCombo]) -> String {
        for combo in combos {
            let s = combo.to_display_string();
            let skip = matches!(
                s.as_str(),
                "Up" | "Down" | "Left" | "Right" | "Home" | "End" | "PageUp" | "PageDown"
            ) || (s.starts_with('F') && s[1..].parse::<u8>().is_ok());
            if !skip {
                return s;
            }
        }
        // All keys were nav/function keys, use first
        combos
            .first()
            .map(|k| k.to_display_string())
            .unwrap_or_else(|| "?".to_string())
    }
}

keybind_scope!(
    list,
    ListAction,
    matches_list,
    list_keys_display,
    bindings_for_list,
    display_list
);
keybind_scope!(
    edit,
    EditAction,
    matches_edit,
    edit_keys_display,
    bindings_for_edit,
    display_edit
);
keybind_scope!(
    help,
    HelpAction,
    matches_help,
    help_keys_display,
    bindings_for_help,
    display_help
);
keybind_scope!(
    graph,
    GraphAction,
    matches_graph,
    graph_keys_display,
    bindings_for_graph,
    display_graph
);
keybind_scope!(
    draw,
    DrawAction,
    matches_draw,
    draw_keys_display,
    bindings_for_draw,
    display_draw
);
keybind_scope!(
    canvas,
    CanvasAction,
    matches_canvas,
    canvas_keys_display,
    bindings_for_canvas,
    display_canvas
);
keybind_scope!(
    backup,
    BackupAction,
    matches_backup,
    backup_keys_display,
    bindings_for_backup,
    display_backup
);
keybind_scope!(
    content_tree,
    ContentTreeAction,
    matches_content_tree,
    content_tree_keys_display,
    bindings_for_content_tree,
    display_content_tree
);
keybind_scope!(
    setup,
    SetupAction,
    matches_setup,
    setup_keys_display,
    bindings_for_setup,
    display_setup
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::KeybindPreset;
    use crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn test_parse_key_combo_simple() {
        let combo = KeyCombo::parse("q").unwrap();
        assert_eq!(combo.keys.len(), 1);
        assert_eq!(combo.keys[0].code, KeyCode::Char('q'));
        assert_eq!(combo.keys[0].modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn test_parse_key_combo_ctrl() {
        let combo = KeyCombo::parse("Ctrl+q").unwrap();
        assert_eq!(combo.keys.len(), 1);
        assert_eq!(combo.keys[0].code, KeyCode::Char('q'));
        assert_eq!(combo.keys[0].modifiers, KeyModifiers::CONTROL);
    }

    #[test]
    fn test_parse_key_combo_ctrl_shift() {
        let combo = KeyCombo::parse("Ctrl+Shift+z").unwrap();
        assert_eq!(combo.keys.len(), 1);
        assert_eq!(combo.keys[0].code, KeyCode::Char('z'));
        assert_eq!(
            combo.keys[0].modifiers,
            KeyModifiers::CONTROL | KeyModifiers::SHIFT
        );
    }

    #[test]
    fn test_parse_special_keys() {
        assert_eq!(
            KeyCombo::parse("Enter").unwrap().keys[0].code,
            KeyCode::Enter
        );
        assert_eq!(KeyCombo::parse("Esc").unwrap().keys[0].code, KeyCode::Esc);
        assert_eq!(KeyCombo::parse("F1").unwrap().keys[0].code, KeyCode::F(1));
        assert_eq!(
            KeyCombo::parse("Delete").unwrap().keys[0].code,
            KeyCode::Delete
        );
    }

    #[test]
    fn test_parse_sequence() {
        let combo = KeyCombo::parse("g g").unwrap();
        assert_eq!(combo.keys.len(), 2);
        assert_eq!(combo.keys[0].code, KeyCode::Char('g'));
        assert_eq!(combo.keys[0].modifiers, KeyModifiers::NONE);
        assert_eq!(combo.keys[1].code, KeyCode::Char('g'));
        assert_eq!(combo.keys[1].modifiers, KeyModifiers::NONE);

        let ctrl_combo = KeyCombo::parse("Ctrl+x Ctrl+s").unwrap();
        assert_eq!(ctrl_combo.keys.len(), 2);
        assert_eq!(ctrl_combo.keys[0].code, KeyCode::Char('x'));
        assert!(ctrl_combo.keys[0].modifiers.contains(KeyModifiers::CONTROL));
        assert_eq!(ctrl_combo.keys[1].code, KeyCode::Char('s'));
        assert!(ctrl_combo.keys[1].modifiers.contains(KeyModifiers::CONTROL));

        // single-key backward compat
        let single = KeyCombo::parse("q").unwrap();
        assert_eq!(single.keys.len(), 1);
        assert_eq!(single.keys[0].code, KeyCode::Char('q'));
    }

    #[test]
    fn test_key_combo_matches() {
        let combo = KeyCombo::ctrl(KeyCode::Char('q'));
        let event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
        assert!(combo.matches(&event));
        let wrong_event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(!combo.matches(&wrong_event));

        // BackTab matching should ignore the SHIFT modifier
        let bt_combo1 = KeyCombo::simple(KeyCode::BackTab);
        let bt_combo2 = KeyCombo::shift(KeyCode::BackTab);
        let bt_event1 = KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE);
        let bt_event2 = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
        assert!(bt_combo1.matches(&bt_event1));
        assert!(bt_combo1.matches(&bt_event2));
        assert!(bt_combo2.matches(&bt_event1));
        assert!(bt_combo2.matches(&bt_event2));

        // Other modifiers like CONTROL should still distinguish them
        let bt_ctrl_event = KeyEvent::new(KeyCode::BackTab, KeyModifiers::CONTROL);
        assert!(!bt_combo1.matches(&bt_ctrl_event));
    }

    #[test]
    fn test_display_single_and_multi() {
        let single = KeyCombo::simple(KeyCode::Char('q'));
        assert_eq!(single.to_display_string(), "q");

        let ctrl_single = KeyCombo::ctrl(KeyCode::Char('s'));
        assert_eq!(ctrl_single.to_display_string(), "Ctrl+s");

        // Multi-key simple (all ascii letters) joins without separator
        let gg = KeyCombo::parse("g g").unwrap();
        assert_eq!(gg.to_display_string(), "gg");

        // Multi-key with modifier uses space
        let ctrl_seq = KeyCombo::parse("Ctrl+x Ctrl+s").unwrap();
        assert_eq!(ctrl_seq.to_display_string(), "Ctrl+x Ctrl+s");

        // Space key in sequence
        let space_seq = KeyCombo::parse("Space f").unwrap();
        assert_eq!(space_seq.to_display_string(), "Space f");
    }

    #[test]
    fn test_default_keybinds() {
        let keybinds = Keybinds::default();
        assert!(!keybinds.list.is_empty());
        assert!(!keybinds.edit.is_empty());
        assert!(!keybinds.help.is_empty());
        assert!(!keybinds.draw.is_empty());
        assert!(!keybinds.canvas.is_empty());
        assert!(!keybinds.backup.is_empty());
        assert!(!keybinds.content_tree.is_empty());

        let toml = keybinds.to_toml();
        assert!(!toml.draw.is_empty());
        assert!(!toml.canvas.is_empty());
        assert!(!toml.content_tree.is_empty());
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("keybinds.toml");
        keybinds.save(&path).unwrap();
        let loaded_keybinds = Keybinds::load(&path).unwrap();
        assert_eq!(loaded_keybinds.draw, keybinds.draw);
        assert_eq!(loaded_keybinds.canvas, keybinds.canvas);
        assert_eq!(loaded_keybinds.backup, keybinds.backup);
        assert_eq!(loaded_keybinds.content_tree, keybinds.content_tree);
    }

    #[test]
    fn test_preview_paging_keybinds() {
        let keybinds = Keybinds::default();

        // New variants are bound in the default maps.
        assert!(keybinds.list.contains_key(&ListAction::PreviewPageUp));
        assert!(keybinds.list.contains_key(&ListAction::PreviewPageDown));
        assert!(keybinds.edit.contains_key(&EditAction::PreviewPageUp));
        assert!(keybinds.edit.contains_key(&EditAction::PreviewPageDown));

        // Display strings match the documented defaults.
        assert_eq!(
            keybinds.list_keys_display(ListAction::PreviewPageUp),
            "Shift+Up"
        );
        assert_eq!(
            keybinds.list_keys_display(ListAction::PreviewPageDown),
            "Shift+Down"
        );
        assert_eq!(
            keybinds.edit_keys_display(EditAction::PreviewPageUp),
            "PageUp"
        );
        assert_eq!(
            keybinds.edit_keys_display(EditAction::PreviewPageDown),
            "PageDown"
        );

        // Round-trip: serialize to snake_case TOML keys and reload.
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("keybinds.toml");
        keybinds.save(&path).unwrap();

        let toml_text = std::fs::read_to_string(&path).unwrap();
        assert!(toml_text.contains("preview_page_up"));
        assert!(toml_text.contains("preview_page_down"));

        let loaded = Keybinds::load(&path).unwrap();
        assert_eq!(loaded.list, keybinds.list);
        assert_eq!(loaded.edit, keybinds.edit);
    }

    #[test]
    fn test_matches_list_action() {
        let keybinds = Keybinds::default();
        let event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(keybinds.matches_list(ListAction::Quit, &event));
    }

    #[test]
    fn test_new_action_displays() {
        let keybinds = Keybinds::default();
        assert_eq!(keybinds.draw_keys_display(DrawAction::SelectDrawTool), "d");
        assert_eq!(keybinds.canvas_keys_display(CanvasAction::Quit), "Esc/q");
        assert_eq!(keybinds.canvas_keys_display(CanvasAction::Save), "Ctrl+s");
    }

    #[test]
    fn test_matcher_sequences_disabled() {
        let mut matcher = KeyMatcher::new();
        let mut bindings: HashMap<ListAction, Vec<KeyCombo>> = HashMap::new();
        bindings.insert(ListAction::JumpToTop, vec![KeyCombo::parse("g g").unwrap()]);
        // With sequences disabled, "g" alone should not match JumpToTop
        let event = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        let result = matcher.resolve(event, &bindings, false, false);
        assert_eq!(result, MatchOutcome::NoMatch);
    }

    #[test]
    fn test_matcher_full_match() {
        let mut matcher = KeyMatcher::new();
        let mut bindings: HashMap<ListAction, Vec<KeyCombo>> = HashMap::new();
        bindings.insert(ListAction::JumpToTop, vec![KeyCombo::parse("g g").unwrap()]);

        // First 'g' should be Pending
        let e1 = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        let r1 = matcher.resolve(e1, &bindings, true, false);
        assert_eq!(r1, MatchOutcome::Pending);

        // Second 'g' within timeout should match
        let e2 = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        let r2 = matcher.resolve(e2, &bindings, true, false);
        assert_eq!(r2, MatchOutcome::Matched(ListAction::JumpToTop, None));
    }

    #[test]
    fn test_matcher_timeout() {
        let mut matcher = KeyMatcher {
            pending: Vec::new(),
            last_event_at: None,
            timeout: std::time::Duration::from_millis(1), // very short
            count: None,
        };
        let mut bindings: HashMap<ListAction, Vec<KeyCombo>> = HashMap::new();
        bindings.insert(ListAction::JumpToTop, vec![KeyCombo::parse("g g").unwrap()]);

        // First 'g' -> Pending
        let e1 = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        let r1 = matcher.resolve(e1, &bindings, true, false);
        assert_eq!(r1, MatchOutcome::Pending);

        // Wait longer than timeout
        std::thread::sleep(std::time::Duration::from_millis(5));

        // Second 'g' after timeout should NOT match (pending cleared)
        let e2 = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        let r2 = matcher.resolve(e2, &bindings, true, false);
        // Should re-start sequence
        assert_eq!(r2, MatchOutcome::Pending);
    }

    #[test]
    fn test_matcher_single_key_still_works() {
        let mut matcher = KeyMatcher::new();
        let mut bindings: HashMap<ListAction, Vec<KeyCombo>> = HashMap::new();
        bindings.insert(ListAction::Quit, vec![KeyCombo::simple(KeyCode::Char('q'))]);
        bindings.insert(ListAction::JumpToTop, vec![KeyCombo::parse("g g").unwrap()]);

        // 'q' alone should still match Quit (length-1)
        let event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        let result = matcher.resolve(event, &bindings, true, false);
        assert_eq!(result, MatchOutcome::Matched(ListAction::Quit, None));
    }

    #[test]
    fn test_matcher_sequence_break() {
        let mut matcher = KeyMatcher::new();
        let mut bindings: HashMap<ListAction, Vec<KeyCombo>> = HashMap::new();
        bindings.insert(ListAction::Quit, vec![KeyCombo::simple(KeyCode::Char('q'))]);
        bindings.insert(ListAction::JumpToTop, vec![KeyCombo::parse("g g").unwrap()]);

        // First event 'g' -> Pending (start of "g g" sequence)
        let e1 = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        let r1 = matcher.resolve(e1, &bindings, true, false);
        assert_eq!(r1, MatchOutcome::Pending);
        assert_eq!(matcher.pending.len(), 1);

        // Second event 'q' -> should break the sequence and immediately match Quit
        let e2 = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        let r2 = matcher.resolve(e2, &bindings, true, false);
        assert_eq!(r2, MatchOutcome::Matched(ListAction::Quit, None));
        assert_eq!(matcher.pending.len(), 0);
    }

    #[test]
    fn test_preset_edit_unchanged() {
        let default_edit = &Keybinds::default().edit;
        for preset in &[
            KeybindPreset::Default,
            KeybindPreset::Helix,
            KeybindPreset::Vim,
            KeybindPreset::Emacs,
        ] {
            assert_eq!(
                &preset.base_keybinds().edit,
                default_edit,
                "preset {preset} must not change edit bindings"
            );
        }
    }

    #[test]
    fn test_helix_preset_bindings() {
        let kb = KeybindPreset::Helix.base_keybinds();
        // gg → JumpToTop
        assert!(
            kb.list
                .get(&ListAction::JumpToTop)
                .unwrap()
                .iter()
                .any(|c| c.to_display_string() == "gg")
        );
        // Space Space → OpenCommandPalette
        assert!(
            kb.list
                .get(&ListAction::OpenCommandPalette)
                .unwrap()
                .iter()
                .any(|c| c.to_display_string() == "Space Space")
        );
        // Space t → NewFromTemplate
        assert!(
            kb.list
                .get(&ListAction::NewFromTemplate)
                .unwrap()
                .iter()
                .any(|c| c.to_display_string() == "Space t")
        );
        // k → MoveUp (with arrow fallback)
        assert!(
            kb.list
                .get(&ListAction::MoveUp)
                .unwrap()
                .iter()
                .any(|c| c.to_display_string() == "k")
        );
        assert!(
            kb.list
                .get(&ListAction::MoveUp)
                .unwrap()
                .iter()
                .any(|c| c.to_display_string() == "Up")
        );
    }

    #[test]
    fn test_vim_preset_bindings() {
        let kb = KeybindPreset::Vim.base_keybinds();
        // : q → Quit
        assert!(
            kb.list
                .get(&ListAction::Quit)
                .unwrap()
                .iter()
                .any(|c| c.to_display_string() == ": q")
        );
        // d d → Delete
        assert!(
            kb.list
                .get(&ListAction::Delete)
                .unwrap()
                .iter()
                .any(|c| c.to_display_string() == "dd")
        );
        // gg → JumpToTop
        assert!(
            kb.list
                .get(&ListAction::JumpToTop)
                .unwrap()
                .iter()
                .any(|c| c.to_display_string() == "gg")
        );
        // gG (from "g G") → JumpToBottom
        assert!(
            kb.list
                .get(&ListAction::JumpToBottom)
                .unwrap()
                .iter()
                .any(|c| c.to_display_string() == "gG")
        );
    }

    #[test]
    fn test_emacs_preset_bindings() {
        let kb = KeybindPreset::Emacs.base_keybinds();
        // Ctrl+p → MoveUp
        assert!(
            kb.list
                .get(&ListAction::MoveUp)
                .unwrap()
                .iter()
                .any(|c| c.to_display_string() == "Ctrl+p")
        );
        // Ctrl+h → Help
        assert!(
            kb.list
                .get(&ListAction::Help)
                .unwrap()
                .iter()
                .any(|c| c.to_display_string() == "Ctrl+h")
        );
        // Ctrl+x Ctrl+c → Quit
        assert!(
            kb.list
                .get(&ListAction::Quit)
                .unwrap()
                .iter()
                .any(|c| c.to_display_string() == "Ctrl+x Ctrl+c")
        );
        // Ctrl+d → Delete
        assert!(
            kb.list
                .get(&ListAction::Delete)
                .unwrap()
                .iter()
                .any(|c| c.to_display_string() == "Ctrl+d")
        );
    }

    #[test]
    fn test_enter_key_resolves_to_open_not_confirm() {
        let kb = KeybindPreset::Default.base_keybinds();
        let mut matcher = KeyMatcher::new();
        let enter_event = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        );
        let outcome = kb.resolve_list(&mut matcher, enter_event, false, false);
        assert_eq!(outcome, MatchOutcome::Matched(ListAction::Open, None));

        // Also verify y and n do not resolve to Confirm/Cancel in list view
        let mut matcher_y = KeyMatcher::new();
        let y_event = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('y'),
            crossterm::event::KeyModifiers::NONE,
        );
        let outcome_y = kb.resolve_list(&mut matcher_y, y_event, false, false);
        assert_eq!(
            outcome_y,
            MatchOutcome::Matched(ListAction::Duplicate, None)
        );
    }

    #[test]
    fn test_canvas_quit_resolves_for_esc_and_q() {
        let kb = Keybinds::default();
        let mut m = crate::keybinds::KeyMatcher::new();

        // Esc is bound to Quit, plus cancel actions (RenameCancel, MenuClose, etc.).
        // HashMap iteration picks non-deterministically — the handler's catch-all
        // checks matches_canvas(Quit) so either route works.
        let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let res = kb.resolve_canvas(&mut m, esc, false, false);
        assert!(
            matches!(res, MatchOutcome::Matched(_, _)),
            "Esc should resolve to some canvas action, got {:?}",
            res
        );

        // q with sequences disabled
        let q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        let res = kb.resolve_canvas(&mut m, q, false, false);
        assert!(
            matches!(res, MatchOutcome::Matched(CanvasAction::Quit, _)),
            "q should resolve to CanvasAction::Quit (seq=false), got {:?}",
            res
        );

        // q with sequences enabled (default)
        let mut m = crate::keybinds::KeyMatcher::new();
        let res = kb.resolve_canvas(&mut m, q, true, false);
        assert!(
            matches!(res, MatchOutcome::Matched(CanvasAction::Quit, _)),
            "q should resolve to CanvasAction::Quit (seq=true), got {:?}",
            res
        );
    }

    #[test]
    fn test_uses_sequences() {
        assert!(!KeybindPreset::Default.uses_sequences());
        assert!(KeybindPreset::Helix.uses_sequences());
        assert!(KeybindPreset::Vim.uses_sequences());
        assert!(KeybindPreset::Emacs.uses_sequences());
    }

    #[test]
    fn test_helix_preset_has_ge() {
        let kb = KeybindPreset::Helix.base_keybinds();
        assert!(
            kb.list
                .get(&ListAction::JumpToBottom)
                .unwrap()
                .iter()
                .any(|c| c.to_display_string() == "ge")
        );
    }

    #[test]
    fn test_pending_display() {
        use std::collections::HashMap;
        let mut m = crate::keybinds::KeyMatcher::new();
        let mut b: HashMap<ListAction, Vec<KeyCombo>> = HashMap::new();
        b.insert(ListAction::JumpToTop, vec![KeyCombo::parse("g g").unwrap()]);
        let e = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        assert_eq!(m.resolve(e, &b, true, false), MatchOutcome::Pending);
        assert_eq!(m.pending_display().as_deref(), Some("g"));
        assert_eq!(
            m.resolve(
                KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
                &b,
                true,
                false
            ),
            MatchOutcome::Matched(ListAction::JumpToTop, None)
        );
        assert_eq!(m.pending_display(), None);
    }

    #[test]
    fn test_count_prefix_simple() {
        let mut matcher = KeyMatcher::new();
        let mut bindings: HashMap<ListAction, Vec<KeyCombo>> = HashMap::new();
        bindings.insert(
            ListAction::MoveDown,
            vec![KeyCombo::simple(KeyCode::Char('j'))],
        );

        // Type '3' — should be consumed as count prefix
        let e1 = KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE);
        let r1 = matcher.resolve(e1, &bindings, true, true);
        assert_eq!(r1, MatchOutcome::Pending);
        assert_eq!(matcher.count, Some(3));

        // Type 'j' — should match MoveDown with count 3
        let e2 = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        let r2 = matcher.resolve(e2, &bindings, true, true);
        assert_eq!(r2, MatchOutcome::Matched(ListAction::MoveDown, Some(3)));
        assert_eq!(matcher.count, None); // consumed
    }

    #[test]
    fn test_count_prefix_zero_alone() {
        let mut matcher = KeyMatcher::new();
        let mut bindings: HashMap<ListAction, Vec<KeyCombo>> = HashMap::new();
        bindings.insert(ListAction::Help, vec![KeyCombo::simple(KeyCode::Char('0'))]);

        // '0' alone should NOT be consumed as count (bare '0' is not a count digit)
        let e = KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE);
        let r = matcher.resolve(e, &bindings, true, true);
        assert_eq!(r, MatchOutcome::Matched(ListAction::Help, None));
        assert_eq!(matcher.count, None);
    }

    #[test]
    fn test_count_prefix_multi_digit() {
        let mut matcher = KeyMatcher::new();
        let mut bindings: HashMap<ListAction, Vec<KeyCombo>> = HashMap::new();
        bindings.insert(
            ListAction::MoveDown,
            vec![KeyCombo::simple(KeyCode::Char('j'))],
        );

        // Type '1' then '0' — accumulates to 10
        let e1 = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE);
        let r1 = matcher.resolve(e1, &bindings, true, true);
        assert_eq!(r1, MatchOutcome::Pending);
        assert_eq!(matcher.count, Some(1));

        let e2 = KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE);
        let r2 = matcher.resolve(e2, &bindings, true, true);
        assert_eq!(r2, MatchOutcome::Pending);
        assert_eq!(matcher.count, Some(10));

        // Type 'j' — should match with count 10
        let e3 = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        let r3 = matcher.resolve(e3, &bindings, true, true);
        assert_eq!(r3, MatchOutcome::Matched(ListAction::MoveDown, Some(10)));
        assert_eq!(matcher.count, None);
    }

    #[test]
    fn test_count_prefix_disabled() {
        let mut matcher = KeyMatcher::new();
        let mut bindings: HashMap<ListAction, Vec<KeyCombo>> = HashMap::new();
        bindings.insert(ListAction::Quit, vec![KeyCombo::simple(KeyCode::Char('q'))]);

        // With counts_enabled=false, 'q' should match normally (no count)
        let e = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        let r = matcher.resolve(e, &bindings, true, false);
        assert_eq!(r, MatchOutcome::Matched(ListAction::Quit, None));
        assert_eq!(matcher.count, None);

        // With counts_enabled=false, digits should pass through (not consumed)
        let mut bindings2: HashMap<ListAction, Vec<KeyCombo>> = HashMap::new();
        bindings2.insert(
            ListAction::MoveDown,
            vec![KeyCombo::simple(KeyCode::Char('j'))],
        );
        let e2 = KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE);
        let r2 = matcher.resolve(e2, &bindings2, true, false);
        assert_eq!(r2, MatchOutcome::NoMatch); // '3' is not bound
    }
    #[test]
    fn test_per_preset_path_load() {
        let dir = std::env::temp_dir().join("clin_test_preset");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("keybinds_vim.toml");
        let _ = std::fs::write(&path, "[list]\nquit = [\": q\"]\n");
        let kb = crate::keybinds::Keybinds::load_layered(
            &path,
            crate::config::KeybindPreset::Vim.base_keybinds(),
        )
        .unwrap_or_default();
        assert!(
            kb.list
                .get(&ListAction::Quit)
                .unwrap()
                .iter()
                .any(|c| c.to_display_string() == ": q")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn test_display_picks_hint_key() {
    let kb = Keybinds::default();
    // Navigation: prefer letter over arrow
    assert_eq!(kb.display_list(ListAction::MoveDown), "j", "MoveDown");
    assert_eq!(kb.display_list(ListAction::MoveUp), "k", "MoveUp");
    // Conventional keys for primary actions
    assert_eq!(kb.display_list(ListAction::Open), "Enter", "Open");
    assert_eq!(kb.display_list(ListAction::Help), "?", "Help");
    assert_eq!(kb.display_list(ListAction::Quit), "q", "Quit");
    // Edit: Tab for CycleFocus, Esc for Back
    assert_eq!(kb.display_edit(EditAction::CycleFocus), "Tab", "CycleFocus");
    assert_eq!(kb.display_edit(EditAction::Back), "Esc", "Back");
    // Canvas: letter for nav, conventional for quit
    assert_eq!(
        kb.display_canvas(CanvasAction::MoveUp),
        "k",
        "Canvas MoveUp"
    );
    assert_eq!(kb.display_canvas(CanvasAction::Quit), "Esc", "Canvas Quit");
    // Backup: Esc for Back (not q)
    assert_eq!(kb.display_backup(BackupAction::Back), "Esc", "Backup Back");
    assert_eq!(
        kb.display_backup(BackupAction::EnterCommit),
        "c",
        "EnterCommit"
    );
}

#[test]
fn test_matches_help_coverage_gap_closed() {
    // matches_help did not exist before macro consolidation; prove it works.
    use crossterm::event::KeyEvent;
    let kb = Keybinds::default();
    // Find any single-key Help binding so matches() (which requires len==1) applies.
    let single = kb
        .help
        .values()
        .flatten()
        .find(|c| c.keys.len() == 1)
        .cloned();
    let Some(combo) = single else {
        return;
    };
    let stroke = &combo.keys[0];
    let event = KeyEvent::new(stroke.code, stroke.modifiers);
    // The generated matches_help must return true for the exact event of a bound combo.
    let matched = kb
        .help
        .iter()
        .any(|(a, cs)| cs.contains(&combo) && kb.matches_help(*a, &event));
    assert!(
        matched,
        "matches_help should fire for a bound single-key Help action"
    );
}
