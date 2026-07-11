//! Persistent, rotating sync log. Sync failures are otherwise invisible in release
//! builds (desktop logs go to stderr, mobile to oslog/logcat), so every push/pull
//! outcome and phase-affecting failure is appended here and surfaced through the
//! `get_sync_diagnostics` command ("Copy sync diagnostics" in both apps).
//!
//! Best-effort by design: logging must never fail or slow a sync, so all I/O errors
//! are swallowed.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::Mutex;

/// Rotate once the active log exceeds this size (one `.1` generation is kept).
const MAX_LOG_BYTES: u64 = 512 * 1024;

pub struct SyncLog {
    path: PathBuf,
    /// Serializes append+rotate across concurrent sync tasks.
    lock: Mutex<()>,
}

impl SyncLog {
    pub fn new(app_data_dir: &std::path::Path) -> Self {
        let dir = app_data_dir.join("logs");
        let _ = fs::create_dir_all(&dir);
        Self {
            path: dir.join("sync.log"),
            lock: Mutex::new(()),
        }
    }

    /// Append one timestamped line, rotating first if the log is over budget.
    pub fn append(&self, line: &str) {
        let Ok(_guard) = self.lock.lock() else {
            return;
        };
        if let Ok(meta) = fs::metadata(&self.path) {
            if meta.len() > MAX_LOG_BYTES {
                let _ = fs::rename(&self.path, self.path.with_extension("log.1"));
            }
        }
        let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        else {
            return;
        };
        let _ = writeln!(file, "{} {line}", chrono::Utc::now().to_rfc3339());
    }

    /// The last `max_bytes` of the active log (for the copy-diagnostics affordance).
    pub fn tail(&self, max_bytes: u64) -> String {
        let Ok(_guard) = self.lock.lock() else {
            return String::new();
        };
        let Ok(mut file) = File::open(&self.path) else {
            return String::new();
        };
        let len = file.metadata().map(|m| m.len()).unwrap_or(0);
        if len > max_bytes {
            let _ = file.seek(SeekFrom::Start(len - max_bytes));
        }
        let mut out = String::new();
        let _ = file.read_to_string(&mut out);
        // A mid-file seek can land inside a line; drop the partial first line.
        if len > max_bytes {
            if let Some(idx) = out.find('\n') {
                out.drain(..=idx);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("crate-synclog-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn append_and_tail_roundtrip() {
        let dir = temp_dir("roundtrip");
        let log = SyncLog::new(&dir);
        log.append("push ok in 1.2s");
        log.append("pull failed kind=permission: HTTP 403");
        let tail = log.tail(64 * 1024);
        assert!(tail.contains("push ok in 1.2s"));
        assert!(tail.contains("kind=permission"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rotates_past_cap() {
        let dir = temp_dir("rotate");
        let log = SyncLog::new(&dir);
        let filler = "x".repeat(1024);
        for _ in 0..600 {
            log.append(&filler);
        }
        log.append("after rotation");
        assert!(dir.join("logs/sync.log.1").exists());
        let active = fs::metadata(dir.join("logs/sync.log")).unwrap().len();
        assert!(active < MAX_LOG_BYTES);
        assert!(log.tail(64 * 1024).contains("after rotation"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tail_drops_partial_first_line() {
        let dir = temp_dir("tail");
        let log = SyncLog::new(&dir);
        for i in 0..100 {
            log.append(&format!("line number {i}"));
        }
        let tail = log.tail(200);
        assert!(!tail.is_empty());
        assert!(tail.starts_with(|c: char| c.is_ascii_digit())); // RFC 3339 year
        let _ = fs::remove_dir_all(&dir);
    }
}
