//! Native view/session state; evaluation and disk discovery remain in the worker.

use super::{eval::Environment, format::Layout, query::QueryResult, worker::LoadedBase};
use crate::keybinds::KeyMatcher;
use ratatui::layout::Rect;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug)]
pub enum Item {
    Group(usize),
    Row(usize),
}

pub struct BasesState {
    pub id: String,
    pub named_view: Option<String>,
    pub environment: Environment,
    pub loaded: Option<LoadedBase>,
    pub items: Vec<Item>,
    pub selected: usize,
    pub column: usize,
    pub offset: usize,
    pub selected_ids: HashSet<String>,
    pub loading: bool,
    pub error: Option<String>,
    pub matcher: KeyMatcher,
    pub body_area: Rect,
    pub hit_rows: Vec<(Rect, usize, usize)>,
}
impl BasesState {
    pub fn new(id: String, named_view: Option<String>) -> Self {
        let environment = Environment {
            host: Some(id.clone()),
            ..Environment::default()
        };
        Self {
            id,
            named_view,
            environment,
            loaded: None,
            items: Vec::new(),
            selected: 0,
            column: 0,
            offset: 0,
            selected_ids: HashSet::new(),
            loading: true,
            error: None,
            matcher: KeyMatcher::default(),
            body_area: Rect::default(),
            hit_rows: Vec::new(),
        }
    }
    pub fn result(&self) -> Option<&QueryResult> {
        self.loaded.as_ref()?.result.as_ref().ok()
    }
    pub fn row(&self) -> Option<&super::query::Row> {
        let Item::Row(index) = self.items.get(self.selected)? else {
            return None;
        };
        self.result()?.rows.get(*index)
    }
    pub fn layout(&self) -> Layout {
        self.loaded.as_ref().map_or(Layout::Table, |loaded| {
            loaded.document.config.views[loaded.view].layout.clone()
        })
    }
    pub fn install(&mut self, loaded: anyhow::Result<LoadedBase>) {
        let previous = self.row().map(|row| row.id.clone());
        self.loading = false;
        self.error = None;
        self.items.clear();
        match loaded {
            Err(error) => {
                self.loaded = None;
                self.error = Some(format!("{error:#}"));
            }
            Ok(loaded) => {
                if let Ok(result) = &loaded.result {
                    if loaded.document.config.views[loaded.view].group_by.is_some() {
                        for (index, group) in result.groups.iter().enumerate() {
                            self.items.push(Item::Group(index));
                            self.items.extend(group.rows.iter().copied().map(Item::Row));
                        }
                    } else {
                        self.items.extend((0..result.rows.len()).map(Item::Row));
                    }
                    self.selected_ids
                        .retain(|id| result.rows.iter().any(|row| &row.id == id));
                } else if let Err(error) = &loaded.result {
                    self.error = Some(format!("{error:#}"));
                }
                self.loaded = Some(loaded);
            }
        }
        self.selected = previous.and_then(|id| self.items.iter().position(|item| matches!(item, Item::Row(index) if self.result().is_some_and(|result| result.rows[*index].id == id)))).unwrap_or(self.selected).min(self.items.len().saturating_sub(1));
        self.column = self.column.min(
            self.loaded
                .as_ref()
                .map_or(0, |loaded| loaded.columns.len().saturating_sub(1)),
        );
    }
}
