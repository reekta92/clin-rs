use std::collections::HashSet;
use std::hash::Hash;

pub struct CanvasSelection<Id: Eq + Hash + Clone> {
    pub primary: Option<Id>,
    pub extra: HashSet<Id>,
}

impl<Id: Eq + Hash + Clone> CanvasSelection<Id> {
    pub fn new() -> Self {
        Self {
            primary: None,
            extra: HashSet::new(),
        }
    }
    pub fn select_only(&mut self, id: Id) {
        self.primary = Some(id);
        self.extra.clear();
    }
    pub fn clear(&mut self) {
        self.primary = None;
        self.extra.clear();
    }
    pub fn clear_set(&mut self) {
        self.extra.clear();
    }
    pub fn replace_set(&mut self, set: HashSet<Id>, primary: Option<Id>) {
        self.extra = set;
        self.primary = primary;
    }
    pub fn add(&mut self, id: Id) {
        self.extra.insert(id);
    }
    pub fn is_selected(&self, id: &Id) -> bool {
        self.primary.as_ref().is_some_and(|p| p == id) || self.extra.contains(id)
    }
    pub fn all(&self) -> HashSet<Id> {
        let mut s = self.extra.clone();
        if let Some(p) = &self.primary {
            s.insert(p.clone());
        }
        s
    }
    pub fn is_empty(&self) -> bool {
        self.primary.is_none() && self.extra.is_empty()
    }
    pub fn count(&self) -> usize {
        self.extra.len() + usize::from(self.primary.is_some())
    }
}

impl<Id: Eq + Hash + Clone> Default for CanvasSelection<Id> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_only_clears_extra() {
        let mut s = CanvasSelection::new();
        s.add("a".to_string());
        s.select_only("b".to_string());
        assert_eq!(s.primary.as_deref(), Some("b"));
        assert!(s.extra.is_empty());
    }

    #[test]
    fn clear_nukes_both() {
        let mut s = CanvasSelection::new();
        s.select_only("a".to_string());
        s.add("b".to_string());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn clear_set_keeps_primary() {
        let mut s = CanvasSelection::new();
        s.select_only("a".to_string());
        s.add("b".to_string());
        s.clear_set();
        assert_eq!(s.primary.as_deref(), Some("a"));
        assert!(s.extra.is_empty());
    }

    #[test]
    fn replace_set_swaps_and_sets_primary() {
        let mut s = CanvasSelection::new();
        let mut set = HashSet::new();
        set.insert("x".to_string());
        set.insert("y".to_string());
        s.replace_set(set, Some("x".to_string()));
        assert_eq!(s.primary.as_deref(), Some("x"));
        assert!(s.extra.contains("y"));
    }

    #[test]
    fn is_selected_primary_and_extra() {
        let mut s = CanvasSelection::new();
        s.select_only("a".to_string());
        s.add("b".to_string());
        assert!(s.is_selected(&"a".to_string()));
        assert!(s.is_selected(&"b".to_string()));
        assert!(!s.is_selected(&"c".to_string()));
    }

    #[test]
    fn all_unions() {
        let mut s = CanvasSelection::new();
        s.select_only("a".to_string());
        s.add("b".to_string());
        let all = s.all();
        assert_eq!(all.len(), 2);
        assert!(all.contains("a"));
        assert!(all.contains("b"));
    }

    #[test]
    fn count_and_is_empty() {
        let mut s = CanvasSelection::new();
        assert!(s.is_empty());
        assert_eq!(s.count(), 0);
        s.select_only("a".to_string());
        assert_eq!(s.count(), 1);
        s.add("b".to_string());
        assert_eq!(s.count(), 2);
    }
}
