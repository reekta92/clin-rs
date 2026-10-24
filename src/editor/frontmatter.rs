//! One working YAML source, presented as properties or raw text above Body.
use super::{EditFocus, NoteEditor};
use crate::text_edit::TextEditTarget;
use crate::{
    app::App,
    config::FrontmatterMode,
    frontmatter::properties::{PropertyFrame, edit_properties, parse_mapping},
};
use anyhow::{Context, Result, bail};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{prelude::*, widgets::*};
use ratatui_textarea::{Input, TextArea};
use serde_yaml_ng::Value;

pub(crate) struct PropertyInput {
    pub original: Option<String>,
    pub name: TextArea<'static>,
    pub value: TextArea<'static>,
    pub value_focus: bool,
}
pub(crate) struct FrontmatterEditor {
    owner: uuid::Uuid,
    pub raw: TextArea<'static>,
    pub visible: bool,
    pub selected: usize,
    pub input: Option<PropertyInput>,
    pub rect: Rect,
    pub list_offset: usize,
    pub loaded_title: String,
}
impl FrontmatterEditor {
    pub fn yaml(&self) -> String {
        self.raw.lines().join("\n")
    }
    pub fn mutation(&self) -> String {
        let input = self
            .input
            .as_ref()
            .map(|input| (input.name.lines(), input.value.lines()));
        format!("{}\n{input:?}", self.yaml())
    }
    pub fn patch(&mut self, key: String, value: Option<Value>) -> Result<()> {
        self.patch_changes(&[(key, value)])
    }
    fn patch_changes(&mut self, changes: &[(String, Option<Value>)]) -> Result<()> {
        let yaml = self.yaml();
        let source = format!(
            "---\n{}{}---\n",
            yaml,
            if yaml.ends_with('\n') { "" } else { "\n" }
        );
        let updated = edit_properties(&source, changes)?;
        let frame = PropertyFrame::parse(&updated)?;
        self.raw.select_all();
        self.raw.insert_str(frame.yaml());
        Ok(())
    }
    pub fn input_mut(&mut self, raw_mode: bool) -> Option<&mut TextArea<'static>> {
        if let Some(input) = &mut self.input {
            Some(if input.value_focus {
                &mut input.value
            } else {
                &mut input.name
            })
        } else if raw_mode {
            Some(&mut self.raw)
        } else {
            None
        }
    }
}
impl NoteEditor {
    pub(crate) fn frontmatter_supported(&self) -> bool {
        self.template_edit_path.is_none()
            && (self.public_document.is_none() || self.public_document_body_only)
            && self.editing_id.as_deref().is_some_and(|id| {
                id.ends_with(".md")
                    || id.ends_with(".txt")
                    || (std::path::Path::new(id).extension().is_none()
                        && self
                            .frontmatter_baseline()
                            .is_some_and(|baseline| !baseline.exists()))
            })
    }
    pub(crate) fn frontmatter_baseline(
        &self,
    ) -> Option<&crate::storage::documents::DocumentSnapshot> {
        self.public_document
            .as_ref()
            .or(self.note_baseline.as_ref())
    }
    pub(crate) fn ensure_frontmatter(&mut self) -> Result<()> {
        if !self.frontmatter_supported() {
            self.frontmatter = None;
            return Ok(());
        }
        let baseline = self
            .frontmatter_baseline()
            .context("missing note baseline")?;
        if self
            .frontmatter
            .as_ref()
            .is_some_and(|state| state.owner == baseline.recovery_id())
        {
            return Ok(());
        }
        let frame = PropertyFrame::parse(baseline.source())?;
        self.frontmatter = Some(FrontmatterEditor {
            owner: baseline.recovery_id(),
            raw: TextArea::from(frame.yaml().replace("\r\n", "\n").split('\n')),
            visible: false,
            selected: 0,
            input: None,
            rect: Rect::default(),
            list_offset: 0,
            loaded_title: crate::events::get_title_text(&self.title_editor).into_owned(),
        });
        Ok(())
    }
    pub(crate) fn frontmatter_mutation(&self) -> String {
        self.frontmatter
            .as_ref()
            .map_or_else(String::new, FrontmatterEditor::mutation)
    }
    pub(crate) fn note_working_source(&self) -> Result<String> {
        let baseline = self
            .frontmatter_baseline()
            .context("missing note baseline")?;
        let frame = PropertyFrame::parse(baseline.source())?;
        let body = self.body.snapshot();
        if let Some(state) = &self.frontmatter {
            if state.input.is_some() {
                bail!(
                    "Finish the property edit with Ctrl+Enter or cancel it with Esc before saving"
                );
            }
            let yaml = state.yaml();
            parse_mapping(&yaml).context("Invalid frontmatter; repair it before saving")?;
            return Ok(frame.with_yaml_and_body(&yaml, &body));
        }
        Ok(frame.with_yaml_and_body(frame.yaml(), &body))
    }
    pub(crate) fn header_pending(&self) -> bool {
        self.frontmatter.as_ref().is_some_and(|state| {
            if self
                .frontmatter_baseline()
                .is_none_or(|baseline| baseline.recovery_id() != state.owner)
            {
                return false;
            }
            state.input.is_some()
                || self
                    .frontmatter_baseline()
                    .and_then(|baseline| PropertyFrame::parse(baseline.source()).ok())
                    .is_none_or(|frame| state.yaml() != frame.yaml().replace("\r\n", "\n"))
        })
    }
    pub(crate) fn adopt_frontmatter(&mut self, source: &str) -> Result<()> {
        if let Some(state) = &mut self.frontmatter {
            let frame = PropertyFrame::parse(source)?;
            let yaml = frame.yaml().replace("\r\n", "\n");
            let yaml = yaml.as_str();
            if state.yaml() != yaml {
                state.raw.select_all();
                state.raw.insert_str(yaml);
            }
            let title = crate::frontmatter::parse(source)
                .0
                .title
                .unwrap_or_else(|| {
                    std::path::Path::new(self.editing_id.as_deref().unwrap_or("Note"))
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Note")
                        .to_owned()
                });
            let current = crate::events::get_title_text(&self.title_editor);
            if current.as_ref() == state.loaded_title || current.as_ref() == title {
                self.title_editor.select_all();
                self.title_editor.insert_str(&title);
            }
            state.loaded_title = title;
        }
        Ok(())
    }
    pub(crate) fn live_note_revision(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        self.body.identity().hash(&mut hash);
        self.body.revision().hash(&mut hash);
        self.frontmatter
            .as_ref()
            .map(|state| state.raw.lines())
            .hash(&mut hash);
        self.frontmatter
            .as_ref()
            .is_some_and(|state| state.input.is_some())
            .hash(&mut hash);
        crate::events::get_title_text(&self.title_editor).hash(&mut hash);
        hash.finish()
    }
    pub(crate) fn live_note_source(&self) -> Option<String> {
        if self.frontmatter_supported() {
            let baseline = self.frontmatter_baseline()?;
            let frame = PropertyFrame::parse(baseline.source()).ok()?;
            let yaml = self
                .frontmatter
                .as_ref()
                .map_or_else(|| frame.yaml().to_owned(), FrontmatterEditor::yaml);
            let source = frame.with_yaml_and_body(&yaml, &self.body.snapshot());
            let title = crate::events::get_title_text(&self.title_editor);
            if self
                .frontmatter
                .as_ref()
                .is_some_and(|state| title.as_ref() != state.loaded_title)
            {
                edit_properties(
                    &source,
                    &[("title".into(), Some(Value::String(title.into_owned())))],
                )
                .ok()
            } else {
                Some(source)
            }
        } else {
            self.public_document_source()
        }
    }
}
impl App {
    pub fn toggle_frontmatter(&mut self) {
        if let Err(error) = self.editor.ensure_frontmatter() {
            self.set_temporary_status(&format!("Frontmatter: {error:#}"));
            return;
        }
        if let Some(state) = &mut self.editor.frontmatter {
            state.visible = !state.visible;
        } else {
            self.set_temporary_status_static("Frontmatter is available for Markdown/text notes");
        }
    }
    pub(crate) fn frontmatter_key(&mut self, key: KeyEvent) {
        let result = self.frontmatter_key_inner(key);
        if let Err(error) = result {
            self.set_temporary_status(&format!("Frontmatter: {error:#}"));
        }
    }
    fn frontmatter_key_inner(&mut self, key: KeyEvent) -> Result<()> {
        let raw_mode = self.config.editor.frontmatter_mode == FrontmatterMode::RawYaml
            && self
                .editor
                .frontmatter
                .as_ref()
                .is_none_or(|state| state.input.is_none());
        let state = self
            .editor
            .frontmatter
            .as_mut()
            .context("frontmatter unavailable")?;
        if !raw_mode
            && key.code == KeyCode::Enter
            && key.modifiers.contains(KeyModifiers::CONTROL)
            && state.input.is_some()
        {
            let input = state.input.as_ref().expect("property input");
            let name = input.name.lines().join("");
            if name.trim().is_empty() {
                bail!("property name cannot be empty");
            }
            let text = input.value.lines().join("\n");
            let mapping = parse_mapping(&format!("value:\n  {}\n", text.replace('\n', "\n  ")))?;
            let value = mapping.get("value").cloned().unwrap_or(Value::Null);
            self.storage
                .load_property_types()?
                .validate(&name, &value)?;
            let original = input.original.clone();
            let mapping = parse_mapping(&state.yaml())?;
            if original.as_deref() != Some(name.as_str()) && mapping.contains_key(name.as_str()) {
                bail!("property already exists");
            }
            let mut changes = Vec::new();
            if let Some(original) = original
                && original != name
            {
                changes.push((original, None));
            }
            changes.push((name, Some(value)));
            state.patch_changes(&changes)?;
            state.input = None;
            return Ok(());
        }
        if !raw_mode && key.code == KeyCode::Esc && state.input.is_some() {
            state.input = None;
            return Ok(());
        }
        if let Some(input) = state.input_mut(raw_mode) {
            if !raw_mode && key.code == KeyCode::Tab {
                state.input.as_mut().expect("input").value_focus ^= true;
                return Ok(());
            }
            crate::text_edit::update_selection_for_move(input, &key);
            if !crate::text_edit::apply_text_shortcuts(&self.keybinds, input, key) {
                input.input(Input::from(key));
            }
            return Ok(());
        }
        let mapping = parse_mapping(&state.yaml()).context(
            "Cannot show properties; set editor.frontmatter_mode = \"raw_yaml\" to repair YAML",
        )?;
        let rows: Vec<_> = mapping.iter().collect();
        state.selected = state.selected.min(rows.len().saturating_sub(1));
        match key.code {
            KeyCode::Up => state.selected = state.selected.saturating_sub(1),
            KeyCode::Down => {
                state.selected = (state.selected + 1).min(rows.len().saturating_sub(1))
            }
            KeyCode::Char('a') if key.modifiers.is_empty() => {
                state.input = Some(PropertyInput {
                    original: None,
                    name: TextArea::default(),
                    value: TextArea::from(["null"]),
                    value_focus: false,
                })
            }
            KeyCode::Enter => {
                if let Some((name, value)) = rows.get(state.selected) {
                    let name = name
                        .as_str()
                        .context("Complex property keys require raw_yaml mode")?
                        .to_owned();
                    if let Value::Bool(value) = value {
                        state.patch(name, Some(Value::Bool(!value)))?;
                    } else {
                        state.input = Some(PropertyInput {
                            original: Some(name.clone()),
                            name: TextArea::from([name]),
                            value: TextArea::from(
                                serde_yaml_ng::to_string(value)?.trim_end().lines(),
                            ),
                            value_focus: true,
                        });
                    }
                }
            }
            KeyCode::Delete | KeyCode::Char('d') => {
                if let Some((name, _)) = rows.get(state.selected) {
                    state.patch(
                        name.as_str()
                            .context("Complex keys require raw_yaml mode")?
                            .to_owned(),
                        None,
                    )?;
                }
            }
            _ => {
                crate::text_edit::apply_text_shortcuts(&self.keybinds, &mut state.raw, key);
            }
        }
        Ok(())
    }
}

/// Shared with hit testing: reserve at most half of the usable editor height.
pub(crate) fn region_height(available: u16, visible: bool) -> u16 {
    if visible { (available / 2).min(10) } else { 0 }
}
pub(crate) fn draw_region(frame: &mut Frame, app: &mut App, focus: EditFocus, area: Rect) -> Rect {
    let _ = app.editor.ensure_frontmatter();
    let height = region_height(
        area.height,
        app.editor
            .frontmatter
            .as_ref()
            .is_some_and(|state| state.visible),
    );
    let rect = Rect::new(area.x, area.y, area.width, height);
    let body = Rect::new(
        area.x,
        area.y + height,
        area.width,
        area.height.saturating_sub(height),
    );
    let Some(state) = &mut app.editor.frontmatter else {
        return body;
    };
    state.rect = rect;
    if height == 0 {
        return body;
    }
    let raw_mode =
        app.config.editor.frontmatter_mode == FrontmatterMode::RawYaml && state.input.is_none();
    let block = Block::bordered()
        .style(app.app_theme.bg_style())
        .border_style(Style::default().fg(if focus == EditFocus::Frontmatter {
            app.app_theme.accent
        } else {
            app.app_theme.border
        }))
        .title(if state.input.is_some() {
            "Property · Tab field · Ctrl+Enter apply · Esc cancel"
        } else if raw_mode {
            "Frontmatter · YAML"
        } else {
            "Properties · a add · Enter edit · Del remove"
        });
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    if !raw_mode && let Some(input) = &mut state.input {
        let chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(inner);
        let name_chunks =
            Layout::horizontal([Constraint::Length(6), Constraint::Min(0)]).split(chunks[0]);
        frame.render_widget(
            Paragraph::new("Name: ").style(app.app_theme.bg_style()),
            name_chunks[0],
        );
        crate::ui::render_textarea_with_theme(
            frame,
            &mut input.name,
            name_chunks[1],
            &app.app_theme,
            focus == EditFocus::Frontmatter && !input.value_focus,
            false,
            Block::default(),
            app.app_theme.bg_style(),
        );
        crate::ui::render_textarea_with_theme(
            frame,
            &mut input.value,
            chunks[1],
            &app.app_theme,
            focus == EditFocus::Frontmatter && input.value_focus,
            false,
            Block::default(),
            app.app_theme.bg_style(),
        );
    } else if raw_mode {
        crate::ui::render_textarea_with_theme(
            frame,
            &mut state.raw,
            inner,
            &app.app_theme,
            focus == EditFocus::Frontmatter,
            false,
            Block::default(),
            app.app_theme.bg_style(),
        );
    } else {
        match parse_mapping(&state.yaml()) {
            Ok(mapping) => {
                let items = mapping
                    .iter()
                    .map(|(name, value)| {
                        let kind = match value {
                            Value::Null => "null",
                            Value::Bool(_) => "boolean",
                            Value::Number(_) => "number",
                            Value::String(_) => "text",
                            Value::Sequence(_) => "list",
                            Value::Mapping(_) => "mapping",
                            Value::Tagged(_) => "tagged",
                        };
                        ListItem::new(format!(
                            "{}  [{kind}]  {}",
                            name.as_str().unwrap_or("<complex key>"),
                            serde_yaml_ng::to_string(value)
                                .unwrap_or_default()
                                .trim()
                                .replace('\n', " ")
                        ))
                    })
                    .collect::<Vec<_>>();
                let item_count = items.len();
                let mut selection = ListState::default().with_selected(Some(state.selected));
                *selection.offset_mut() = state.list_offset;
                frame.render_stateful_widget(
                    List::new(items)
                        .style(app.app_theme.bg_style())
                        .highlight_style(
                            Style::default()
                                .fg(app.app_theme.highlight_fg)
                                .bg(app.app_theme.highlight_bg),
                        ),
                    inner,
                    &mut selection,
                );
                state.list_offset = selection.offset();
                crate::ui::paint_list_hover(
                    frame,
                    inner,
                    &selection,
                    item_count,
                    app.mouse_pos,
                    app.app_theme.hover_style(),
                );
            }
            Err(error) => frame.render_widget(
                Paragraph::new(format!(
                    "{error}. Use frontmatter_mode = \"raw_yaml\" to repair."
                ))
                .wrap(Wrap { trim: false }),
                inner,
            ),
        }
    }
    body
}

impl App {
    pub(crate) fn frontmatter_mouse(
        &mut self,
        mouse: crossterm::event::MouseEvent,
        focus: &mut EditFocus,
        selection: &mut crate::text_edit::MouseTextSelection,
    ) -> bool {
        use crossterm::event::{MouseButton, MouseEventKind};
        let Some(state) = &mut self.editor.frontmatter else {
            return false;
        };
        if !state.visible || !state.rect.contains((mouse.column, mouse.row).into()) {
            return false;
        }
        *focus = EditFocus::Frontmatter;
        let mut inner = Block::bordered().inner(state.rect);
        let raw = self.config.editor.frontmatter_mode == FrontmatterMode::RawYaml
            && state.input.is_none();
        if let Some(input) = &mut state.input {
            let chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(inner);
            let name_chunks =
                Layout::horizontal([Constraint::Length(6), Constraint::Min(0)]).split(chunks[0]);
            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                input.value_focus = mouse.row != chunks[0].y;
            }
            inner = if input.value_focus {
                chunks[1]
            } else {
                name_chunks[1]
            };
        }
        if let Some(input) = state.input_mut(raw) {
            let (row, col) = input.scroll_offset();
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    crate::events::move_textarea_cursor_to_mouse(
                        input,
                        inner,
                        mouse.column,
                        mouse.row,
                        usize::from(row),
                        usize::from(col),
                    );
                    selection.active = true;
                }
                MouseEventKind::Drag(MouseButton::Left) => {
                    if !input.has_selection() {
                        input.start_selection();
                    }
                    crate::events::move_textarea_cursor_to_mouse(
                        input,
                        inner,
                        mouse.column,
                        mouse.row,
                        usize::from(row),
                        usize::from(col),
                    );
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    selection.active = false;
                    if self.config.editor.copy_on_select {
                        let _ = crate::text_edit::copy_mouse_selection(input);
                    }
                }
                _ => {
                    input.input(Input::from(mouse));
                }
            }
        } else {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    state.selected =
                        state.list_offset + usize::from(mouse.row.saturating_sub(inner.y))
                }
                MouseEventKind::ScrollDown => state.selected = state.selected.saturating_add(1),
                MouseEventKind::ScrollUp => state.selected = state.selected.saturating_sub(1),
                _ => {}
            }
        }
        true
    }
}

impl App {
    pub(crate) fn prepare_editor_note_source(&self) -> Result<String> {
        let baseline = self
            .editor
            .frontmatter_baseline()
            .context("missing note baseline")?;
        let source = self.editor.note_working_source()?;
        let title = crate::events::get_title_text(&self.editor.title_editor);
        let old_title = self
            .editor
            .frontmatter
            .as_ref()
            .map(|state| state.loaded_title.clone())
            .unwrap_or_else(|| {
                crate::frontmatter::parse(baseline.source())
                    .0
                    .title
                    .unwrap_or_default()
            });
        self.storage.prepare_note_source(
            baseline.id(),
            Some(baseline),
            &source,
            &title,
            title.as_ref() != old_title,
            None,
            crate::ui::now_unix_secs(),
        )
    }
}
