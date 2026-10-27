use super::{dav, ical, store, view};
use crate::storage::Storage;
use chrono::{Duration, NaiveDate};
use std::{fs, path::Path};

fn date(value: &str) -> NaiveDate {
    value.parse().unwrap()
}

pub(super) fn storage(root: &Path) -> Storage {
    fs::create_dir_all(root.join(".clin")).unwrap();
    Storage {
        data_dir: root.into(),
        notes_dir: root.into(),
        config_dir: root.join(".clin"),
        templates_dir: root.join(".clin/templates"),
        frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        skip_dir_patterns: Vec::new(),
    }
}

fn document(events: &str) -> String {
    format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:test\r\n{events}END:VCALENDAR\r\n")
}

#[test]
fn asynchronous_events_share_the_editing_snapshot_until_reload() {
    let root = tempfile::tempdir().unwrap();
    let storage = storage(root.path());
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap();
    let from = date("2026-10-01");
    let until = date("2026-11-01");
    let mut external = store::Service::default();
    let mut draft = ical::Draft::new(date("2026-10-23"));
    draft.title = "Original".into();
    external
        .update(&storage, |data| {
            data.resources
                .push(store::Resource::new(draft.build(None)?, None));
            Ok(())
        })
        .unwrap();
    let mut service = store::Service::default();
    let wait = |service: &mut store::Service, from, until| {
        assert!(
            service
                .events_async(&storage, from, until, &pool)
                .unwrap()
                .is_none()
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !service.poll_events() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        service
            .events_async(&storage, from, until, &pool)
            .unwrap()
            .unwrap()
            .0
            .to_vec()
    };
    assert_eq!(wait(&mut service, from, until)[0].title, "Original");
    assert!(
        service.store.is_some(),
        "initial async load must also install the editing snapshot"
    );
    external
        .update(&storage, |data| {
            draft.title = "External change".into();
            data.resources[0].source = draft.build(None)?;
            Ok(())
        })
        .unwrap();
    let events = wait(&mut service, from.pred_opt().unwrap(), until);
    assert_eq!(events[0].title, "Original");
    assert!(
        service.update(&storage, |_| Ok(())).is_err(),
        "stale edits must still be rejected"
    );
    service.events_async(&storage, from, until, &pool).unwrap();
    service.reload(&storage).unwrap();
    assert_eq!(wait(&mut service, from, until)[0].title, "External change");
}

#[test]
fn all_day_and_timed_events_validate_and_round_trip() {
    let mut draft = ical::Draft::new(date("2026-10-23"));
    draft.title = "Planning, notes; café 🦀".into();
    draft.description = "Line one\nLine two \\ text".into();
    let source = draft.build(None).unwrap();
    let events = ical::occurrences(&source, "id", date("2026-10-22"), date("2026-10-26")).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].title, draft.title);
    assert_eq!(events[0].description, draft.description);
    assert!(events[0].on_date(date("2026-10-23")));
    assert!(!events[0].on_date(date("2026-10-24")));
    draft.start = "2026-10-23T10:30:00+03:00".into();
    draft.end = "2026-10-23T11:30:00+03:00".into();
    let timed = draft.build(Some(&source)).unwrap();
    assert!(timed.contains("DTSTART:20261023T073000Z"));
    draft.end = "2026-10-23".into();
    assert!(draft.build(None).is_err());
    draft.end.clone_from(&draft.start);
    assert!(draft.build(None).is_err());
    draft.end = "2026-10-23T11:30:00+03:00".into();
    draft.repeat = "FREQ=INVALID".into();
    assert!(draft.build(None).is_err());
    draft.repeat = "FREQ=DAILY\r\nSUMMARY:injection".into();
    assert!(draft.build(None).is_err());
}

#[test]
fn recurrence_honors_exclusions_additions_and_moved_or_cancelled_instances() {
    let source = document(
        "BEGIN:VEVENT\r\nUID:series\r\nSUMMARY:Weekly\r\nDTSTART:20261019T090000Z\r\nDTEND:20261019T100000Z\r\nRRULE:FREQ=WEEKLY;COUNT=3\r\nEXDATE:20261026T090000Z\r\nRDATE:20261022T090000Z\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:series\r\nRECURRENCE-ID:20261102T090000Z\r\nSUMMARY:Moved\r\nDTSTART:20261103T110000Z\r\nDTEND:20261103T120000Z\r\nEND:VEVENT\r\n",
    );
    let events = ical::occurrences(&source, "id", date("2026-10-18"), date("2026-11-05")).unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events.iter().filter(|e| e.title == "Moved").count(), 1);
    assert!(!events.iter().any(|e| e.start.date() == date("2026-10-26")));
    let mut draft = ical::Draft::from_source(&source).unwrap();
    draft.title = "Changed series title".into();
    assert!(
        draft
            .build(Some(&source))
            .unwrap()
            .contains("RECURRENCE-ID:20261102T090000Z")
    );
    draft.start = "2026-10-20T09:00:00Z".into();
    assert!(draft.build(Some(&source)).is_err());
    let cancelled = source.replace("SUMMARY:Moved", "STATUS:CANCELLED\r\nSUMMARY:Moved");
    assert_eq!(
        ical::occurrences(&cancelled, "id", date("2026-10-18"), date("2026-11-05"))
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn recurring_all_day_and_dst_keep_calendar_dates() {
    if std::env::var_os("CLIN_TEST_CALENDAR_DST").is_none() {
        for timezone in ["UTC", "Europe/Vilnius"] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "calendar::tests::recurring_all_day_and_dst_keep_calendar_dates",
                ])
                .env("CLIN_TEST_CALENDAR_DST", "1")
                .env("TZ", timezone)
                .output()
                .unwrap();
            assert!(output.status.success(), "{timezone}: {output:?}");
        }
        return;
    }
    let source = document(
        "BEGIN:VEVENT\r\nUID:days\r\nSUMMARY:Days\r\nDTSTART;VALUE=DATE:20261023\r\nDTEND;VALUE=DATE:20261025\r\nRRULE:FREQ=DAILY;COUNT=3\r\nEXDATE;VALUE=DATE:20261024\r\nEND:VEVENT\r\n",
    );
    let events = ical::occurrences(&source, "id", date("2026-10-23"), date("2026-10-28")).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].start.date(), date("2026-10-23"));
    assert_eq!(events[1].start.date(), date("2026-10-25"));
    assert!(events[0].on_date(date("2026-10-24")));
    assert!(!events[0].on_date(date("2026-10-25")));
    let dst = document(
        "BEGIN:VEVENT\r\nUID:dst\r\nSUMMARY:DST\r\nDTSTART;TZID=Europe/Berlin:20261024T120000\r\nDURATION:P1D\r\nRRULE:FREQ=DAILY;COUNT=3\r\nEND:VEVENT\r\n",
    );
    let events = ical::occurrences(&dst, "id", date("2026-10-23"), date("2026-10-29")).unwrap();
    assert_eq!(events.len(), 3);
    let local = |value: &str| {
        chrono::DateTime::parse_from_rfc3339(value)
            .unwrap()
            .with_timezone(&chrono::Local)
            .naive_local()
    };
    assert_eq!(events[0].start, local("2026-10-24T12:00:00+02:00"));
    assert_eq!(events[0].end, local("2026-10-25T12:00:00+01:00"));
    assert_eq!(
        events[1].end.signed_duration_since(events[1].start),
        Duration::hours(24)
    );
}

#[test]
fn importing_and_editing_preserve_alarms_unknown_properties_and_timezone_components() {
    let source = document(
        "BEGIN:VTIMEZONE\r\nTZID:Europe/Berlin\r\nX-CUSTOM:zone\r\nEND:VTIMEZONE\r\nBEGIN:VEVENT\r\nUID:preserve\r\nSUMMARY:Folded long\r\n  title\r\nDTSTART;TZID=Europe/Berlin:20261023T100000\r\nDTEND;TZID=Europe/Berlin:20261023T110000\r\nX-CUSTOM:a\r\nX-CUSTOM:b\r\nATTENDEE;CN=User:mailto:user@example.org\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT15M\r\nDESCRIPTION:Reminder\r\nEND:VALARM\r\nEND:VEVENT\r\n",
    );
    let imported = ical::split_import(&source).unwrap();
    let mut draft = ical::Draft::from_source(&imported[0]).unwrap();
    draft.title = "Renamed".into();
    let edited = draft.build(Some(&imported[0])).unwrap();
    for expected in [
        "BEGIN:VTIMEZONE",
        "X-CUSTOM:a",
        "X-CUSTOM:b",
        "BEGIN:VALARM",
        "TRIGGER:-PT15M",
        "ATTENDEE;CN=User",
        "DTSTART;TZID=Europe/Berlin:20261023T100000",
    ] {
        assert!(edited.contains(expected), "{expected}");
    }
    assert!(ical::parse(&(source.clone() + &source)).is_err());
    assert!(ical::parse("BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nEND:VCALENDAR").is_err());
}

#[test]
fn calendar_writes_detect_stale_state_and_import_export_do_not_replace_user_files() {
    let temp = tempfile::tempdir().unwrap();
    let storage = storage(temp.path());
    let mut draft = ical::Draft::new(date("2026-10-23"));
    draft.title = "Event".into();
    let source = draft.build(None).unwrap();
    let path = temp.path().join("import.ics");
    fs::write(&path, &source).unwrap();
    let mut first = store::Service::default();
    let mut stale = store::Service::default();
    stale.ensure_loaded(&storage).unwrap();
    assert_eq!(first.import(&storage, &path).unwrap(), 1);
    assert_eq!(first.import(&storage, &path).unwrap(), 0);
    assert!(stale.update(&storage, |_| Ok(())).is_err());
    let export = temp.path().join("export.ics");
    assert_eq!(first.export(&storage, &export).unwrap(), 1);
    assert_eq!(
        ical::split_import(&fs::read_to_string(&export).unwrap())
            .unwrap()
            .len(),
        1
    );
    fs::write(&export, "user data").unwrap();
    assert!(first.export(&storage, &export).is_err());
    assert_eq!(fs::read_to_string(export).unwrap(), "user data");
    let before = fs::read(temp.path().join(".clin/calendar.json")).unwrap();
    fs::write(&path, "invalid calendar").unwrap();
    assert!(first.import(&storage, &path).is_err());
    assert_eq!(
        fs::read(temp.path().join(".clin/calendar.json")).unwrap(),
        before
    );
    fs::write(temp.path().join(".clin/calendar.json"), "corrupt").unwrap();
    assert!(store::Store::load(&storage).is_err());
    assert_eq!(
        fs::read_to_string(temp.path().join(".clin/calendar.json")).unwrap(),
        "corrupt"
    );
}

#[test]
fn note_properties_preserve_body_and_links_survive_note_rename() {
    let temp = tempfile::tempdir().unwrap();
    let mut storage = storage(temp.path());
    storage
        .create_document(
            "Note.md",
            "---\ncustom: 42 # keep this\n---\n# Note\nBody stays.\n",
        )
        .unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    store::toggle_note_link(&storage, "Note.md", &id).unwrap();
    assert_eq!(store::note_links(&storage).unwrap()[&id], ["Note.md"]);
    let renamed = storage.rename_note("Note.md", "Renamed").unwrap();
    assert_eq!(
        store::note_links(&storage).unwrap()[&id],
        std::slice::from_ref(&renamed)
    );
    let content = fs::read_to_string(storage.note_path(&renamed)).unwrap();
    assert!(content.contains("# keep this"));
    assert!(content.contains("Body stays."));
    store::toggle_note_link(&storage, &renamed, &id).unwrap();
    assert!(!store::note_links(&storage).unwrap().contains_key(&id));
}

#[test]
fn month_navigation_and_grid_handle_leap_years_and_week_start() {
    assert_eq!(view::shift_month(date("2024-01-31"), 1), date("2024-02-29"));
    assert_eq!(
        view::shift_month(date("2025-03-31"), -1),
        date("2025-02-28")
    );
    let cells = view::month_cells(
        ratatui::layout::Rect::new(3, 2, 77, 18),
        date("2026-11-01"),
        crate::config::WeekStart::Monday,
    );
    assert_eq!(cells.len(), 42);
    assert_eq!(cells[0].1, date("2026-10-26"));
    assert_eq!(cells[41].1, date("2026-12-06"));
    assert_eq!(cells[41].0.right(), 80);
}

#[test]
fn caldav_configuration_rejects_credentials_in_urls_and_plaintext_remote_transport() {
    for url in [
        "http://calendar.example.org/",
        "https://user:password@example.org/calendar/",
        "https://example.org/#secret",
        "file:///tmp/calendar",
    ] {
        assert!(dav::validate_url(url).is_err(), "{url}");
    }
    assert!(dav::validate_url("http://127.0.0.1:49147/calendar/").is_ok());
    assert!(dav::validate_url("https://example.org/calendar/").is_ok());
    let config: crate::config::ClinConfig = toml::from_str("[[calendar.caldav]]\nname = 'Work'\nurl = 'https://example.org/dav/'\nusername = 'user'\npassword_env = 'CLIN_CALDAV_PASSWORD'\n").unwrap();
    assert_eq!(config.calendar.caldav.len(), 1);
    assert_eq!(config.validate(), Vec::<String>::new());
}

#[test]
#[ignore = "requires the local Postman CalDAV simulation; see docs/CALENDAR.md"]
fn caldav_simulation_round_trip_and_conflicts() {
    let base = std::env::var("CLIN_CALDAV_TEST_URL")
        .expect("CLIN_CALDAV_TEST_URL must point to the local Postman simulation");
    let temp = tempfile::tempdir().unwrap();
    let storage = storage(temp.path());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let client = reqwest::Client::new();
        client
            .post(format!("{base}__reset"))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
        let account = dav::Account {
            name: "Simulation".into(),
            url: base.clone(),
            ..dav::Account::default()
        };
        let result = dav::sync(&storage, std::slice::from_ref(&account))
            .await
            .unwrap();
        assert!(result.contains("Synced 1"));
        let store = store::Store::load(&storage).unwrap();
        assert_eq!(store.data.resources.len(), 1);
        let collection = store.data.collections[0].url.clone();
        let mut service = store::Service::default();
        service.ensure_loaded(&storage).unwrap();
        let mut draft = ical::Draft::new(date("2026-10-23"));
        draft.title = "Created in clin".into();
        let source = draft.build(None).unwrap();
        let id = service
            .update(&storage, |data| {
                let r = store::Resource::new(source, Some(collection));
                let id = r.id.clone();
                data.resources.push(r);
                Ok(id)
            })
            .unwrap();
        dav::sync(&storage, std::slice::from_ref(&account))
            .await
            .unwrap();
        service.reload(&storage).unwrap();
        assert!(
            service
                .data()
                .resources
                .iter()
                .find(|r| r.id == id)
                .unwrap()
                .remote
                .is_some()
        );
        service
            .update(&storage, |data| {
                let r = data.resources.iter_mut().find(|r| r.id == id).unwrap();
                let mut draft = ical::Draft::from_source(&r.source)?;
                draft.title = "Edited in clin".into();
                r.source = draft.build(Some(&r.source))?;
                Ok(())
            })
            .unwrap();
        dav::sync(&storage, std::slice::from_ref(&account))
            .await
            .unwrap();
        service.reload(&storage).unwrap();
        let remote = service
            .data()
            .resources
            .iter()
            .find(|r| r.id == id)
            .unwrap()
            .remote
            .as_ref()
            .unwrap()
            .clone();
        assert!(remote.source.contains("Edited in clin"));
        let mut remote_draft = ical::Draft::from_source(&remote.source).unwrap();
        remote_draft.title = "Concurrent remote change".into();
        client
            .put(&remote.href)
            .header("If-Match", &remote.etag)
            .body(remote_draft.build(Some(&remote.source)).unwrap())
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
        service
            .update(&storage, |data| {
                let r = data.resources.iter_mut().find(|r| r.id == id).unwrap();
                let mut draft = ical::Draft::from_source(&r.source)?;
                draft.title = "Concurrent local change".into();
                r.source = draft.build(Some(&r.source))?;
                Ok(())
            })
            .unwrap();
        assert!(
            dav::sync(&storage, std::slice::from_ref(&account))
                .await
                .unwrap()
                .contains("1 conflict")
        );
        service.reload(&storage).unwrap();
        let conflicted = service
            .data()
            .resources
            .iter()
            .find(|r| r.id == id)
            .unwrap();
        assert!(conflicted.source.contains("Concurrent local change"));
        assert!(
            conflicted
                .conflict
                .as_ref()
                .unwrap()
                .remote
                .as_ref()
                .unwrap()
                .source
                .contains("Concurrent remote change")
        );
        service
            .update(&storage, |data| {
                let r = data.resources.iter_mut().find(|r| r.id == id).unwrap();
                r.remote = r.conflict.take().unwrap().remote;
                Ok(())
            })
            .unwrap();
        dav::sync(&storage, std::slice::from_ref(&account))
            .await
            .unwrap();
        service.reload(&storage).unwrap();
        assert!(
            service
                .data()
                .resources
                .iter()
                .find(|r| r.id == id)
                .unwrap()
                .remote
                .as_ref()
                .unwrap()
                .source
                .contains("Concurrent local change")
        );
        service
            .update(&storage, |data| {
                let r = data.resources.iter_mut().find(|r| r.id == id).unwrap();
                let mut draft = ical::Draft::from_source(&r.source)?;
                draft.title = "Local race edit".into();
                r.source = draft.build(Some(&r.source))?;
                Ok(())
            })
            .unwrap();
        client
            .post(format!("{base}__race"))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
        assert!(
            dav::sync(&storage, std::slice::from_ref(&account))
                .await
                .unwrap()
                .contains("1 conflict")
        );
        service.reload(&storage).unwrap();
        let raced = service
            .data()
            .resources
            .iter()
            .find(|r| r.id == id)
            .unwrap();
        assert!(raced.source.contains("Local race edit"));
        assert!(
            raced
                .conflict
                .as_ref()
                .unwrap()
                .remote
                .as_ref()
                .unwrap()
                .source
                .contains("Remote race winner")
        );
        service
            .update(&storage, |data| {
                let r = data.resources.iter_mut().find(|r| r.id == id).unwrap();
                let incoming = r.conflict.take().unwrap().remote.unwrap();
                r.source.clone_from(&incoming.source);
                r.remote = Some(incoming);
                Ok(())
            })
            .unwrap();
        service
            .update(&storage, |data| {
                data.resources
                    .iter_mut()
                    .find(|r| r.id == id)
                    .unwrap()
                    .deleted = true;
                Ok(())
            })
            .unwrap();
        dav::sync(&storage, std::slice::from_ref(&account))
            .await
            .unwrap();
        service.reload(&storage).unwrap();
        assert!(!service.data().resources.iter().any(|r| r.id == id));
        let before = fs::read(storage.data_dir.join(".clin/calendar.json")).unwrap();
        client
            .post(format!("{base}__fail"))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
        assert!(dav::sync(&storage, &[account]).await.is_err());
        assert_eq!(
            fs::read(storage.data_dir.join(".clin/calendar.json")).unwrap(),
            before
        );
    });
}
