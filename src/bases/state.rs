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
    pub(super) expand_requested: bool,
    pub(crate) embedded_request: Option<super::embedding::Request>,
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
    pub(super) creating: bool,
    pub error: Option<String>,
    pub matcher: KeyMatcher,
    pub body_area: Rect,
    pub(super) scrollbar: crate::ui::scrollbar::ScrollbarMeta,
    pub(super) scroll_drag: Option<i32>,
    pub hit_rows: Vec<(Rect, usize, usize)>,
    pub(super) hit_groups: Vec<(Rect, usize)>,
    pub(super) media: super::media::PublicImages,
    pub(super) drag: Option<Item>,
    pub(super) focus_id: Option<String>,
    pub(super) focus_group: Option<super::eval::Identity>,
    pub(super) search_text: String,
    pub(super) search_popup: Option<crate::ui::quick_search::QuickSearch<usize>>,
    pub(super) search_before: (String, Option<String>, Option<super::eval::Identity>),
    collapsed_groups: HashSet<super::eval::Identity>,
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
            expand_requested: false,
            embedded_request: None,
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
            creating: false,
            error: None,
            matcher: KeyMatcher::default(),
            body_area: Rect::default(),
            scrollbar: crate::ui::scrollbar::ScrollbarMeta::default(),
            scroll_drag: None,
            hit_rows: Vec::new(),
            hit_groups: Vec::new(),
            media: super::media::PublicImages::default(),
            drag: None,
            focus_id: None,
            focus_group: None,
            search_text: String::new(),
            search_popup: None,
            search_before: (String::new(), None, None),
            collapsed_groups: HashSet::new(),
            dialog: None,
            write_task: None,
            edit_intent: None,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
    pub(crate) fn for_creation(
        id: String,
        previous_mode: crate::app::ViewMode,
        storage: crate::storage::Storage,
        pool: &rayon::ThreadPool,
    ) -> Self {
        let mut state = Self::new(id.clone(), None);
        state.previous_mode = previous_mode;
        state.loading = false;
        state.creating = true;
        state.write_task = Some(super::write::WriteTask::spawn(
            storage,
            pool,
            super::write::Operation::CreateBase { id },
        ));
        state
    }
    pub(crate) fn has_popup(&self) -> bool {
        self.dialog.is_some() || self.search_popup.is_some()
    }
    pub(crate) fn is_writing(&self) -> bool {
        self.write_task.as_ref().is_some_and(|task| task.writing)
    }
    pub(crate) fn palette_input_available(&self) -> bool {
        !self.has_popup()
            && self.drag.is_none()
            && self.scroll_drag.is_none()
            && self.write_task.as_ref().is_none_or(|task| task.writing)
    }
    pub(crate) fn palette_command_available(&self, id: &str) -> bool {
        if !self.palette_input_available() {
            return false;
        }
        match id {
            "bases.refresh" => true,
            "bases.configure"
            | "bases.next_view"
            | "bases.previous_view"
            | "bases.new_note"
            | "bases.export"
            | "bases.search"
            | "bases.select_all"
            | "bases.previous_property"
            | "bases.next_property" => self.loaded.is_some() && self.write_task.is_none(),
            "bases.undo" => !self.undo.is_empty() && self.write_task.is_none(),
            "bases.redo" => !self.redo.is_empty() && self.write_task.is_none(),
            "bases.toggle_group" => self.focused_group().is_some(),
            _ => self.row().is_some() && self.write_task.is_none(),
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
    fn group_ids(&self) -> &[super::eval::Identity] {
        self.loaded
            .as_ref()
            .map_or(&[], |loaded| loaded.group_ids.as_slice())
    }
    pub(super) fn group_identity(&self, index: usize) -> Option<super::eval::Identity> {
        self.group_ids().get(index).cloned()
    }
    pub(super) fn group_collapsed(&self, index: usize) -> bool {
        self.group_ids()
            .get(index)
            .is_some_and(|id| self.collapsed_groups.contains(id))
    }
    pub(super) fn toggle_group(&mut self) -> anyhow::Result<()> {
        self.ensure_idle()?;
        if matches!(self.layout(), Layout::Kanban) {
            anyhow::bail!("Kanban columns do not collapse");
        }
        let (index, _) = self
            .focused_group()
            .ok_or_else(|| anyhow::anyhow!("select a grouped section"))?;
        let id = self.group_ids()[index].clone();
        if !self.collapsed_groups.remove(&id) {
            self.collapsed_groups.insert(id.clone());
        }
        self.rebuild_items(None, Some(id));
        Ok(())
    }
    pub(super) fn focus_search_row(&mut self, row: usize) {
        let group = self.result().and_then(|result| {
            result
                .groups
                .iter()
                .position(|group| group.rows.contains(&row))
        });
        if let Some(id) = group.and_then(|index| self.group_ids().get(index)).cloned() {
            self.collapsed_groups.remove(&id);
        }
        self.rebuild_items(None, None);
        if let Some(position) = self
            .items
            .iter()
            .position(|item| matches!(item,Item::Row(index) if *index==row))
        {
            self.selected = position;
        }
    }
    pub(super) fn rebuild_items(
        &mut self,
        previous: Option<&str>,
        previous_group: Option<super::eval::Identity>,
    ) {
        self.items.clear();
        self.hit_rows.clear();
        self.hit_groups.clear();
        self.drag = None;
        if let Some(loaded) = &self.loaded
            && let Ok(result) = &loaded.result
        {
            if loaded.document.config.views[loaded.view].group_by.is_some() {
                for (index, group) in result.groups.iter().enumerate() {
                    self.items.push(Item::Group(index));
                    if matches!(self.layout(), Layout::Kanban) || !self.group_collapsed(index) {
                        self.items.extend(group.rows.iter().copied().map(Item::Row));
                    }
                }
            } else {
                self.items.extend((0..result.rows.len()).map(Item::Row));
            }
        }
        let row = previous.and_then(|id| {
            self.items.iter().position(|item| match item {
                Item::Row(index) => self
                    .result()
                    .is_some_and(|result| result.rows[*index].id == id),
                Item::Group(_) => false,
            })
        });
        let group = previous_group
            .and_then(|id| self.group_ids().iter().position(|group| *group == id))
            .and_then(|index| {
                self.items
                    .iter()
                    .position(|item| matches!(item,Item::Group(group) if *group==index))
            });
        self.selected = row
            .or(group)
            .unwrap_or(self.selected)
            .min(self.items.len().saturating_sub(1));
        self.offset = self.offset.min(self.items.len().saturating_sub(1));
    }
    pub fn install(&mut self, loaded: anyhow::Result<LoadedBase>) {
        let previous = self
            .focus_id
            .take()
            .or_else(|| self.row().map(|row| row.id.clone()));
        let previous_group = self.focus_group.take().or_else(|| {
            self.focused_group()
                .and_then(|(index, _)| self.group_ids().get(index))
                .cloned()
        });
        let same_grouping =
            loaded
                .as_ref()
                .ok()
                .zip(self.loaded.as_ref())
                .is_some_and(|(next, old)| {
                    let next = &next.document.config.views[next.view];
                    let old = &old.document.config.views[old.view];
                    next.name == old.name
                        && next.group_by.as_ref().map(|group| &group.property)
                            == old.group_by.as_ref().map(|group| &group.property)
                });
        if !same_grouping {
            self.collapsed_groups.clear();
        }
        self.drag = None;
        self.hit_groups.clear();
        self.hit_rows.clear();
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
        self.rebuild_items(previous.as_deref(), previous_group);
        if let Some(popup) = self.search_popup.as_mut() {
            popup.results = self
                .loaded
                .as_ref()
                .and_then(|loaded| loaded.result.as_ref().ok())
                .map_or_else(Vec::new, |result| (0..result.rows.len()).collect());
            popup.selected = popup.selected.min(popup.results.len().saturating_sub(1));
            popup.scroll_to_selected(10);
            popup.info = Some(format!("{} matches", popup.results.len()));
        }
        self.column = self.column.min(
            self.loaded
                .as_ref()
                .map_or(0, |loaded| loaded.columns.len().saturating_sub(1)),
        );
    }
}
