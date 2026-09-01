<p align="center">
  <img src="assets/icons/logo-primary-dark.svg" alt="Codirigent Logo" width="128" height="128" />
</p>

<h1 align="center">Codirigent</h1>

<p align="center">
  <strong>A terminal workspace for running multiple AI coding CLIs in parallel</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/status-alpha-orange" alt="Project Status" />
  <img src="https://img.shields.io/badge/version-0.1.0-blue" alt="Version" />
  <img src="https://img.shields.io/badge/license-GPL--3.0-blue" alt="License" />
  <img src="https://img.shields.io/badge/rust-1.75%2B-red" alt="Rust Version" />
</p>

<p align="center">
  <a href="https://codirigent.dev">Website</a> ·
  <a href="https://github.com/oso95/Codirigent/releases/latest">Download</a> ·
  <a href="https://github.com/oso95/Codirigent/issues">Report a Bug</a> ·
  <a href="./README.zh-TW.md">繁體中文</a> ·
  <a href="./README.zh-CN.md">简体中文</a>

</p>

---

https://github.com/user-attachments/assets/51b821fd-dfc3-40f0-b1f3-e8727045f474

---

If you're running Claude Code, Codex, or Gemini across multiple projects at the same time, you know the pain: opening terminals, `cd`-ing into repos, arranging windows, losing track of which agent is doing what.

Codirigent is a Tmux-style workspace built for this workflow. Open it once, and your sessions are already where you left them — right directory, right layout, right agent.

## Features

**Multiple sessions, one view** — run Claude Code, Codex, and Gemini side by side. Each session shows a real-time status indicator:

| Status | | Meaning |
|--------|---|---------|
| Idle | ![gray](https://img.shields.io/badge/●-gray) | Shell idle, no agent activity |
| Working | ![amber](https://img.shields.io/badge/●-f59e0b) | Agent is generating a response |
| Attention | ![rose](https://img.shields.io/badge/●-f43f5e) | Agent is waiting for user input or permission |
| Ready | ![green](https://img.shields.io/badge/●-22c55e) | Agent finished, response is waiting in an unfocused session |

---

**Custom layouts** — arrange sessions in any grid configuration and save them. Drag and drop session headers to rearrange positions on the fly.

---

**Synced file tree** — the file explorer always reflects whichever session is focused, so you always know where you are.

---

**Git worktree support** — run agents on isolated branches simultaneously without conflicts.

---

**Session resume** — Codirigent detects and resumes previous Claude Code and Codex sessions automatically, so you pick up right where you left off.

---

**Smart clipboard** — paste text, files, or images into any session. File paths are automatically converted to shell-friendly formats for the target CLI.

## Download

> **Early alpha** — expect rough edges. [Feedback welcome.](https://github.com/oso95/Codirigent/issues)

### Windows

Download the `.msi` installer from the [latest release](https://github.com/oso95/Codirigent/releases/latest).

> **SmartScreen warning:** Windows may show "Windows protected your PC" on first install. The MSI is code-signed but SmartScreen requires reputation history. Click **More info → Run anyway** to proceed.

### macOS

Download the `.dmg` from the [latest release](https://github.com/oso95/Codirigent/releases/latest).

## Hook Setup (Recommended)

Codirigent uses lightweight hooks to track agent status in real time — showing whether each session is Working, Needs Attention, or has a Response Ready. If hooks are unavailable, Codirigent falls back to its reader/detector path, which is less precise.

**Hooks are installed automatically** on first launch for supported CLIs. Codirigent registers its `codirigent-hook` binary into each CLI's configuration:

| CLI | Config file | Auto-installed |
|-----|-------------|----------------|
| Claude Code | `~/.claude/settings.json` | Yes |
| Codex CLI | `~/.codex/config.toml` | Yes |
| Gemini CLI | `~/.gemini/settings.json` | Yes |

To verify hooks are installed, check that `codirigent-hook` appears in your CLI's config file. If you move or reinstall Codirigent, relaunch it once to re-register the hooks with the updated binary path.

## Build from Source

**Prerequisites:** Rust 1.75+, Windows or macOS

```bash
git clone https://github.com/oso95/Codirigent.git
cd Codirigent
cargo install --path . --all-features
cargo install --path crates/codirigent-hook
```

This installs both `codirigent` and `codirigent-hook` to `~/.cargo/bin/`. The hook binary is required for real-time agent status tracking (see [Hook Setup](#hook-setup-recommended)).

To run without installing:

```bash
cargo run --all-features
```

> Linux support is not yet complete.

## Development

```bash
cargo test --all --all-targets        # run tests
cargo fmt --all                       # format
cargo clippy --all -- -D warnings     # lint
```

## Contributing

Open an issue before major changes. PRs welcome.

## License

GPL-3.0 — see [LICENSE](LICENSE).
