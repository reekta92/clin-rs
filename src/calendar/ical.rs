//! iCalendar is the interchange format; retain properties the editor does not own.
use anyhow::{Context, Result, bail, ensure};
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use icalendar::parser::{Calendar, Component as RawComponent, Property as RawProperty};
use icalendar::{CalendarDateTime, Component, DatePerhapsTime, Event, EventLike, Property};
use std::collections::{BTreeMap, HashSet};

pub const MAX_BYTES: usize = 16 * 1024 * 1024;
pub type Document = Calendar<'static>;

#[derive(Clone, Debug)]
pub struct Occurrence {
    pub resource: String,
    pub title: String,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub all_day: bool,
    pub location: String,
    pub description: String,
    pub recurring: bool,
}

impl Occurrence {
    pub fn on_date(&self, date: NaiveDate) -> bool {
        let end_date = if self.end.time() == chrono::NaiveTime::MIN && self.end > self.start {
            self.end.date().pred_opt().unwrap_or(self.end.date())
        } else {
            self.end.date()
        };
        self.start.date() <= date && end_date >= date
    }
}

fn own_component(c: RawComponent<'_>) -> RawComponent<'static> {
    RawComponent {
        name: c.name.into_owned(),
        properties: c.properties.into_iter().map(own_property).collect(),
        components: c.components.into_iter().map(own_component).collect(),
    }
}

fn own_property(p: RawProperty<'_>) -> RawProperty<'static> {
    RawProperty {
        name: p.name.into_owned(),
        val: p.val.into_owned(),
        params: p
            .params
            .into_iter()
            .map(|p| icalendar::parser::Parameter {
                key: p.key.into_owned(),
                val: p.val.map(|v| v.into_owned()),
            })
            .collect(),
    }
}

pub fn parse(source: &str) -> Result<Document> {
    ensure!(source.len() <= MAX_BYTES, "iCalendar exceeds 16 MiB");
    let source = icalendar::parser::unfold(source.trim_start_matches('\u{feff}'));
    let source = source.trim();
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
    let parsed = icalendar::parser::read_calendar(source)
        .map_err(|_| anyhow::anyhow!("Invalid iCalendar syntax"))?;
    ensure!(
        parsed
            .properties
            .iter()
            .any(|p| p.name == "VERSION" && p.val == "2.0"),
        "Expected iCalendar VERSION:2.0"
    );
    Ok(Calendar {
        properties: parsed.properties.into_iter().map(own_property).collect(),
        components: parsed.components.into_iter().map(own_component).collect(),
    })
}

pub fn value<'a>(c: &'a RawComponent<'_>, key: &str) -> Option<&'a str> {
    c.find_prop(key).map(|p| p.val.as_str())
}

pub fn uid(doc: &Document) -> Result<String> {
    let mut ids = HashSet::new();
    for event in doc.components.iter().filter(|c| c.name == "VEVENT") {
        let uid = value(event, "UID").context("VEVENT requires UID")?;
        ensure!(!uid.is_empty(), "VEVENT requires nonempty UID");
        ids.insert(uid.to_owned());
    }
    ensure!(
        ids.len() == 1,
        "A calendar resource must contain one event series"
    );
    Ok(ids.into_iter().next().expect("one UID"))
}

pub fn split_import(source: &str) -> Result<Vec<String>> {
    let doc = parse(source)?;
    let mut series: BTreeMap<String, Vec<RawComponent<'static>>> = BTreeMap::new();
    let mut zones = Vec::new();
    for c in doc.components {
        match c.name.as_str() {
            "VEVENT" => {
                let id = value(&c, "UID").context("VEVENT requires UID")?.to_owned();
                ensure!(!id.is_empty(), "VEVENT requires nonempty UID");
                series.entry(id).or_default().push(c);
            }
            "VTIMEZONE" => zones.push(c),
            _ => bail!("Import supports VEVENT calendars; other components were not imported"),
        }
    }
    ensure!(!series.is_empty(), "No events in iCalendar file");
    ensure!(
        series.len() <= 10_000,
        "iCalendar exceeds 10000 event series"
    );
    Ok(series
        .into_values()
        .map(|mut events| {
            events.extend(zones.clone());
            Calendar {
                properties: doc.properties.clone(),
                components: events,
            }
            .to_string()
        })
        .collect())
}

fn event(c: &RawComponent<'_>) -> Event {
    let typed: icalendar::CalendarComponent = c.clone().into();
    typed.as_event().expect("VEVENT checked by caller").clone()
}

fn instant(value: DatePerhapsTime) -> Result<(DateTime<icalendar::Tz>, bool)> {
    let (time, tz, all_day) = match value {
        DatePerhapsTime::Date(date) => (
            date.and_time(chrono::NaiveTime::MIN),
            icalendar::Tz::UTC,
            true,
        ),
        DatePerhapsTime::DateTime(CalendarDateTime::Utc(time)) => {
            return Ok((time.with_timezone(&icalendar::Tz::UTC), false));
        }
        DatePerhapsTime::DateTime(CalendarDateTime::Floating(time)) => {
            (time, icalendar::Tz::LOCAL, false)
        }
        DatePerhapsTime::DateTime(CalendarDateTime::WithTimezone { date_time, tzid }) => {
            let tz: icalendar::Tz = tzid
                .parse::<chrono_tz::Tz>()
                .map_err(|_| anyhow::anyhow!("Unsupported timezone: {tzid}"))?
                .into();
            (date_time, tz, false)
        }
    };
    Ok((
        tz.from_local_datetime(&time)
            .single()
            .context("Ambiguous or nonexistent local time; use an explicit UTC offset")?,
        all_day,
    ))
}

fn duration(e: &Event, start: DateTime<icalendar::Tz>, all_day: bool) -> Result<Duration> {
    let duration = if let Some(end) = e.get_end() {
        let (end, end_all_day) = instant(end)?;
        ensure!(
            end_all_day == all_day,
            "DTSTART and DTEND must use the same value type"
        );
        ensure!(
            e.property_value("DURATION").is_none(),
            "VEVENT has both DTEND and DURATION"
        );
        end.signed_duration_since(start)
    } else if let Some(raw) = e.property_value("DURATION") {
        let parsed = iso8601::duration(raw.trim_start_matches('+'))
            .map_err(|_| anyhow::anyhow!("Invalid event duration"))?;
        if let iso8601::Duration::YMDHMS { year, month, .. } = parsed {
            ensure!(
                year == 0 && month == 0,
                "Event duration cannot contain years or months"
            );
        }
        Duration::from_std(parsed.into()).context("Event duration out of range")?
    } else if all_day {
        Duration::days(1)
    } else {
        Duration::zero()
    };
    ensure!(duration >= Duration::zero(), "Event ends before it starts");
    ensure!(
        duration > Duration::zero()
            || (e.get_end().is_none() && e.property_value("DURATION").is_none()),
        "DTEND and DURATION must specify a positive duration"
    );
    ensure!(
        !all_day || duration.num_seconds() % 86_400 == 0,
        "All-day duration must contain whole days"
    );
    Ok(duration)
}

fn event_end(
    e: &Event,
    start: DateTime<icalendar::Tz>,
    duration: Duration,
) -> Result<DateTime<icalendar::Tz>> {
    // RFC 5545 durations add calendar days before elapsed hours across DST changes.
    let days = if let Some(raw) = e.property_value("DURATION") {
        match iso8601::duration(raw.trim_start_matches('+'))
            .map_err(|_| anyhow::anyhow!("Invalid duration"))?
        {
            iso8601::Duration::Weeks(weeks) => u64::from(weeks) * 7,
            iso8601::Duration::YMDHMS { day, .. } => u64::from(day),
        }
    } else {
        0
    };
    let elapsed = duration - Duration::days(i64::try_from(days).context("Duration out of range")?);
    start
        .checked_add_days(chrono::Days::new(days))
        .and_then(|time| time.checked_add_signed(elapsed))
        .context("Event end is out of range or falls in a DST gap")
}

pub fn occurrences(
    source: &str,
    resource: &str,
    from: NaiveDate,
    until: NaiveDate,
) -> Result<Vec<Occurrence>> {
    let doc = parse(source)?;
    uid(&doc)?;
    let components: Vec<_> = doc
        .components
        .iter()
        .filter(|c| c.name == "VEVENT")
        .collect();
    let masters: Vec<_> = components
        .iter()
        .filter(|c| value(c, "RECURRENCE-ID").is_none())
        .collect();
    ensure!(masters.len() <= 1, "Duplicate master VEVENT");
    let mut overrides = HashSet::new();
    for c in &components {
        if let Some(property) = c.find_prop("RECURRENCE-ID") {
            ensure!(
                !property.params.iter().any(|p| p.key == "RANGE"),
                "RANGE=THISANDFUTURE is not supported; original retained"
            );
            let e = event(c);
            let (id, _) = instant(e.get_recurrence_id().context("Invalid RECURRENCE-ID")?)?;
            ensure!(
                overrides.insert(id.timestamp()),
                "Duplicate recurrence override"
            );
        }
    }
    let mut output = Vec::new();
    for c in components {
        let e = event(c);
        if e.property_value("STATUS") == Some("CANCELLED") {
            continue;
        }
        let (start, all_day) = instant(e.get_start().context("Invalid or missing DTSTART")?)?;
        let duration = duration(&e, start, all_day)?;
        let recurring =
            e.property_value("RRULE").is_some() || e.multi_properties().contains_key("RDATE");
        let starts = if recurring && e.get_recurrence_id().is_none() {
            let mut recurrence_event = e.clone();
            if all_day {
                recurrence_event.starts(start.with_timezone(&Utc));
                for key in ["EXDATE", "RDATE"] {
                    if let Some(properties) = e.multi_properties().get(key) {
                        recurrence_event.remove_multi_property(key);
                        for p in properties {
                            let dates = p
                                .value()
                                .split(',')
                                .map(|d| {
                                    if d.len() == 8 {
                                        format!("{d}T000000Z")
                                    } else {
                                        d.to_owned()
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join(",");
                            recurrence_event.append_multi_property(Property::new(key, dates));
                        }
                    }
                }
            }
            let lower = from
                .and_time(chrono::NaiveTime::MIN)
                .and_utc()
                .with_timezone(&icalendar::Tz::UTC)
                .checked_sub_signed(duration + Duration::days(2))
                .context("Event duration out of range")?;
            let upper = until
                .and_time(chrono::NaiveTime::MIN)
                .and_utc()
                .with_timezone(&icalendar::Tz::UTC)
                .checked_add_signed(Duration::days(2))
                .context("Query date out of range")?;
            let result = recurrence_event
                .get_recurrence()?
                .after(lower)
                .before(upper)
                .all(4096);
            ensure!(
                !result.limited,
                "Recurrence expansion exceeded safety limit; narrow the date range"
            );
            result
                .dates
                .into_iter()
                .filter(|d| !overrides.contains(&d.timestamp()))
                .collect()
        } else {
            vec![start]
        };
        for start in starts {
            let end = event_end(&e, start, duration)?;
            let local_start = if all_day {
                start.naive_local()
            } else {
                start.with_timezone(&Local).naive_local()
            };
            let local_end = if all_day {
                end.naive_local()
            } else {
                end.with_timezone(&Local).naive_local()
            };
            if local_start.date() >= until || local_end < from.and_time(chrono::NaiveTime::MIN) {
                continue;
            }
            output.push(Occurrence {
                resource: resource.to_owned(),
                title: e.get_summary().unwrap_or("Untitled event").to_owned(),
                start: local_start,
                end: local_end,
                all_day,
                location: e.get_location().unwrap_or_default().to_owned(),
                description: e.get_description().unwrap_or_default().to_owned(),
                recurring: recurring || e.get_recurrence_id().is_some(),
            });
        }
    }
    output.sort_by_key(|o| (o.start, !o.all_day, o.title.clone()));
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
    pub fn new(date: NaiveDate) -> Self {
        Self {
            title: String::new(),
            start: date.to_string(),
            end: date.succ_opt().unwrap_or(date).to_string(),
            location: String::new(),
            description: String::new(),
            repeat: String::new(),
        }
    }

    pub fn from_source(source: &str) -> Result<Self> {
        let doc = parse(source)?;
        let c = doc
            .components
            .iter()
            .find(|c| c.name == "VEVENT" && value(c, "RECURRENCE-ID").is_none())
            .context("Event has no editable master")?;
        let e = event(c);
        let (start, all_day) = instant(e.get_start().context("Missing DTSTART")?)?;
        let end = event_end(&e, start, duration(&e, start, all_day)?)?;
        let format = |time: DateTime<icalendar::Tz>| {
            if all_day {
                time.date_naive().to_string()
            } else {
                time.with_timezone(&Local).to_rfc3339()
            }
        };
        Ok(Self {
            title: e.get_summary().unwrap_or_default().to_owned(),
            start: format(start),
            end: format(end),
            location: e.get_location().unwrap_or_default().to_owned(),
            description: e.get_description().unwrap_or_default().to_owned(),
            repeat: value(c, "RRULE").unwrap_or_default().to_owned(),
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
            let mut e = Event::new();
            e.uid(&uuid::Uuid::new_v4().to_string())
                .timestamp(Utc::now());
            Calendar {
                properties: vec![
                    Property::new("VERSION", "2.0").into(),
                    Property::new("PRODID", "-//clin//Calendar//EN").into(),
                ],
                components: vec![e.into()],
            }
        };
        let old = previous.map(Self::from_source).transpose()?;
        let time_changed = old.as_ref().is_none_or(|old| {
            old.start != self.start || old.end != self.end || old.repeat != self.repeat
        });
        if time_changed {
            ensure!(
                !doc.components
                    .iter()
                    .any(|c| value(c, "RECURRENCE-ID").is_some()
                        || value(c, "EXDATE").is_some()
                        || value(c, "RDATE").is_some()),
                "Cannot reschedule a series with exceptions; edit its iCalendar source"
            );
        }
        let master = doc
            .components
            .iter_mut()
            .find(|c| c.name == "VEVENT" && value(c, "RECURRENCE-ID").is_none())
            .context("Missing master VEVENT")?;
        for (key, text) in [
            ("SUMMARY", self.title.as_str()),
            ("LOCATION", &self.location),
            ("DESCRIPTION", &self.description),
        ] {
            set_property(master, key, Some(Property::new(key, text)));
        }
        if time_changed {
            let start = input_time(&self.start)?;
            let end = input_time(&self.end)?;
            ensure!(
                matches!(
                    (&start, &end),
                    (DatePerhapsTime::Date(_), DatePerhapsTime::Date(_))
                        | (DatePerhapsTime::DateTime(_), DatePerhapsTime::DateTime(_))
                ),
                "Start and end must both be dates or date-times"
            );
            ensure!(
                instant(end.clone())?.0 > instant(start.clone())?.0,
                "Event end must be after start (all-day end is exclusive)"
            );
            set_property(master, "DTSTART", Some(start.to_property("DTSTART")));
            set_property(master, "DTEND", Some(end.to_property("DTEND")));
            set_property(master, "DURATION", None);
            set_property(
                master,
                "RRULE",
                (!self.repeat.trim().is_empty())
                    .then(|| Property::new("RRULE", self.repeat.trim())),
            );
        }
        let sequence = value(master, "SEQUENCE")
            .unwrap_or("0")
            .parse::<u64>()
            .context("Invalid event sequence")?
            .checked_add(1)
            .context("Event sequence overflow")?;
        set_property(
            master,
            "SEQUENCE",
            Some(Property::new("SEQUENCE", sequence.to_string())),
        );
        set_property(
            master,
            "DTSTAMP",
            Some(Property::new(
                "DTSTAMP",
                Utc::now().format("%Y%m%dT%H%M%SZ").to_string(),
            )),
        );
        let output = doc.to_string();
        let date = instant(input_time(&self.start)?)?.0.date_naive();
        occurrences(
            &output,
            "validation",
            date,
            date.succ_opt().context("Date out of range")?,
        )?;
        Ok(output)
    }
}

fn set_property(c: &mut RawComponent<'static>, key: &str, property: Option<Property>) {
    c.properties.retain(|p| p.name != key);
    if let Some(property) = property {
        c.properties.push(property.into());
    }
}

fn input_time(text: &str) -> Result<DatePerhapsTime> {
    if let Ok(date) = NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d") {
        return Ok(date.into());
    }
    if let Ok(time) = DateTime::parse_from_rfc3339(text.trim()) {
        return Ok(time.with_timezone(&Utc).into());
    }
    let local = NaiveDateTime::parse_from_str(text.trim(), "%Y-%m-%dT%H:%M")
        .context("Use YYYY-MM-DD, YYYY-MM-DDTHH:MM, or an RFC3339 date-time")?;
    Ok(Local
        .from_local_datetime(&local)
        .single()
        .context("Ambiguous or nonexistent local time; supply a UTC offset")?
        .with_timezone(&Utc)
        .into())
}
