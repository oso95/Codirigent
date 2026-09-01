# Session Resume

How Codirigent persists Claude Code session identity across restarts and
resumes sessions at the same permission level — so the agent picks up exactly
where it left off.

---

## Overview

```
First run
─────────────────────────────────────────────────────────────────
Codirigent spawns PTY
  │  sets CODIRIGENT_SESSION_ID=<N> in env
  ▼
Claude Code starts, generates its own UUID session ID
  │
  │  on UserPromptSubmit / Stop / Notification
  ▼
codirigent-hook fires → writes signals/<claude-uuid>.json
  │  contains codirigent_session_id = "<N>"
  ▼
check_hook_signals() reads signal
  │  filename stem = claude_session_id (UUID)
  │  saves to Session.claude_session_id
  ▼
save_state_to_disk() persists to .codirigent/state.json


Next run (Codirigent restart)
─────────────────────────────────────────────────────────────────
restore_sessions_from_disk() reads state.json
  │  finds claude_session_id + working_directory per session
  ▼
read_claude_permission_mode(claude_session_id)
  │  scans ~/.claude/projects/*/<claude_session_id>.jsonl in reverse
  │  finds the permissionMode that was active
  ▼
Builds resume command:
  "claude --resume <claude_session_id>"
  + " --dangerously-skip-permissions"  (if bypassPermissions)
  ▼
Sends command to new PTY shell → Claude Code resumes conversation
```

---

## Three IDs Involved

It helps to keep these distinct:

| ID | Type | Owner | Purpose |
|----|------|-------|---------|
| `SessionId` | `u64` | Codirigent | Internal session key, passed to PTY via `CODIRIGENT_SESSION_ID` env var |
| `claude_session_id` | UUID string | Claude Code | Claude Code's own session ID; used with `--resume` and as signal filename |
| `codirigent_session_id` | string (=`SessionId`) | hook binary | Written into the signal JSON; links the signal file back to Codirigent's session |

---

## Step 1 — Spawning and ID Injection

**`crates/codirigent-session/src/manager.rs` → `create_session()`**

When Codirigent creates a session it injects `CODIRIGENT_SESSION_ID` into the
PTY environment:

```rust
let id_str = id.0.to_string();
let env_vars: &[(&str, &str)] = &[("CODIRIGENT_SESSION_ID", &id_str)];
// ... spawn PTY with env_vars
```

Claude Code inherits this variable and passes it to all child processes,
including `codirigent-hook`. At this point `Session.claude_session_id` is
`None` — Claude Code has not yet identified itself.

---

## Step 2 — Discovering the Claude Code Session ID

**`crates/codirigent-ui/src/workspace/impl_output_polling.rs` → `check_hook_signals()`**

Every ~1 second the poll loop reads all `.json` files from the signals
directory. The **filename stem** of each file is the Claude Code UUID (the
`session_id` field from the hook payload):

```
signals/
  abc-1234-def.json   ← stem = "abc-1234-def" = claude_session_id
```

The JSON content provides `codirigent_session_id` which maps back to
Codirigent's `SessionId`:

```json
{
  "status": "working",
  "codirigent_session_id": "7",
  "ts": 1710000000
}
```

Matching logic:

1. Parse `codirigent_session_id` → `SessionId(7)`
2. Look up session 7 in the workspace
3. If `session.claude_session_id` is not yet set, store the filename stem
4. Persist on the next `save_state_to_disk()` call

```rust
mgr.with_session_state_mut(session_id, |state| {
    state.session.claude_session_id = Some(claude_session_id.clone());
});
```

The session ID is also persisted eagerly: any time `claude_session_id` changes
(including the very first assignment), a disk save is triggered so Codirigent
can survive an immediate crash after the first hook fires.

---

## Step 3 — Persisting to Disk

**`crates/codirigent-ui/src/workspace/gpui.rs` → `save_state_to_disk()`**

State is written to `.codirigent/state.json` (relative to project root) using
an atomic write (temp file + rename). The relevant fields per session:

```json
{
  "sessions": [
    {
      "id": 7,
      "name": "Backend API",
      "working_directory": "/Users/you/project",
      "claude_session_id": "abc-1234-def",
      "claude_permission_mode": "bypassPermissions",
      "group": null,
      "color": "#6366f1"
    }
  ],
  "layout": "Grid2x2"
}
```

`claude_permission_mode` is stored alongside `claude_session_id` so Codirigent
does not need to re-scan the JSONL on every startup for sessions whose
permission mode is already known.

---

## Step 4 — Detecting the Permission Mode (KEY)

**`crates/codirigent-ui/src/workspace/impl_session_lifecycle.rs` → `read_claude_permission_mode()`**

This is what makes the resume seamless: Codirigent resumes Claude Code **at
the same permission level it was running at before**.

Claude Code stores its session log at:

```
~/.claude/projects/<project-hash>/<claude_session_id>.jsonl
```

Multiple project directories may exist (Claude Code can use slightly different
path hashing). Codirigent scans all subdirectories of `~/.claude/projects/` to
find the right file.

The JSONL is scanned **in reverse** because `permissionMode` is not present on
every line — only on lines that recorded a permission state change. Scanning in
reverse finds the most recent value without reading the entire (potentially
large) file:

```rust
for line in lines.iter().rev() {
    if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
        if let Some(mode) = entry.get("permissionMode").and_then(|v| v.as_str()) {
            return Some(mode.to_string());
        }
    }
}
```

### Permission Mode → Resume Flag

| `permissionMode` in JSONL | Flag added to resume command |
|---------------------------|------------------------------|
| `"bypassPermissions"`     | `--dangerously-skip-permissions` |
| `"default"` / `"acceptEdits"` / anything else | (none) |
| Not found / JSONL missing | (none — resume without flag) |

If the original session was running with `bypassPermissions`, the resumed
session gets `--dangerously-skip-permissions` so the agent can continue without
being blocked by permission prompts it would have been allowed to skip before.

---

## Step 5 — Sending the Resume Command

**`crates/codirigent-ui/src/workspace/impl_session_lifecycle.rs` → `restore_sessions_from_disk()`**

On startup, for each session with a saved `claude_session_id`:

```rust
let permission_mode = read_claude_permission_mode(claude_id).unwrap_or_default();
let mut cmd = format!("claude --resume {}", claude_id);
if permission_mode == "bypassPermissions" {
    cmd.push_str(" --dangerously-skip-permissions");
}
cmd.push('\r');
mgr.send_input(session_id, cmd.as_bytes())?;
```

This sends the command into the PTY shell as if the user typed it. Claude Code
loads the conversation history from its own JSONL store and continues from the
last message.

---

## Data Structures

**`Session`** — `crates/codirigent-core/src/types/session.rs`

```rust
pub struct Session {
    pub id: SessionId,
    pub name: String,
    pub working_directory: PathBuf,
    pub claude_session_id: Option<String>,   // UUID, None until first hook fires
    pub group: Option<String>,
    pub color: Option<String>,
    // ... other fields
}
```

**`PersistentSession`** — `crates/codirigent-core/src/persistence.rs`

```rust
pub struct PersistentSession {
    pub id: SessionId,
    pub name: String,
    pub working_directory: PathBuf,
    pub claude_session_id: Option<String>,        // written to state.json
    pub claude_permission_mode: Option<String>,   // cached from JSONL scan
    // ... other fields
}
```

---

## Signal File Location

| Platform | Path |
|----------|------|
| Windows  | `%APPDATA%\codirigent\signals\<claude-uuid>.json` |
| Linux/macOS | `$XDG_CONFIG_HOME/codirigent/signals/` or `~/.config/codirigent/signals/` |

State file: `.codirigent/state.json` (project-relative)

Claude Code JSONL: `~/.claude/projects/<project-hash>/<claude-uuid>.jsonl`

---

## Edge Cases

**Session ID not yet known at shutdown** — if Codirigent is killed before the
first hook fires (no prompt was ever submitted), `claude_session_id` is `None`.
On restore the session opens a fresh Claude Code instance rather than resuming.

**JSONL file missing** — if `~/.claude/projects/` does not contain the session
file (e.g., user deleted history), `read_claude_permission_mode` returns `None`
and the session resumes without `--dangerously-skip-permissions`. The
conversation history will also be unavailable to Claude Code.

**Multiple project directories** — Claude Code sometimes hashes paths
differently (e.g., `\\?\` canonical prefix on Windows produces a different
directory name). Codirigent scans all subdirectories so it finds the file
regardless of which hash was used.

**Stale signals** — signals older than 600 seconds are discarded. After a
restart the new PTY session generates new hook signals; there is no conflict
with the old ones because they time out.

---

## Replicating for Codex CLI / Gemini CLI

The resume mechanism has two parts: **conversation history** and **permission
level**. For other CLIs:

### Conversation history

Check whether the CLI supports a `--resume` / `--continue` / `--session` flag
that accepts a session identifier. If it does:

1. Store the CLI-specific session identifier in `Session.claude_session_id`
   (the field name is Claude-specific but it holds any string ID).
2. On restore, send the appropriate command, e.g.:
   - Codex: `codex --session <id>` (verify exact flag with Codex docs)
   - Gemini CLI: check `gemini --help` for a resume/continue flag

### Permission level

Each CLI has its own permission/safety model:

| CLI | Permission concept | Flag equivalent |
|-----|--------------------|-----------------|
| Claude Code | `permissionMode: bypassPermissions` | `--dangerously-skip-permissions` |
| Codex CLI | `--full-auto` (no confirmations) | TBD — read Codex session log format |
| Gemini CLI | TBD | TBD |

To support a new CLI:

1. Identify where the CLI stores its session log and what field records the
   permission/safety mode.
2. Write an equivalent of `read_claude_permission_mode()` that reads that log.
3. Map the detected mode to the correct CLI flag.
4. Extend the restore branch in `restore_sessions_from_disk()` to call the
   right function based on detected CLI type.

The rest of the infrastructure — `CODIRIGENT_SESSION_ID` injection, signal
files, `state.json`, `SessionId` matching — is already CLI-agnostic and
requires no changes.
