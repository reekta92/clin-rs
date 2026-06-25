use std::collections::HashMap;
use std::fs;
use std::path::Path;
use anyhow::{Context, Result};
use crossterm::event::KeyEvent;
use super::{
    Keybinds, KeybindsToml, KeyCombo, ListAction, EditAction, HelpAction, GraphAction,
    DrawAction, CanvasAction, BackupAction, ContentTreeAction, KeyMatcher, MatchOutcome
};

impl Keybinds {
    pub fn load(path: &Path) -> Result<Self> {
        Self::load_layered(path, Self::default())
    }

    pub fn load_layered(path: &Path, base: Keybinds) -> Result<Self> {
        let mut keybinds = base;

        if !path.exists() {
            return Ok(keybinds);
        }

        let content = fs::read_to_string(path).context("failed to read keybinds file")?;

        let toml: KeybindsToml =
            toml::from_str(&content).context("failed to parse keybinds file")?;

        for (action, combos_str) in &toml.list {
            let combos: Vec<KeyCombo> = combos_str
                .iter()
                .filter_map(|s| KeyCombo::parse(s))
                .collect();
            if !combos.is_empty() {
                keybinds.list.insert(*action, combos);
            }
        }

        for (action, combos_str) in &toml.edit {
            let combos: Vec<KeyCombo> = combos_str
                .iter()
                .filter_map(|s| KeyCombo::parse(s))
                .collect();
            if !combos.is_empty() {
                keybinds.edit.insert(*action, combos);
            }
        }

        for (action, combos_str) in &toml.help {
            let combos: Vec<KeyCombo> = combos_str
                .iter()
                .filter_map(|s| KeyCombo::parse(s))
                .collect();
            if !combos.is_empty() {
                keybinds.help.insert(*action, combos);
            }
        }

        for (action, combos_str) in &toml.graph {
            let combos: Vec<KeyCombo> = combos_str
                .iter()
                .filter_map(|s| KeyCombo::parse(s))
                .collect();
            if !combos.is_empty() {
                keybinds.graph.insert(*action, combos);
            }
        }

        for (action, combos_str) in &toml.draw {
            let combos: Vec<KeyCombo> = combos_str
                .iter()
                .filter_map(|s| KeyCombo::parse(s))
                .collect();
            if !combos.is_empty() {
                keybinds.draw.insert(*action, combos);
            }
        }

        for (action, combos_str) in &toml.canvas {
            let combos: Vec<KeyCombo> = combos_str
                .iter()
                .filter_map(|s| KeyCombo::parse(s))
                .collect();
            if !combos.is_empty() {
                keybinds.canvas.insert(*action, combos);
            }
        }

        for (action, combos_str) in &toml.backup {
            let combos: Vec<KeyCombo> = combos_str
                .iter()
                .filter_map(|s| KeyCombo::parse(s))
                .collect();
            if !combos.is_empty() {
                keybinds.backup.insert(*action, combos);
            }
        }

        for (action, combos_str) in &toml.content_tree {
            let combos: Vec<KeyCombo> = combos_str
                .iter()
                .filter_map(|s| KeyCombo::parse(s))
                .collect();
            if !combos.is_empty() {
                keybinds.content_tree.insert(*action, combos);
            }
        }
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
        let mut toml = KeybindsToml::default();

        for (action, combos) in &self.list {
            let values: Vec<String> = combos.iter().map(KeyCombo::to_display_string).collect();
            toml.list.insert(*action, values);
        }

        for (action, combos) in &self.edit {
            let values: Vec<String> = combos.iter().map(KeyCombo::to_display_string).collect();
            toml.edit.insert(*action, values);
        }

        for (action, combos) in &self.help {
            let values: Vec<String> = combos.iter().map(KeyCombo::to_display_string).collect();
            toml.help.insert(*action, values);
        }

        for (action, combos) in &self.graph {
            let values: Vec<String> = combos.iter().map(KeyCombo::to_display_string).collect();
            toml.graph.insert(*action, values);
        }

        for (action, combos) in &self.draw {
            let values: Vec<String> = combos.iter().map(KeyCombo::to_display_string).collect();
            toml.draw.insert(*action, values);
        }

        for (action, combos) in &self.canvas {
            let values: Vec<String> = combos.iter().map(KeyCombo::to_display_string).collect();
            toml.canvas.insert(*action, values);
        }

        for (action, combos) in &self.backup {
            let values: Vec<String> = combos.iter().map(KeyCombo::to_display_string).collect();
            toml.backup.insert(*action, values);
        }

        for (action, combos) in &self.content_tree {
            let values: Vec<String> = combos.iter().map(KeyCombo::to_display_string).collect();
            toml.content_tree.insert(*action, values);
        }
        toml
    }

    pub fn matches_list(&self, action: ListAction, event: &KeyEvent) -> bool {
        self.list
            .get(&action)
            .is_some_and(|combos| combos.iter().any(|c| c.matches(event)))
    }

    pub fn matches_edit(&self, action: EditAction, event: &KeyEvent) -> bool {
        self.edit
            .get(&action)
            .is_some_and(|combos| combos.iter().any(|c| c.matches(event)))
    }

    pub fn matches_draw(&self, action: DrawAction, event: &KeyEvent) -> bool {
        self.draw
            .get(&action)
            .is_some_and(|combos| combos.iter().any(|c| c.matches(event)))
    }

    pub fn matches_canvas(&self, action: CanvasAction, event: &KeyEvent) -> bool {
        self.canvas
            .get(&action)
            .is_some_and(|combos| combos.iter().any(|c| c.matches(event)))
    }

    pub fn matches_backup(&self, action: BackupAction, event: &KeyEvent) -> bool {
        self.backup
            .get(&action)
            .is_some_and(|combos| combos.iter().any(|c| c.matches(event)))
    }

    pub fn list_keys_display(&self, action: ListAction) -> String {
        self.list
            .get(&action)
            .map(|combos| {
                combos
                    .iter()
                    .map(KeyCombo::to_display_string)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default()
    }

    pub fn edit_keys_display(&self, action: EditAction) -> String {
        self.edit
            .get(&action)
            .map(|combos| {
                combos
                    .iter()
                    .map(KeyCombo::to_display_string)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default()
    }

    pub fn help_keys_display(&self, action: HelpAction) -> String {
        self.help
            .get(&action)
            .map(|combos| {
                combos
                    .iter()
                    .map(KeyCombo::to_display_string)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default()
    }

    pub fn graph_keys_display(&self, action: GraphAction) -> String {
        self.graph
            .get(&action)
            .map(|combos| {
                combos
                    .iter()
                    .map(KeyCombo::to_display_string)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default()
    }

    pub fn draw_keys_display(&self, action: DrawAction) -> String {
        self.draw
            .get(&action)
            .map(|combos| {
                combos
                    .iter()
                    .map(KeyCombo::to_display_string)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default()
    }

    pub fn canvas_keys_display(&self, action: CanvasAction) -> String {
        self.canvas
            .get(&action)
            .map(|combos| {
                combos
                    .iter()
                    .map(KeyCombo::to_display_string)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default()
    }

    pub fn backup_keys_display(&self, action: BackupAction) -> String {
        self.backup
            .get(&action)
            .map(|combos| {
                combos
                    .iter()
                    .map(KeyCombo::to_display_string)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default()
    }

    pub fn content_tree_keys_display(&self, action: ContentTreeAction) -> String {
        self.content_tree
            .get(&action)
            .map(|combos| {
                combos
                    .iter()
                    .map(KeyCombo::to_display_string)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default()
    }

    pub fn bindings_for_edit(&self) -> &HashMap<EditAction, Vec<KeyCombo>> {
        &self.edit
    }
    pub fn bindings_for_help(&self) -> &HashMap<HelpAction, Vec<KeyCombo>> {
        &self.help
    }
    pub fn bindings_for_graph(&self) -> &HashMap<GraphAction, Vec<KeyCombo>> {
        &self.graph
    }
    pub fn bindings_for_draw(&self) -> &HashMap<DrawAction, Vec<KeyCombo>> {
        &self.draw
    }
    pub fn bindings_for_canvas(&self) -> &HashMap<CanvasAction, Vec<KeyCombo>> {
        &self.canvas
    }
    pub fn bindings_for_backup(&self) -> &HashMap<BackupAction, Vec<KeyCombo>> {
        &self.backup
    }
    pub fn bindings_for_content_tree(&self) -> &HashMap<ContentTreeAction, Vec<KeyCombo>> {
        &self.content_tree
    }

    // -- Resolve wrappers (delegate to KeyMatcher::resolve) --
    pub fn resolve_list(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
    ) -> MatchOutcome<ListAction> {
        let mut filtered = self.list.clone();
        filtered.remove(&ListAction::Confirm);
        filtered.remove(&ListAction::Cancel);
        m.resolve(event, &filtered, seq)
    }
    pub fn resolve_edit(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
    ) -> MatchOutcome<EditAction> {
        m.resolve(event, self.bindings_for_edit(), seq)
    }
    pub fn resolve_help(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
    ) -> MatchOutcome<HelpAction> {
        m.resolve(event, self.bindings_for_help(), seq)
    }
    pub fn resolve_graph(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
    ) -> MatchOutcome<GraphAction> {
        m.resolve(event, self.bindings_for_graph(), seq)
    }
    pub fn resolve_draw(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
    ) -> MatchOutcome<DrawAction> {
        m.resolve(event, self.bindings_for_draw(), seq)
    }
    pub fn resolve_canvas(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
    ) -> MatchOutcome<CanvasAction> {
        m.resolve(event, self.bindings_for_canvas(), seq)
    }
    pub fn resolve_backup(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
    ) -> MatchOutcome<BackupAction> {
        m.resolve(event, self.bindings_for_backup(), seq)
    }
    pub fn resolve_content_tree(
        &self,
        m: &mut KeyMatcher,
        event: KeyEvent,
        seq: bool,
    ) -> MatchOutcome<ContentTreeAction> {
        m.resolve(event, self.bindings_for_content_tree(), seq)
    }

    /// Pick the best key combo to display in hint bars.
    /// Skips arrow keys, function keys, and page-nav keys to prefer
    /// letter keys (j/k) or conventional keys (Enter, Esc, Tab).
    fn pick_hint_key(combos: &[KeyCombo]) -> String {
        for combo in combos.iter() {
            let s = combo.to_display_string();
            let skip = matches!(s.as_str(), "Up" | "Down" | "Left" | "Right" | "Home" | "End" | "PageUp" | "PageDown")
                || (s.starts_with('F') && s[1..].parse::<u8>().is_ok());
            if !skip {
                return s;
            }
        }
        // All keys were nav/function keys, use first
        combos.first().map(|k| k.to_display_string()).unwrap_or_else(|| "?".to_string())
    }

    pub fn display_list(&self, action: ListAction) -> String {
        self.list.get(&action).map(|v| Self::pick_hint_key(v)).unwrap_or_else(|| "?".to_string())
    }
    pub fn display_edit(&self, action: EditAction) -> String {
        self.edit.get(&action).map(|v| Self::pick_hint_key(v)).unwrap_or_else(|| "?".to_string())
    }
    pub fn display_help(&self, action: HelpAction) -> String {
        self.help.get(&action).map(|v| Self::pick_hint_key(v)).unwrap_or_else(|| "?".to_string())
    }
    pub fn display_graph(&self, action: GraphAction) -> String {
        self.graph.get(&action).map(|v| Self::pick_hint_key(v)).unwrap_or_else(|| "?".to_string())
    }
    pub fn display_draw(&self, action: DrawAction) -> String {
        self.draw.get(&action).map(|v| Self::pick_hint_key(v)).unwrap_or_else(|| "?".to_string())
    }
    pub fn display_canvas(&self, action: CanvasAction) -> String {
        self.canvas.get(&action).map(|v| Self::pick_hint_key(v)).unwrap_or_else(|| "?".to_string())
    }
    pub fn display_backup(&self, action: BackupAction) -> String {
        self.backup.get(&action).map(|v| Self::pick_hint_key(v)).unwrap_or_else(|| "?".to_string())
    }
    pub fn display_content_tree(&self, action: ContentTreeAction) -> String {
        self.content_tree.get(&action).map(|v| Self::pick_hint_key(v)).unwrap_or_else(|| "?".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};
    use crate::config::KeybindPreset;

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
        let result = matcher.resolve(event, &bindings, false);
        assert_eq!(result, MatchOutcome::NoMatch);
    }

    #[test]
    fn test_matcher_full_match() {
        let mut matcher = KeyMatcher::new();
        let mut bindings: HashMap<ListAction, Vec<KeyCombo>> = HashMap::new();
        bindings.insert(ListAction::JumpToTop, vec![KeyCombo::parse("g g").unwrap()]);

        // First 'g' should be Pending
        let e1 = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        let r1 = matcher.resolve(e1, &bindings, true);
        assert_eq!(r1, MatchOutcome::Pending);

        // Second 'g' within timeout should match
        let e2 = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        let r2 = matcher.resolve(e2, &bindings, true);
        assert_eq!(r2, MatchOutcome::Matched(ListAction::JumpToTop));
    }

    #[test]
    fn test_matcher_timeout() {
        let mut matcher = KeyMatcher {
            pending: Vec::new(),
            last_event_at: None,
            timeout: std::time::Duration::from_millis(1), // very short
        };
        let mut bindings: HashMap<ListAction, Vec<KeyCombo>> = HashMap::new();
        bindings.insert(ListAction::JumpToTop, vec![KeyCombo::parse("g g").unwrap()]);

        // First 'g' -> Pending
        let e1 = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        let r1 = matcher.resolve(e1, &bindings, true);
        assert_eq!(r1, MatchOutcome::Pending);

        // Wait longer than timeout
        std::thread::sleep(std::time::Duration::from_millis(5));

        // Second 'g' after timeout should NOT match (pending cleared)
        let e2 = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        let r2 = matcher.resolve(e2, &bindings, true);
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
        let result = matcher.resolve(event, &bindings, true);
        assert_eq!(result, MatchOutcome::Matched(ListAction::Quit));
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
        let outcome = kb.resolve_list(&mut matcher, enter_event, false);
        assert_eq!(outcome, MatchOutcome::Matched(ListAction::Open));

        // Also verify y and n do not resolve to Confirm/Cancel in list view
        let mut matcher_y = KeyMatcher::new();
        let y_event = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('y'),
            crossterm::event::KeyModifiers::NONE,
        );
        let outcome_y = kb.resolve_list(&mut matcher_y, y_event, false);
        assert_eq!(outcome_y, MatchOutcome::Matched(ListAction::Duplicate));
    }

    #[test]
    fn test_canvas_quit_resolves_for_esc_and_q() {
        let kb = Keybinds::default();
        let mut m = crate::keybinds::KeyMatcher::new();

        // Esc is bound to Quit, plus cancel actions (RenameCancel, MenuClose, etc.).
        // HashMap iteration picks non-deterministically — the handler's catch-all
        // checks matches_canvas(Quit) so either route works.
        let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let res = kb.resolve_canvas(&mut m, esc, false);
        assert!(
            matches!(res, MatchOutcome::Matched(_)),
            "Esc should resolve to some canvas action, got {:?}",
            res
        );

        // q with sequences disabled
        let q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        let res = kb.resolve_canvas(&mut m, q, false);
        assert!(
            matches!(res, MatchOutcome::Matched(CanvasAction::Quit)),
            "q should resolve to CanvasAction::Quit (seq=false), got {:?}",
            res
        );

        // q with sequences enabled (default)
        let mut m = crate::keybinds::KeyMatcher::new();
        let res = kb.resolve_canvas(&mut m, q, true);
        assert!(
            matches!(res, MatchOutcome::Matched(CanvasAction::Quit)),
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
        assert_eq!(
            m.resolve(e, &b, true),
            MatchOutcome::Pending
        );
        assert_eq!(m.pending_display().as_deref(), Some("g"));
        assert_eq!(
            m.resolve(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE), &b, true),
            MatchOutcome::Matched(ListAction::JumpToTop)
        );
        assert_eq!(m.pending_display(), None);
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
    assert_eq!(kb.display_canvas(CanvasAction::MoveUp), "k", "Canvas MoveUp");
    assert_eq!(kb.display_canvas(CanvasAction::Quit), "Esc", "Canvas Quit");
    // Backup: Esc for Back (not q)
    assert_eq!(kb.display_backup(BackupAction::Back), "Esc", "Backup Back");
    assert_eq!(kb.display_backup(BackupAction::EnterCommit), "c", "EnterCommit");
}
