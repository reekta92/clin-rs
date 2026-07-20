#[cfg(test)]
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
                        if parts.len() >= 2 {
                            if let Ok(kb) = parts[1].parse::<usize>() {
                                return kb / 1024;
                            }
                        }
                    }
                }
            }
        }
        0
    }

    fn generate_vault_if_needed(dir: &PathBuf, notes: usize, folders: usize) {
        if dir.exists() {
            if let Ok(entries) = std::fs::read_dir(dir) {
                if entries.count() > 0 {
                    return;
                }
            }
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

    #[test]
    #[ignore]
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
}
