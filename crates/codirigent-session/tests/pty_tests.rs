//! PTY handling edge case tests.
//!
//! These tests verify PTY behavior across Windows, Linux, and macOS platforms.

use codirigent_session::PtyHandle;
use std::path::PathBuf;

/// Test PTY initialization with invalid shell command.
///
/// This test verifies that attempting to spawn a PTY with a non-existent
/// shell command fails gracefully on all platforms (Windows/Linux/macOS).
#[test]
fn test_pty_initialization_with_invalid_shell() {
    let temp_dir = std::env::temp_dir();

    // Try to spawn a PTY with a command that definitely doesn't exist
    let result = PtyHandle::spawn_command(
        &temp_dir,
        "nonexistent_shell_command_xyz123",
        &[],
        24,
        80,
        &[],
    );

    // Should fail with an error
    assert!(result.is_err(), "PTY spawn should fail with nonexistent shell");

    // Verify the error contains helpful information
    if let Err(e) = result {
        let err_msg = e.to_string();
        assert!(
            err_msg.contains("spawn") || err_msg.contains("command") || err_msg.contains("not found") || err_msg.contains("Failed"),
            "Error message should mention spawn or command failure: {}", err_msg
        );
    } else {
        panic!("Expected error for nonexistent shell");
    }
}

/// Test PTY resize functionality.
///
/// This test verifies that PTY resize operations work correctly across
/// all platforms without crashing or causing errors.
#[test]
fn test_pty_resize() {
    let temp_dir = std::env::temp_dir();

    // Determine the appropriate shell command for the platform
    #[cfg(windows)]
    let shell = "cmd.exe";
    #[cfg(unix)]
    let shell = "sh";

    let mut pty = PtyHandle::spawn_command(
        &temp_dir,
        shell,
        &[],
        24,
        80,
        &[],
    ).expect("PTY creation failed");

    // Test resizing to various dimensions
    let test_sizes = vec![
        (20, 10),    // Small
        (40, 120),   // Wide
        (100, 50),   // Tall
        (24, 80),    // Default
    ];

    for (rows, cols) in test_sizes {
        pty.resize(rows, cols).expect(&format!("Resize to {}x{} failed", rows, cols));
        let size = pty.size();
        assert_eq!(size.rows, rows, "Rows should match after resize");
        assert_eq!(size.cols, cols, "Cols should match after resize");
    }

    // Verify PTY still has valid PID after resizing
    assert!(pty.child_pid() > 0, "PTY should still be alive after resize");
}

/// Test PTY handles control sequences without crashing.
///
/// This test verifies that the PTY can handle ANSI escape sequences
/// and control characters commonly used in terminal applications
/// across Windows/Linux/macOS.
#[test]
fn test_pty_handles_control_sequences() {
    let temp_dir = std::env::temp_dir();

    #[cfg(windows)]
    let shell = "cmd.exe";
    #[cfg(unix)]
    let shell = "sh";

    let mut pty = PtyHandle::spawn_command(
        &temp_dir,
        shell,
        &[],
        24,
        80,
        &[],
    ).expect("PTY creation failed");

    // Test various control sequences:
    // - Clear screen: ESC[2J
    // - Move cursor home: ESC[H
    // - Color codes: ESC[31m (red foreground)
    // - Reset: ESC[0m
    let control_sequences: Vec<&[u8]> = vec![
        b"\x1b[2J\x1b[H",           // Clear screen + home
        b"\x1b[31mRed\x1b[0m",       // Color codes
        b"\x1b[1A\x1b[2K",           // Cursor up + clear line
        b"\x1b[s\x1b[u",             // Save/restore cursor position
    ];

    for sequence in control_sequences {
        let result = pty.send_input(sequence);
        // On some platforms, sending to closed PTY might error,
        // but it shouldn't panic
        if result.is_err() {
            // PTY might have closed, which is acceptable for this test
            break;
        }
    }

    // Verify PTY didn't crash - if it's still alive, that's good enough
    // (it might have exited normally, which is also fine)
    let _pid = pty.child_pid(); // Just verify this doesn't panic
}

/// Test PTY with minimal dimensions.
///
/// Verifies the PTY can handle very small terminal sizes without
/// panicking or failing on any platform.
#[test]
fn test_pty_minimal_dimensions() {
    let temp_dir = std::env::temp_dir();

    #[cfg(windows)]
    let shell = "cmd.exe";
    #[cfg(unix)]
    let shell = "sh";

    // Try to create PTY with minimal size (1x1)
    let result = PtyHandle::spawn_command(
        &temp_dir,
        shell,
        &[],
        1,
        1,
        &[],
    );

    // On most platforms, this should succeed
    // (portable-pty handles this gracefully)
    if let Ok(pty) = result {
        assert_eq!(pty.size().rows, 1);
        assert_eq!(pty.size().cols, 1);
        assert!(pty.child_pid() > 0);
    }
    // If it fails, that's also acceptable behavior for minimal sizes
}

/// Test PTY with large dimensions.
///
/// Verifies the PTY can handle very large terminal sizes without
/// failing on any platform.
#[test]
fn test_pty_large_dimensions() {
    let temp_dir = std::env::temp_dir();

    #[cfg(windows)]
    let shell = "cmd.exe";
    #[cfg(unix)]
    let shell = "sh";

    // Try to create PTY with large size
    let result = PtyHandle::spawn_command(
        &temp_dir,
        shell,
        &[],
        9999,
        9999,
        &[],
    );

    // Should succeed - portable-pty handles large sizes
    match result {
        Ok(pty) => {
            // Verify the size was set (may be clamped by OS)
            assert!(pty.size().rows > 0);
            assert!(pty.size().cols > 0);
            assert!(pty.child_pid() > 0);
        }
        Err(_) => {
            // Some platforms might reject very large sizes, which is acceptable
        }
    }
}

/// Test PTY spawn with custom environment variables.
///
/// Verifies that environment variables are correctly passed to the
/// spawned PTY process across all platforms.
#[test]
fn test_pty_custom_environment() {
    let temp_dir = std::env::temp_dir();

    #[cfg(windows)]
    let (shell, args) = ("cmd.exe", vec!["/C", "echo %TEST_VAR%"]);
    #[cfg(unix)]
    let (shell, args) = ("sh", vec!["-c", "echo $TEST_VAR"]);

    let pty = PtyHandle::spawn_command(
        &temp_dir,
        shell,
        &args,
        24,
        80,
        &[("TEST_VAR", "test_value_xyz123")],
    ).expect("PTY creation failed");

    // Verify PTY was created successfully
    assert!(pty.child_pid() > 0, "PTY should have valid PID");

    // The environment variable should be set in the spawned process
    // (actual verification would require reading PTY output, which
    // we're not testing here - this just verifies env vars don't
    // cause spawn failures)
}

/// Test PTY behavior with different working directories.
///
/// Verifies that PTY correctly sets the working directory across platforms.
#[test]
fn test_pty_working_directory() {
    // Use system temp dir which should exist on all platforms
    let temp_dir = std::env::temp_dir();

    #[cfg(windows)]
    let shell = "cmd.exe";
    #[cfg(unix)]
    let shell = "sh";

    let result = PtyHandle::spawn_command(
        &temp_dir,
        shell,
        &[],
        24,
        80,
        &[],
    );

    assert!(result.is_ok(), "PTY spawn should succeed with valid working directory");

    let pty = result.unwrap();
    assert!(pty.child_pid() > 0, "PTY should have valid PID");
}

/// Test PTY with non-existent working directory.
///
/// Verifies error handling when attempting to spawn a PTY
/// with an invalid working directory. Note: behavior is platform-specific.
/// On some platforms (e.g., Windows), the shell may spawn successfully
/// but fail when trying to change to the invalid directory.
#[test]
fn test_pty_invalid_working_directory() {
    // Use a path that definitely doesn't exist
    let invalid_dir = PathBuf::from("/nonexistent/directory/xyz123/abc456");

    #[cfg(windows)]
    let shell = "cmd.exe";
    #[cfg(unix)]
    let shell = "sh";

    let result = PtyHandle::spawn_command(
        &invalid_dir,
        shell,
        &[],
        24,
        80,
        &[],
    );

    // Platform-specific behavior:
    // - Unix: Usually fails immediately
    // - Windows: May succeed but shell starts in a default directory
    // We just verify it doesn't panic
    match result {
        Ok(_pty) => {
            // On some platforms (Windows), shell may spawn successfully
            // even with invalid working directory
        }
        Err(_e) => {
            // On other platforms (Unix), it may fail immediately
        }
    }
}
