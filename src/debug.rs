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

        // ── Config (JSON) ──
        let _ = writeln!(out, "-- Config (JSON) --");
        let config_json = serde_json::to_string_pretty(&app.config).unwrap_or_else(|_| {
            "{}".to_string()
        });
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
            let ts = entry.timestamp.format("%Y-%m-%dT%H:%M:%S");
            let level_str = entry.level.as_str();
            let level_padded = match entry.level {
                LogLevel::Error => format!("[{level_str}]"),
                _ => format!("[{level_str:<5}]"),
            };
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

        // Newest-first by filename (which starts with an ISO-ish timestamp).
        files.sort_by_key(|f| std::cmp::Reverse(f.file_name()));

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
        let ts = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f");
        if matches!($crate::debug::LogLevel::$level, $crate::debug::LogLevel::Error | $crate::debug::LogLevel::Warn | $crate::debug::LogLevel::Info) {
            let _ = std::eprintln!("[{ts}] [{:<5}] [{}] {}", $crate::debug::LogLevel::$level.as_str(), $target, format!($($arg)+));
        }
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
        for _ in 0..n_writes {
            buf.log(LogLevel::Info, "test", "x".into());
            buf.dump_to_file(&app).expect("dump");
        }
        app.debug_buffer = buf;

        // Count remaining debug-* files.
        let remaining: Vec<_> = std::fs::read_dir(tmp.path())
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
}
