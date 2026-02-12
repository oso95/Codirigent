//! Shared utilities for CLI session readers (Claude, Codex, Gemini).
//!
//! Contains common file I/O, timestamp checking, and directory scanning
//! functions used by multiple session reader implementations.

use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Read the last `max_bytes` of a file as a UTF-8 string.
///
/// If the file is smaller than `max_bytes`, returns the entire file.
/// If the file is larger, seeks to `max_bytes` before the end and discards
/// the first partial line to ensure clean line boundaries.
///
/// Returns `None` if the file cannot be opened or read.
pub fn read_file_tail(path: &Path, max_bytes: u64) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let file_len = file.metadata().ok()?.len();
    let seeked = file_len > max_bytes;

    if seeked {
        file.seek(SeekFrom::End(-(max_bytes as i64))).ok()?;
    }

    let mut buf = String::new();
    file.read_to_string(&mut buf).ok()?;

    // If we seeked into the middle, discard the first partial line
    if seeked {
        if let Some(pos) = buf.find('\n') {
            buf = buf[pos + 1..].to_string();
        }
    }

    Some(buf)
}

/// Check whether an RFC 3339 timestamp string is within `threshold_secs` of now.
///
/// Returns `Some(true)` if the timestamp is recent, `Some(false)` if stale,
/// or `None` if the timestamp cannot be parsed.
pub fn is_timestamp_recent(timestamp: &str, threshold_secs: i64) -> Option<bool> {
    use chrono::{DateTime, Utc};
    let parsed = timestamp.parse::<DateTime<Utc>>().ok()?;
    let elapsed = Utc::now().signed_duration_since(parsed);
    Some(elapsed.num_seconds().abs() <= threshold_secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_read_file_tail_small_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("small.txt");
        fs::write(&path, "line1\nline2\nline3\n").unwrap();

        let result = read_file_tail(&path, 1024);
        assert_eq!(result, Some("line1\nline2\nline3\n".to_string()));
    }

    #[test]
    fn test_read_file_tail_truncates_large_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("large.txt");
        // Write 100 lines, each ~20 chars
        let mut content = String::new();
        for i in 0..100 {
            content.push_str(&format!("line number {:04}\n", i));
        }
        fs::write(&path, &content).unwrap();

        // Only read last 50 bytes
        let result = read_file_tail(&path, 50).unwrap();
        // Should not start mid-line (first partial line discarded)
        assert!(result.starts_with("line"));
        assert!(result.len() <= 50);
    }

    #[test]
    fn test_read_file_tail_nonexistent() {
        let result = read_file_tail(Path::new("/nonexistent/file.txt"), 1024);
        assert!(result.is_none());
    }

    #[test]
    fn test_read_file_tail_empty_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("empty.txt");
        fs::write(&path, "").unwrap();

        let result = read_file_tail(&path, 1024);
        assert_eq!(result, Some(String::new()));
    }

    #[test]
    fn test_is_timestamp_recent_now() {
        let now = chrono::Utc::now().to_rfc3339();
        assert_eq!(is_timestamp_recent(&now, 60), Some(true));
    }

    #[test]
    fn test_is_timestamp_recent_old() {
        // A timestamp from 2020 should not be recent
        assert_eq!(is_timestamp_recent("2020-01-01T00:00:00Z", 60), Some(false));
    }

    #[test]
    fn test_is_timestamp_recent_invalid() {
        assert_eq!(is_timestamp_recent("not-a-date", 60), None);
    }
}
