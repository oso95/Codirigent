# Hook and Status System

End-to-end description of how Codirigent tracks agent session status — from
hook registration through signal files, polling, UI display, and desktop
notifications. Written so the same pattern can be replicated for Codex CLI and
Gemini CLI.

---

## Overview

```
Agent CLI (e.g. Claude Code)
  │
  │  fires hook on lifecycle event
  ▼
codirigent-hook binary
  │
  │  writes signal file
  ▼
~/.../codirigent/signals/<claude-session-id>.json
  │
  │  polled every ~1 s on UI thread
  ▼
check_hook_signals()  (impl_output_polling.rs)
  │
  │  updates CachedCliStatus
  ▼
SessionStatus enum  →  UI (header badge, sidebar badge, theme color)
                    →  Desktop notification (on transition)
```

---

## 1. Hook Registration

Claude Code supports lifecycle hooks configured in `~/.claude/settings.json`.
Codirigent registers `codirigent-hook` for three events:

| Hook event        | When it fires                                    |
|-------------------|--------------------------------------------------|
| `UserPromptSubmit`| User sends a message — agent starts working      |
| `Stop`            | Agent finishes generating a response             |
| `Notification`    | Claude Code emits a notification (e.g. permission prompt, idle prompt) |

Example settings entry (abbreviated):

```json
{
  "hooks": {
    "UserPromptSubmit": [{ "hooks": [{ "type": "command", "command": "/path/to/codirigent-hook" }] }],
    "Stop":             [{ "hooks": [{ "type": "command", "command": "/path/to/codirigent-hook" }] }],
    "Notification":     [{ "hooks": [{ "type": "command", "command": "/path/to/codirigent-hook" }] }]
  }
}
```

The hook binary path is written as a full absolute path (with quotes if it
contains spaces) so it works regardless of `$PATH`.

---

## 2. `codirigent-hook` Binary

**Crate:** `crates/codirigent-hook`

Claude Code calls the hook binary for each event, passing a JSON payload on
stdin:

```json
{
  "session_id": "abc-123",
  "hook_event_name": "Stop",
  "cwd": "/Users/you/project",
  "notification_type": "permission_prompt"
}
```

### Session ID Matching

`CODIRIGENT_SESSION_ID` is the key. When Codirigent spawns a Claude Code PTY
session, it injects this environment variable set to Codirigent's own internal
`SessionId` (a `u64`). Because Claude Code inherits and passes its environment
to child processes, the hook binary receives this variable automatically.

If `CODIRIGENT_SESSION_ID` is not set, the hook exits immediately — this means
the Claude Code instance was started outside of Codirigent and we should not
track it.

### Status Mapping

```
hook_event_name       notification_type    → signal status
─────────────────────────────────────────────────────────
UserPromptSubmit      (any)               → "working"
Stop                  (any)               → "response_ready"
Notification          "permission_prompt" → "needs_attention"
Notification          "idle_prompt"       → "response_ready"
Notification          anything else       → "idle"
(unknown event)       (any)               → "idle"
```

Claude Code can emit `idle_prompt` immediately after `Stop`. Both events mean
that Claude has finished and is waiting for the next user prompt, so they map
to the same `response_ready` signal. This also prevents a later notification
from overwriting an unread completion with `idle` before the next UI poll.

### Generic Agent fallback

Agents without a dedicated hook or JSONL reader use the terminal detector's
visible-screen semantic rules. The detector reconstructs the current viewport
after each PTY output batch and classifies interaction semantics such as:

- a menu containing allow/approve, deny/reject, and select/confirm actions →
  `needs_attention`
- an Agent-style empty input prompt plus interaction chrome, or an explicit
  “ready for next task” state → `response_ready`

These rules do not require the Agent to have a `CliType` enum variant. A new
integration with unique terminal wording can add a `StatusRule` declaring its
target status and required/excluded regex features; old custom prompt regexes
continue to map to `needs_attention`.

### Signal File

The hook writes a small JSON file to the signals directory:

```json
{
  "status": "response_ready",
  "codirigent_session_id": "7",
  "ts": 1710000000
}
```

- `status`: one of `working`, `response_ready`, `needs_attention`, `idle`
- `codirigent_session_id`: Codirigent's internal session ID (from the env var)
- `ts`: Unix timestamp in seconds (used to discard stale signals)

**File path:**

| Platform | Path |
|----------|------|
| Windows  | `%APPDATA%\codirigent\signals\<claude-session-id>.json` |
| Linux/macOS | `$XDG_CONFIG_HOME/codirigent/signals/<claude-session-id>.json` (falls back to `~/.config/codirigent/signals/`) |

The filename stem is the **Claude Code session ID** (the `session_id` field
from the hook payload). This is a UUID-like string. The file is validated to
contain only alphanumerics, hyphens, and underscores before use as a filename.

---

## 3. Signal Polling (`check_hook_signals`)

**File:** `crates/codirigent-ui/src/workspace/impl_output_polling.rs`

`check_hook_signals` is called on every UI poll cycle (~1 s). It:

1. Reads all `.json` files from the signals directory.
2. Discards signals older than 600 seconds (`ts` guard).
3. Skips files where `codirigent_session_id` is absent (Claude Code started
   outside Codirigent).
4. Parses `codirigent_session_id` as a `u64` → `SessionId`.
5. Maps the status string to a `SessionStatus` value.
6. Writes the result into `CachedCliStatus` (held in `cli_readers`).

### Focus-Aware Status

`response_ready` is mapped differently depending on whether the session is
currently focused:

```
signal status     session focused?   → SessionStatus
─────────────────────────────────────────────────────
response_ready    yes                → Idle
response_ready    no                 → ResponseReady
```

If the user is already looking at the session, the response is visible — there
is no need for a badge.

### Cache TTL

Hook-signal cache entries have a TTL of **600 seconds**
(`HOOK_SIGNAL_CACHE_TTL`). This matches the stale-signal discard guard so a
long-running task that sends `working` once does not lose its status before the
next hook fires.

JSONL-based entries (Codex/Gemini) use a shorter TTL of **120 seconds**
(`GENERIC_SHELL_JSONL_CACHE_TTL`).

### Immediate Cache Downgrade on Focus

When the user clicks a `ResponseReady` session, `select_session_with_cx`
immediately downgrades its `CachedCliStatus` from `ResponseReady` to `Idle`
without waiting for the next poll. This makes the badge disappear instantly.

---

## 4. `SessionStatus` Enum

**File:** `crates/codirigent-core/src/types/status.rs`

```rust
pub enum SessionStatus {
    Idle,           // Shell idle, no activity
    Working,        // Agent generating a response
    NeedsAttention, // Waiting for user input or permission
    ResponseReady,  // Agent finished, session not focused
    Error,          // Error detected
}
```

---

## 5. Visual Display

### Session Header (`terminal_header.rs`)

Each session pane has a header showing a `StatusIndicator`:

| Status          | Label       | Color    | Animated |
|-----------------|-------------|----------|----------|
| Idle            | "Idle"      | `#52525b` (gray)   | No  |
| Working         | "Working"   | `#f59e0b` (amber)  | Yes |
| NeedsAttention  | "Attention" | `#f43f5e` (rose)   | Yes |
| ResponseReady   | "Ready"     | `#22c55e` (green)  | No  |
| Error           | "Error"     | `#ef4444` (red)    | No  |

### Sidebar Badge (`sidebar/types.rs`)

The session list sidebar shows a `StatusBadge` pill next to each session:

| Status          | Text        | Background              | Text color |
|-----------------|-------------|-------------------------|------------|
| Idle            | "Idle"      | gray @ 20%              | gray       |
| Working         | "Working"   | teal @ 15%              | yellow     |
| NeedsAttention  | "Attention" | rose @ 15%              | rose       |
| ResponseReady   | "Ready"     | green @ 15%             | green      |
| Error           | "Error"     | red @ 15%               | red        |

### Theme Colors (`theme.rs`)

`CodirigentTheme` exposes per-status `Hsla` colors through `status_color()`:

| Status          | Dark theme    | Light theme  |
|-----------------|---------------|--------------|
| ResponseReady   | `#22c55e` (Green-500) | `#16a34a` (Green-600) |

---

## 6. Desktop Notifications

**File:** `crates/codirigent-detector/src/notification.rs`

All notifications go through `NotificationManager`, which enforces:

1. **Master toggle** (`settings.desktop`) — disables everything when false.
2. **Per-type toggle** — each notification type can be individually disabled.
3. **Per-session cooldown** — suppresses repeat notifications within
   `cooldown_seconds` (default 30 s).

### Notification Types

| `NotificationType`  | Setting field      | Trigger                                      |
|---------------------|--------------------|----------------------------------------------|
| `InputRequired`     | `input_required`   | Session waiting for user message             |
| `PermissionPrompt`  | `permission_prompt`| Agent needs tool permission                  |
| `ResponseReady`     | `response_ready`   | Agent finished, session not focused          |
| `TaskCompleted`     | `task_completed`   | Assigned task completed                      |
| `TaskFailed`        | `task_failed`      | Assigned task failed                         |
| `Error`             | `error`            | Session error detected                       |

### When ResponseReady Notification Fires

The notification fires only on the transition `Working → ResponseReady`, not on
every poll:

```rust
if new_status == SessionStatus::ResponseReady && prev_status == SessionStatus::Working {
    // send notification
}
```

This prevents duplicate notifications if the poll reads the same signal file
multiple times before the next hook event.

### Platform Implementation

| Platform | Mechanism |
|----------|-----------|
| macOS    | `osascript` — `display notification "..." with title "..."` |
| Windows  | `winrt-notification` crate — Windows toast via PowerShell App ID |

---

## 7. Settings

**File:** `crates/codirigent-core/src/config.rs`

```rust
pub struct NotificationSettings {
    pub desktop: bool,            // master toggle
    pub sound: bool,
    pub input_required: bool,     // default true
    pub task_completed: bool,     // default true
    pub task_failed: bool,        // default true
    pub permission_prompt: bool,  // default true
    pub response_ready: bool,     // default true
    pub error: bool,              // default true
    pub cooldown_seconds: u64,    // default 30
}
```

All per-type fields use `#[serde(default = "default_true")]` so existing config
files without the field default to `true` on load.

The settings panel (`workspace/settings_panels.rs`) renders a toggle row for
each of these fields.

---

## 8. Replicating for Codex CLI / other CLIs

Codirigent now installs native hooks for both Claude Code and Gemini CLI.
Codex still relies on its notify hook plus JSONL log polling, and Gemini keeps
its session reader as a higher-fidelity fallback.

To replicate hook-level status precision for another CLI, you need three
things:

### Step 1 — Detect lifecycle events

Find the equivalent hook or event callback mechanism in the CLI.
Options, in order of preference:

1. **Native hooks** (like Claude Code's `UserPromptSubmit`/`Stop`) — ideal.
2. **Log file / JSONL output** — parse output to detect start/finish patterns.
3. **Output terminal markers** — detect prompt lines, spinner patterns, etc.

If the CLI exposes a hook/plugin system, register a small binary (equivalent to
`codirigent-hook`) that receives event payloads and writes signal files in the
same format.

### Step 2 — Write signal files in the same format

The signal file format is intentionally minimal and CLI-agnostic:

```json
{
  "status": "working",
  "codirigent_session_id": "<N>",
  "ts": <unix-seconds>
}
```

Write to the same signals directory. The filename should be a safe unique
identifier (e.g. the CLI's own session/process ID).

The four valid status strings are: `working`, `response_ready`,
`needs_attention`, `idle`.

### Step 3 — No changes needed in the consumer

`check_hook_signals` reads all `.json` files in the signals directory regardless
of which CLI wrote them. Matching is done on `codirigent_session_id`, not on
filename or CLI type. As long as `CODIRIGENT_SESSION_ID` is set in the
environment when Codirigent spawns the CLI session, the signal will be matched
correctly.

### Mapping CLI events to statuses

Decide how each CLI lifecycle event maps to a status string:

| Event semantics                        | Status string       |
|----------------------------------------|---------------------|
| User submitted a prompt / task started | `working`           |
| Agent finished responding              | `response_ready`    |
| Waiting for user input                 | `needs_attention`   |
| Permission/tool approval needed        | `needs_attention`   |
| Session idle                           | `idle`              |

### What to build per CLI

| Component | Claude Code / Gemini CLI | Codex CLI |
|-----------|-------------|----------------|
| Hook binary | `codirigent-hook` (registered in the CLI config file) | Notify/log bridge registered in the CLI's equivalent config |
| Session env var | `CODIRIGENT_SESSION_ID` set by Codirigent on PTY spawn | Same env var — no change needed |
| Signal file location | `%APPDATA%\codirigent\signals\` | Same directory — shared |
| Consumer (`check_hook_signals`) | Reads all `.json` files | No change needed |
| `SessionStatus` enum | Existing variants cover all cases | No change needed |
| UI display | Existing header/sidebar/theme | No change needed |
| Notifications | Existing `NotificationManager` | No change needed |

The only new code required is the CLI-specific hook or log bridge that writes
the signal file.
