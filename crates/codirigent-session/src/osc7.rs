//! OSC 7 escape sequence parser for working directory detection.
//!
//! Modern terminals and shells use OSC 7 (Operating System Command 7) to
//! communicate the current working directory. The shell emits an escape
//! sequence after each command containing a `file://` URI with the CWD.
//!
//! This is the same protocol used by Ghostty, WezTerm, iTerm2, and
//! Windows Terminal for CWD tracking.
//!
//! # OSC 7 Format
//!
//! ```text
//! ESC ] 7 ; file://<hostname>/<path> BEL
//! ESC ] 7 ; file://<hostname>/<path> ESC \
//! ```
//!
//! Where:
//! - ESC = 0x1B
//! - BEL = 0x07
//! - hostname may be empty (three slashes: `file:///path`)
//! - On Windows paths look like `file:///C:/Users/...`
//! - On Unix paths look like `file://hostname/home/...`

use std::path::PathBuf;

/// Prefix bytes for the OSC 7 sequence: ESC ] 7 ;
const OSC7_PREFIX: &[u8] = b"\x1b]7;";

/// Extract the last OSC 7 working directory path from a byte slice.
///
/// Scans the data for OSC 7 escape sequences and returns the path
/// from the last one found (most recent directory change).
///
/// Returns `None` if no complete OSC 7 sequence is found.
///
/// # Example
///
/// ```
/// use codirigent_session::osc7::extract_osc7_path;
/// use std::path::PathBuf;
///
/// // Unix-style path
/// let data = b"\x1b]7;file:///home/user/project\x07";
/// let path = extract_osc7_path(data);
/// assert!(path.is_some());
///
/// // No OSC 7 sequence
/// let data = b"normal terminal output";
/// assert!(extract_osc7_path(data).is_none());
/// ```
pub fn extract_osc7_path(data: &[u8]) -> Option<PathBuf> {
    let mut last_path: Option<PathBuf> = None;
    let mut search_from = 0;

    while search_from < data.len() {
        // Find the next OSC 7 prefix
        let prefix_pos = find_subsequence(&data[search_from..], OSC7_PREFIX);
        let prefix_pos = match prefix_pos {
            Some(pos) => search_from + pos,
            None => break,
        };

        let uri_start = prefix_pos + OSC7_PREFIX.len();
        if uri_start >= data.len() {
            break;
        }

        // Find the terminator: BEL (0x07) or ST (ESC \)
        let terminator_pos = find_osc_terminator(&data[uri_start..]);
        let terminator_pos = match terminator_pos {
            Some(pos) => uri_start + pos,
            None => break, // Incomplete sequence, skip
        };

        // Extract the URI string
        let uri_bytes = &data[uri_start..terminator_pos];
        if let Ok(uri_str) = std::str::from_utf8(uri_bytes) {
            if let Some(path) = parse_file_uri(uri_str) {
                last_path = Some(path);
            }
        }

        // Advance past this sequence
        search_from = terminator_pos + 1;
    }

    last_path
}

/// Find the position of the OSC terminator (BEL or ST).
///
/// Returns the byte offset of the terminator relative to the input slice.
fn find_osc_terminator(data: &[u8]) -> Option<usize> {
    for (i, &byte) in data.iter().enumerate() {
        // BEL terminator
        if byte == 0x07 {
            return Some(i);
        }
        // ST terminator (ESC \)
        if byte == 0x1b && i + 1 < data.len() && data[i + 1] == b'\\' {
            return Some(i);
        }
    }
    None
}

/// Parse a `file://` URI into a filesystem path.
///
/// Handles:
/// - `file:///path` (empty hostname)
/// - `file://hostname/path` (with hostname, ignored)
/// - `file:///C:/path` (Windows drive letter)
/// - Percent-encoded characters (e.g., `%20` for space)
fn parse_file_uri(uri: &str) -> Option<PathBuf> {
    let stripped = uri.strip_prefix("file://")?;

    // Find the path part (after the hostname)
    // If starts with /, hostname is empty
    let path_str = if stripped.starts_with('/') {
        stripped
    } else {
        // hostname/path - skip the hostname
        let slash_pos = stripped.find('/')?;
        &stripped[slash_pos..]
    };

    // Percent-decode the path
    let decoded = percent_decode(path_str);

    // On Windows, file:///C:/Users/... gives us /C:/Users/...
    // We need to strip the leading slash before the drive letter
    #[cfg(windows)]
    {
        let trimmed = decoded.trim_start_matches('/');
        if trimmed.len() >= 2 && trimmed.as_bytes()[1] == b':' {
            return Some(PathBuf::from(trimmed));
        }
        // Fallback: UNC or relative
        Some(PathBuf::from(&decoded))
    }

    #[cfg(not(windows))]
    {
        Some(PathBuf::from(&decoded))
    }
}

/// Percent-decode a URI string.
///
/// Converts `%XX` sequences to the corresponding byte value.
fn percent_decode(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = &input[i + 1..i + 3];
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                result.push(byte as char);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }

    result
}

/// Find the position of a subsequence within a slice.
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_unix_path_bel_terminator() {
        let data = b"\x1b]7;file:///home/user/project\x07";
        let path = extract_osc7_path(data).unwrap();
        assert_eq!(path, PathBuf::from("/home/user/project"));
    }

    #[test]
    fn test_extract_unix_path_st_terminator() {
        let data = b"\x1b]7;file:///home/user/project\x1b\\";
        let path = extract_osc7_path(data).unwrap();
        assert_eq!(path, PathBuf::from("/home/user/project"));
    }

    #[test]
    fn test_extract_unix_path_with_hostname() {
        let data = b"\x1b]7;file://myhost/home/user/project\x07";
        let path = extract_osc7_path(data).unwrap();
        assert_eq!(path, PathBuf::from("/home/user/project"));
    }

    #[cfg(windows)]
    #[test]
    fn test_extract_windows_path() {
        let data = b"\x1b]7;file:///C:/Users/osobo/Documents\x07";
        let path = extract_osc7_path(data).unwrap();
        assert_eq!(path, PathBuf::from("C:/Users/osobo/Documents"));
    }

    #[cfg(windows)]
    #[test]
    fn test_extract_windows_path_backslash() {
        // Some shells emit forward slashes in the URI even on Windows
        let data = b"\x1b]7;file:///C:/Users/osobo/My%20Documents\x07";
        let path = extract_osc7_path(data).unwrap();
        assert_eq!(path, PathBuf::from("C:/Users/osobo/My Documents"));
    }

    #[test]
    fn test_extract_last_osc7_from_multiple() {
        let data = b"output\x1b]7;file:///first/path\x07more output\x1b]7;file:///second/path\x07";
        let path = extract_osc7_path(data).unwrap();
        #[cfg(not(windows))]
        assert_eq!(path, PathBuf::from("/second/path"));
        #[cfg(windows)]
        assert_eq!(path, PathBuf::from("/second/path"));
    }

    #[test]
    fn test_no_osc7_returns_none() {
        let data = b"normal terminal output with no escape sequences";
        assert!(extract_osc7_path(data).is_none());
    }

    #[test]
    fn test_incomplete_osc7_returns_none() {
        // Prefix found but no terminator
        let data = b"\x1b]7;file:///home/user/project";
        assert!(extract_osc7_path(data).is_none());
    }

    #[test]
    fn test_osc7_embedded_in_output() {
        let data = b"PS C:\\> cd Documents\r\n\x1b]7;file:///C:/Users/osobo/Documents\x07PS C:\\Users\\osobo\\Documents> ";
        let path = extract_osc7_path(data);
        assert!(path.is_some());
    }

    #[test]
    fn test_percent_decode_spaces() {
        let decoded = percent_decode("/path/with%20spaces/dir");
        assert_eq!(decoded, "/path/with spaces/dir");
    }

    #[test]
    fn test_percent_decode_special_chars() {
        let decoded = percent_decode("/path/%23hash/%25percent");
        assert_eq!(decoded, "/path/#hash/%percent");
    }

    #[test]
    fn test_percent_decode_no_encoding() {
        let decoded = percent_decode("/normal/path");
        assert_eq!(decoded, "/normal/path");
    }

    #[test]
    fn test_parse_file_uri_unix() {
        let path = parse_file_uri("file:///home/user").unwrap();
        #[cfg(not(windows))]
        assert_eq!(path, PathBuf::from("/home/user"));
        #[cfg(windows)]
        assert!(path.to_string_lossy().contains("home"));
    }

    #[test]
    fn test_parse_file_uri_with_hostname() {
        let path = parse_file_uri("file://myhost/home/user").unwrap();
        #[cfg(not(windows))]
        assert_eq!(path, PathBuf::from("/home/user"));
        #[cfg(windows)]
        assert!(path.to_string_lossy().contains("home"));
    }

    #[test]
    fn test_parse_file_uri_invalid() {
        assert!(parse_file_uri("http://example.com").is_none());
        assert!(parse_file_uri("not a uri").is_none());
    }

    #[test]
    fn test_find_osc_terminator_bel() {
        let data = b"some text\x07more";
        assert_eq!(find_osc_terminator(data), Some(9));
    }

    #[test]
    fn test_find_osc_terminator_st() {
        let data = b"some text\x1b\\more";
        assert_eq!(find_osc_terminator(data), Some(9));
    }

    #[test]
    fn test_find_osc_terminator_none() {
        let data = b"no terminator here";
        assert!(find_osc_terminator(data).is_none());
    }

    #[test]
    fn test_empty_data() {
        assert!(extract_osc7_path(b"").is_none());
    }

    #[test]
    fn test_current_process_cwd_roundtrip() {
        // Simulate what a shell would emit for the current directory
        let cwd = std::env::current_dir().unwrap();
        let cwd_str = cwd.to_string_lossy();

        #[cfg(windows)]
        let uri = format!("file:///{}", cwd_str.replace('\\', "/"));
        #[cfg(not(windows))]
        let uri = format!("file://{}", cwd_str);

        let sequence = format!("\x1b]7;{}\x07", uri);
        let path = extract_osc7_path(sequence.as_bytes()).unwrap();

        // Normalize for comparison
        let expected = cwd.to_string_lossy().to_string();
        let got = path.to_string_lossy().to_string();

        #[cfg(windows)]
        {
            // On Windows, forward/backward slashes are equivalent
            assert_eq!(
                got.replace('/', "\\"),
                expected,
                "CWD roundtrip failed: got {} expected {}",
                got,
                expected
            );
        }
        #[cfg(not(windows))]
        assert_eq!(got, expected, "CWD roundtrip failed");
    }
}
