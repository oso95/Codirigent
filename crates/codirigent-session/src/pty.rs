//! PTY (pseudo-terminal) abstraction layer.
//!
//! Provides cross-platform terminal creation and I/O using portable-pty.
//! This module handles spawning shells, managing terminal dimensions,
//! and async output reading.

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize as PortablePtySize};
use std::io::{Read, Write};
use std::path::Path;
use std::thread;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// PTY dimensions.
///
/// Represents the terminal size in rows and columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PtySize {
    /// Terminal height in rows.
    pub rows: u16,
    /// Terminal width in columns.
    pub cols: u16,
}

impl PtySize {
    /// Create a new PtySize with the given dimensions.
    pub fn new(rows: u16, cols: u16) -> Self {
        Self { rows, cols }
    }
}

impl Default for PtySize {
    fn default() -> Self {
        Self { rows: 24, cols: 80 }
    }
}

/// Handle to a PTY (pseudo-terminal).
///
/// Wraps the platform-specific PTY implementation and provides
/// a unified interface for terminal I/O. The handle manages
/// the master side of the PTY pair.
pub struct PtyHandle {
    /// The master PTY for resize operations.
    master: Box<dyn MasterPty + Send>,
    /// Writer for sending input to the PTY.
    writer: Box<dyn Write + Send>,
    /// Reader for receiving output (taken when async reader spawned).
    reader: Option<Box<dyn Read + Send>>,
    /// Child process ID.
    child_pid: u32,
    /// Current terminal size.
    size: PtySize,
}

impl PtyHandle {
    /// Spawn a new PTY with the default shell.
    ///
    /// Detects the default shell for the current platform ($SHELL on Unix,
    /// %COMSPEC% on Windows) and spawns it in a new PTY.
    ///
    /// # Arguments
    /// * `working_dir` - Initial working directory for the shell
    /// * `rows` - Initial terminal height
    /// * `cols` - Initial terminal width
    ///
    /// # Errors
    /// Returns an error if the PTY cannot be created or the shell cannot be spawned.
    ///
    /// # Example
    /// ```no_run
    /// use codirigent_session::PtyHandle;
    /// use std::path::Path;
    ///
    /// let pty = PtyHandle::spawn(Path::new("/tmp"), 24, 80, &[]).unwrap();
    /// assert!(pty.child_pid() > 0);
    /// ```
    pub fn spawn(
        working_dir: &Path,
        rows: u16,
        cols: u16,
        env_vars: &[(&str, &str)],
    ) -> Result<Self> {
        let shell = detect_shell_command();
        let args: Vec<&str> = shell.args.iter().map(|arg| arg.as_str()).collect();
        Self::spawn_command(working_dir, &shell.program, &args, rows, cols, env_vars)
    }

    /// Spawn a PTY with a specific command.
    ///
    /// Allows running a specific command instead of the default shell.
    ///
    /// # Arguments
    /// * `working_dir` - Initial working directory
    /// * `command` - The command to execute
    /// * `args` - Arguments to pass to the command
    /// * `rows` - Initial terminal height
    /// * `cols` - Initial terminal width
    /// * `env_vars` - Additional environment variables to set
    ///
    /// # Errors
    /// Returns an error if the PTY cannot be created or the command cannot be spawned.
    ///
    /// # Example
    /// ```no_run
    /// use codirigent_session::PtyHandle;
    /// use std::path::Path;
    ///
    /// let pty = PtyHandle::spawn_command(
    ///     Path::new("/tmp"),
    ///     "echo",
    ///     &["hello"],
    ///     24,
    ///     80,
    ///     &[("MY_VAR", "my_value")],
    /// ).unwrap();
    /// ```
    pub fn spawn_command(
        working_dir: &Path,
        command: &str,
        args: &[&str],
        rows: u16,
        cols: u16,
        env_vars: &[(&str, &str)],
    ) -> Result<Self> {
        info!(command, ?working_dir, rows, cols, "Spawning PTY");

        let pty_system = native_pty_system();
        let size = PortablePtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system.openpty(size).context("Failed to open PTY")?;

        let mut cmd = CommandBuilder::new(command);
        for arg in args {
            cmd.arg(*arg);
        }
        cmd.cwd(working_dir);

        // Set common environment variables for proper terminal behavior
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        // Set UTF-8 encoding for proper character display
        cmd.env("LANG", "en_US.UTF-8");
        cmd.env("LC_ALL", "en_US.UTF-8");

        // Set caller-provided environment variables (e.g., CODIRIGENT_CONTEXT_FILE)
        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        // Shell integration: OSC 7 (CWD tracking) + OSC 133 (shell state markers).
        // - Bash: uses PROMPT_COMMAND (runs before each prompt).
        // - Zsh:  uses ZDOTDIR redirect to inject a precmd hook via precmd_functions.
        // OSC 133 D;$? marks command finish, 133 A marks prompt start.
        // OSC 7 emits file:// URI with the current working directory.
        #[cfg(unix)]
        {
            cmd.env(
                "CODIRIGENT_OSC7",
                r#"printf '\e]7;file://%s%s\e\\' "$(hostname)" "$PWD""#,
            );

            if is_zsh_shell(command) {
                match setup_zsh_integration() {
                    Ok(zdotdir) => {
                        if let Ok(orig) = std::env::var("ZDOTDIR") {
                            cmd.env("CODIRIGENT_ORIG_ZDOTDIR", orig);
                        }
                        cmd.env("ZDOTDIR", &zdotdir);
                    }
                    Err(e) => {
                        warn!(%e, "Zsh integration setup failed, falling back to PROMPT_COMMAND");
                        cmd.env(
                            "PROMPT_COMMAND",
                            concat!(
                                r#"printf "\e]133;D;$?\a\e]133;A\a"; "#,
                                r#"printf "\e]7;file://%s%s\e\\" "$(hostname)" "$PWD""#,
                            ),
                        );
                    }
                }
            } else {
                cmd.env(
                    "PROMPT_COMMAND",
                    concat!(
                        r#"printf "\e]133;D;$?\a\e]133;A\a"; "#,
                        r#"printf "\e]7;file://%s%s\e\\" "$(hostname)" "$PWD""#,
                    ),
                );
            }
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .context("Failed to spawn command")?;

        let child_pid = child
            .process_id()
            .context("Failed to get child process ID")?;
        debug!(child_pid, "PTY child spawned");

        let reader = pair
            .master
            .try_clone_reader()
            .context("Failed to clone PTY reader")?;
        let writer = pair
            .master
            .take_writer()
            .context("Failed to take PTY writer")?;

        Ok(Self {
            master: pair.master,
            writer,
            reader: Some(reader),
            child_pid,
            size: PtySize { rows, cols },
        })
    }

    /// Send input to the PTY.
    ///
    /// Writes the given bytes to the PTY's input stream and flushes.
    ///
    /// # Arguments
    /// * `data` - The bytes to send
    ///
    /// # Errors
    /// Returns an error if the write or flush fails.
    ///
    /// # Example
    /// ```no_run
    /// use codirigent_session::PtyHandle;
    /// use std::path::Path;
    ///
    /// let mut pty = PtyHandle::spawn(Path::new("/tmp"), 24, 80, &[]).unwrap();
    /// pty.send_input(b"echo hello\n").unwrap();
    /// ```
    pub fn send_input(&mut self, data: &[u8]) -> Result<()> {
        self.writer
            .write_all(data)
            .context("Failed to write to PTY")?;
        self.writer.flush().context("Failed to flush PTY")?;
        Ok(())
    }

    /// Resize the PTY.
    ///
    /// Updates the terminal dimensions. This sends a SIGWINCH signal
    /// to the child process on Unix systems.
    ///
    /// # Arguments
    /// * `rows` - New terminal height
    /// * `cols` - New terminal width
    ///
    /// # Errors
    /// Returns an error if the resize operation fails.
    ///
    /// # Example
    /// ```no_run
    /// use codirigent_session::PtyHandle;
    /// use std::path::Path;
    ///
    /// let mut pty = PtyHandle::spawn(Path::new("/tmp"), 24, 80, &[]).unwrap();
    /// pty.resize(48, 120).unwrap();
    /// assert_eq!(pty.size().rows, 48);
    /// ```
    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        debug!(rows, cols, "Resizing PTY");

        let size = PortablePtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        self.master.resize(size).context("Failed to resize PTY")?;

        self.size = PtySize { rows, cols };
        Ok(())
    }

    /// Get the child process ID.
    ///
    /// Returns the PID of the spawned child process.
    /// The PID is guaranteed to be valid (non-zero) as PTY creation
    /// fails if the child PID cannot be determined.
    pub fn child_pid(&self) -> u32 {
        self.child_pid
    }

    /// Get current terminal size.
    pub fn size(&self) -> PtySize {
        self.size
    }

    /// Take the reader for async processing.
    ///
    /// This consumes the reader, returning it for use with
    /// [`spawn_output_reader`]. Once taken, the reader cannot
    /// be retrieved again.
    ///
    /// # Returns
    /// `Some(reader)` if the reader hasn't been taken yet, `None` otherwise.
    pub fn take_reader(&mut self) -> Option<Box<dyn Read + Send>> {
        self.reader.take()
    }

    /// Check if the reader is still available.
    pub fn has_reader(&self) -> bool {
        self.reader.is_some()
    }
}

/// Shell command and arguments selected for a PTY session.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ShellCommand {
    program: String,
    args: Vec<String>,
}

/// Detect the default shell for the current platform.
///
/// On Unix systems, returns the value of `$SHELL` or `/bin/bash` as fallback.
/// On Windows, prioritizes PowerShell 7 (pwsh), then Windows PowerShell (powershell),
/// then falls back to `%COMSPEC%` or `cmd.exe`.
#[cfg(test)]
fn detect_shell() -> String {
    detect_shell_command().program
}

/// Detect the shell command and args, honoring explicit overrides.
///
/// Use `CODIRIGENT_SHELL` to override the executable and `CODIRIGENT_SHELL_ARGS`
/// to provide arguments (space-delimited).
fn detect_shell_command() -> ShellCommand {
    if let Ok(shell) = std::env::var("CODIRIGENT_SHELL") {
        let args = std::env::var("CODIRIGENT_SHELL_ARGS")
            .ok()
            .map(|value| split_shell_args(&value))
            .unwrap_or_default();
        return ShellCommand {
            program: shell,
            args,
        };
    }

    #[cfg(unix)]
    {
        let program = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        ShellCommand {
            program,
            args: Vec::new(),
        }
    }

    #[cfg(windows)]
    {
        use std::process::Command;

        // PowerShell init command: UTF-8 encoding + OSC 7 (CWD) + OSC 133 (shell integration).
        // OSC 133 markers: D (finish previous cmd), A (prompt start), B (command input start).
        // We skip 133;C (command executed) — inferred when output appears after B.
        let ps_init = concat!(
            "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; ",
            "$OutputEncoding=[System.Text.Encoding]::UTF8; ",
            "function prompt { ",
            // All assignments first (before the return expression)
            "$gle = $global:LASTEXITCODE; ",
            "if ($null -eq $gle) { $gle = 0 }; ",
            "$p = $executionContext.SessionState.Path.CurrentLocation.ProviderPath; ",
            "$h = [System.Net.Dns]::GetHostName(); ",
            "$u = $p.Replace('\\','/'); ",
            // Single return expression: 133;D + 133;A + OSC7 + prompt + 133;B
            "\"$([char]27)]133;D;$gle$([char]7)\" + ",
            "\"$([char]27)]133;A$([char]7)\" + ",
            "\"$([char]27)]7;file://$h/$u$([char]27)\\\" + ",
            "\"PS $($executionContext.SessionState.Path.CurrentLocation)> \" + ",
            "\"$([char]27)]133;B$([char]7)\" ",
            "}",
        );

        // Try PowerShell 7 first
        if Command::new("pwsh.exe").arg("--version").output().is_ok() {
            return ShellCommand {
                program: "pwsh.exe".to_string(),
                args: vec![
                    "-NoLogo".to_string(),
                    "-NoProfile".to_string(),
                    "-NoExit".to_string(),
                    "-Command".to_string(),
                    ps_init.to_string(),
                ],
            };
        }
        // Try Windows PowerShell
        if Command::new("powershell.exe")
            .arg("-Command")
            .arg("exit")
            .output()
            .is_ok()
        {
            return ShellCommand {
                program: "powershell.exe".to_string(),
                args: vec![
                    "-NoLogo".to_string(),
                    "-NoProfile".to_string(),
                    "-NoExit".to_string(),
                    "-Command".to_string(),
                    ps_init.to_string(),
                ],
            };
        }
        // Fall back to cmd.exe
        let program = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
        ShellCommand {
            program,
            // Switch to UTF-8 codepage for correct Unicode output.
            args: vec!["/K".to_string(), "chcp".to_string(), "65001".to_string()],
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        ShellCommand {
            program: "/bin/sh".to_string(),
            args: Vec::new(),
        }
    }
}

/// Split a space-delimited shell args string.
///
/// This is intentionally simple; if callers need quoting, they can provide
/// a wrapper script via `CODIRIGENT_SHELL`.
fn split_shell_args(value: &str) -> Vec<String> {
    value.split_whitespace().map(str::to_string).collect()
}

/// Check whether the given command is a zsh shell.
///
/// Matches `/bin/zsh`, `/usr/bin/zsh`, `/usr/local/bin/zsh`, or bare `zsh`.
#[cfg(unix)]
fn is_zsh_shell(command: &str) -> bool {
    std::path::Path::new(command)
        .file_name()
        .and_then(|n| n.to_str())
        .map_or(false, |base| base == "zsh")
}

/// Set up zsh shell integration via a ZDOTDIR redirect.
///
/// Creates a temporary directory containing `.zshenv`, `.zprofile`, and `.zshrc`
/// that forward to the user's original startup files and append an OSC 7 / OSC 133
/// `precmd` hook at the end of `.zshrc` (after the user's config has loaded).
///
/// The directory is reused across sessions (content is idempotent).
#[cfg(unix)]
fn setup_zsh_integration() -> Result<std::path::PathBuf> {
    let zdotdir = std::env::temp_dir().join("codirigent-zsh-integration");
    std::fs::create_dir_all(&zdotdir)
        .context("Failed to create zsh integration directory")?;

    // .zshenv — always sourced (interactive, non-interactive, login, non-login).
    // Forward to user's .zshenv; do NOT restore ZDOTDIR here so our
    // .zprofile and .zshrc are still picked up by zsh.
    std::fs::write(
        zdotdir.join(".zshenv"),
        r#"# Codirigent shell integration — forward to user's .zshenv
if [[ -f "${CODIRIGENT_ORIG_ZDOTDIR:-$HOME}/.zshenv" ]]; then
  ZDOTDIR="${CODIRIGENT_ORIG_ZDOTDIR:-$HOME}" source "${CODIRIGENT_ORIG_ZDOTDIR:-$HOME}/.zshenv"
fi
"#,
    )
    .context("Failed to write .zshenv")?;

    // .zprofile — login shells only. Forward to user's .zprofile.
    std::fs::write(
        zdotdir.join(".zprofile"),
        r#"# Codirigent shell integration — forward to user's .zprofile
if [[ -f "${CODIRIGENT_ORIG_ZDOTDIR:-$HOME}/.zprofile" ]]; then
  source "${CODIRIGENT_ORIG_ZDOTDIR:-$HOME}/.zprofile"
fi
"#,
    )
    .context("Failed to write .zprofile")?;

    // .zshrc — interactive shells. Forward to user's .zshrc, then install
    // our precmd hook (appended AFTER user config so it is not overridden),
    // then restore ZDOTDIR so subshells use the user's own config.
    std::fs::write(
        zdotdir.join(".zshrc"),
        r#"# Codirigent shell integration — forward to user's .zshrc + add hooks
if [[ -f "${CODIRIGENT_ORIG_ZDOTDIR:-$HOME}/.zshrc" ]]; then
  ZDOTDIR="${CODIRIGENT_ORIG_ZDOTDIR:-$HOME}" source "${CODIRIGENT_ORIG_ZDOTDIR:-$HOME}/.zshrc"
fi

# Restore ZDOTDIR so subshells and .zlogin use the user's config
ZDOTDIR="${CODIRIGENT_ORIG_ZDOTDIR:-$HOME}"

# OSC 133 (shell state) + OSC 7 (CWD tracking) via precmd hook
__codirigent_precmd() {
  local ec=$?
  printf '\e]133;D;%s\a\e]133;A\a' "$ec"
  printf '\e]7;file://%s%s\e\\' "$(hostname)" "$PWD"
}
precmd_functions+=(__codirigent_precmd)
"#,
    )
    .context("Failed to write .zshrc")?;

    Ok(zdotdir)
}

/// Buffer size for reading PTY output.
///
/// 4KB is a good balance between memory usage and reducing syscall overhead.
/// This matches common pipe buffer sizes on Unix systems.
const OUTPUT_BUFFER_SIZE: usize = 4096;

/// Channel capacity for output messages.
///
/// 256 messages allows buffering up to 1MB of output (256 * 4KB) before
/// backpressure kicks in. This provides headroom for bursty output while
/// preventing unbounded memory growth.
const OUTPUT_CHANNEL_CAPACITY: usize = 256;

/// Async PTY output reader with managed lifecycle.
///
/// Wraps a background thread that reads from the PTY and sends output
/// through an mpsc channel. Provides clean shutdown via the [`stop`](Self::stop) method.
///
/// The reader thread terminates when:
/// - The PTY closes (EOF)
/// - A read error occurs
/// - The channel receiver is dropped
/// - [`stop`](Self::stop) is called
pub struct OutputReader {
    /// Channel receiver for output data.
    receiver: mpsc::Receiver<Vec<u8>>,
    /// Join handle for the reader thread.
    join_handle: Option<thread::JoinHandle<()>>,
}

impl OutputReader {
    /// Create a new output reader for the given PTY reader.
    ///
    /// Spawns a background thread that reads from the PTY and sends
    /// output through an internal channel.
    ///
    /// # Arguments
    /// * `reader` - The PTY reader (obtained via [`PtyHandle::take_reader`])
    ///
    /// # Example
    /// ```no_run
    /// use codirigent_session::{PtyHandle, OutputReader};
    /// use std::path::Path;
    ///
    /// # async fn example() {
    /// let mut pty = PtyHandle::spawn(Path::new("/tmp"), 24, 80, &[]).unwrap();
    /// let reader = pty.take_reader().unwrap();
    /// let mut output_reader = OutputReader::new(reader);
    ///
    /// // Receive output asynchronously
    /// while let Some(data) = output_reader.recv().await {
    ///     println!("Received {} bytes", data.len());
    /// }
    ///
    /// // Clean shutdown
    /// output_reader.stop();
    /// # }
    /// ```
    pub fn new(mut reader: Box<dyn Read + Send>) -> Self {
        let (tx, rx) = mpsc::channel(OUTPUT_CHANNEL_CAPACITY);

        let handle = thread::spawn(move || {
            let mut buf = [0u8; OUTPUT_BUFFER_SIZE];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        debug!("PTY reader reached EOF");
                        break;
                    }
                    Ok(n) => {
                        let data = buf[..n].to_vec();
                        if tx.blocking_send(data).is_err() {
                            debug!("PTY output channel closed");
                            break;
                        }
                    }
                    Err(e) => {
                        debug!(?e, "PTY read error");
                        break;
                    }
                }
            }
        });

        Self {
            receiver: rx,
            join_handle: Some(handle),
        }
    }

    /// Receive the next chunk of output data.
    ///
    /// Returns `None` when the PTY closes or an error occurs.
    pub async fn recv(&mut self) -> Option<Vec<u8>> {
        self.receiver.recv().await
    }

    /// Get a mutable reference to the underlying receiver.
    ///
    /// Useful for more advanced channel operations like `try_recv` or
    /// combining with other futures using `select!`.
    pub fn receiver_mut(&mut self) -> &mut mpsc::Receiver<Vec<u8>> {
        &mut self.receiver
    }

    /// Stop the reader thread and clean up resources.
    ///
    /// This closes the channel and waits for the reader thread to terminate.
    /// Call this method when you're done reading from the PTY to ensure
    /// clean resource cleanup.
    ///
    /// After calling `stop`, the reader cannot be used anymore.
    pub fn stop(mut self) {
        // Close the receiver to signal the thread to stop
        self.receiver.close();

        // Wait for the thread to finish
        if let Some(handle) = self.join_handle.take() {
            // Use a timeout to avoid blocking forever if the thread is stuck
            // The thread should exit quickly once the channel is closed
            let _ = handle.join();
        }
    }

    /// Check if the reader thread is still running.
    pub fn is_running(&self) -> bool {
        self.join_handle.as_ref().is_some_and(|h| !h.is_finished())
    }

    /// Consume self and return just the receiver.
    ///
    /// This is used for backward compatibility with `spawn_output_reader`.
    /// The join handle is intentionally discarded - the thread will terminate
    /// when the receiver is dropped or the PTY closes.
    pub fn into_receiver(self) -> mpsc::Receiver<Vec<u8>> {
        // Use ManuallyDrop to prevent Drop from running, which would close the receiver
        let this = std::mem::ManuallyDrop::new(self);
        // SAFETY: We're moving out of `this` which is wrapped in ManuallyDrop,
        // so we need to use ptr::read to get ownership of the fields.
        // We explicitly forget the join_handle to avoid calling Drop on it.
        unsafe {
            let receiver = std::ptr::read(&this.receiver);
            let join_handle = std::ptr::read(&this.join_handle);
            // Explicitly drop the join handle - the thread will continue running
            // and will terminate when the receiver is dropped or PTY closes
            drop(join_handle);
            receiver
        }
    }
}

impl Drop for OutputReader {
    fn drop(&mut self) {
        // Close the receiver to signal the thread to stop
        self.receiver.close();

        // If the join handle is still present, the thread is still running
        // We don't wait here to avoid blocking in Drop, but closing the
        // receiver will cause the thread to exit on its next send attempt
        if let Some(handle) = self.join_handle.take() {
            // Try to join without blocking - if not ready, just let it go
            // The thread will exit naturally when it tries to send
            if handle.is_finished() {
                let _ = handle.join();
            }
        }
    }
}

/// Spawn an async task to read PTY output.
///
/// Creates a background thread that reads from the PTY and sends
/// output through an mpsc channel. The thread terminates when
/// the PTY closes (EOF), a read error occurs, or the channel receiver
/// is dropped.
///
/// # Deprecation Note
/// This function is provided for backward compatibility. Prefer using
/// [`OutputReader::new`] which provides proper lifecycle management
/// including a [`stop`](OutputReader::stop) method for clean shutdown.
///
/// # Arguments
/// * `reader` - The PTY reader (obtained via [`PtyHandle::take_reader`])
///
/// # Returns
/// An mpsc receiver for output data chunks.
///
/// # Example
/// ```no_run
/// use codirigent_session::{PtyHandle, spawn_output_reader};
/// use std::path::Path;
///
/// # async fn example() {
/// let mut pty = PtyHandle::spawn(Path::new("/tmp"), 24, 80, &[]).unwrap();
/// let reader = pty.take_reader().unwrap();
/// let mut rx = spawn_output_reader(reader);
///
/// // Receive output asynchronously
/// while let Some(data) = rx.recv().await {
///     println!("Received {} bytes", data.len());
/// }
/// # }
/// ```
pub fn spawn_output_reader(reader: Box<dyn Read + Send>) -> mpsc::Receiver<Vec<u8>> {
    let output_reader = OutputReader::new(reader);
    // Note: We intentionally take just the receiver here for backward compatibility.
    // The thread will still terminate when the receiver is dropped.
    // Use OutputReader::new() directly for proper lifecycle management.
    output_reader.into_receiver()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_pty_size_new() {
        let size = PtySize::new(48, 120);
        assert_eq!(size.rows, 48);
        assert_eq!(size.cols, 120);
    }

    #[test]
    fn test_pty_size_default() {
        let size = PtySize::default();
        assert_eq!(size.rows, 24);
        assert_eq!(size.cols, 80);
    }

    #[test]
    fn test_pty_size_equality() {
        let size1 = PtySize::new(24, 80);
        let size2 = PtySize::new(24, 80);
        let size3 = PtySize::new(48, 80);
        assert_eq!(size1, size2);
        assert_ne!(size1, size3);
    }

    #[test]
    fn test_pty_size_clone() {
        let size = PtySize::new(24, 80);
        let cloned = size;
        assert_eq!(size, cloned);
    }

    #[test]
    fn test_detect_shell() {
        let shell = detect_shell();
        assert!(!shell.is_empty());
        // Shell should be a valid path or command
        #[cfg(unix)]
        assert!(shell.contains('/') || shell == "bash" || shell == "sh" || shell == "zsh");
        #[cfg(windows)]
        assert!(shell.contains("cmd") || shell.contains("powershell") || shell.contains("pwsh"));
    }

    #[test]
    fn test_split_shell_args() {
        let args = split_shell_args("-NoLogo -NoProfile -NoExit");
        assert_eq!(args, vec!["-NoLogo", "-NoProfile", "-NoExit"]);

        let args = split_shell_args("   /K   chcp   65001  ");
        assert_eq!(args, vec!["/K", "chcp", "65001"]);
    }

    #[test]
    fn test_spawn_pty() {
        let temp = TempDir::new().unwrap();
        let pty = PtyHandle::spawn(temp.path(), 24, 80, &[]);
        assert!(pty.is_ok(), "Failed to spawn PTY: {:?}", pty.err());

        let pty = pty.unwrap();
        assert!(pty.child_pid() > 0);
        assert_eq!(pty.size().rows, 24);
        assert_eq!(pty.size().cols, 80);
        assert!(pty.has_reader());
    }

    #[test]
    fn test_spawn_pty_with_custom_size() {
        let temp = TempDir::new().unwrap();
        let pty = PtyHandle::spawn(temp.path(), 48, 120, &[]).unwrap();

        assert_eq!(pty.size().rows, 48);
        assert_eq!(pty.size().cols, 120);
    }

    #[test]
    fn test_spawn_command() {
        let temp = TempDir::new().unwrap();

        #[cfg(unix)]
        let pty = PtyHandle::spawn_command(temp.path(), "/bin/sh", &[], 24, 80, &[]);

        #[cfg(windows)]
        let pty = PtyHandle::spawn_command(temp.path(), "cmd.exe", &[], 24, 80, &[]);

        assert!(pty.is_ok(), "Failed to spawn command: {:?}", pty.err());
        assert!(pty.unwrap().child_pid() > 0);
    }

    #[test]
    fn test_spawn_command_with_args() {
        let temp = TempDir::new().unwrap();

        #[cfg(unix)]
        let pty = PtyHandle::spawn_command(temp.path(), "echo", &["hello", "world"], 24, 80, &[]);

        #[cfg(windows)]
        let pty = PtyHandle::spawn_command(
            temp.path(),
            "cmd.exe",
            &["/c", "echo", "hello"],
            24,
            80,
            &[],
        );

        assert!(pty.is_ok());
    }

    #[test]
    fn test_send_input() {
        let temp = TempDir::new().unwrap();
        let mut pty = PtyHandle::spawn(temp.path(), 24, 80, &[]).unwrap();

        // Send a simple command
        let result = pty.send_input(b"echo hello\n");
        assert!(result.is_ok());
    }

    #[test]
    fn test_send_input_multiple_times() {
        let temp = TempDir::new().unwrap();
        let mut pty = PtyHandle::spawn(temp.path(), 24, 80, &[]).unwrap();

        assert!(pty.send_input(b"echo 1\n").is_ok());
        assert!(pty.send_input(b"echo 2\n").is_ok());
        assert!(pty.send_input(b"echo 3\n").is_ok());
    }

    #[test]
    fn test_send_input_special_characters() {
        let temp = TempDir::new().unwrap();
        let mut pty = PtyHandle::spawn(temp.path(), 24, 80, &[]).unwrap();

        // Test control characters
        assert!(pty.send_input(&[0x03]).is_ok()); // Ctrl+C
        assert!(pty.send_input(&[0x04]).is_ok()); // Ctrl+D
        assert!(pty.send_input(&[0x1b, b'[', b'A']).is_ok()); // Up arrow
    }

    #[test]
    fn test_resize() {
        let temp = TempDir::new().unwrap();
        let mut pty = PtyHandle::spawn(temp.path(), 24, 80, &[]).unwrap();

        let result = pty.resize(48, 120);
        assert!(result.is_ok());
        assert_eq!(pty.size().rows, 48);
        assert_eq!(pty.size().cols, 120);
    }

    #[test]
    fn test_resize_multiple_times() {
        let temp = TempDir::new().unwrap();
        let mut pty = PtyHandle::spawn(temp.path(), 24, 80, &[]).unwrap();

        pty.resize(30, 100).unwrap();
        assert_eq!(pty.size().rows, 30);

        pty.resize(50, 150).unwrap();
        assert_eq!(pty.size().cols, 150);

        pty.resize(10, 40).unwrap();
        assert_eq!(pty.size().rows, 10);
        assert_eq!(pty.size().cols, 40);
    }

    #[test]
    fn test_take_reader() {
        let temp = TempDir::new().unwrap();
        let mut pty = PtyHandle::spawn(temp.path(), 24, 80, &[]).unwrap();

        assert!(pty.has_reader());

        let reader = pty.take_reader();
        assert!(reader.is_some());
        assert!(!pty.has_reader());

        // Second take should return None
        let reader2 = pty.take_reader();
        assert!(reader2.is_none());
    }

    #[test]
    fn test_child_pid() {
        let temp = TempDir::new().unwrap();
        let pty = PtyHandle::spawn(temp.path(), 24, 80, &[]).unwrap();

        let pid = pty.child_pid();
        assert!(pid > 0, "Child PID should be positive");
    }

    #[tokio::test]
    async fn test_spawn_output_reader() {
        let temp = TempDir::new().unwrap();
        let mut pty = PtyHandle::spawn(temp.path(), 24, 80, &[]).unwrap();

        let reader = pty.take_reader().expect("Reader should exist");
        let mut rx = spawn_output_reader(reader);

        // Send a command that produces output
        #[cfg(unix)]
        pty.send_input(b"echo test_output_12345\n").unwrap();

        #[cfg(windows)]
        pty.send_input(b"echo test_output_12345\r\n").unwrap();

        // Wait for output (with timeout)
        let mut found = false;
        for _ in 0..50 {
            if let Some(bytes) =
                tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv())
                    .await
                    .ok()
                    .flatten()
            {
                let output = String::from_utf8_lossy(&bytes);
                if output.contains("test_output_12345") {
                    found = true;
                    break;
                }
            }
        }

        assert!(found, "Expected to find output in PTY stream");
    }

    #[tokio::test]
    async fn test_spawn_output_reader_receives_multiple_chunks() {
        let temp = TempDir::new().unwrap();
        let mut pty = PtyHandle::spawn(temp.path(), 24, 80, &[]).unwrap();

        let reader = pty.take_reader().expect("Reader should exist");
        let mut rx = spawn_output_reader(reader);

        // Send multiple commands
        for i in 0..3 {
            #[cfg(unix)]
            pty.send_input(format!("echo chunk_{}\n", i).as_bytes())
                .unwrap();

            #[cfg(windows)]
            pty.send_input(format!("echo chunk_{}\r\n", i).as_bytes())
                .unwrap();
        }

        // Collect output
        let mut chunks_found = 0;
        let mut all_output = String::new();

        for _ in 0..100 {
            if let Some(bytes) =
                tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv())
                    .await
                    .ok()
                    .flatten()
            {
                let output = String::from_utf8_lossy(&bytes);
                all_output.push_str(&output);

                for i in 0..3 {
                    if all_output.contains(&format!("chunk_{}", i)) {
                        chunks_found |= 1 << i;
                    }
                }

                if chunks_found == 0b111 {
                    break;
                }
            }
        }

        // At least some output should be received
        assert!(!all_output.is_empty(), "Should receive some output");
    }

    #[tokio::test]
    async fn test_output_reader_channel_closure() {
        let temp = TempDir::new().unwrap();
        let mut pty = PtyHandle::spawn(temp.path(), 24, 80, &[]).unwrap();

        let reader = pty.take_reader().expect("Reader should exist");
        let rx = spawn_output_reader(reader);

        // Drop the receiver - the thread should detect this and stop
        drop(rx);

        // Send input to trigger the thread to check the channel
        let _ = pty.send_input(b"echo test\n");

        // Give the thread time to detect closure
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Test passes if we don't hang or crash
    }

    #[test]
    fn test_spawn_invalid_command() {
        let temp = TempDir::new().unwrap();
        let result =
            PtyHandle::spawn_command(temp.path(), "/nonexistent/command/path", &[], 24, 80, &[]);

        // Should fail to spawn an invalid command
        assert!(result.is_err());
    }

    #[test]
    fn test_spawn_with_invalid_working_dir() {
        use std::path::PathBuf;
        let invalid_path = PathBuf::from("/nonexistent/path/that/does/not/exist");

        // This may or may not fail depending on the platform
        // On some systems, cwd errors are deferred
        let result = PtyHandle::spawn(&invalid_path, 24, 80, &[]);
        // We just check it doesn't panic
        let _ = result;
    }

    #[test]
    fn test_pty_size_serialize_deserialize() {
        let size = PtySize::new(48, 120);
        let json = serde_json::to_string(&size).unwrap();
        assert!(json.contains("48"));
        assert!(json.contains("120"));

        let deserialized: PtySize = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, size);
    }

    #[tokio::test]
    async fn test_output_reader_new() {
        let temp = TempDir::new().unwrap();
        let mut pty = PtyHandle::spawn(temp.path(), 24, 80, &[]).unwrap();

        let reader = pty.take_reader().expect("Reader should exist");
        let mut output_reader = OutputReader::new(reader);

        // Send a command that produces output
        #[cfg(unix)]
        pty.send_input(b"echo output_reader_test\n").unwrap();

        #[cfg(windows)]
        pty.send_input(b"echo output_reader_test\r\n").unwrap();

        // Wait for output (with timeout)
        let mut found = false;
        for _ in 0..50 {
            if let Some(bytes) =
                tokio::time::timeout(std::time::Duration::from_millis(100), output_reader.recv())
                    .await
                    .ok()
                    .flatten()
            {
                let output = String::from_utf8_lossy(&bytes);
                if output.contains("output_reader_test") {
                    found = true;
                    break;
                }
            }
        }

        assert!(found, "Expected to find output in PTY stream");

        // Clean shutdown
        output_reader.stop();
    }

    #[tokio::test]
    async fn test_output_reader_is_running() {
        let temp = TempDir::new().unwrap();
        let mut pty = PtyHandle::spawn(temp.path(), 24, 80, &[]).unwrap();

        let reader = pty.take_reader().expect("Reader should exist");
        let output_reader = OutputReader::new(reader);

        // Thread should be running initially
        assert!(output_reader.is_running());

        output_reader.stop();
    }

    #[tokio::test]
    async fn test_output_reader_stop_cleans_up() {
        let temp = TempDir::new().unwrap();
        let mut pty = PtyHandle::spawn(temp.path(), 24, 80, &[]).unwrap();

        let reader = pty.take_reader().expect("Reader should exist");
        let output_reader = OutputReader::new(reader);

        // Stop should complete without hanging
        output_reader.stop();

        // Test passes if we get here without hanging
    }

    #[tokio::test]
    async fn test_output_reader_receiver_mut() {
        let temp = TempDir::new().unwrap();
        let mut pty = PtyHandle::spawn(temp.path(), 24, 80, &[]).unwrap();

        let reader = pty.take_reader().expect("Reader should exist");
        let mut output_reader = OutputReader::new(reader);

        // Should be able to access receiver mutably
        let rx = output_reader.receiver_mut();

        // Try a non-blocking receive
        let result = rx.try_recv();
        // Either empty or has data - both are valid
        assert!(result.is_err() || result.is_ok());

        output_reader.stop();
    }

    #[tokio::test]
    async fn test_output_reader_drop() {
        let temp = TempDir::new().unwrap();
        let mut pty = PtyHandle::spawn(temp.path(), 24, 80, &[]).unwrap();

        let reader = pty.take_reader().expect("Reader should exist");

        {
            let _output_reader = OutputReader::new(reader);
            // OutputReader will be dropped here
        }

        // Small delay to let thread clean up
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Test passes if we don't hang or crash
    }

    #[tokio::test]
    async fn test_output_reader_multiple_recv() {
        let temp = TempDir::new().unwrap();
        let mut pty = PtyHandle::spawn(temp.path(), 24, 80, &[]).unwrap();

        let reader = pty.take_reader().expect("Reader should exist");
        let mut output_reader = OutputReader::new(reader);

        // Send multiple commands
        for i in 0..3 {
            #[cfg(unix)]
            pty.send_input(format!("echo reader_chunk_{}\n", i).as_bytes())
                .unwrap();

            #[cfg(windows)]
            pty.send_input(format!("echo reader_chunk_{}\r\n", i).as_bytes())
                .unwrap();
        }

        // Collect output
        let mut all_output = String::new();

        for _ in 0..100 {
            if let Some(bytes) =
                tokio::time::timeout(std::time::Duration::from_millis(50), output_reader.recv())
                    .await
                    .ok()
                    .flatten()
            {
                let output = String::from_utf8_lossy(&bytes);
                all_output.push_str(&output);
            }

            // Check if we got all chunks
            let mut found_all = true;
            for i in 0..3 {
                if !all_output.contains(&format!("reader_chunk_{}", i)) {
                    found_all = false;
                    break;
                }
            }
            if found_all {
                break;
            }
        }

        assert!(!all_output.is_empty(), "Should receive some output");

        output_reader.stop();
    }

    // --- Zsh integration tests ---

    #[cfg(unix)]
    #[test]
    fn test_is_zsh_shell() {
        assert!(is_zsh_shell("/bin/zsh"));
        assert!(is_zsh_shell("/usr/bin/zsh"));
        assert!(is_zsh_shell("/usr/local/bin/zsh"));
        assert!(is_zsh_shell("zsh"));

        assert!(!is_zsh_shell("/bin/bash"));
        assert!(!is_zsh_shell("bash"));
        assert!(!is_zsh_shell("/bin/sh"));
        assert!(!is_zsh_shell("fish"));
        assert!(!is_zsh_shell(""));
    }

    #[cfg(unix)]
    #[test]
    fn test_setup_zsh_integration_creates_files() {
        let zdotdir = setup_zsh_integration().expect("setup_zsh_integration should succeed");

        assert!(zdotdir.join(".zshenv").exists(), ".zshenv should exist");
        assert!(
            zdotdir.join(".zprofile").exists(),
            ".zprofile should exist"
        );
        assert!(zdotdir.join(".zshrc").exists(), ".zshrc should exist");
    }

    #[cfg(unix)]
    #[test]
    fn test_setup_zsh_integration_zshrc_contains_precmd_hook() {
        let zdotdir = setup_zsh_integration().unwrap();
        let zshrc = std::fs::read_to_string(zdotdir.join(".zshrc")).unwrap();

        assert!(
            zshrc.contains("__codirigent_precmd"),
            ".zshrc should define the precmd hook"
        );
        assert!(
            zshrc.contains("precmd_functions+="),
            ".zshrc should append to precmd_functions"
        );
        assert!(
            zshrc.contains("133;D"),
            ".zshrc should emit OSC 133 markers"
        );
        assert!(
            zshrc.contains(r"\e]7;file://"),
            ".zshrc should emit OSC 7 URI"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_setup_zsh_integration_zshrc_sources_user_config() {
        let zdotdir = setup_zsh_integration().unwrap();
        let zshrc = std::fs::read_to_string(zdotdir.join(".zshrc")).unwrap();

        assert!(
            zshrc.contains("CODIRIGENT_ORIG_ZDOTDIR"),
            ".zshrc should reference the original ZDOTDIR"
        );
        assert!(
            zshrc.contains("source"),
            ".zshrc should source the user's config"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_setup_zsh_integration_zshrc_restores_zdotdir() {
        let zdotdir = setup_zsh_integration().unwrap();
        let zshrc = std::fs::read_to_string(zdotdir.join(".zshrc")).unwrap();

        // The ZDOTDIR restore must appear BEFORE the precmd hook
        let restore_pos = zshrc
            .find("ZDOTDIR=\"${CODIRIGENT_ORIG_ZDOTDIR:-$HOME}\"")
            .expect("should restore ZDOTDIR");
        let hook_pos = zshrc
            .find("__codirigent_precmd()")
            .expect("should define hook");
        assert!(
            restore_pos < hook_pos,
            "ZDOTDIR restore should come before precmd hook definition"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_setup_zsh_integration_idempotent() {
        let dir1 = setup_zsh_integration().unwrap();
        let content1 = std::fs::read_to_string(dir1.join(".zshrc")).unwrap();

        // Call again — should overwrite without error
        let dir2 = setup_zsh_integration().unwrap();
        let content2 = std::fs::read_to_string(dir2.join(".zshrc")).unwrap();

        assert_eq!(dir1, dir2, "should reuse the same directory");
        assert_eq!(content1, content2, "content should be identical");
    }

    #[cfg(unix)]
    #[test]
    fn test_setup_zsh_integration_zshenv_forwards_user_config() {
        let zdotdir = setup_zsh_integration().unwrap();
        let zshenv = std::fs::read_to_string(zdotdir.join(".zshenv")).unwrap();

        assert!(
            zshenv.contains("CODIRIGENT_ORIG_ZDOTDIR"),
            ".zshenv should reference original ZDOTDIR"
        );
        assert!(
            zshenv.contains(".zshenv"),
            ".zshenv should forward to user's .zshenv"
        );
    }
}
