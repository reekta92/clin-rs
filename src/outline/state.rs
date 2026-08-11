use crate::outline::parse::TreeNode;
use std::collections::HashSet;

pub struct OutlineState {
    pub note_id: String,
    pub note_title: String,
    pub nodes: Vec<TreeNode>,
    pub selected: usize,          // index into `nodes`
    pub expanded: HashSet<usize>, // header node-indices that are expanded
    pub load_error: bool,
    pub keybinds: crate::keybinds::Keybinds,
    pub seq_matcher: crate::keybinds::KeyMatcher,
    pub last_area: ratatui::layout::Rect,
    pub mouse_pos: Option<(u16, u16)>,
    pub last_tree_scroll: Option<crate::ui::scrollbar::ScrollbarMeta>,
    pub scroll_drag: Option<crate::ui::scrollbar::ScrollDrag>,
    pub tree_scroll_offset: usize,
    pub tree_list_rect: ratatui::layout::Rect,
}

impl OutlineState {
    /// `load_error=true` variant for unloadable notes.
    pub fn error(
        note_id: String,
        keybinds: crate::keybinds::Keybinds,
        seq_matcher: crate::keybinds::KeyMatcher,
    ) -> Self {
        Self {
            note_id,
            note_title: String::new(),
            nodes: vec![],
            selected: 0,
            expanded: HashSet::new(),
            load_error: true,
            keybinds,
            seq_matcher,
            last_area: ratatui::layout::Rect::default(),
            mouse_pos: None,
            last_tree_scroll: None,
            scroll_drag: None,
            tree_scroll_offset: 0,
            tree_list_rect: ratatui::layout::Rect::default(),
        }
    }

    pub fn new(
        note_id: String,
        title: &str,
        content: &str,
        keybinds: crate::keybinds::Keybinds,
        seq_matcher: crate::keybinds::KeyMatcher,
    ) -> Self {
        let nodes = crate::outline::parse::parse_outline(title, content);
        let mut expanded = HashSet::new();
        for (i, n) in nodes.iter().enumerate() {
            if matches!(n.kind, crate::outline::parse::NodeKind::Header { .. }) {
                expanded.insert(i); // default: all headers expanded
            }
        }
        Self {
            note_id,
            note_title: title.to_string(),
            nodes,
            selected: 0,
            expanded,
            load_error: false,
            keybinds,
            seq_matcher,
            last_area: ratatui::layout::Rect::default(),
            mouse_pos: None,
            last_tree_scroll: None,
            scroll_drag: None,
            tree_scroll_offset: 0,
            tree_list_rect: ratatui::layout::Rect::default(),
        }
    }

    /// Visible node indices given current `expanded`.
    pub fn visible_indices(&self) -> Vec<usize> {
        let mut visible = Vec::new();
        let mut skip_until_depth = None;

        for (i, n) in self.nodes.iter().enumerate() {
            if let Some(limit_depth) = skip_until_depth {
                if n.depth > limit_depth {
                    continue; // Skip node since it is in a collapsed subtree
                } else {
                    skip_until_depth = None; // Out of collapsed subtree
                }
            }

            visible.push(i);

            if self.is_header(i) && !self.expanded.contains(&i) {
                // If it is a header and not expanded, we collapse its subtree.
                // We skip all subsequent nodes with depth > its own.
                skip_until_depth = Some(n.depth);
            }
        }
        visible
    }

    pub fn is_header(&self, i: usize) -> bool {
        self.nodes
            .get(i)
            .is_some_and(|n| matches!(n.kind, crate::outline::parse::NodeKind::Header { .. }))
    }

    pub fn move_up(&mut self) {
        let visible = self.visible_indices();
        if visible.is_empty() {
            return;
        }
        let pos = visible
            .iter()
            .position(|&x| x == self.selected)
            .unwrap_or_else(|| {
                visible
                    .iter()
                    .position(|&x| x > self.selected)
                    .unwrap_or(visible.len())
                    .saturating_sub(1)
            });
        if pos > 0 {
            self.selected = visible[pos - 1];
        } else {
            self.selected = visible[0];
        }
    }

    pub fn move_down(&mut self) {
        let visible = self.visible_indices();
        if visible.is_empty() {
            return;
        }
        let pos = visible
            .iter()
            .position(|&x| x == self.selected)
            .unwrap_or_else(|| {
                visible
                    .iter()
                    .position(|&x| x > self.selected)
                    .unwrap_or(visible.len())
                    .saturating_sub(1)
            });
        if pos + 1 < visible.len() {
            self.selected = visible[pos + 1];
        } else {
            self.selected = visible[visible.len() - 1];
        }
    }

    pub fn toggle_collapse(&mut self) {
        if self.is_header(self.selected) {
            if self.expanded.contains(&self.selected) {
                self.expanded.remove(&self.selected);
            } else {
                self.expanded.insert(self.selected);
            }
        }
    }

    pub fn expand_all(&mut self) {
        for i in 0..self.nodes.len() {
            if self.is_header(i) {
                self.expanded.insert(i);
            }
        }
    }

    pub fn collapse_all(&mut self) {
        self.expanded.clear();
        // Keep root expanded so depth-1 headers show
        if !self.nodes.is_empty() {
            self.expanded.insert(0);
        }
    }

    /// Mouse-wheel free-scroll by `delta` rows, keeping the selection visible.
    /// No-op before the first render (`tree_list_rect` is zero-height then).
    pub fn wheel_scroll(&mut self, delta: i32) {
        let visible = self.visible_indices();
        let len = visible.len();
        let viewport = self.tree_list_rect.height as usize;
        self.tree_scroll_offset =
            crate::ui::scroll_viewport(self.tree_scroll_offset, delta, len, viewport);
        if let Some(pos) = visible.iter().position(|&x| x == self.selected) {
            let clamped =
                crate::ui::clamp_selected_to_view(pos, self.tree_scroll_offset, len, viewport);
            self.selected = visible[clamped];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    fn test_outline_state() {
        let content = "
# H1
Some intro.
## H2
- Item 1
";
        let mut state = OutlineState::new(
            "id".to_string(),
            "Title",
            content,
            crate::keybinds::Keybinds::default(),
            crate::keybinds::KeyMatcher::new(),
        );
        assert!(!state.load_error);
        assert_eq!(state.note_title, "Title");
        assert_eq!(state.nodes.len(), 5);

        // By default all headers are expanded (indices 0, 1, 3 are headers)
        assert!(state.expanded.contains(&0));
        assert!(state.expanded.contains(&1));
        assert!(state.expanded.contains(&3));

        let visible = state.visible_indices();
        assert_eq!(visible, vec![0, 1, 2, 3, 4]);

        // Collapse H2 (index 3)
        state.selected = 3;
        state.toggle_collapse();
        assert!(!state.expanded.contains(&3));

        let visible_after_collapse = state.visible_indices();
        // index 4 is child of H2, should be skipped
        assert_eq!(visible_after_collapse, vec![0, 1, 2, 3]);

        // Navigation check
        state.selected = 3;
        state.move_down(); // should clamp at 3 because 4 is collapsed/invisible
        assert_eq!(state.selected, 3);

        state.move_up(); // should move to 2
        assert_eq!(state.selected, 2);

        // Collapse all
        state.collapse_all();
        // Only root (0) should remain expanded
        assert!(state.expanded.contains(&0));
        assert!(!state.expanded.contains(&1));
        assert!(!state.expanded.contains(&3));

        let visible_collapsed_all = state.visible_indices();
        // index 1 is visible, but H1 is collapsed, so indices > 1 with depth > depth(1)=1 are hidden.
        // index 1 depth is 1. index 2 depth is 2. index 3 depth is 2. index 4 depth is 3.
        // So only 0 (root) and 1 (H1) are visible.
        assert_eq!(visible_collapsed_all, vec![0, 1]);
    }
}
