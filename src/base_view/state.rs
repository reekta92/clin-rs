use crate::app_theme::AppThemeColors;
use crate::base::{
    BaseFile, BaseView,
    eval_pipeline::{BaseRow, ColumnDef, EvalResult, evaluate},
    expr::Value,
    parse_base,
    props::FileProps,
};
use crate::keybinds::{KeyMatcher, Keybinds};
use crate::storage::Storage;
use anyhow::Result;
use ratatui_textarea::TextArea;
use std::cmp::Ordering;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListMarker {
    #[default]
    Bullet,
    Numbered,
    None,
}
pub struct CellEdit {
    pub row_id: String,
    pub prop: String,
    pub original: String,
    pub input: TextArea<'static>,
}

pub struct BaseState {
    pub base_id: String,
    pub list_marker: ListMarker,
    pub base: BaseFile,
    pub view_index: usize,
    pub result: EvalResult,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub sort: Option<(String, crate::base::model::SortDirection)>,
    pub edit: Option<CellEdit>,
    pub raw_edit: Option<TextArea<'static>>,
    pub keybinds: Keybinds,
    pub seq_matcher: KeyMatcher,
    pub theme: AppThemeColors,
    pub last_area: ratatui::layout::Rect,
    pub error: Option<String>,
    pub status: Option<String>,
    pub storage: Storage,
    pub table_state: std::cell::RefCell<ratatui::widgets::TableState>,
    pub col_offset: std::cell::Cell<usize>,
    pub cards_per_screen: std::cell::Cell<usize>,
    pub cached_files: Vec<(String, FileProps, crate::frontmatter::Frontmatter)>,
}

impl BaseState {
    pub fn new(
        base_id: String,
        text: &str,
        keybinds: Keybinds,
        seq_matcher: KeyMatcher,
        theme: AppThemeColors,
        storage: Storage,
    ) -> Self {
        let mut state = Self {
            base_id,
            base: BaseFile::default(),
            view_index: 0,
            result: EvalResult {
                groups: Vec::new(),
                columns: Vec::new(),
                summaries: BTreeMap::new(),
            },
            cursor_row: 0,
            cursor_col: 0,
            sort: None,
            edit: None,
            raw_edit: None,
            keybinds,
            seq_matcher,
            theme,
            last_area: ratatui::layout::Rect::default(),
            error: None,
            status: None,
            storage: storage.clone(),
            table_state: std::cell::RefCell::new(ratatui::widgets::TableState::default()),
            col_offset: std::cell::Cell::new(0),
            cards_per_screen: std::cell::Cell::new(0),
            list_marker: ListMarker::Bullet,
            cached_files: Vec::new(),
        };

        match parse_base(text) {
            Ok(base) => {
                state.base = base;
                if state.base.views.is_empty() {
                    state.error = Some("Base has no views".to_string());
                } else {
                    state.refresh();
                }
            }
            Err(e) => {
                state.error = Some(format!("Failed to parse base file: {}", e));
            }
        }

        state
    }

    pub fn rebuild_cache(&mut self) {
        let mut files = Vec::new();
        let ids = self.storage.list_note_ids(false, false).unwrap_or_default();
        for id in ids {
            if id.ends_with(".base") {
                continue;
            }
            if let Ok(fm) = self.storage.load_frontmatter(&id)
                && let Ok(file_props) = FileProps::from_storage(&self.storage, &id, &fm)
            {
                files.push((id, file_props, fm));
            }
        }
        self.cached_files = files;
    }

    pub fn evaluate_view(&mut self) {
        if self.error.is_some() {
            return;
        }

        let view = match self.base.views.get(self.view_index) {
            Some(v) => v,
            None => {
                self.error = Some("Selected view does not exist".to_string());
                return;
            }
        };

        self.status = None;

        let files = self.cached_files.clone();

        match evaluate(&self.base, view, files) {
            Ok(res) => {
                self.result = res;
                // apply in-memory sort if active
                if let Some((ref col_key, dir)) = self.sort {
                    for g in &mut self.result.groups {
                        g.rows.sort_by(|a, b| {
                            let val_a = a.values.get(col_key).unwrap_or(&Value::Null);
                            let val_b = b.values.get(col_key).unwrap_or(&Value::Null);
                            let mut ord = val_a.partial_cmp_loose(val_b).unwrap_or(Ordering::Equal);
                            if dir == crate::base::model::SortDirection::Desc {
                                ord = ord.reverse();
                            }
                            ord
                        });
                    }
                }
                // Clamp cursors
                let total_rows = self.total_rows();
                if self.cursor_row >= total_rows && total_rows > 0 {
                    self.cursor_row = total_rows - 1;
                }
                if self.cursor_col >= self.result.columns.len() && !self.result.columns.is_empty() {
                    self.cursor_col = self.result.columns.len() - 1;
                }
            }
            Err(e) => {
                self.error = Some(format!("Evaluation failed: {}", e));
            }
        }
    }

    pub fn active_view(&self) -> Option<&BaseView> {
        self.base.views.get(self.view_index)
    }

    pub fn cycle_list_marker(&mut self) {
        if self
            .active_view()
            .is_some_and(|v| v.r#type == crate::base::model::ViewType::List)
        {
            self.list_marker = match self.list_marker {
                ListMarker::Bullet => ListMarker::Numbered,
                ListMarker::Numbered => ListMarker::None,
                ListMarker::None => ListMarker::Bullet,
            };
        }
    }

    pub fn refresh(&mut self) {
        self.rebuild_cache();
        self.evaluate_view();
    }

    pub fn total_rows(&self) -> usize {
        self.result.groups.iter().map(|g| g.rows.len()).sum()
    }

    pub fn get_row(&self, idx: usize) -> Option<&BaseRow> {
        let mut current = 0;
        for g in &self.result.groups {
            if idx < current + g.rows.len() {
                return Some(&g.rows[idx - current]);
            }
            current += g.rows.len();
        }
        None
    }

    pub fn selected_row(&self) -> Option<&BaseRow> {
        self.get_row(self.cursor_row)
    }

    pub fn selected_col(&self) -> Option<&ColumnDef> {
        self.result.columns.get(self.cursor_col)
    }

    pub fn row_value_display(&self, row: &BaseRow, col_key: &str) -> String {
        row.values
            .get(col_key)
            .map(|v| v.to_string())
            .unwrap_or_default()
    }

    pub fn move_up(&mut self) {
        self.cursor_row = self.cursor_row.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        let total = self.total_rows();
        if total > 0 && self.cursor_row + 1 < total {
            self.cursor_row += 1;
        }
    }

    fn page_size(&self) -> usize {
        self.last_area.height.saturating_sub(2).max(1) as usize
    }

    fn page_step(&self) -> usize {
        if self
            .active_view()
            .is_some_and(|v| v.r#type == crate::base::model::ViewType::Cards)
        {
            self.cards_per_screen.get().max(1)
        } else {
            self.page_size()
        }
    }

    pub fn page_up(&mut self) {
        let p = self.page_step();
        self.cursor_row = self.cursor_row.saturating_sub(p);
    }

    pub fn page_down(&mut self) {
        let total = self.total_rows();
        if total > 0 {
            let p = self.page_step();
            self.cursor_row = (self.cursor_row + p).min(total - 1);
        }
    }

    pub fn jump_to_top(&mut self) {
        self.cursor_row = 0;
    }

    pub fn jump_to_bottom(&mut self) {
        let t = self.total_rows();
        self.cursor_row = t.saturating_sub(1);
    }

    pub fn move_left(&mut self) {
        self.cursor_col = self.cursor_col.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        let total = self.result.columns.len();
        if total > 0 && self.cursor_col + 1 < total {
            self.cursor_col += 1;
        }
    }

    pub fn start_edit(&mut self) {
        let row = match self.selected_row() {
            Some(r) => r,
            None => return,
        };
        let col = match self.selected_col() {
            Some(c) => c,
            None => return,
        };
        if col.key.starts_with("file.") || col.key.starts_with("formula.") {
            self.status = Some("Cannot edit this cell".to_string());
            return;
        }
        if row.id.ends_with(".clin") {
            self.status = Some("Cannot edit encrypted note inline; decrypt first.".to_string());
            return;
        }
        let original = self.row_value_display(row, &col.key);
        let input = TextArea::new(vec![original.clone()]);
        self.edit = Some(CellEdit {
            row_id: row.id.clone(),
            prop: col.key.clone(),
            original,
            input,
        });
    }

    pub fn cancel_edit(&mut self) {
        self.edit = None;
    }

    pub fn commit_edit(&mut self) -> Result<()> {
        let edit = match &self.edit {
            Some(e) => e,
            None => return Ok(()),
        };
        let row_id = edit.row_id.clone();
        let prop = edit.prop.clone();
        let new_str = edit.input.lines().join("");

        // capture original value type BEFORE releasing the edit borrow
        let original_val = self
            .selected_row()
            .and_then(|r| r.values.get(&prop).cloned());

        self.storage
            .update_frontmatter(&row_id, |fm| match prop.as_str() {
                "title" => {
                    fm.title = Some(new_str.clone());
                }
                "tags" => {
                    fm.tags = new_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                "pinned" => {
                    fm.pinned = new_str.trim().to_lowercase() == "true";
                }
                "links" => {
                    fm.links = Some(
                        new_str
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect(),
                    );
                }
                "updated_at" => {
                    fm.updated_at = new_str.parse().ok();
                }
                "original_ext" => {
                    fm.original_ext = Some(new_str.clone());
                }
                _ => {
                    let new_val = match original_val {
                        Some(Value::Num(_)) => {
                            if let Ok(n) = new_str.parse::<f64>() {
                                serde_yaml_ng::to_value(n)
                                    .unwrap_or(serde_yaml_ng::Value::String(new_str.clone()))
                            } else {
                                serde_yaml_ng::Value::String(new_str.clone())
                            }
                        }
                        Some(Value::Bool(_)) => {
                            let b = new_str.trim().to_lowercase() == "true";
                            serde_yaml_ng::Value::Bool(b)
                        }
                        Some(Value::List(_)) => {
                            let seq: Vec<serde_yaml_ng::Value> = new_str
                                .split(',')
                                .map(|s| s.trim())
                                .filter(|s| !s.is_empty())
                                .map(|s| serde_yaml_ng::Value::String(s.to_string()))
                                .collect();
                            serde_yaml_ng::Value::Sequence(seq)
                        }
                        _ => serde_yaml_ng::Value::String(new_str.clone()),
                    };
                    fm.extra.insert(prop.clone(), new_val);
                }
            })?;

        self.edit = None;

        match self.storage.load_frontmatter(&row_id) {
            Ok(fm) => {
                if let Ok(file_props) = FileProps::from_storage(&self.storage, &row_id, &fm) {
                    if let Some(pos) = self
                        .cached_files
                        .iter()
                        .position(|(id, _, _)| id == &row_id)
                    {
                        self.cached_files[pos] = (row_id, file_props, fm);
                    } else {
                        self.cached_files.push((row_id, file_props, fm));
                    }
                    self.status = None;
                } else {
                    self.status = Some("Failed to load metadata for updated note".to_string());
                }
            }
            Err(e) => {
                self.status = Some(format!("Failed to reload updated note: {}", e));
            }
        }
        self.evaluate_view();
        Ok(())
    }

    pub fn cycle_view(&mut self) {
        if self.base.views.is_empty() {
            return;
        }
        self.view_index = (self.view_index + 1) % self.base.views.len();
        self.evaluate_view();
    }

    pub fn set_sort(&mut self, col_idx: usize, dir: crate::base::model::SortDirection) {
        if let Some(col) = self.result.columns.get(col_idx) {
            self.sort = Some((col.key.clone(), dir));
            // trigger sort reload in-memory:
            let col_key = col.key.clone();
            for g in &mut self.result.groups {
                g.rows.sort_by(|a, b| {
                    let val_a = a.values.get(&col_key).unwrap_or(&Value::Null);
                    let val_b = b.values.get(&col_key).unwrap_or(&Value::Null);
                    let mut ord = val_a.partial_cmp_loose(val_b).unwrap_or(Ordering::Equal);
                    if dir == crate::base::model::SortDirection::Desc {
                        ord = ord.reverse();
                    }
                    ord
                });
            }
        }
    }

    pub fn start_raw_edit(&mut self) {
        let text = match crate::base::serialize_base(&self.base) {
            Ok(t) => t,
            Err(e) => {
                self.status = Some(format!("Failed to open base editor: {}", e));
                return;
            }
        };
        let mut ta = TextArea::from(text.lines());
        ta.set_style(self.theme.bg_style());
        // Apply theme to textarea
        ta.set_cursor_line_style(self.theme.bg_style());
        ta.set_cursor_style(
            ratatui::style::Style::default()
                .fg(self.theme.highlight_fg)
                .bg(self.theme.highlight_bg),
        );
        self.raw_edit = Some(ta);
    }

    pub fn save_raw_edit(&mut self) -> Result<()> {
        let text = match &self.raw_edit {
            Some(ta) => ta.lines().join("\n"),
            None => return Ok(()),
        };
        let _base = parse_base(&text).map_err(|e| anyhow::anyhow!("{}", e))?;
        self.base = _base;
        crate::fsutil::atomic_write_str(&self.storage.note_path(&self.base_id), &text)?;
        self.raw_edit = None;
        self.evaluate_view();
        Ok(())
    }

    pub fn cancel_raw_edit(&mut self) {
        self.raw_edit = None;
    }

    pub fn export_csv(&self) -> Result<std::path::PathBuf> {
        let stem = std::path::Path::new(&self.base_id)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("export");
        let folder = std::path::Path::new(&self.base_id)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        let rel = if folder.as_os_str().is_empty() {
            format!("{}.csv", stem)
        } else {
            format!("{}/{}.csv", folder.display(), stem)
        };
        let path = self.storage.note_path(&rel);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut wtr = csv::Writer::from_path(&path)?;
        wtr.write_record(self.result.columns.iter().map(|c| c.display.as_str()))?;
        for g in &self.result.groups {
            for row in &g.rows {
                wtr.write_record(self.result.columns.iter().map(|c| {
                    row.values
                        .get(&c.key)
                        .map(|v| v.to_string())
                        .unwrap_or_default()
                }))?;
            }
        }
        wtr.flush()?;
        Ok(path)
    }

    pub fn copy_table(&self) -> usize {
        use std::fmt::Write;
        let mut tsv = String::new();
        // Header
        for (i, col) in self.result.columns.iter().enumerate() {
            if i > 0 {
                let _ = write!(tsv, "\t");
            }
            let _ = write!(tsv, "{}", col.display);
        }
        let _ = writeln!(tsv);
        // Rows
        let mut count = 0;
        for g in &self.result.groups {
            for row in &g.rows {
                for (i, col) in self.result.columns.iter().enumerate() {
                    if i > 0 {
                        let _ = write!(tsv, "\t");
                    }
                    let val = row
                        .values
                        .get(&col.key)
                        .map(|v| v.to_string())
                        .unwrap_or_default();
                    let _ = write!(tsv, "{}", val);
                }
                let _ = writeln!(tsv);
                count += 1;
            }
        }
        crate::text_edit::set_clipboard_text(&tsv);
        count
    }

    pub fn new_note_folder(&self) -> String {
        std::path::Path::new(&self.base_id)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .to_string()
    }
}
