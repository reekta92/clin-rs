//! Opt-in release acceptance over a representative disposable public vault.
use super::*;
use crate::bases::{eval::Environment, format::BaseDocument, query::Query};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

fn rss_kib() -> Option<usize> {
    fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find(|line| line.starts_with("VmRSS:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}
fn report(label: &str, mut samples: Vec<Duration>, limit: Duration) {
    samples.sort();
    let p95 = samples[(samples.len() * 95 / 100).min(samples.len() - 1)];
    eprintln!(
        "{label}: p50 {:?}; p95 {p95:?}; max {:?}",
        samples[samples.len() / 2],
        samples.last().unwrap()
    );
    assert!(p95 <= limit, "{label} p95 exceeds {limit:?}");
}

#[test]
#[ignore = "full 5,000-note native acceptance; run with --release --ignored --nocapture"]
fn native_5000_note_acceptance() {
    let (root, mut app) = fixture();
    let vault = app.storage.notes_dir.clone();
    fs::create_dir_all(vault.join(".obsidian")).unwrap();
    fs::write(vault.join(".obsidian/types.json"), r#"{"types":{"price":"number","amount":"number","score":"number","done":"checkbox","day":"date","when":"datetime","labels":"multitext","tags":"tags","owner":"text","description":"text"}}"#).unwrap();
    for index in 0..5000 {
        let id = if index == 0 {
            "public.md".to_owned()
        } else {
            format!("group{:02}/note{:04}.md", index % 20, index / 20)
        };
        let path = vault.join(&id);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let links = (0..9)
            .map(|offset| {
                format!(
                    "[[group{:02}/note{:04}.md]]",
                    (index + offset + 1) % 20,
                    (index + offset + 1) % 5000 / 20
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        let source = format!(
            "---\nprice: {}\namount: {}\nscore: {}\nstatus: group{}\ndone: {}\nday: 2026-10-15\nwhen: 2026-10-15T12:30:00\nlabels: [one, two]\ntags: [work, bench/nested]\nowner: owner{}\ndescription: '{}'\nempty: ''\nzero: 0\nfalse_value: false\nnullable: null\nlink: '[[public.md]]'\nweight: 1.5\ncategory: native\npriority: 3\ncolor: '#f00'\n---\n{links}\n{}",
            index % 50,
            index % 7,
            index,
            index % 20,
            index % 2 == 0,
            index % 10,
            "Public fixture. ".repeat(45),
            "Body. ".repeat(1700)
        );
        fs::write(path, source).unwrap();
    }
    for index in 0..500 {
        fs::write(vault.join(format!("attachment{index:03}.bin")), [0_u8; 64]).unwrap();
    }
    let order = "[file.name, price, amount, score, status, done, day, when, labels, tags, owner, description, empty, zero, false_value, nullable, link, weight, category, priority, formula.total]";
    let mut source =
        "filters: file.ext == 'md'\nformulas: {total: 'price * amount'}\nviews:\n".to_owned();
    for layout in ["table", "list", "cards", "kanban"] {
        source.push_str(&format!("  - type: {layout}\n    name: {layout}\n    order: {order}\n    sort: [{{property: formula.total, direction: DESC}}, {{property: file.name}}]\n    groupBy: {{property: status}}\n    summaries: {{formula.total: Sum}}\n"));
    }
    let baseline = app.storage.load_document("board.base").unwrap();
    app.storage.save_document(&baseline, &source).unwrap();
    app.request_notes_reconcile();
    let deadline = Instant::now() + Duration::from_secs(20);
    while app.notes.len() < 5000 || app.catalog_status.is_some() {
        while let Ok(event) = app.catalog_event_rx.try_recv() {
            app.handle_catalog_event(event);
        }
        assert!(
            Instant::now() < deadline,
            "host catalog availability timed out"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    let baseline_rss = rss_kib();
    let cold = Instant::now();
    key(&mut app, KeyCode::Char('r'));
    idle(&mut app);
    eprintln!("cold metadata + first query: {:?}", cold.elapsed());
    assert!(cold.elapsed() <= Duration::from_secs(3));
    assert!(
        app.bases_state.as_ref().unwrap().error.is_none(),
        "{}",
        app.bases_state
            .as_ref()
            .unwrap()
            .error
            .as_deref()
            .unwrap_or("")
    );
    assert_eq!(
        app.bases_state
            .as_ref()
            .unwrap()
            .result()
            .unwrap()
            .rows
            .len(),
        5000
    );
    eprintln!("RSS after cold query: {:?} KiB", rss_kib());
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    for layout in ["table", "list", "cards", "kanban"] {
        let mut samples = Vec::new();
        for _ in 0..50 {
            let start = Instant::now();
            terminal
                .draw(|frame| crate::ui::draw_ui(frame, &mut app, crate::editor::EditFocus::Body))
                .unwrap();
            samples.push(start.elapsed());
        }
        report(
            &format!("{layout} native 120x40 render (no terminal I/O)"),
            samples,
            Duration::from_millis(16),
        );
        eprintln!("RSS after {layout}: {:?} KiB", rss_kib());
        key(&mut app, KeyCode::Tab);
        idle(&mut app);
    }
    key(&mut app, KeyCode::Char('r'));
    let input = Instant::now();
    key(&mut app, KeyCode::Char('j'));
    eprintln!("native input while indexing: {:?}", input.elapsed());
    assert!(input.elapsed() <= Duration::from_millis(50));
    idle(&mut app);
    eprintln!("RSS after index refresh: {:?} KiB", rss_kib());
    let (sender, receiver) = std::sync::mpsc::sync_channel(32);
    app.fs_event_rx = Some(receiver);
    let mut refresh = Vec::new();
    for index in 0..10 {
        let host = app.storage.load_document("public.md").unwrap();
        app.storage
            .save_document(
                &host,
                &host.source().replace(
                    &format!("score: {index}\n"),
                    &format!("score: {}\n", index + 1),
                ),
            )
            .unwrap();
        let start = Instant::now();
        sender
            .send(crate::app::WatchedFsEvent {
                observed_at: start,
                event: notify::Event::new(notify::EventKind::Modify(
                    notify::event::ModifyKind::Data(notify::event::DataChange::Content),
                ))
                .add_path(vault.join("public.md")),
            })
            .unwrap();
        loop {
            crate::process_watcher_events(&mut app);
            app.poll_bases();
            let current = app
                .bases_state
                .as_ref()
                .unwrap()
                .result()
                .unwrap()
                .rows
                .iter()
                .find(|row| row.id == "public.md")
                .unwrap();
            if matches!(current.values["score"], crate::bases::value::Value::Number(value) if value == f64::from(index + 1))
            {
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(2));
        }
        refresh.push(start.elapsed());
    }
    report(
        "single-note watcher-to-results (including 250ms debounce)",
        refresh,
        Duration::from_millis(500),
    );
    eprintln!("RSS after watcher refreshes: {:?} KiB", rss_kib());
    app.bases_state = None;
    app.mode = ViewMode::List;
    let renderer = super::embeds::markdown(
        "![[board.base#table]]\n\n![[board.base#list]]\n",
        Some("public.md"),
    );
    for cycle in 0..4 {
        app.base_embeds.begin_frame();
        app.poll_bases();
        app.invalidate_bases();
        let start = Instant::now();
        loop {
            super::embeds::screen(&mut app, &renderer);
            app.poll_bases();
            if !app.base_embeds.is_loading() {
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(2));
        }
        eprintln!(
            "two live embeds after close/reopen {cycle}: {:?}; RSS {:?} KiB",
            start.elapsed(),
            rss_kib()
        );
    }
    if let (Some(baseline), Some(current)) = (baseline_rss, rss_kib()) {
        let extra = current.saturating_sub(baseline);
        eprintln!(
            "incremental Bases process RSS: {extra} KiB (host {baseline} KiB; current {current} KiB; no decoded covers)"
        );
        assert!(extra <= 128 * 1024);
    }
    // Measure native state before standalone evaluator microbenchmarks retain allocator arenas.
    let metadata = Arc::new(
        app.storage
            .load_vault_metadata(&AtomicBool::new(false))
            .unwrap(),
    );
    let query = Query::compile(&BaseDocument::parse(&source).unwrap(), 0).unwrap();
    let mut warm = Vec::new();
    for _ in 0..30 {
        let start = Instant::now();
        let result = query
            .execute(&metadata, &Environment::default(), &AtomicBool::new(false))
            .unwrap();
        warm.push(start.elapsed());
        assert_eq!(result.rows.len(), 5000);
    }
    report("warm full query", warm, Duration::from_millis(150));
    let flag = Arc::new(AtomicBool::new(false));
    let worker_flag = flag.clone();
    let worker_metadata = metadata.clone();
    let worker = std::thread::spawn(move || {
        query.execute(&worker_metadata, &Environment::default(), &worker_flag)
    });
    std::thread::sleep(Duration::from_millis(2));
    let cancel = Instant::now();
    flag.store(true, Ordering::Relaxed);
    let error = worker.join().unwrap().unwrap_err();
    eprintln!(
        "ordinary query cancellation: {:?}; {error}",
        cancel.elapsed()
    );
    assert!(cancel.elapsed() <= Duration::from_millis(100));
    drop(metadata);
    assert_eq!(
        fs::read(vault.join("private.clin")).unwrap(),
        b"not a public document"
    );
    eprintln!(
        "retained disposable acceptance vault: {}",
        root.keep().display()
    );
}
