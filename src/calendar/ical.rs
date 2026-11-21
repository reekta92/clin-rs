//! iCalendar is the interchange format; retain properties the editor does not own.
use anyhow::{Context, Result, bail, ensure};
use calcard_next::common::PartialDateTime;
use calcard_next::icalendar::{
    ICalendar as Document, ICalendarComponent as Component, ICalendarComponentType as Kind,
    ICalendarEntry as Entry, ICalendarParameter as Parameter,
    ICalendarParameterName as ParameterName, ICalendarProperty as Property, ICalendarStatus,
    ICalendarValue as Value, ICalendarValueType, dates::TimeOrDelta,
};
use jiff::{
    SignedDuration, Timestamp, ToSpan, Zoned,
    civil::{Date, DateTime, Time},
    tz::{Offset, TimeZone},
};
use std::collections::{BTreeMap, HashSet};

pub const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Occurrence {
    pub resource: String,
    pub title: String,
    pub start: DateTime,
    pub end: DateTime,
    pub all_day: bool,
    pub location: String,
    pub description: String,
    pub recurring: bool,
}

impl Occurrence {
    pub fn on_date(&self, date: Date) -> bool {
        let end_date = if self.end.time() == Time::MIN && self.end > self.start {
            self.end.date().yesterday().unwrap_or(self.end.date())
        } else {
            self.end.date()
        };
        self.start.date() <= date && end_date >= date
    }
}

pub fn parse(source: &str) -> Result<Document> {
    ensure!(source.len() <= MAX_BYTES, "iCalendar exceeds 16 MiB");
    let source = source.trim_start_matches('\u{feff}').trim();
    ensure!(
        source.starts_with("BEGIN:VCALENDAR") && source.ends_with("END:VCALENDAR"),
        "Expected one VCALENDAR document"
    );
    let mut stack = Vec::new();
    let mut roots = 0;
    for line in source.lines().map(str::trim_end) {
        if let Some(kind) = line.strip_prefix("BEGIN:") {
            if stack.is_empty() {
                roots += 1;
            }
            stack.push(kind);
            ensure!(stack.len() <= 32, "iCalendar nesting exceeds safety limit");
        } else if let Some(kind) = line.strip_prefix("END:") {
            ensure!(stack.pop() == Some(kind), "Mismatched iCalendar component");
        }
    }
    ensure!(
        roots == 1 && stack.is_empty(),
        "Expected one complete VCALENDAR document"
    );
    let (parsed, diagnostics) = Document::parse_with_diagnostics(source)
        .map_err(|_| anyhow::anyhow!("Invalid iCalendar syntax"))?;
    ensure!(
        diagnostics.is_empty(),
        "Invalid iCalendar syntax. The original document was retained."
    );
    let root = parsed.components.first().context("Missing VCALENDAR")?;
    ensure!(
        root.component_type == Kind::VCalendar && text(root, Property::Version) == Some("2.0"),
        "Expected iCalendar VERSION:2.0"
    );
    Ok(parsed)
}

fn value(component: &Component, name: Property) -> Option<&Value> {
    component.property(&name)?.values.first()
}

fn text(component: &Component, name: Property) -> Option<&str> {
    value(component, name)?.as_text()
}

fn top_level(doc: &Document) -> impl Iterator<Item = (u32, &Component)> {
    doc.components[0]
        .component_ids
        .iter()
        .map(|&id| (id, &doc.components[id as usize]))
}

pub fn uid(doc: &Document) -> Result<String> {
    let mut ids = HashSet::new();
    for (_, event) in top_level(doc).filter(|(_, c)| c.component_type == Kind::VEvent) {
        let uid = event.uid().context("VEVENT requires UID")?;
        ensure!(!uid.is_empty(), "VEVENT requires nonempty UID");
        ids.insert(uid);
    }
    ensure!(
        ids.len() == 1,
        "A calendar resource must contain one event series"
    );
    Ok(ids.into_iter().next().expect("one UID").to_owned())
}

fn new_document(entries: Vec<Entry>) -> Document {
    Document {
        components: vec![Component {
            entries,
            ..Component::default()
        }],
    }
}

fn empty_calendar() -> Document {
    new_document(vec![
        Entry::new(Property::Version).with_value("2.0"),
        Entry::new(Property::Prodid).with_value("-//clin//Calendar//EN"),
    ])
}

fn copy_component(source: &Document, id: u32, target: &mut Document) -> u32 {
    let component = &source.components[id as usize];
    let new_id = target.components.len() as u32;
    target.components.push(Component {
        component_type: component.component_type.clone(),
        entries: component.entries.clone(),
        component_ids: Vec::new(),
    });
    let children = component
        .component_ids
        .iter()
        .map(|&child| copy_component(source, child, target))
        .collect();
    target.components[new_id as usize].component_ids = children;
    new_id
}

fn append_component(source: &Document, id: u32, target: &mut Document) {
    let new_id = copy_component(source, id, target);
    target.components[0].component_ids.push(new_id);
}

fn render(doc: &Document) -> Result<String> {
    let mut output = String::new();
    doc.write_to(&mut output)?;
    Ok(output)
}

pub fn split_import(source: &str) -> Result<Vec<String>> {
    let doc = parse(source)?;
    let mut series = BTreeMap::<&str, Vec<u32>>::new();
    let mut zones = Vec::new();
    for (id, component) in top_level(&doc) {
        match component.component_type {
            Kind::VEvent => {
                let uid = component.uid().context("VEVENT requires UID")?;
                ensure!(!uid.is_empty(), "VEVENT requires nonempty UID");
                series.entry(uid).or_default().push(id);
            }
            Kind::VTimezone => zones.push(id),
            _ => bail!("Import supports VEVENT calendars; other components were not imported"),
        }
    }
    ensure!(!series.is_empty(), "No events in iCalendar file");
    ensure!(
        series.len() <= 10_000,
        "iCalendar exceeds 10000 event series"
    );
    series
        .into_values()
        .map(|ids| {
            let mut out = new_document(doc.components[0].entries.clone());
            for id in ids.into_iter().chain(zones.iter().copied()) {
                append_component(&doc, id, &mut out);
            }
            render(&out)
        })
        .collect()
}

pub fn export<'a>(sources: impl IntoIterator<Item = &'a str>) -> Result<(String, usize)> {
    let mut out = empty_calendar();
    let mut count = 0;
    let mut zones = BTreeMap::new();
    let mut uids = HashSet::new();
    for source in sources {
        let doc = parse(source)?;
        ensure!(
            uids.insert(uid(&doc)?),
            "Calendars contain duplicate event UIDs; export would be ambiguous"
        );
        for (id, component) in top_level(&doc) {
            if component.component_type == Kind::VTimezone {
                let name = text(component, Property::Tzid).context("VTIMEZONE requires TZID")?;
                let mut unique = new_document(Vec::new());
                append_component(&doc, id, &mut unique);
                let contents = render(&unique)?;
                if let Some(previous) = zones.insert(name.to_owned(), contents.clone()) {
                    ensure!(
                        previous == contents,
                        "Calendars contain conflicting timezone definitions"
                    );
                    continue;
                }
            }
            append_component(&doc, id, &mut out);
        }
        count += 1;
    }
    Ok((render(&out)?, count))
}

fn instant(entry: &Entry) -> Result<(Zoned, bool)> {
    let Some(Value::PartialDateTime(date)) = entry.values.first() else {
        bail!("Invalid calendar date");
    };
    let civil = DateTime::new(
        i16::try_from(date.year.context("Missing calendar year")?)?,
        i8::try_from(date.month.context("Missing calendar month")?)?,
        i8::try_from(date.day.context("Missing calendar day")?)?,
        i8::try_from(date.hour.unwrap_or(0))?,
        i8::try_from(date.minute.unwrap_or(0))?,
        i8::try_from(date.second.unwrap_or(0))?,
        0,
    )?;
    let all_day = date.hour.is_none();
    let zone = if all_day {
        TimeZone::UTC
    } else if let Some(hours) = date.tz_hour {
        let seconds = i32::from(hours) * 3600 + i32::from(date.tz_minute.unwrap_or(0)) * 60;
        Offset::from_seconds(if date.tz_minus { -seconds } else { seconds })?.to_time_zone()
    } else if let Some(tzid) = entry.params.iter().find_map(|parameter| {
        (parameter.name == ParameterName::Tzid)
            .then(|| parameter.value.as_text())
            .flatten()
    }) {
        TimeZone::get(tzid).with_context(|| format!("Unsupported timezone: {tzid}"))?
    } else {
        TimeZone::system()
    };
    let time = zone
        .into_ambiguous_zoned(civil)
        .unambiguous()
        .context("Ambiguous or nonexistent local time; use an explicit UTC offset")?;
    Ok((time, all_day))
}

fn event_end(event: &Component, start: &Zoned, all_day: bool) -> Result<Zoned> {
    let end = if let Some(entry) = event.property(&Property::Dtend) {
        ensure!(
            !event.has_property(&Property::Duration),
            "VEVENT has both DTEND and DURATION"
        );
        let (end, end_all_day) = instant(entry)?;
        ensure!(
            end_all_day == all_day,
            "DTSTART and DTEND must use the same value type"
        );
        end
    } else if let Some(duration) = value(event, Property::Duration) {
        let Value::Duration(duration) = duration else {
            bail!("Invalid event duration");
        };
        ensure!(!duration.neg, "Event ends before it starts");
        let days = i64::from(duration.weeks) * 7 + i64::from(duration.days);
        let seconds = i64::from(duration.hours) * 3600
            + i64::from(duration.minutes) * 60
            + i64::from(duration.seconds);
        ensure!(
            !all_day || seconds % 86_400 == 0,
            "All-day duration must contain whole days"
        );
        // RFC 5545 durations add calendar days before elapsed hours across DST changes.
        let wall = start
            .datetime()
            .checked_add(jiff::Span::new().try_days(days)?)?;
        start
            .time_zone()
            .to_ambiguous_zoned(wall)
            .unambiguous()?
            .checked_add(SignedDuration::from_secs(seconds))?
    } else if all_day {
        start.checked_add(1.days())?
    } else {
        start.clone()
    };
    ensure!(&end >= start, "Event ends before it starts");
    ensure!(
        &end > start
            || (!event.has_property(&Property::Dtend) && !event.has_property(&Property::Duration)),
        "DTEND and DURATION must specify a positive duration"
    );
    Ok(end)
}

pub fn occurrences(
    source: &str,
    resource: &str,
    from: Date,
    until: Date,
) -> Result<Vec<Occurrence>> {
    let doc = parse(source)?;
    uid(&doc)?;
    let mut masters = 0;
    let mut overrides = HashSet::new();
    for (_, event) in top_level(&doc).filter(|(_, c)| c.component_type == Kind::VEvent) {
        if let Some(id) = event.property(&Property::RecurrenceId) {
            ensure!(
                !id.params.iter().any(|p| p.name == ParameterName::Range),
                "RANGE=THISANDFUTURE is not supported; original retained"
            );
            ensure!(
                overrides.insert(instant(id)?.0.timestamp()),
                "Duplicate recurrence override"
            );
        } else {
            masters += 1;
        }
        if event.status() != Some(&ICalendarStatus::Cancelled) {
            ensure!(
                value(event, Property::Rrule)
                    .is_none_or(|rule| matches!(rule, Value::RecurrenceRule(_))),
                "Invalid RRULE"
            );
            let (start, all_day) = instant(
                event
                    .property(&Property::Dtstart)
                    .context("Missing DTSTART")?,
            )?;
            event_end(event, &start, all_day)?;
        }
    }
    ensure!(masters <= 1, "Duplicate master VEVENT");
    let system = TimeZone::system();
    let lower = system.to_timestamp(from.at(0, 0, 0, 0))?;
    let upper = system.to_timestamp(until.at(0, 0, 0, 0))?;
    let expanded = doc.expand_dates_between(&system, lower, upper, 4096);
    ensure!(
        expanded.errors.is_empty(),
        "Calendar recurrence expansion failed: {:?}",
        expanded.errors
    );
    let mut output = Vec::new();
    for instance in expanded.events {
        let event = &doc.components[instance.comp_id as usize];
        if event.component_type != Kind::VEvent
            || event.status() == Some(&ICalendarStatus::Cancelled)
        {
            continue;
        }
        let all_day = instant(
            event
                .property(&Property::Dtstart)
                .context("Missing DTSTART")?,
        )?
        .1;
        let end = match instance.end {
            TimeOrDelta::Time(time) => time,
            TimeOrDelta::Delta(duration) => instance.start.checked_add(duration)?,
        };
        let local = |time: Zoned| {
            if all_day {
                time.datetime()
            } else {
                time.with_time_zone(system.clone()).datetime()
            }
        };
        let start = local(instance.start);
        let end = local(end);
        if start.date() >= until || end < from.at(0, 0, 0, 0) {
            continue;
        }
        output.push(Occurrence {
            resource: resource.to_owned(),
            title: text(event, Property::Summary)
                .unwrap_or("Untitled event")
                .to_owned(),
            start,
            end,
            all_day,
            location: text(event, Property::Location)
                .unwrap_or_default()
                .to_owned(),
            description: text(event, Property::Description)
                .unwrap_or_default()
                .to_owned(),
            recurring: event.is_recurrent_or_override() || instance.series_comp_id.is_some(),
        });
    }
    output.sort_by(|a, b| (a.start, !a.all_day, &a.title).cmp(&(b.start, !b.all_day, &b.title)));
    Ok(output)
}

#[derive(Clone, Debug)]
pub struct Draft {
    pub title: String,
    pub start: String,
    pub end: String,
    pub location: String,
    pub description: String,
    pub repeat: String,
}

impl Draft {
    pub fn new(date: Date) -> Self {
        Self {
            title: String::new(),
            start: date.to_string(),
            end: date.tomorrow().unwrap_or(date).to_string(),
            location: String::new(),
            description: String::new(),
            repeat: String::new(),
        }
    }

    pub fn from_source(source: &str) -> Result<Self> {
        let doc = parse(source)?;
        let event = top_level(&doc)
            .map(|(_, c)| c)
            .find(|c| c.component_type == Kind::VEvent && !c.is_recurrence_override())
            .context("Event has no editable master")?;
        let (start, all_day) = instant(
            event
                .property(&Property::Dtstart)
                .context("Missing DTSTART")?,
        )?;
        let end = event_end(event, &start, all_day)?;
        let format = |time: Zoned| {
            if all_day {
                time.date().to_string()
            } else {
                time.with_time_zone(TimeZone::system())
                    .strftime("%Y-%m-%dT%H:%M:%S%:z")
                    .to_string()
            }
        };
        let repeat = match value(event, Property::Rrule) {
            Some(Value::RecurrenceRule(rule)) => rule.to_string(),
            Some(Value::Text(text)) => text.clone(),
            _ => String::new(),
        };
        Ok(Self {
            title: text(event, Property::Summary)
                .unwrap_or_default()
                .to_owned(),
            start: format(start),
            end: format(end),
            location: text(event, Property::Location)
                .unwrap_or_default()
                .to_owned(),
            description: text(event, Property::Description)
                .unwrap_or_default()
                .to_owned(),
            repeat,
        })
    }

    pub fn build(&self, previous: Option<&str>) -> Result<String> {
        ensure!(!self.title.trim().is_empty(), "Event title is required");
        ensure!(
            !self.repeat.trim().contains(['\r', '\n']),
            "RRULE must be one line"
        );
        let mut doc = if let Some(source) = previous {
            parse(source)?
        } else {
            let mut doc = empty_calendar();
            doc.components[0].component_ids.push(1);
            doc.components.push(Component {
                component_type: Kind::VEvent,
                entries: vec![
                    Entry::new(Property::Uid).with_value(uuid::Uuid::new_v4().to_string()),
                ],
                component_ids: Vec::new(),
            });
            doc
        };
        let old = previous.map(Self::from_source).transpose()?;
        let time_changed = old.as_ref().is_none_or(|old| {
            old.start != self.start || old.end != self.end || old.repeat != self.repeat
        });
        if time_changed {
            ensure!(
                !top_level(&doc).any(|(_, c)| c.is_recurrence_override()
                    || c.has_property(&Property::Exdate)
                    || c.has_property(&Property::Rdate)),
                "Cannot reschedule a series with exceptions; edit its iCalendar source"
            );
        }
        let id = top_level(&doc)
            .find(|(_, c)| c.component_type == Kind::VEvent && !c.is_recurrence_override())
            .context("Missing master VEVENT")?
            .0;
        let master = &mut doc.components[id as usize];
        for (name, text) in [
            (Property::Summary, self.title.as_str()),
            (Property::Location, &self.location),
            (Property::Description, &self.description),
        ] {
            let entry = Entry::new(name.clone()).with_value(text);
            set_property(master, name, Some(entry));
        }
        let (start, all_day) = input_time(&self.start)?;
        if time_changed {
            let (end, end_all_day) = input_time(&self.end)?;
            ensure!(
                all_day == end_all_day,
                "Start and end must both be dates or date-times"
            );
            ensure!(
                end > start,
                "Event end must be after start (all-day end is exclusive)"
            );
            set_property(
                master,
                Property::Dtstart,
                Some(date_entry(Property::Dtstart, &start, all_day)?),
            );
            set_property(
                master,
                Property::Dtend,
                Some(date_entry(Property::Dtend, &end, all_day)?),
            );
            set_property(master, Property::Duration, None);
            set_property(
                master,
                Property::Rrule,
                (!self.repeat.trim().is_empty())
                    .then(|| Entry::new(Property::Rrule).with_value(self.repeat.trim())),
            );
        }
        let sequence = match value(master, Property::Sequence) {
            Some(Value::Integer(sequence)) if *sequence >= 0 => *sequence,
            Some(_) => bail!("Invalid event sequence"),
            None => 0,
        }
        .checked_add(1)
        .context("Event sequence overflow")?;
        set_property(
            master,
            Property::Sequence,
            Some(Entry::new(Property::Sequence).with_value(sequence)),
        );
        let now = Timestamp::now().to_zoned(TimeZone::UTC);
        set_property(
            master,
            Property::Dtstamp,
            Some(date_entry(Property::Dtstamp, &now, false)?),
        );
        let output = render(&doc)?;
        let date = start.date();
        occurrences(&output, "validation", date, date.tomorrow()?)?;
        Ok(output)
    }
}

fn set_property(component: &mut Component, name: Property, entry: Option<Entry>) {
    component.entries.retain(|entry| entry.name != name);
    component.entries.extend(entry);
}

fn date_entry(name: Property, value: &Zoned, all_day: bool) -> Result<Entry> {
    let mut date = PartialDateTime {
        year: Some(u16::try_from(value.year())?),
        month: Some(u8::try_from(value.month())?),
        day: Some(u8::try_from(value.day())?),
        ..PartialDateTime::default()
    };
    if all_day {
        Ok(Entry::new(name)
            .with_param(Parameter::new(
                ParameterName::Value,
                ICalendarValueType::Date,
            ))
            .with_value(Value::PartialDateTime(Box::new(date))))
    } else {
        date.hour = Some(u8::try_from(value.hour())?);
        date.minute = Some(u8::try_from(value.minute())?);
        date.second = Some(u8::try_from(value.second())?);
        date.tz_hour = Some(0);
        date.tz_minute = Some(0);
        Ok(Entry::new(name).with_value(Value::PartialDateTime(Box::new(date))))
    }
}

fn input_time(text: &str) -> Result<(Zoned, bool)> {
    let text = text.trim();
    if let Ok(date) = Date::strptime("%Y-%m-%d", text) {
        return Ok((TimeZone::UTC.to_zoned(date.at(0, 0, 0, 0))?, true));
    }
    if let Ok(time) = text.parse::<Timestamp>() {
        return Ok((time.to_zoned(TimeZone::UTC), false));
    }
    let local = DateTime::strptime("%Y-%m-%dT%H:%M", text)
        .context("Use YYYY-MM-DD, YYYY-MM-DDTHH:MM, or an RFC3339 date-time")?;
    let time = TimeZone::system()
        .into_ambiguous_zoned(local)
        .unambiguous()
        .context("Ambiguous or nonexistent local time; supply a UTC offset")?;
    Ok((time.with_time_zone(TimeZone::UTC), false))
}
