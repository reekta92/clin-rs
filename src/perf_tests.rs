#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use crate::app::App;
    use crate::editor_document::EditorDocument;
    use crate::storage::Storage;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;
    use std::time::Instant;

    fn get_peak_rss_mb() -> usize {
        #[cfg(target_os = "linux")]
        {
            if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
                for line in status.lines() {
                    if line.starts_with("VmHWM:") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2
                            && let Ok(kb) = parts[1].parse::<usize>()
                        {
                            return kb / 1024;
                        }
                    }
                }
            }
        }
        0
    }

    fn generate_vault_if_needed(dir: &PathBuf, notes: usize, folders: usize) {
        if dir.exists()
            && let Ok(entries) = std::fs::read_dir(dir)
            && entries.count() > 0
        {
            return;
        }
        let status = std::process::Command::new("python3")
            .arg("dev_scripts/generate_perf_vault.py")
            .arg("--output")
            .arg(dir)
            .arg("--notes")
            .arg(notes.to_string())
            .arg("--folders")
            .arg(folders.to_string())
            .arg("--tags")
            .arg("1000")
            .arg("--avg-links")
            .arg("4")
            .arg("--isolate-ratio")
            .arg("0.0")
            .arg("--bytes-per-note")
            .arg("2048")
            .arg("--common-token-every")
            .arg("2")
            .arg("--seed")
            .arg("42")
            .status()
            .expect("failed generating test vault");
        assert!(status.success());
    }

    #[ignore = "performance test, run manually"]
    fn large_notes_vault_perf() {
        let enforce = std::env::var("CLIN_PERF_ENFORCE").as_deref() == Ok("1");
        let general_dir = PathBuf::from("/tmp/clin-perf-notes");
        let flat_dir = PathBuf::from("/tmp/clin-perf-flat");

        generate_vault_if_needed(&general_dir, 10000, 500);
        generate_vault_if_needed(&flat_dir, 10000, 1);

        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();

        let t0 = Instant::now();
        let storage = Storage {
            data_dir: general_dir.clone(),
            config_dir: general_dir.clone(),
            notes_dir: general_dir.clone(),
            templates_dir: general_dir.clone(),
            key: [1u8; 32],
            skip_dir_patterns: vec![],
        };
        let mut app = App::new_deferred(storage).unwrap();
        terminal
            .draw(|f| crate::ui::draw_ui(f, &mut app, crate::editor::EditFocus::Body))
            .unwrap();
        let warm_first_frame_ms = t0.elapsed().as_millis();
        println!("Warm first frame time: {} ms", warm_first_frame_ms);

        if enforce {
            assert!(
                warm_first_frame_ms <= 250,
                "warm first frame <= 250ms, got {}",
                warm_first_frame_ms
            );
        }

        let mut frame_times = Vec::new();
        for _ in 0..200 {
            let start = Instant::now();
            terminal
                .draw(|f| crate::ui::draw_ui(f, &mut app, crate::editor::EditFocus::Body))
                .unwrap();
            frame_times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        frame_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p95_idx = (frame_times.len() as f64 * 0.95) as usize;
        let p95_frame_ms = frame_times[p95_idx];
        println!("Tree frame p95: {:.2} ms", p95_frame_ms);

        if enforce {
            assert!(
                p95_frame_ms <= 8.0,
                "tree frame p95 <= 8ms, got {:.2}ms",
                p95_frame_ms
            );
        }

        let rss_mb = get_peak_rss_mb();
        println!("Peak RSS: {} MiB", rss_mb);
        if enforce && rss_mb > 0 {
            assert!(rss_mb <= 300, "Peak RSS <= 300 MiB, got {} MiB", rss_mb);
        }

        println!("Performance test completed successfully!");
    }

    /// Prove that the per-line highlight memo produces identical results to a full rebuild.
    #[test]
    fn edit_highlight_memo_matches_full_rebuild() {
        let _lock = crate::config::ConfigTestGuard::lock();
        use crate::storage::Storage;
        use tempfile::tempdir;

        let temp_dir = tempdir().expect("tempdir");
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        std::fs::create_dir_all(&data_dir).expect("create data");
        std::fs::create_dir_all(&config_dir).expect("create config");
        std::fs::create_dir_all(&notes_dir).expect("create notes");
        std::fs::create_dir_all(&templates_dir).expect("create templates");

        let storage = Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
        };

        let lines = vec![
            "# Heading 1".to_string(),
            "paragraph with **bold** and `code` inline.".to_string(),
            "".to_string(),
            "```rust".to_string(),
            "fn main() {".to_string(),
            "    println!(\"hello\");".to_string(),
            "}".to_string(),
            "```".to_string(),
            "- list item".to_string(),
            "- [ ] task".to_string(),
            "> blockquote".to_string(),
        ];

        // App A: full rebuild (fresh editor, stale cache)
        let mut app_a = App::new(storage).expect("app a");
        app_a.editor.body = EditorDocument::from_lines(lines.clone());
        app_a.editor.show_line_numbers = true;
        app_a.request_editor_preview_update();
        let backend = ratatui::backend::TestBackend::new(120, 20);
        let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
        terminal
            .draw(|f| {
                crate::ui::render_editor_document_with_theme(
                    f,
                    &mut app_a.editor.body,
                    f.area(),
                    &app_a.app_theme,
                    true,
                    true,
                    ratatui::widgets::Block::default(),
                    app_a.app_theme.bg_style(),
                );
                crate::ui::overlay_markdown_highlight(f, &mut app_a, f.area());
            })
            .unwrap();
        let cache_a: Vec<Vec<ratatui::style::Style>> = app_a
            .editor
            .md_highlight_cache
            .iter()
            .map(|rc| rc.to_vec())
            .collect();

        // App B: same doc, fresh — should match A (memo path == full rebuild)
        let storage_b = Storage {
            data_dir: temp_dir.path().join("data_b"),
            config_dir: temp_dir.path().join("config_b"),
            notes_dir: temp_dir.path().join("notes_b"),
            templates_dir: temp_dir.path().join("templates_b"),
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
        };
        std::fs::create_dir_all(storage_b.data_dir.clone()).unwrap();
        std::fs::create_dir_all(storage_b.config_dir.clone()).unwrap();
        std::fs::create_dir_all(storage_b.notes_dir.clone()).unwrap();
        std::fs::create_dir_all(storage_b.templates_dir.clone()).unwrap();
        let mut app_b = App::new(storage_b).expect("app b");
        app_b.editor.body = EditorDocument::from_lines(lines.clone());
        app_b.editor.show_line_numbers = true;
        app_b.request_editor_preview_update();
        let backend = ratatui::backend::TestBackend::new(120, 20);
        let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
        terminal
            .draw(|f| {
                crate::ui::render_editor_document_with_theme(
                    f,
                    &mut app_b.editor.body,
                    f.area(),
                    &app_b.app_theme,
                    true,
                    true,
                    ratatui::widgets::Block::default(),
                    app_b.app_theme.bg_style(),
                );
                crate::ui::overlay_markdown_highlight(f, &mut app_b, f.area());
            })
            .unwrap();
        let cache_b: Vec<Vec<ratatui::style::Style>> = app_b
            .editor
            .md_highlight_cache
            .iter()
            .map(|rc| rc.to_vec())
            .collect();

        assert_eq!(
            cache_a, cache_b,
            "full rebuilds must produce identical caches"
        );

        // Edit one line, redraw A — memo should still match a full rebuild of the edited doc
        app_a
            .editor
            .body
            .move_cursor(ratatui_textarea::CursorMove::Jump(1, 10));
        app_a.editor.body.insert_str("x");
        app_a.request_editor_preview_update();
        let backend = ratatui::backend::TestBackend::new(120, 20);
        let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
        terminal
            .draw(|f| {
                crate::ui::render_editor_document_with_theme(
                    f,
                    &mut app_a.editor.body,
                    f.area(),
                    &app_a.app_theme,
                    true,
                    true,
                    ratatui::widgets::Block::default(),
                    app_a.app_theme.bg_style(),
                );
                crate::ui::overlay_markdown_highlight(f, &mut app_a, f.area());
            })
            .unwrap();

        let mut edited_lines = lines;
        edited_lines[1].insert(10, 'x');
        let storage_c = Storage {
            data_dir: temp_dir.path().join("data_c"),
            config_dir: temp_dir.path().join("config_c"),
            notes_dir: temp_dir.path().join("notes_c"),
            templates_dir: temp_dir.path().join("templates_c"),
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
        };
        std::fs::create_dir_all(storage_c.data_dir.clone()).unwrap();
        std::fs::create_dir_all(storage_c.config_dir.clone()).unwrap();
        std::fs::create_dir_all(storage_c.notes_dir.clone()).unwrap();
        std::fs::create_dir_all(storage_c.templates_dir.clone()).unwrap();
        let mut app_c = App::new(storage_c).expect("app c");
        app_c.editor.body = EditorDocument::from_lines(edited_lines);
        app_c.editor.show_line_numbers = true;
        app_c.request_editor_preview_update();
        let backend = ratatui::backend::TestBackend::new(120, 20);
        let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
        terminal
            .draw(|f| {
                crate::ui::render_editor_document_with_theme(
                    f,
                    &mut app_c.editor.body,
                    f.area(),
                    &app_c.app_theme,
                    true,
                    true,
                    ratatui::widgets::Block::default(),
                    app_c.app_theme.bg_style(),
                );
                crate::ui::overlay_markdown_highlight(f, &mut app_c, f.area());
            })
            .unwrap();
        let cache_c: Vec<Vec<ratatui::style::Style>> = app_c
            .editor
            .md_highlight_cache
            .iter()
            .map(|rc| rc.to_vec())
            .collect();
        let cache_a2: Vec<Vec<ratatui::style::Style>> = app_a
            .editor
            .md_highlight_cache
            .iter()
            .map(|rc| rc.to_vec())
            .collect();
        assert_eq!(
            cache_a2, cache_c,
            "memo path must match full rebuild after edit"
        );
    }

    /// Times the highlight overlay on 5000 lines, 60 keystrokes.
    /// Guarantees per-keystroke overhead stays under 8ms.
    #[ignore = "performance test, run manually"]
    #[test]
    fn edit_view_highlight_perf() {
        let _lock = crate::config::ConfigTestGuard::lock();
        use crate::storage::Storage;
        use tempfile::tempdir;

        let temp_dir = tempdir().expect("tempdir");
        let dirs: Vec<_> = ["data", "config", "notes", "templates"]
            .iter()
            .map(|d| {
                let p = temp_dir.path().join(d);
                std::fs::create_dir_all(&p).expect("create dir");
                p
            })
            .collect();

        let storage = Storage {
            data_dir: dirs[0].clone(),
            config_dir: dirs[1].clone(),
            notes_dir: dirs[2].clone(),
            templates_dir: dirs[3].clone(),
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
        };

        // Generate 5000-line synthetic markdown
        let mut lines = Vec::with_capacity(5000);
        let cycle = [
            "# Section",
            "paragraph with normal text content here.",
            "more text with **bold** styling embedded.",
            "",
            "```rust",
            "fn main() {",
            "    let x: i32 = 42;",
            "    println!(\"val: {x}\");",
            "}",
            "```",
            "- list item one",
            "- list item two",
            "- [ ] unchecked task",
            "> a blockquote line",
        ];
        for i in 0..5000 {
            lines.push(cycle[i % cycle.len()].to_string());
            if i % 40 == 0 && i > 0 {
                lines.push(format!("## Subheading {i}"));
            }
        }

        let mut app = App::new(storage).expect("app");
        app.editor.body = EditorDocument::from_lines(lines);
        app.editor.show_line_numbers = true;
        app.request_editor_preview_update();

        let enforce = std::env::var("CLIN_PERF_ENFORCE").as_deref() == Ok("1");

        let t0 = std::time::Instant::now();
        for _ in 0..60 {
            app.editor.body.insert_str("x");
            app.request_editor_preview_update();
            let backend = ratatui::backend::TestBackend::new(120, 40);
            let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
            terminal
                .draw(|f| {
                    crate::ui::render_editor_document_with_theme(
                        f,
                        &mut app.editor.body,
                        f.area(),
                        &app.app_theme,
                        true,
                        true,
                        ratatui::widgets::Block::default(),
                        app.app_theme.bg_style(),
                    );
                    crate::ui::overlay_markdown_highlight(f, &mut app, f.area());
                })
                .unwrap();
        }
        let elapsed_ms = t0.elapsed().as_millis();
        let per_key_ms = elapsed_ms as f64 / 60.0;
        println!("edit_view_highlight_perf: {elapsed_ms}ms total, {per_key_ms:.2}ms/keystroke");

        if enforce {
            assert!(
                elapsed_ms <= 480,
                "60 keystrokes <= 480ms (8ms/key), got {elapsed_ms}ms"
            );
        }
    }

    fn editor_fixture(lines: usize) -> Vec<String> {
        let patterns = [
            "# Heading αβγ e\u{301} 🚀",
            "Prose with **bold**, [[links]], CJK 漢字, emoji 🚀, and combining e\u{301}.",
            "- [ ] Task item with markdown and Unicode αβγ.",
            "```rust",
            "fn main() { println!(\"code with emoji 🚀\"); }",
            "```",
            "> Quoted prose with links and punctuation.",
        ];
        (0..lines)
            .map(|index| {
                let mut line = format!(
                    "{} fixture-line-{index:06}",
                    patterns[index % patterns.len()]
                );
                while line.len() < 96 {
                    line.push_str(" padding");
                }
                assert!((80..=120).contains(&line.len()), "fixture line length");
                line
            })
            .collect()
    }

    fn percentile(samples: &mut [std::time::Duration], pct: f64) -> std::time::Duration {
        samples.sort_unstable();
        let index = ((samples.len() - 1) as f64 * pct).ceil() as usize;
        samples[index]
    }

    fn run_edit_pairs(
        line_count: usize,
        mut edit_pair: impl FnMut(usize, usize),
    ) -> Vec<std::time::Duration> {
        let positions = [0, line_count / 2, line_count.saturating_sub(1)];
        let mut samples = Vec::with_capacity(positions.len() * 200);
        for row in positions {
            for _ in 0..20 {
                edit_pair(row, 0);
            }
            for _ in 0..200 {
                let started = Instant::now();
                edit_pair(row, 0);
                samples.push(started.elapsed());
            }
        }
        samples
    }

    fn move_raw_to(
        textarea: &mut ratatui_textarea::TextArea<'static>,
        row: usize,
        line_count: usize,
    ) {
        if row == line_count.saturating_sub(1) {
            textarea.move_cursor(ratatui_textarea::CursorMove::Bottom);
            textarea.move_cursor(ratatui_textarea::CursorMove::Head);
        } else {
            textarea.move_cursor(ratatui_textarea::CursorMove::Jump(row as u16, 0));
        }
    }

    fn move_document_to(document: &mut EditorDocument, row: usize, line_count: usize) {
        if row == line_count.saturating_sub(1) {
            document.move_cursor(ratatui_textarea::CursorMove::Bottom);
            document.move_cursor(ratatui_textarea::CursorMove::Head);
        } else {
            document.move_cursor(ratatui_textarea::CursorMove::Jump(row as u16, 0));
        }
    }

    fn find_once(lines: &[String], query: &str) -> usize {
        let query = query.to_lowercase();
        lines
            .iter()
            .filter(|line| line.to_lowercase().contains(&query))
            .count()
    }

    /// Measured decision input for `EditorDocument` backend selection.
    #[ignore = "performance test, run manually"]
    #[test]
    fn editor_buffer_comparison_perf() {
        let enforce = std::env::var("CLIN_PERF_ENFORCE").as_deref() == Ok("1");
        let mut replacement_required = false;

        for line_count in [5_000_usize, 100_000] {
            let lines = editor_fixture(line_count);
            for wrap in [
                ratatui_textarea::WrapMode::None,
                ratatui_textarea::WrapMode::WordOrGlyph,
            ] {
                let mut raw = ratatui_textarea::TextArea::from(lines.clone());
                raw.set_wrap_mode(wrap);
                let mut raw_samples = run_edit_pairs(line_count, |row, _| {
                    move_raw_to(&mut raw, row, line_count);
                    assert!(raw.insert_str("x"));
                    move_raw_to(&mut raw, row, line_count);
                    assert!(raw.delete_str(1));
                });

                let mut document = EditorDocument::from_lines(lines.clone());
                document.set_wrap_mode(wrap);
                let mut document_samples = run_edit_pairs(line_count, |row, _| {
                    move_document_to(&mut document, row, line_count);
                    assert!(document.insert_str("x").content_changed);
                    move_document_to(&mut document, row, line_count);
                    assert!(document.delete_str(1).content_changed);
                });

                let raw_median = percentile(&mut raw_samples, 0.50);
                let raw_p95 = percentile(&mut raw_samples, 0.95);
                let document_median = percentile(&mut document_samples, 0.50);
                let document_p95 = percentile(&mut document_samples, 0.95);
                replacement_required |= document_p95 > std::time::Duration::from_millis(1);
                println!(
                    "editor_buffer_comparison_perf lines={line_count} wrap={wrap:?} raw median={raw_median:?} p95={raw_p95:?} document median={document_median:?} p95={document_p95:?}"
                );

                for query in ["fixture-line-000001", "漢字", "absent-token"] {
                    let mut raw_find = Vec::with_capacity(100);
                    let mut document_find = Vec::with_capacity(100);
                    for _ in 0..100 {
                        let started = Instant::now();
                        let _ = find_once(raw.lines(), query);
                        raw_find.push(started.elapsed());
                        let started = Instant::now();
                        let _ = find_once(document.lines(), query);
                        document_find.push(started.elapsed());
                    }
                    println!(
                        "editor_find_perf lines={line_count} wrap={wrap:?} query={query:?} raw_p95={:?} document_p95={:?}",
                        percentile(&mut raw_find, 0.95),
                        percentile(&mut document_find, 0.95),
                    );
                }
            }
        }
        println!(
            "editor_buffer_comparison_perf replacement_required={replacement_required} profile={} cpu={}",
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
            std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_string()),
        );
        if enforce {
            assert!(
                !replacement_required,
                "current wrapped TextArea exceeds 1ms body-mutation p95; Rope cutover required"
            );
        }
    }
}
