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
    pub previous_mode: crate::app::ViewMode,
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
    pub(super) dialog: Option<super::authoring::Dialog>,
    pub(super) write_task: Option<super::write::WriteTask>,
    pub(super) edit_intent: Option<super::authoring::EditIntent>,
    pub(super) undo: Vec<super::edits::Transaction>,
    pub(super) redo: Vec<super::edits::Transaction>,
}
impl BasesState {
    pub fn new(id: String, named_view: Option<String>) -> Self {
        let environment = Environment {
            host: Some(id.clone()),
            ..Environment::default()
        };
        Self {
            id,
            previous_mode: crate::app::ViewMode::List,
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
            dialog: None,
            write_task: None,
            edit_intent: None,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
    pub fn result(&self) -> Option<&QueryResult> {
        self.loaded
            .as_ref()?
            .result
            .as_ref()
            .ok()
            .map(std::sync::Arc::as_ref)
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
    pub(super) fn focused_group(&self) -> Option<(usize, Option<usize>)> {
        let result = self.result()?;
        match *self.items.get(self.selected)? {
            Item::Group(index) => Some((index, None)),
            Item::Row(row) => result.groups.iter().enumerate().find_map(|(index, group)| {
                group
                    .rows
                    .iter()
                    .position(|candidate| *candidate == row)
                    .map(|position| (index, Some(position)))
            }),
        }
    }
    pub(super) fn move_group(&mut self, right: bool, count: usize) {
        let Some((current, position)) = self.focused_group() else {
            return;
        };
        let result = self.result().expect("focused group has result");
        let next = if right {
            current
                .saturating_add(count)
                .min(result.groups.len().saturating_sub(1))
        } else {
            current.saturating_sub(count)
        };
        let target = position.and_then(|position| {
            let rows = &result.groups[next].rows;
            rows.get(position.min(rows.len().saturating_sub(1)))
                .copied()
        });
        if let Some(index) = self.items.iter().position(|item| match (*item, target) {
            (Item::Row(row), Some(target)) => row == target,
            (Item::Group(group), None) => group == next,
            _ => false,
        }) {
            self.selected = index;
        }
    }
    pub(super) fn move_row(&mut self, down: bool, count: usize) {
        let (first, last) = if matches!(self.layout(), Layout::Kanban) {
            self.focused_group()
                .and_then(|(group, _)| {
                    let first = self
                        .items
                        .iter()
                        .position(|item| matches!(item,Item::Group(index) if *index==group))?;
                    Some((first, first + self.result()?.groups[group].rows.len()))
                })
                .unwrap_or((0, self.items.len().saturating_sub(1)))
        } else {
            (0, self.items.len().saturating_sub(1))
        };
        self.selected = if down {
            self.selected.saturating_add(count).min(last)
        } else {
            self.selected.saturating_sub(count).max(first)
        };
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
                    let live_ids: HashSet<_> =
                        result.rows.iter().map(|row| row.id.as_str()).collect();
                    self.selected_ids
                        .retain(|id| live_ids.contains(id.as_str()));
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
