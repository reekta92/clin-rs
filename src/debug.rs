use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;

use crate::app::App;

// ---------------------------------------------------------------------------
// Log level
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

impl LogLevel {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            LogLevel::Error => "ERROR",
            LogLevel::Warn => "WARN",
            LogLevel::Info => "INFO",
            LogLevel::Debug => "DEBUG",
        }
    }
}

// ---------------------------------------------------------------------------
// A single ring-buffer entry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: chrono::DateTime<Local>,
    pub level: LogLevel,
    pub target: &'static str,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Ring buffer holding the most recent N entries
// ---------------------------------------------------------------------------

pub struct DebugBuffer {
    entries: VecDeque<LogEntry>,
    max_size: usize,
    dump_dir: PathBuf,
    dump_count: usize,
}

/// Channel sender for off-thread log entries.
/// Level, target, message — timestamp added on the receiving end.
pub type LogSender = std::sync::mpsc::Sender<(LogLevel, &'static str, String)>;
pub type LogReceiver = std::sync::mpsc::Receiver<(LogLevel, &'static str, String)>;

impl DebugBuffer {
    /// Create a new buffer.  `max_size` caps the ring.  `data_dir` is the
    /// application data directory; a `debug/` subdirectory is created inside it.
    pub fn new(max_size: usize, data_dir: &Path) -> Self {
        let dump_dir = data_dir.join("debug");
        // Ignore creation error — dump_to_file will surface it when needed.
        let _ = fs::create_dir_all(&dump_dir);
        DebugBuffer {
            entries: VecDeque::with_capacity(max_size),
            max_size,
            dump_dir,
            dump_count: 0,
        }
    }

    /// Push a new entry.  Drops the oldest once `max_size` is reached.
    pub fn log(&mut self, level: LogLevel, target: &'static str, message: String) {
        if self.entries.len() >= self.max_size {
            self.entries.pop_front();
        }
        self.entries.push_back(LogEntry {
            timestamp: Local::now(),
            level,
            target,
            message,
        });
    }

    /// Convenience: render + write in one call.
    pub fn dump_to_file(&mut self, app: &App) -> anyhow::Result<PathBuf> {
        let content = self.render_dump(app);
        self.write_dump(content)
    }
    /// Write pre-rendered dump content to a timestamped file.
    pub fn write_dump(&mut self, content: String) -> anyhow::Result<PathBuf> {
        let _ = fs::create_dir_all(&self.dump_dir);
        let ts = Local::now().format("%Y%m%dT%H%M%S");
        let stem = if self.dump_count > 0 {
            format!("clin-debug-{ts}-{}.log", self.dump_count)
        } else {
            format!("clin-debug-{ts}.log")
        };
        self.dump_count += 1;
        let path = self.dump_dir.join(&stem);
        crate::fsutil::atomic_write(&path, content.as_bytes())?;
        self.prune_old_dumps(10);
        Ok(path)
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

/// Recursively redact sensitive config fields from a JSON Value tree.
fn redact_config_value(value: &mut serde_json::Value, path: &str) {
    const SENSITIVE: &[&str] = &[
        "core.storage_path",
        "core.previous_storage_path",
        "core.default_folder",
        "backup.remote_url",
        "backup.remote_name",
        "editor.external_command",
    ];
    match value {
        serde_json::Value::Object(map) => {
            let keys: Vec<String> = map.keys().cloned().collect();
            for key in &keys {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                if SENSITIVE.contains(&child_path.as_str()) {
                    map.insert(key.clone(), serde_json::Value::String("***redacted***".to_string()));
                } else if let Some(child) = map.get_mut(key) {
                    Self::redact_config_value(child, &child_path);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for (i, item) in arr.iter_mut().enumerate() {
                Self::redact_config_value(item, &format!("{path}[{i}]"));
            }
        }
        _ => {}
    }
}

    pub(crate) fn render_dump(&self, app: &App) -> String {
        use std::fmt::Write;

        let ts = Local::now().format("%Y-%m-%dT%H:%M:%S%.3f");
        let mut out = String::new();
        let _ = writeln!(out, "=== clin debug dump {ts} ===");
        let _ = writeln!(out);

        // ── App state ──
        let _ = writeln!(out, "-- App state --");
        let _ = writeln!(out, "Mode: {:?}", app.mode);
        let _ = writeln!(out, "Notes loaded: {}", app.notes.len());
        let _ = writeln!(out, "View: {:?}", app.list.notes_layout);
        if app.popups.has_any() {
            let _ = writeln!(out, "Popups: active");
        } else {
            let _ = writeln!(out, "Popups: none");
        }
        let _ = writeln!(out);

        // ── Config (JSON, sensitive fields redacted) ──
        let _ = writeln!(out, "-- Config (JSON) --");
        let mut config_value = serde_json::to_value(&app.config).unwrap_or_else(|_| {
            serde_json::Value::Object(serde_json::Map::new())
        });
        Self::redact_config_value(&mut config_value, "");
        let config_json = serde_json::to_string_pretty(&config_value).unwrap_or_else(|_| "{}".to_string());
        let _ = writeln!(out, "{config_json}");
        let _ = writeln!(out);

        // ── Ring buffer ──
        let total = self.max_size;
        let present = self.entries.len();
        let _ = writeln!(
            out,
            "-- Ring buffer entries ({total} max, {present} present) --"
        );
        for entry in &self.entries {
            let ts = entry.timestamp.format("%Y-%m-%dT%H:%M:%S%.3f");
            let level_str = entry.level.as_str();
            let level_padded = format!("[{level_str:<5}]");
            let _ = writeln!(
                out,
                "[{ts}] {level_padded} [{}] {}",
                entry.target, entry.message
            );
        }
        out
    }

    /// Keep the N most recent dump files (sort by filename).
    fn prune_old_dumps(&self, keep: usize) {
        let entries = match fs::read_dir(&self.dump_dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        let mut files: Vec<std::fs::DirEntry> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .is_some_and(|n| n.starts_with("clin-debug-") && n.ends_with(".log"))
            })
            .collect();

        // Newest-first by mtime; use filename as tiebreaker for same-second dumps.
        files.sort_by(|a, b| {
            let a_mtime = a.metadata().and_then(|m| m.modified()).ok();
            let b_mtime = b.metadata().and_then(|m| m.modified()).ok();
            b_mtime.cmp(&a_mtime).then_with(|| b.file_name().cmp(&a.file_name()))
        });

        for f in files.into_iter().skip(keep) {
            let _ = fs::remove_file(f.path());
        }
    }
}

// ---------------------------------------------------------------------------
// Convenience macro – requires `app` to be a mutable binding of `App`.
// ---------------------------------------------------------------------------

#[macro_export]
macro_rules! debug_log {
    ($app:expr, $level:ident, $target:expr, $($arg:tt)+) => {
        $app.debug_buffer.log(
            $crate::debug::LogLevel::$level,
            $target,
            format!($($arg)+),
        );
    };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    // ------------------------------------------------------------------
    // Helper: build a minimal App for dump tests
    // ------------------------------------------------------------------
    fn dummy_app() -> App {
        let tmp = tempfile::tempdir().expect("tempdir");
        let data_dir = tmp.path().to_path_buf();
        let config_dir = data_dir.join("config");
        let notes_dir = tmp.path().join("notes");
        let templates_dir = tmp.path().join("templates");
        std::fs::create_dir_all(&config_dir).ok();
        std::fs::create_dir_all(&notes_dir).ok();
        std::fs::create_dir_all(&templates_dir).ok();

        use crate::storage::Storage;
        let storage = Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0u8; 32],
        };

        let mut app = App::new(storage).expect("App::new");
        app.debug_buffer = DebugBuffer::new(10, tmp.path());
        app
    }

    // ------------------------------------------------------------------
    // Basic log / wrap behaviour
    // ------------------------------------------------------------------
    #[test]
    fn test_buffer_wrap() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut buf = DebugBuffer::new(5, tmp.path());

        for i in 0..7 {
            buf.log(LogLevel::Info, "test", format!("entry {i}"));
        }
        // After 7 pushes in 5-slot buffer: entries 0,1 dropped, 2..6 kept.
        assert_eq!(buf.entries.len(), 5);
        assert!(buf.entries[0].message.contains("entry 2"));
        assert!(buf.entries[4].message.contains("entry 6"));
    }

    // ------------------------------------------------------------------
    // Dump creates a file with expected content
    // ------------------------------------------------------------------
    #[test]
    fn test_dump_creates_file() {
        let mut app = dummy_app();

        app.debug_buffer
            .log(LogLevel::Info, "test", "hello".into());
        app.debug_buffer
            .log(LogLevel::Error, "storage", "boom".into());

        let content = app.debug_buffer.render_dump(&app);
        let path = app.debug_buffer.write_dump(content).expect("dump_to_file");

        assert!(path.exists(), "dump file should exist");
        assert!(
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("clin-debug-"),
            "bad filename prefix"
        );
        assert!(
            path.extension().and_then(|e| e.to_str()) == Some("log"),
            "bad extension"
        );

        let mut content = String::new();
        std::fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();

        assert!(content.contains("=== clin debug dump"));
        assert!(content.contains("-- App state --"));
        assert!(content.contains("-- Config (JSON) --"));
        assert!(content.contains("-- Ring buffer entries"));
        assert!(content.contains("[INFO ] [test]"));
        assert!(content.contains("***redacted***"), "sensitive fields should be redacted");
        // Verify known-sensitive fields are redacted in JSON output.
        assert!(
            content.contains(r#""storage_path": "***redacted***"#),
            "storage_path value should be redacted, not plaintext"
        );
    }

    // ------------------------------------------------------------------
    // Dump rotation (keep N most recent)
    // ------------------------------------------------------------------
    #[test]
    fn test_dump_rotation() {
        let tmp = tempfile::tempdir().expect("tempdir");

        // Write keep+3 dumps so rotation kicks in.
        let keep = 10;
        let n_writes = keep + 3;
        // We need an App for each dump call; reuse a dummy across writes.
        let mut app = dummy_app();
        // Override the buffer with ours (same dump_dir).
        app.debug_buffer = DebugBuffer::new(100, tmp.path());

        // Temporarily take ownership to make repeated dumps.
        let mut buf = std::mem::replace(&mut app.debug_buffer, DebugBuffer::new(100, tmp.path()));
        let mut filenames: Vec<String> = Vec::new();
        for _ in 0..n_writes {
            buf.log(LogLevel::Info, "test", "x".into());
            let path = buf.dump_to_file(&app).expect("dump");
            filenames.push(path.file_name().unwrap().to_string_lossy().to_string());
            // Ensure distinct mtimes for deterministic sort ordering.
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        app.debug_buffer = buf;
        // Count remaining debug-* files (written to the debug/ subdirectory).
        let dump_dir = app.debug_buffer.dump_dir.clone();
        let mut remaining: Vec<_> = std::fs::read_dir(&dump_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .is_some_and(|n| n.starts_with("clin-debug-"))
            })
            .collect();

        assert!(
            remaining.len() <= keep,
            "expected ≤{keep} files, got {}",
            remaining.len()
        );

        // Verify the kept files are the most recent ones.
        // Sorted newest-first by mtime means the first `keep` filenames
        // (from the end of the creation order) should survive.
        let expected_kept: Vec<&str> = filenames.iter().rev().take(keep).map(|s| s.as_str()).collect();
        let mut kept_names: Vec<String> = remaining.iter().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        kept_names.sort();
        let mut expected_sorted: Vec<&str> = expected_kept.clone();
        expected_sorted.sort();
        assert_eq!(kept_names, expected_sorted, "kept files should be the most recent ones");
    }

    // ------------------------------------------------------------------
    // Log-level formatting
    // ------------------------------------------------------------------
    #[test]
    fn test_log_level_padding() {
        let level_str = |lvl| {
            let s = match lvl {
                LogLevel::Error => "[ERROR]",
                LogLevel::Warn => "[WARN ]",
                LogLevel::Info => "[INFO ]",
                LogLevel::Debug => "[DEBUG]",
            };
            s.to_string()
        };
        assert_eq!(level_str(LogLevel::Error), "[ERROR]");
        assert_eq!(level_str(LogLevel::Warn), "[WARN ]");
        assert_eq!(level_str(LogLevel::Info), "[INFO ]");
        assert_eq!(level_str(LogLevel::Debug), "[DEBUG]");
    }
    // ------------------------------------------------------------------
    // Empty buffer dump
    // ------------------------------------------------------------------
    #[test]
    fn test_empty_buffer_dump() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut buf = DebugBuffer::new(5, tmp.path());
        let app = dummy_app();
        let out = buf.render_dump(&app);
        assert!(out.contains("=== clin debug dump"));
        assert!(out.contains("-- App state --"));
        assert!(out.contains("-- Ring buffer entries (5 max, 0 present) --"));
    }

    // ------------------------------------------------------------------
    // Large message in dump
    // ------------------------------------------------------------------
    #[test]
    fn test_large_message() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut buf = DebugBuffer::new(5, tmp.path());
        let big = "A".repeat(10_240);
        buf.log(LogLevel::Info, "test", big.clone());
        let app = dummy_app();
        let out = buf.render_dump(&app);
        assert!(out.contains(&big));
    }

    // ------------------------------------------------------------------
    // Unicode message survives round-trip
    // ------------------------------------------------------------------
    #[test]
    fn test_unicode_message() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut buf = DebugBuffer::new(5, tmp.path());
        // Emoji, CJK, and combining marks.
        let msg = "Hello \u{1f600} \u{4e2d}\u{6587} caff\u{e8} na\u{307}e".to_string();
        buf.log(LogLevel::Info, "test", msg.clone());
        let app = dummy_app();
        let out = buf.render_dump(&app);
        assert!(out.contains(&msg));
    }
}
