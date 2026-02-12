# Codirigent

A terminal-based development environment with clipboard integration and session management.

## Building and Running

### Prerequisites

- Rust toolchain (stable channel)
- Windows, macOS, or Linux

### Quick Start

#### Development Build

```bash
# Build and run (no GUI)
cargo run

# Build and run in release mode (recommended for performance)
cargo run --release
```

#### With Full GUI (GPUI)

**Note:** Currently, the GUI feature has platform compatibility issues on Windows. See [Known Issues](#known-issues) below.

```bash
# macOS/Linux only
cargo run --release --features gpui-full
```

### Building Only

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release
```

The compiled binary will be located at:
- Debug: `target/debug/codirigent.exe` (Windows) or `target/debug/codirigent` (Unix)
- Release: `target/release/codirigent.exe` (Windows) or `target/release/codirigent` (Unix)

## Project Structure

```
codirigent/
├── crates/
│   ├── codirigent-core/      # Core data structures and types
│   ├── codirigent-detector/  # Session detection and notifications
│   ├── codirigent-filetree/  # File tree visualization
│   ├── codirigent-session/   # Session management
│   └── codirigent-ui/        # UI and clipboard handling
└── src/                       # Main application entry point
```

## Features

- **Smart Clipboard Integration**: Cross-platform clipboard monitoring with support for text and images
- **Session Management**: Track and manage terminal sessions
- **File Tree Visualization**: Navigate project structures efficiently
- **PTY Integration**: Pseudo-terminal support for interactive shells

## Platform-Specific Notes

### Windows

- Clipboard support uses the Win32 API via `clipboard-win` crate
- Full GUI features (`gpui-full`) are not currently supported due to dependency conflicts
- Use standard build without feature flags: `cargo run --release`

### macOS

- Uses native macOS clipboard APIs
- Full GUI support available with `--features gpui-full`
- Notifications via AppleScript integration

### Linux

- Uses X11 or Wayland clipboard protocols
- Requires `notify-send` for desktop notifications
- Full GUI support available with `--features gpui-full`

## Testing

### Run All Tests

```bash
cargo test
```

### Run Tests for Specific Crate

```bash
# Test clipboard functionality
cargo test --package codirigent-ui --lib platform::clipboard_windows

# Test core functionality
cargo test --package codirigent-core

# Test session management
cargo test --package codirigent-session
```

### Run with Logging

```bash
RUST_LOG=debug cargo run
```

## Known Issues

### Windows GUI Build Failure

The `--features gpui-full` flag currently fails on Windows with the error:

```
error[E0433]: failed to resolve: could not find `unix` in `os`
  --> core-foundation-0.10.1\src\filedescriptor.rs:19:14
```

**Cause**: The GPUI dependency tree includes `core-foundation`, a macOS-specific library that should not be compiled on Windows.

**Workaround**: Run without the `gpui-full` feature on Windows:
```bash
cargo run --release
```

**Fix**: The project's `Cargo.toml` needs platform-specific dependency configuration:
```toml
[target.'cfg(target_os = "macos")'.dependencies]
core-foundation = "..."
```

## Recent Fixes

### Windows Clipboard Compilation (2026-02-02)

Fixed compilation errors related to `clipboard-win` v5.4 API changes:
- Updated `seq_num()` handling to work with `Option<NonZeroU32>`
- Changed `.unwrap_or(0)` to `.map_or(0, |nz| nz.get())`
- All clipboard tests passing

See commit: `871c202` for details.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes with tests
4. Ensure all tests pass: `cargo test`
5. Submit a pull request

## License

[License information to be added]

## Development Notes

### Clipboard Implementation

The clipboard implementation is platform-specific:

- **Windows** (`clipboard_windows.rs`): Uses `clipboard-win` crate with DIB (Device Independent Bitmap) format
- **macOS** (`clipboard_macos.rs`): Native Pasteboard APIs
- **Linux** (`clipboard_linux.rs`): X11/Wayland clipboard protocols

### Session Detection

Sessions are detected through:
- PTY session monitoring
- Process tree analysis
- Input detector integration

### Terminal Integration

Uses `portable-pty` for cross-platform pseudo-terminal support with:
- Shell command execution
- Interactive terminal sessions
- Output capture and redirection

### Performance Guidelines

- See [Clone Optimization](docs/development/clone-optimization.md) for best practices
- Run benchmarks before major refactors: `cargo bench`
