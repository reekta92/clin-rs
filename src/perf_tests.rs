#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use crate::app::App;
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
        app_a.editor.editor = ratatui_textarea::TextArea::from(lines.clone());
        app_a.editor.show_line_numbers = true;
        app_a.request_editor_preview_update();
        let backend = ratatui::backend::TestBackend::new(120, 20);
        let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
        terminal
            .draw(|f| {
                crate::ui::render_textarea_with_theme(
                    f,
                    &mut app_a.editor.editor,
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
        app_b.editor.editor = ratatui_textarea::TextArea::from(lines.clone());
        app_b.editor.show_line_numbers = true;
        app_b.request_editor_preview_update();
        let backend = ratatui::backend::TestBackend::new(120, 20);
        let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
        terminal
            .draw(|f| {
                crate::ui::render_textarea_with_theme(
                    f,
                    &mut app_b.editor.editor,
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
            .editor
            .move_cursor(ratatui_textarea::CursorMove::Jump(1, 10));
        app_a.editor.editor.insert_str("x");
        app_a.request_editor_preview_update();
        let backend = ratatui::backend::TestBackend::new(120, 20);
        let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
        terminal
            .draw(|f| {
                crate::ui::render_textarea_with_theme(
                    f,
                    &mut app_a.editor.editor,
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
        app_c.editor.editor = ratatui_textarea::TextArea::from(edited_lines);
        app_c.editor.show_line_numbers = true;
        app_c.request_editor_preview_update();
        let backend = ratatui::backend::TestBackend::new(120, 20);
        let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
        terminal
            .draw(|f| {
                crate::ui::render_textarea_with_theme(
                    f,
                    &mut app_c.editor.editor,
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
        app.editor.editor = ratatui_textarea::TextArea::from(lines);
        app.editor.show_line_numbers = true;
        app.request_editor_preview_update();

        let enforce = std::env::var("CLIN_PERF_ENFORCE").as_deref() == Ok("1");

        let t0 = std::time::Instant::now();
        for _ in 0..60 {
            app.editor.editor.insert_str("x");
            app.request_editor_preview_update();
            let backend = ratatui::backend::TestBackend::new(120, 40);
            let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
            terminal
                .draw(|f| {
                    crate::ui::render_textarea_with_theme(
                        f,
                        &mut app.editor.editor,
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
}
