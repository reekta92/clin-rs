use super::{
    ical::{Draft, Occurrence},
    store::{self, Resource},
};
use crate::{
    app::{App, ViewMode},
    app_theme::AppThemeColors,
    config::WeekStart,
    overlay::{OverlayResult, OverlayView},
};
use anyhow::{Context, Result, ensure};
use chrono::{Datelike, Duration, Local, NaiveDate, Timelike};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use ratatui::{prelude::*, widgets::*};
use ratatui_textarea::TextArea;
use std::collections::BTreeMap;

const HELP: &str = "m/w/d/a  Month / Week / Day / Agenda\nhjkl / Arrows  Navigate dates/events    PgUp/PgDn  Previous/next period\nt  Today    J/K  Select event    Tab  Next view\nn  New event    e  Edit series    Delete  Delete series\nL  Link note (or schedule note on an empty date)\no / Enter  Open linked note    c  Resolve sync conflict\ny  Copy inline date/event reference\ni  Import .ics    E  Export .ics (new file only)\ns  Sync CalDAV    r  Reload from disk\nEsc  Return to Notes    ?  This help\n\nDate input: YYYY-MM-DD (all day; end date exclusive)\nTime input: YYYY-MM-DDTHH:MM (local), or RFC3339 with offset.\nRepeat: RFC 5545 RRULE, e.g. FREQ=WEEKLY;BYDAY=MO,WE\nEdits and deletes apply to the entire series.\nTimes display in your system timezone. Note links remain local.";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Display {
    #[default]
    Month,
    Week,
    Day,
    Agenda,
}

impl Display {
    fn label(self) -> &'static str {
        match self {
            Self::Month => "Month",
            Self::Week => "Week",
            Self::Day => "Day",
            Self::Agenda => "Agenda",
        }
    }
    fn next(self) -> Self {
        match self {
            Self::Month => Self::Week,
            Self::Week => Self::Day,
            Self::Day => Self::Agenda,
            Self::Agenda => Self::Month,
        }
    }
}

struct LinksJob {
    revision: u64,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    receiver: std::sync::mpsc::Receiver<Result<BTreeMap<String, Vec<String>>>>,
}
impl Drop for LinksJob {
    fn drop(&mut self) {
        self.cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

pub struct View {
    pub date: NaiveDate,
    pub display: Display,
    pub dialog: Option<Dialog>,
    selected: usize,
    events: Vec<Occurrence>,
    visible: Vec<usize>,
    cells: Vec<(Rect, NaiveDate)>,
    rows: Vec<(Rect, usize)>,
    links: BTreeMap<String, Vec<String>>,
    links_revision: Option<u64>,
    links_job: Option<LinksJob>,
    events_key: Option<(NaiveDate, NaiveDate, u64)>,
    visible_key: Option<(NaiveDate, Display)>,
    pending_resource: Option<String>,
    pub error: Option<String>,
    hour_scroll: u32,
}

pub enum Dialog {
    Form(Box<Form>),
    Path {
        import: bool,
        input: TextArea<'static>,
    },
    Notes {
        popup: crate::ui::quick_search::QuickSearch<(String, String)>,
        resource: Option<String>,
        open: bool,
    },
    Delete(String),
    Conflict(String),
    Help,
}

pub struct Form {
    id: Option<String>,
    inputs: Vec<TextArea<'static>>,
    focus: usize,
    collection: usize,
    choices: Vec<(Option<String>, String)>,
    note: Option<String>,
    rects: Vec<(Rect, usize)>,
}

pub fn month_start(date: NaiveDate) -> NaiveDate {
    date.with_day(1).expect("first day")
}

pub fn week_start(date: NaiveDate, start: WeekStart) -> NaiveDate {
    let day = match start {
        WeekStart::Sunday => date.weekday().num_days_from_sunday(),
        WeekStart::Monday => date.weekday().num_days_from_monday(),
    };
    date.checked_sub_signed(Duration::days(i64::from(day)))
        .unwrap_or(date)
}

pub fn shift_month(date: NaiveDate, months: i32) -> NaiveDate {
    if months < 0 {
        date.checked_sub_months(chrono::Months::new(months.unsigned_abs()))
    } else {
        date.checked_add_months(chrono::Months::new(months as u32))
    }
    .unwrap_or(date)
}

pub fn month_cells(area: Rect, date: NaiveDate, start: WeekStart) -> Vec<(Rect, NaiveDate)> {
    let first = week_start(month_start(date), start);
    (0..42u16)
        .filter_map(|index| {
            let col = index % 7;
            let row = index / 7;
            let left = u32::from(area.width) * u32::from(col) / 7;
            let right = u32::from(area.width) * u32::from(col + 1) / 7;
            let top = u32::from(area.height) * u32::from(row) / 6;
            let bottom = u32::from(area.height) * u32::from(row + 1) / 6;
            Some((
                Rect::new(
                    area.x + left as u16,
                    area.y + top as u16,
                    (right - left) as u16,
                    (bottom - top) as u16,
                ),
                first.checked_add_signed(Duration::days(i64::from(index)))?,
            ))
        })
        .collect()
}

impl View {
    pub fn new(date: NaiveDate) -> Self {
        Self {
            date,
            display: Display::Month,
            dialog: None,
            selected: 0,
            events: Vec::new(),
            visible: Vec::new(),
            cells: Vec::new(),
            rows: Vec::new(),
            links: BTreeMap::new(),
            links_revision: None,
            links_job: None,
            events_key: None,
            visible_key: None,
            pending_resource: None,
            error: None,
            hour_scroll: 7,
        }
    }

    pub fn palette_input_available(&self) -> bool {
        self.dialog.is_none()
    }

    pub fn select_resource(&mut self, app: &mut App, id: &str) {
        self.pending_resource = Some(id.to_owned());
        self.refresh(app);
        if let Some(index) = self
            .visible
            .iter()
            .position(|index| self.events[*index].resource == id)
        {
            self.selected = index;
        }
    }

    pub(crate) fn render_pending(&self) -> bool {
        self.links_job.is_some()
    }

    pub(crate) fn suspend(&mut self) {
        self.links_job = None;
    }

    pub(crate) fn refresh(&mut self, app: &mut App) -> bool {
        let mut changed = false;
        let from = week_start(month_start(self.date), app.list.week_start);
        let until = from
            .checked_add_signed(Duration::days(75))
            .unwrap_or(self.date);
        let key = (from, until, app.calendar.revision());
        if self.events_key != Some(key) {
            match app
                .calendar
                .events_async(&app.storage, from, until, &app.notes_worker_pool)
            {
                Ok(Some((events, warnings))) => {
                    self.events = events.to_vec();
                    if !warnings.is_empty() {
                        self.error = Some(warnings.join("; "));
                    }
                    self.events_key = Some(key);
                    self.visible_key = None;
                    changed = true;
                }
                Ok(None) => {
                    if !self.events.is_empty() {
                        self.events.clear();
                        self.visible.clear();
                        self.rows.clear();
                        self.visible_key = None;
                        changed = true;
                    }
                }
                Err(error) => {
                    self.error = Some(format!("{error:#}"));
                    self.events.clear();
                    self.visible.clear();
                    self.rows.clear();
                    self.events_key = Some(key);
                    changed = true;
                }
            }
        }
        if self.visible_key != Some((self.date, self.display)) {
            self.visible = self
                .events
                .iter()
                .enumerate()
                .filter(|(_, event)| {
                    if self.display == Display::Agenda {
                        event.start.date() >= self.date
                            && event.start.date()
                                < self
                                    .date
                                    .checked_add_signed(Duration::days(30))
                                    .unwrap_or(NaiveDate::MAX)
                    } else {
                        event.on_date(self.date)
                    }
                })
                .map(|(i, _)| i)
                .collect();
            self.selected = self.selected.min(self.visible.len().saturating_sub(1));
            self.visible_key = Some((self.date, self.display));
        }
        if let Some(id) = &self.pending_resource
            && self.events_key == Some(key)
        {
            if let Some(index) = self
                .visible
                .iter()
                .position(|index| self.events[*index].resource == *id)
            {
                self.selected = index;
            }
            self.pending_resource = None;
        }
        if self.links_revision != Some(app.notes_revision) {
            if self
                .links_job
                .as_ref()
                .is_none_or(|job| job.revision != app.notes_revision)
            {
                let (sender, receiver) = std::sync::mpsc::sync_channel(1);
                let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let token = cancel.clone();
                let storage = app.storage.clone();
                self.links_job = Some(LinksJob {
                    revision: app.notes_revision,
                    cancel,
                    receiver,
                });
                app.notes_worker_pool.spawn(move || {
                    let result = store::note_links_cancelled(&storage, &token);
                    if !token.load(std::sync::atomic::Ordering::Relaxed) {
                        let _ = sender.send(result);
                    }
                });
            }
            if let Some(result) =
                self.links_job
                    .as_ref()
                    .and_then(|job| match job.receiver.try_recv() {
                        Ok(result) => Some(result),
                        Err(std::sync::mpsc::TryRecvError::Empty) => None,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            Some(Err(anyhow::anyhow!("Calendar note-link worker stopped")))
                        }
                    })
            {
                match result {
                    Ok(links) => self.links = links,
                    Err(error) => self.error = Some(format!("Cannot load note links: {error:#}")),
                }
                self.links_revision = Some(app.notes_revision);
                self.links_job = None;
                changed = true;
            }
        }
        changed
    }

    fn selected_event(&self) -> Option<&Occurrence> {
        self.visible
            .get(self.selected)
            .and_then(|i| self.events.get(*i))
    }

    fn editable(app: &App, resource: &Resource) -> Result<()> {
        ensure!(!app.calendar.busy(), "Calendar sync in progress");
        ensure!(
            resource.conflict.is_none(),
            "Resolve this event's sync conflict first (c)"
        );
        if let Some(url) = &resource.collection {
            ensure!(
                app.calendar.data().collections.iter().any(|c| &c.url == url
                    && c.writable
                    && app
                        .config
                        .calendar
                        .caldav
                        .iter()
                        .any(|account| account.url == c.account)),
                "Calendar is read-only or no longer configured"
            );
        }
        Ok(())
    }

    pub fn begin_form(&mut self, app: &App, id: Option<&str>, note: Option<String>) -> Result<()> {
        let resource = id.and_then(|id| app.calendar.data().resources.iter().find(|r| r.id == id));
        if let Some(resource) = resource {
            Self::editable(app, resource)?;
        }
        let mut draft = resource
            .map(|r| Draft::from_source(&r.source))
            .transpose()?
            .unwrap_or_else(|| Draft::new(self.date));
        if let Some(note) = &note
            && let Some(summary) = app.notes.iter().find(|n| &n.id == note)
        {
            draft.title.clone_from(&summary.title);
        }
        let values = [
            draft.title,
            draft.start,
            draft.end,
            draft.location,
            draft.description,
            draft.repeat,
        ];
        let inputs = values
            .into_iter()
            .map(|value| {
                let mut input = crate::ui::make_popup_textarea(&app.app_theme, "");
                input.insert_str(&value);
                input.move_cursor(ratatui_textarea::CursorMove::Head);
                input
            })
            .collect();
        let mut choices = vec![(None, "Local".into())];
        choices.extend(
            app.calendar
                .data()
                .collections
                .iter()
                .filter(|c| {
                    c.writable
                        && app
                            .config
                            .calendar
                            .caldav
                            .iter()
                            .any(|account| account.url == c.account)
                })
                .map(|c| (Some(c.url.clone()), c.name.clone())),
        );
        let collection = resource
            .and_then(|r| choices.iter().position(|(url, _)| *url == r.collection))
            .unwrap_or(0);
        self.dialog = Some(Dialog::Form(Box::new(Form {
            id: id.map(str::to_owned),
            inputs,
            focus: 0,
            collection,
            choices,
            note,
            rects: Vec::new(),
        })));
        self.error = None;
        Ok(())
    }

    pub fn begin_path(&mut self, app: &App, import: bool) {
        let mut input = crate::ui::make_popup_textarea(&app.app_theme, "Path to .ics file");
        if !import {
            input.insert_str(
                app.storage
                    .notes_dir
                    .join(format!(
                        "calendar-{}.ics",
                        Local::now().format("%Y%m%d-%H%M%S")
                    ))
                    .to_string_lossy(),
            );
        }
        self.dialog = Some(Dialog::Path { import, input });
        self.error = None;
    }

    fn begin_notes(&mut self, app: &App, open: bool) {
        let resource = self.selected_event().map(|e| e.resource.clone());
        if open && resource.is_none() {
            return;
        }
        let mut popup = crate::ui::quick_search::QuickSearch::new(
            if open {
                "Open linked note"
            } else {
                "Link note / schedule note"
            },
            &app.app_theme,
        );
        self.filter_notes(app, &mut popup, resource.as_deref(), open);
        self.dialog = Some(Dialog::Notes {
            popup,
            resource,
            open,
        });
    }

    fn filter_notes(
        &self,
        app: &App,
        popup: &mut crate::ui::quick_search::QuickSearch<(String, String)>,
        resource: Option<&str>,
        open: bool,
    ) {
        let query = popup.query().to_lowercase();
        let linked = resource.and_then(|id| self.links.get(id));
        popup.results = app
            .notes
            .iter()
            .filter(|n| {
                n.id.ends_with(".md")
                    && (!open || linked.is_some_and(|ids| ids.contains(&n.id)))
                    && format!("{} {}", n.title, n.id)
                        .to_lowercase()
                        .contains(&query)
            })
            .map(|n| (n.id.clone(), n.title.clone()))
            .collect();
        popup.results.sort_by(|a, b| a.0.cmp(&b.0));
        popup.selected = popup.selected.min(popup.results.len().saturating_sub(1));
    }

    fn save_form(&mut self, app: &mut App, form: &mut Form) -> Result<()> {
        let values: Vec<_> = form.inputs.iter().map(|i| i.lines().join("\n")).collect();
        let draft = Draft {
            title: values[0].clone(),
            start: values[1].clone(),
            end: values[2].clone(),
            location: values[3].clone(),
            description: values[4].clone(),
            repeat: values[5].clone(),
        };
        if let Some(id) = &form.id {
            let resource = app
                .calendar
                .data()
                .resources
                .iter()
                .find(|r| &r.id == id)
                .context("Event no longer exists")?;
            Self::editable(app, resource)?;
        }
        let id = app.calendar.update(&app.storage, |data| {
            if let Some(id) = &form.id {
                let resource = data
                    .resources
                    .iter_mut()
                    .find(|r| &r.id == id)
                    .context("Event no longer exists")?;
                resource.source = draft.build(Some(&resource.source))?;
                Ok(id.clone())
            } else {
                let resource =
                    Resource::new(draft.build(None)?, form.choices[form.collection].0.clone());
                let id = resource.id.clone();
                data.resources.push(resource);
                Ok(id)
            }
        })?;
        form.id = Some(id.clone());
        if let Some(note) = &form.note {
            store::toggle_note_link(&app.storage, note, &id)
                .context("Event saved; could not link note")?;
            app.refresh_note_single(None, note);
            form.note = None;
            self.links_revision = None;
        }
        Ok(())
    }

    fn delete(&mut self, app: &mut App, id: &str) -> Result<()> {
        let resource = app
            .calendar
            .data()
            .resources
            .iter()
            .find(|r| r.id == id)
            .context("Event no longer exists")?;
        Self::editable(app, resource)?;
        app.calendar.update(&app.storage, |data| {
            if let Some(resource) = data.resources.iter_mut().find(|r| r.id == id) {
                if resource.remote.is_some() {
                    resource.deleted = true;
                } else {
                    data.resources.retain(|r| r.id != id);
                }
            }
            Ok(())
        })
    }

    fn resolve_conflict(&mut self, app: &mut App, id: &str, keep_local: bool) -> Result<()> {
        app.calendar.update(&app.storage, |data| {
            let resource = data
                .resources
                .iter_mut()
                .find(|r| r.id == id)
                .context("Event no longer exists")?;
            let conflict = resource
                .conflict
                .take()
                .context("No conflict for this event")?;
            if keep_local {
                resource.remote = conflict.remote;
                if resource.deleted && resource.remote.is_none() {
                    data.resources.retain(|r| r.id != id);
                }
            } else if let Some(remote) = conflict.remote {
                resource.source.clone_from(&remote.source);
                resource.remote = Some(remote);
                resource.deleted = false;
            } else {
                data.resources.retain(|r| r.id != id);
            }
            Ok(())
        })
    }

    fn dialog_event(
        &mut self,
        app: &mut App,
        event: Event,
        dialog: &mut Dialog,
    ) -> Result<(OverlayResult, bool)> {
        let mut close = false;
        let mut result = OverlayResult::Continue;
        if let Event::Key(key) = &event
            && key.code == KeyCode::Esc
        {
            self.error = None;
            return Ok((result, true));
        }
        match dialog {
            Dialog::Form(form) => match event {
                Event::Key(key)
                    if key.code == KeyCode::Char('s')
                        && key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.save_form(app, form)?;
                    close = true;
                }
                Event::Key(key) => match key.code {
                    KeyCode::Tab => form.focus = (form.focus + 1) % 7,
                    KeyCode::BackTab => form.focus = (form.focus + 6) % 7,
                    KeyCode::Enter if form.focus == 6 => {
                        self.save_form(app, form)?;
                        close = true;
                    }
                    KeyCode::Enter => form.focus += 1,
                    KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
                        if form.focus == 6 && form.id.is_none() =>
                    {
                        let len = form.choices.len();
                        form.collection = if key.code == KeyCode::Left {
                            (form.collection + len - 1) % len
                        } else {
                            (form.collection + 1) % len
                        };
                    }
                    _ if form.focus < 6 => {
                        crate::events::handle_popup_text_input(
                            key,
                            &mut form.inputs[form.focus],
                            &app.keybinds,
                        );
                    }
                    _ => {}
                },
                Event::Paste(text) if form.focus < 6 => {
                    form.inputs[form.focus].insert_str(&text);
                }
                Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                    if let Some((_, index)) = form
                        .rects
                        .iter()
                        .find(|(rect, _)| rect.contains(Position::new(mouse.column, mouse.row)))
                    {
                        form.focus = *index;
                    }
                }
                _ => {}
            },
            Dialog::Path { import, input } => match event {
                Event::Key(key) if key.code == KeyCode::Enter => {
                    let path = crate::config::expand_path(input.lines().join("").trim());
                    let count = if *import {
                        app.calendar.import(&app.storage, &path)?
                    } else {
                        app.calendar.export(&app.storage, &path)?
                    };
                    app.set_temporary_status(&format!(
                        "{} {count} event series",
                        if *import { "Imported" } else { "Exported" }
                    ));
                    close = true;
                }
                Event::Key(key) => {
                    crate::events::handle_popup_text_input(key, input, &app.keybinds);
                }
                Event::Paste(text) => {
                    input.insert_str(text.replace(['\r', '\n'], ""));
                }
                _ => {}
            },
            Dialog::Notes {
                popup,
                resource,
                open,
            } => {
                if let Event::Key(key) = event {
                    use crate::ui::quick_search::QuickSearchAction;
                    match crate::ui::quick_search::handle_quick_search_keys(
                        popup,
                        key,
                        &app.keybinds,
                        12,
                    ) {
                        QuickSearchAction::Cancel => close = true,
                        QuickSearchAction::Submit => {
                            if let Some((id, _)) = popup.results.get(popup.selected).cloned() {
                                if *open {
                                    result = OverlayResult::NoteOpened(id);
                                } else if let Some(resource) = resource {
                                    store::toggle_note_link(&app.storage, &id, resource)?;
                                    app.refresh_note_single(None, &id);
                                    self.links_revision = None;
                                } else {
                                    self.begin_form(app, None, Some(id))?;
                                }
                                close = true;
                            }
                        }
                        QuickSearchAction::Edited => {
                            self.filter_notes(app, popup, resource.as_deref(), *open);
                            popup.scroll_offset = 0;
                        }
                        _ => {}
                    }
                } else if let Event::Paste(text) = event {
                    popup.input.insert_str(text.replace(['\r', '\n'], ""));
                    self.filter_notes(app, popup, resource.as_deref(), *open);
                }
            }
            Dialog::Delete(id) => {
                if let Event::Key(key) = event
                    && key.code == KeyCode::Char('y')
                {
                    self.delete(app, id)?;
                    close = true;
                }
            }
            Dialog::Conflict(id) => {
                if let Event::Key(key) = event {
                    match key.code {
                        KeyCode::Char('l') => {
                            self.resolve_conflict(app, id, true)?;
                            close = true;
                        }
                        KeyCode::Char('r') => {
                            self.resolve_conflict(app, id, false)?;
                            close = true;
                        }
                        _ => {}
                    }
                }
            }
            Dialog::Help => {
                if matches!(event, Event::Key(_)) {
                    close = true;
                }
            }
        }
        if close {
            self.error = None;
        }
        Ok((result, close))
    }

    fn key(&mut self, app: &mut App, key: KeyEvent) -> Result<OverlayResult> {
        let mut days = 0;
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(OverlayResult::Exit),
            KeyCode::Char('m') => self.display = Display::Month,
            KeyCode::Char('w') => self.display = Display::Week,
            KeyCode::Char('d') => self.display = Display::Day,
            KeyCode::Char('a') => self.display = Display::Agenda,
            KeyCode::Tab => self.display = self.display.next(),
            KeyCode::Left | KeyCode::Char('h') => days = -1,
            KeyCode::Right | KeyCode::Char('l') => days = 1,
            KeyCode::Up | KeyCode::Char('k')
                if matches!(self.display, Display::Month | Display::Week) =>
            {
                days = -7
            }
            KeyCode::Down | KeyCode::Char('j')
                if matches!(self.display, Display::Month | Display::Week) =>
            {
                days = 7
            }
            KeyCode::Up | KeyCode::Char('k' | 'K') => {
                self.selected = self.selected.saturating_sub(1)
            }
            KeyCode::Down | KeyCode::Char('j' | 'J') => {
                self.selected = (self.selected + 1).min(self.visible.len().saturating_sub(1))
            }
            KeyCode::PageUp | KeyCode::PageDown => {
                let step = if key.code == KeyCode::PageUp { -1 } else { 1 };
                if self.display == Display::Month {
                    self.date = shift_month(self.date, step);
                } else {
                    days = i64::from(step)
                        * match self.display {
                            Display::Week => 7,
                            Display::Agenda => 30,
                            _ => 1,
                        };
                }
            }
            KeyCode::Char('t') => {
                self.date = Local::now().date_naive();
                self.selected = 0;
            }
            KeyCode::Char('n') => self.begin_form(app, None, None)?,
            KeyCode::Char('e') => {
                if let Some(id) = self.selected_event().map(|e| e.resource.clone()) {
                    self.begin_form(app, Some(&id), None)?;
                }
            }
            KeyCode::Delete | KeyCode::Char('x') => {
                if let Some(id) = self.selected_event().map(|e| e.resource.clone()) {
                    self.dialog = Some(Dialog::Delete(id));
                }
            }
            KeyCode::Char('L') => self.begin_notes(app, false),
            KeyCode::Char('o') | KeyCode::Enter => self.begin_notes(app, true),
            KeyCode::Char('y') => {
                let target = self.selected_event().map_or_else(
                    || self.date.to_string(),
                    |event| {
                        format!(
                            "{}@{}",
                            event.resource,
                            if event.on_date(self.date) {
                                self.date
                            } else {
                                event.start.date()
                            }
                        )
                    },
                );
                let result =
                    crate::text_edit::write_system_clipboard(&format!("[[calendar:{target}]]"));
                app.set_temporary_status(crate::text_edit::clipboard_notice(result));
            }
            KeyCode::Char('i') => self.begin_path(app, true),
            KeyCode::Char('E') => self.begin_path(app, false),
            KeyCode::Char('s') => app
                .calendar
                .sync(&app.storage, app.config.calendar.caldav.clone())?,
            KeyCode::Char('r') => {
                ensure!(!app.calendar.busy(), "Sync in progress");
                app.calendar.reload(&app.storage)?;
                self.links_revision = None;
                self.error = None;
            }
            KeyCode::Char('c') => {
                let id = self
                    .selected_event()
                    .map(|e| e.resource.clone())
                    .filter(|id| {
                        app.calendar
                            .data()
                            .resources
                            .iter()
                            .any(|r| &r.id == id && r.conflict.is_some())
                    })
                    .or_else(|| {
                        app.calendar
                            .data()
                            .resources
                            .iter()
                            .find(|r| r.conflict.is_some())
                            .map(|r| r.id.clone())
                    });
                if let Some(id) = id {
                    self.dialog = Some(Dialog::Conflict(id));
                }
            }
            KeyCode::Char('?') | KeyCode::F(1) => self.dialog = Some(Dialog::Help),
            _ => {}
        }
        if days != 0 {
            self.date = self
                .date
                .checked_add_signed(Duration::days(days))
                .unwrap_or(self.date);
            self.selected = 0;
        }
        Ok(OverlayResult::Continue)
    }
}

impl OverlayView for View {
    fn overlay_render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        self.refresh(app);
        self.render(frame, area, app);
    }

    fn overlay_handle_event(
        &mut self,
        event: Event,
        app: &mut App,
        _area: Rect,
    ) -> Result<OverlayResult> {
        if matches!(&event, Event::Key(key) if key.kind == KeyEventKind::Release) {
            return Ok(OverlayResult::Continue);
        }
        self.refresh(app);
        if let Some(mut dialog) = self.dialog.take() {
            return match self.dialog_event(app, event, &mut dialog) {
                Ok((result, close)) => {
                    if !close {
                        self.dialog = Some(dialog);
                    }
                    Ok(result)
                }
                Err(error) => {
                    self.dialog = Some(dialog);
                    self.error = Some(format!("{error:#}"));
                    Ok(OverlayResult::Continue)
                }
            };
        }
        let result = match event {
            Event::Key(key) => self.key(app, key),
            Event::Mouse(mouse) => {
                if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                    let position = Position::new(mouse.column, mouse.row);
                    if (app.status.trim().is_empty() || app.status == "Ready")
                        && let Some((_, index)) = app
                            .header_tabs
                            .iter()
                            .find(|(rect, _)| rect.contains(position))
                        && let Some(display) =
                            [Display::Month, Display::Week, Display::Day, Display::Agenda]
                                .get(*index)
                    {
                        self.display = *display;
                    } else if let Some((_, date)) =
                        self.cells.iter().find(|(rect, _)| rect.contains(position))
                    {
                        self.date = *date;
                        self.selected = 0;
                    } else if let Some((_, index)) =
                        self.rows.iter().find(|(rect, _)| rect.contains(position))
                    {
                        self.selected = *index;
                    }
                } else if mouse.kind == MouseEventKind::ScrollDown {
                    if self.display == Display::Day {
                        self.hour_scroll = (self.hour_scroll + 1).min(23);
                    } else {
                        self.selected =
                            (self.selected + 1).min(self.visible.len().saturating_sub(1));
                    }
                } else if mouse.kind == MouseEventKind::ScrollUp {
                    if self.display == Display::Day {
                        self.hour_scroll = self.hour_scroll.saturating_sub(1);
                    } else {
                        self.selected = self.selected.saturating_sub(1);
                    }
                }
                Ok(OverlayResult::Continue)
            }
            _ => Ok(OverlayResult::Continue),
        };
        match result {
            Ok(result) => Ok(result),
            Err(error) => {
                self.error = Some(format!("{error:#}"));
                Ok(OverlayResult::Continue)
            }
        }
    }
}

fn event_time(e: &Occurrence) -> String {
    if e.all_day {
        "All day".into()
    } else {
        e.start.format("%H:%M").to_string()
    }
}

impl View {
    pub(crate) fn footer_detail(&self, app: &App) -> (String, bool) {
        if let Some(error) = &self.error {
            return (
                crate::fsutil::sanitize_for_terminal(error).into_owned(),
                true,
            );
        }
        let conflicts = app
            .calendar
            .data()
            .resources
            .iter()
            .filter(|r| r.conflict.is_some())
            .count();
        let mut details = Vec::new();
        if conflicts > 0 {
            details.push(format!("{conflicts} sync conflict(s): c to resolve"));
        }
        if app.calendar.busy() {
            details.push("Syncing…".into());
        }
        (details.join(" | "), conflicts > 0)
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let theme = &app.app_theme;
        let [header, content] = crate::ui::chrome::content_rows(area);
        frame.render_widget(Block::default().style(theme.bg_style()), content);
        self.cells.clear();
        self.rows.clear();
        let mut ctx = crate::statusline::StatuslineContext::for_view(app, ViewMode::Calendar);
        ctx.calendar = Some(self);
        ctx.area = Some(header);
        let (left, right) = crate::statusline::render_header(
            &ctx,
            &app.config.statusline,
            ViewMode::Calendar,
            theme,
        );
        let displays = [Display::Month, Display::Week, Display::Day, Display::Agenda];
        let tabs: Vec<_> = displays
            .iter()
            .map(|display| (display.label(), None))
            .collect();
        let active = displays
            .iter()
            .position(|display| *display == self.display)
            .unwrap_or(0);
        app.header_tabs = crate::ui::draw_header_tabs(
            frame,
            header,
            theme,
            left,
            crate::ui::build_tab_spans(&tabs, active, None, theme, false, app.config.ui.icon_mode),
            active,
            right,
            Some(app.status.as_ref()),
            app.load_spinner_tick,
            app.mouse_pos,
        );
        if area.width < 28 || content.height < 5 {
            frame.render_widget(
                Paragraph::new("Calendar needs 28 columns × 7 rows; Esc returns.")
                    .style(theme.bg_style()),
                content,
            );
        } else {
            let (body, divider) = if area.width >= 76 {
                let [main, divider, sidebar] = Layout::horizontal([
                    Constraint::Percentage(65),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ])
                .areas(content);
                ([main, sidebar], Some(divider))
            } else {
                (
                    Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)])
                        .areas(content),
                    None,
                )
            };
            match self.display {
                Display::Month => self.render_month(frame, body[0], app),
                Display::Week => self.render_week(frame, body[0], app),
                Display::Day => self.render_day(frame, body[0], app),
                Display::Agenda => self.render_agenda(frame, body[0], theme, true, false),
            }
            self.render_inspector(frame, body[1], app);
            if let Some(divider) = divider {
                crate::ui::draw_dim_vline(frame, divider, theme.muted);
            }
        }
        if let Some(dialog) = &mut self.dialog {
            render_dialog(frame, area, dialog, self.error.as_deref(), &self.links, app);
        }
    }

    fn render_month(&mut self, frame: &mut Frame, area: Rect, app: &App) {
        let theme = &app.app_theme;
        let inner = calendar_pane(
            frame,
            area,
            &self.date.format("%B %Y").to_string(),
            theme,
            theme.bg_style(),
        );
        if inner.height < 2 {
            return;
        }
        weekday_header(
            frame,
            Rect::new(inner.x, inner.y, inner.width, 1),
            app.list.week_start,
            theme,
        );
        let grid = Rect::new(inner.x, inner.y + 1, inner.width, inner.height - 1);
        self.cells = month_cells(grid, self.date, app.list.week_start);
        for (index, (rect, date)) in self.cells.iter().enumerate() {
            let items: Vec<_> = self.events.iter().filter(|e| e.on_date(*date)).collect();
            let mut lines = vec![Line::from(format!(
                "{:>2}{}",
                date.day(),
                if items.is_empty() {
                    "".into()
                } else {
                    format!(" ·{}", items.len())
                }
            ))];
            let capacity = rect.height.saturating_sub(1) as usize;
            for e in items.iter().take(capacity) {
                lines.push(Line::from(format!(
                    "{} {}",
                    if e.all_day {
                        "·".into()
                    } else {
                        e.start.format("%H:%M").to_string()
                    },
                    e.title
                )));
            }
            let current_month =
                date.month() == self.date.month() && date.year() == self.date.year();
            let mut style = if current_month {
                calendar_section_style(theme, (index / 7 + index % 7) % 2 != 0, false)
                    .fg(theme.text)
            } else {
                let mut style = theme.preview_bg_style().fg(theme.muted);
                if let Some(bg) = theme.bg
                    && theme.preview_bg() == Some(bg)
                {
                    style = style.bg(crate::app_theme::mix_colors(bg, theme.muted, 0.12));
                }
                style
            };
            if *date == Local::now().date_naive() {
                style = style.fg(theme.success).add_modifier(Modifier::BOLD);
            }
            if *date == self.date {
                style = style.bg(theme.highlight_bg).fg(theme.highlight_fg);
            }
            frame.render_widget(Block::default().style(style), *rect);
            frame.render_widget(
                Paragraph::new(lines).style(style),
                Rect {
                    width: rect.width.saturating_sub(1),
                    ..*rect
                },
            );
        }
    }

    fn render_week(&mut self, frame: &mut Frame, area: Rect, app: &App) {
        let start = week_start(self.date, app.list.week_start);
        for col in 0..7u16 {
            let x = u32::from(area.width) * u32::from(col) / 7;
            let end = u32::from(area.width) * u32::from(col + 1) / 7;
            let rect = Rect::new(area.x + x as u16, area.y, (end - x) as u16, area.height);
            let date = start
                .checked_add_signed(Duration::days(i64::from(col)))
                .unwrap_or(start);
            self.cells.push((rect, date));
            let inner = calendar_pane(
                frame,
                rect,
                &date.format("%a %d").to_string(),
                &app.app_theme,
                calendar_section_style(&app.app_theme, col % 2 != 0, false),
            );
            if date == self.date {
                frame.render_widget(
                    Paragraph::new(date.format("%a %d").to_string()).style(
                        Style::default()
                            .fg(app.app_theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Rect::new(
                        rect.x + u16::from(rect.width > 0),
                        rect.y,
                        rect.width.saturating_sub(2),
                        rect.height.min(1),
                    ),
                );
            }
            let lines: Vec<_> = self
                .events
                .iter()
                .filter(|e| e.on_date(date))
                .flat_map(|e| {
                    [
                        Line::from(Span::styled(
                            event_time(e),
                            Style::default().fg(app.app_theme.accent),
                        )),
                        Line::from(e.title.clone()),
                        Line::from(""),
                    ]
                })
                .collect();
            frame.render_widget(
                Paragraph::new(lines).style(
                    calendar_section_style(&app.app_theme, col % 2 != 0, false)
                        .fg(app.app_theme.text),
                ),
                inner,
            );
        }
    }

    fn render_day(&mut self, frame: &mut Frame, area: Rect, app: &App) {
        let inner = calendar_pane(
            frame,
            area,
            &format!("{} · local time", self.date.format("%a %d %b")),
            &app.app_theme,
            app.app_theme.bg_style(),
        );
        let all_day = self
            .events
            .iter()
            .filter(|e| e.on_date(self.date) && e.all_day)
            .map(|e| e.title.as_str())
            .collect::<Vec<_>>()
            .join(" · ");
        let mut lines = vec![Line::from(format!("All day  {all_day}"))];
        for hour in self.hour_scroll..24 {
            let names = self
                .events
                .iter()
                .filter(|e| {
                    !e.all_day
                        && e.on_date(self.date)
                        && (e.start.date() < self.date && hour == 0
                            || e.start.date() == self.date && e.start.hour() == hour)
                })
                .map(|e| format!("{} {}", e.start.format("%H:%M"), e.title))
                .collect::<Vec<_>>()
                .join(" · ");
            lines.push(Line::from(format!("{hour:02}:00  {names}")));
        }
        for (index, line) in lines.into_iter().take(inner.height as usize).enumerate() {
            frame.render_widget(
                Paragraph::new(line).style(
                    calendar_section_style(&app.app_theme, index % 2 != 0, false)
                        .fg(app.app_theme.text),
                ),
                Rect::new(inner.x, inner.y + index as u16, inner.width, 1),
            );
        }
    }

    fn render_agenda(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        theme: &AppThemeColors,
        dates: bool,
        sidebar: bool,
    ) {
        let title = if dates {
            "Agenda · next 30 days".to_owned()
        } else {
            self.date.format("%a %d %b").to_string()
        };
        let inner = calendar_pane(
            frame,
            area,
            &title,
            theme,
            calendar_section_style(theme, false, sidebar),
        );
        let height = inner.height as usize;
        let offset = self.selected.saturating_sub(height.saturating_sub(1));
        for (row, (index, event)) in self
            .visible
            .iter()
            .enumerate()
            .skip(offset)
            .take(height)
            .map(|(i, index)| (i - offset, (i, &self.events[*index])))
        {
            let prefix = if dates {
                format!("{} ", event.start.format("%m-%d"))
            } else {
                String::new()
            };
            let rect = Rect::new(inner.x, inner.y + row as u16, inner.width, 1);
            frame.render_widget(
                Paragraph::new(format!(
                    "{prefix}{} {}{}",
                    event_time(event),
                    event.title,
                    if self.links.contains_key(&event.resource) {
                        " [notes]"
                    } else {
                        ""
                    }
                ))
                .style(if index == self.selected {
                    Style::default()
                        .bg(theme.highlight_bg)
                        .fg(theme.highlight_fg)
                } else {
                    calendar_section_style(theme, index % 2 != 0, sidebar).fg(theme.text)
                }),
                rect,
            );
            self.rows.push((rect, index));
        }
        if self.visible.is_empty() {
            frame.render_widget(
                Paragraph::new("No events. n: new · l: schedule note")
                    .style(Style::default().fg(theme.muted)),
                inner,
            );
        }
    }

    fn render_inspector(&mut self, frame: &mut Frame, area: Rect, app: &App) {
        let chunks =
            Layout::vertical([Constraint::Percentage(42), Constraint::Percentage(58)]).split(area);
        self.render_agenda(
            frame,
            chunks[0],
            &app.app_theme,
            self.display == Display::Agenda,
            true,
        );
        let detail_style = calendar_section_style(&app.app_theme, true, true);
        let inner = calendar_pane(
            frame,
            chunks[1],
            "Event / notes",
            &app.app_theme,
            detail_style,
        );
        let lines = if let Some(event) = self.selected_event() {
            let resource = app
                .calendar
                .data()
                .resources
                .iter()
                .find(|r| r.id == event.resource);
            let calendar = resource
                .and_then(|r| r.collection.as_ref())
                .and_then(|url| {
                    app.calendar
                        .data()
                        .collections
                        .iter()
                        .find(|c| &c.url == url)
                })
                .map_or("Local", |c| c.name.as_str());
            let mut lines = vec![
                Line::from(Span::styled(
                    event.title.clone(),
                    Style::default()
                        .fg(app.app_theme.accent)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(calendar.to_owned()),
                Line::from(if event.all_day {
                    format!("{} to {} (exclusive)", event.start.date(), event.end.date())
                } else {
                    format!(
                        "{} to {}",
                        event.start.format("%Y-%m-%d %H:%M"),
                        event.end.format("%Y-%m-%d %H:%M")
                    )
                }),
            ];
            if event.recurring {
                lines.push(Line::from("Recurring series · e edits whole series"));
            }
            if resource.is_some_and(|r| r.conflict.is_some()) {
                lines.push(Line::from(Span::styled(
                    "Sync conflict · c to resolve",
                    Style::default().fg(app.app_theme.warning),
                )));
            }
            if !event.location.is_empty() {
                lines.push(Line::from(format!("Location: {}", event.location)));
            }
            if !event.description.is_empty() {
                lines.push(Line::from(event.description.clone()));
            }
            lines.push(Line::from(""));
            lines.push(Line::from("Linked notes (l: link/unlink · o: open):"));
            for note in self.links.get(&event.resource).into_iter().flatten() {
                lines.push(Line::from(note.clone()));
            }
            lines
        } else {
            vec![
                Line::from("Select a date or event."),
                Line::from("n: create event · L: schedule note"),
            ]
        };
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .style(detail_style.fg(app.app_theme.text)),
            inner,
        );
    }
}

fn calendar_section_style(theme: &AppThemeColors, alternate: bool, sidebar: bool) -> Style {
    let base = if sidebar {
        theme.preview_bg()
    } else {
        theme.bg
    };
    base.map_or_else(Style::default, |bg| {
        Style::default().bg(if alternate {
            crate::app_theme::mix_colors(bg, theme.text, 0.04)
        } else {
            bg
        })
    })
}

fn calendar_pane(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    theme: &AppThemeColors,
    style: Style,
) -> Rect {
    let block = Block::default()
        .style(style)
        .padding(Padding::new(1, 1, 1, 0));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(title.to_owned()).style(style.fg(theme.muted).add_modifier(Modifier::BOLD)),
        Rect::new(
            area.x + u16::from(area.width > 0),
            area.y,
            area.width.saturating_sub(2),
            area.height.min(1),
        ),
    );
    inner
}

fn weekday_header(frame: &mut Frame, area: Rect, start: WeekStart, theme: &AppThemeColors) {
    let days = match start {
        WeekStart::Sunday => ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"],
        WeekStart::Monday => ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"],
    };
    for (col, day) in days.iter().enumerate() {
        let x = u32::from(area.width) * col as u32 / 7;
        let end = u32::from(area.width) * (col + 1) as u32 / 7;
        frame.render_widget(
            Paragraph::new(*day).style(Style::default().fg(theme.muted)),
            Rect::new(area.x + x as u16, area.y, (end - x) as u16, area.height),
        );
    }
}

fn render_dialog(
    frame: &mut Frame,
    area: Rect,
    dialog: &mut Dialog,
    error: Option<&str>,
    links: &BTreeMap<String, Vec<String>>,
    app: &App,
) {
    if let Dialog::Notes {
        popup, resource, ..
    } = dialog
    {
        let links = resource
            .as_ref()
            .and_then(|id| links.get(id))
            .cloned()
            .unwrap_or_default();
        crate::ui::quick_search::draw_quick_search(
            frame,
            area,
            popup,
            &app.app_theme,
            12,
            |(id, title), _, _| {
                Line::from(format!(
                    "{} {title} · {id}",
                    if links.contains(id) {
                        "[linked]"
                    } else {
                        "[ ]"
                    }
                ))
            },
            app.config.ui.icon_mode,
        );
        return;
    }
    let size = match dialog {
        Dialog::Form(_) | Dialog::Help => crate::ui::PopupSize::Large,
        _ => crate::ui::PopupSize::Medium,
    };
    let rect = crate::ui::centered_rect(size, area);
    frame.render_widget(Clear, rect);
    let title = match dialog {
        Dialog::Form(form) => {
            if form.id.is_some() {
                "Edit event series"
            } else {
                "New event"
            }
        }
        Dialog::Path { import, .. } => {
            if *import {
                "Import iCalendar"
            } else {
                "Export iCalendar"
            }
        }
        Dialog::Delete(_) => "Delete event series?",
        Dialog::Conflict(_) => "Resolve CalDAV conflict",
        Dialog::Help => "Calendar help",
        Dialog::Notes { .. } => unreachable!(),
    };
    let block = Block::bordered()
        .title(format!(" {title} "))
        .style(app.app_theme.bg_style())
        .border_style(Style::default().fg(app.app_theme.accent));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    let parts = Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).split(inner);
    let footer = match dialog {
        Dialog::Form(_) => "Tab: next field · Ctrl+S: save · Esc: cancel",
        Dialog::Path { .. } => "Enter: confirm · Esc: cancel",
        Dialog::Delete(_) => "y: delete entire series · Esc: cancel",
        Dialog::Conflict(_) => "l: keep local · r: use remote · Esc: cancel",
        _ => "Any key: close",
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(error.unwrap_or(footer)),
            Line::from(if error.is_some() { footer } else { "" }),
        ])
        .style(Style::default().fg(if error.is_some() {
            app.app_theme.warning
        } else {
            app.app_theme.muted
        })),
        parts[1],
    );
    match dialog {
        Dialog::Form(form) => {
            let labels = ["Title", "Start (date or local date-time)", "End (all-day end exclusive)", "Location", "Description", "Repeat (RRULE, blank = once)", "Calendar (Left/Right; Enter saves)"];
            form.rects.clear();
            let capacity = (parts[0].height / 2).max(1) as usize;
            let offset = form.focus.saturating_sub(capacity.saturating_sub(1));
            for (row, index) in (offset..7).take(capacity).enumerate() {
                let rect = Rect::new(parts[0].x, parts[0].y + row as u16 * 2, parts[0].width, 2.min(parts[0].bottom().saturating_sub(parts[0].y + row as u16 * 2)));
                form.rects.push((rect, index));
                frame.render_widget(Paragraph::new(labels[index]).style(Style::default().fg(if form.focus == index { app.app_theme.accent } else { app.app_theme.muted })), Rect::new(rect.x, rect.y, rect.width, 1));
                let input_rect = Rect::new(rect.x, rect.y + 1, rect.width, rect.height.saturating_sub(1));
                if index < 6 {
                    let input = &mut form.inputs[index];
                    input.set_style(app.app_theme.bg_style());
                    input.set_cursor_style(if form.focus == index { Style::default().add_modifier(Modifier::REVERSED) } else { app.app_theme.bg_style() });
                    frame.render_widget(&*input, input_rect);
                } else { frame.render_widget(Paragraph::new(form.choices[form.collection].1.clone()).style(Style::default().fg(app.app_theme.text)), input_rect); }
            }
        }
        Dialog::Path { input, .. } => frame.render_widget(&*input, parts[0]),
        Dialog::Delete(_) => frame.render_widget(Paragraph::new("Delete every occurrence of this event? Notes remain intact. A synced deletion reaches the server on the next sync.").wrap(Wrap { trim: false }).style(app.app_theme.bg_style()), parts[0]),
        Dialog::Conflict(id) => {
            let text = app.calendar.data().resources.iter().find(|r| &r.id == id).map_or_else(|| "Event unavailable".into(), |r| {
                let local = Draft::from_source(&r.source).map_or_else(|_| r.id.clone(), |d| format!("{} · {} to {}", d.title, d.start, d.end));
                let remote = r.conflict.as_ref().and_then(|c| c.remote.as_ref()).map_or_else(|| "Deleted remotely".into(), |r| Draft::from_source(&r.source).map_or_else(|_| "Remote source retained".into(), |d| format!("{} · {} to {}", d.title, d.start, d.end)));
                format!("Local{}: {local}\n\nRemote: {remote}\n\nKeep local queues its version for the next sync. Use remote replaces the local event. Note links remain local.", if r.deleted { " (pending deletion)" } else { "" })
            });
            frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }).style(app.app_theme.bg_style()), parts[0]);
        }
        Dialog::Help => frame.render_widget(Paragraph::new(HELP).wrap(Wrap { trim: false }).style(app.app_theme.bg_style()), parts[0]),
        Dialog::Notes { .. } => {}
    }
}

pub fn draw_widget(frame: &mut Frame, rect: Rect, app: &mut App, bottom_border: bool, strip: Rect) {
    let area = crate::ui::widget_strip_area(frame, rect, strip, bottom_border, &app.app_theme);
    if area.width < 14 || area.height < 8 {
        return;
    }
    let date = Local::now().date_naive();
    let width = area.width.min(35);
    let area = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - 8) / 2,
        width,
        8,
    );
    frame.render_widget(
        Paragraph::new(format!("{} · Calendar", date.format("%B %Y")))
            .style(Style::default().fg(app.app_theme.accent)),
        Rect::new(area.x, area.y, area.width, 1),
    );
    weekday_header(
        frame,
        Rect::new(area.x, area.y + 1, area.width, 1),
        app.list.week_start,
        &app.app_theme,
    );
    let from = week_start(month_start(date), app.list.week_start);
    let until = from + Duration::days(42);
    let (events, error) =
        match app
            .calendar
            .events_async(&app.storage, from, until, &app.notes_worker_pool)
        {
            Ok(Some((events, _))) => (events, false),
            Ok(None) => (&[][..], false),
            Err(_) => (&[][..], true),
        };
    for (cell, day) in month_cells(
        Rect::new(area.x, area.y + 2, area.width, 6),
        date,
        app.list.week_start,
    ) {
        let mut style = Style::default().fg(if day.month() == date.month() {
            app.app_theme.text
        } else {
            app.app_theme.muted
        });
        if events.iter().any(|e| e.on_date(day)) {
            style = style.fg(app.app_theme.accent).add_modifier(Modifier::BOLD);
        }
        if day == date {
            style = style
                .fg(app.app_theme.highlight_fg)
                .bg(app.app_theme.accent);
        }
        frame.render_widget(
            Paragraph::new(format!(
                "{:>2}{}",
                day.day(),
                if events.iter().any(|e| e.on_date(day)) {
                    "·"
                } else {
                    " "
                }
            ))
            .style(style),
            cell,
        );
    }
    if error {
        frame.render_widget(
            Paragraph::new("Calendar unavailable; open to inspect")
                .style(Style::default().fg(app.app_theme.warning)),
            Rect::new(area.x, area.y, area.width, 1),
        );
    }
}

pub fn widget_date(
    rect: Rect,
    strip: Rect,
    bottom_border: bool,
    col: u16,
    row: u16,
    start: WeekStart,
) -> NaiveDate {
    let date = Local::now().date_naive();
    let area = crate::ui::widget_strip_content_area(rect, strip, bottom_border);
    if area.width < 14 || area.height < 8 {
        return date;
    }
    let width = area.width.min(35);
    let grid = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - 8) / 2 + 2,
        width,
        6,
    );
    month_cells(grid, date, start)
        .into_iter()
        .find(|(r, _)| r.contains(Position::new(col, row)))
        .map_or(date, |(_, d)| d)
}
