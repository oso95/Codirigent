# Output Polling And Status

This document explains the runtime side of the workspace module: PTY output,
status updates, background polling, and related side effects.

## What `impl_output_polling.rs` Owns

`crates/codirigent-ui/src/workspace/impl_output_polling.rs` is the polling
root. It keeps:

- shared constants and helper types
- adaptive polling cadence
- detector maintenance orchestration
- clipboard preview maintenance
- stale proposal cleanup
- output-pipeline feature toggles (`LEGACY_PIPELINE`, `SHADOW_STATUS`)

It delegates narrower responsibility clusters into child modules under
`workspace/impl_output_polling/`.

## Polling Model

There are two cadences:

### Fast output cadence

Used when terminals are actively producing output.

Main responsibilities:

- drain PTY output
- apply terminal runtime snapshots
- keep focused sessions responsive

Key entry point:

- `poll_output()` in `output_runtime.rs`

### Slower maintenance cadence

Used for work that does not need to run every active frame.

Main responsibilities:

- hook signal scanning
- JSONL checks
- git refresh
- detector maintenance
- clipboard preview updates
- compaction timeout cleanup

Key entry point:

- `poll_maintenance()` in `impl_output_polling.rs`

## Child Modules

### `output_runtime.rs`

Hot-path output scheduling and application.

Owns:

- event-driven session readiness via `output_dispatcher`
- focused-session prioritization
- legacy fallback drain of `sessions_with_pending_output()`
- background preparation of drained PTY output
- UI-thread application of prepared output

Start here when:

- output is delayed
- the wrong session is prioritized
- a session without a terminal runtime gets dropped instead of retried

### `status_reconcile.rs`

Applies session status and all major side effects.

Inputs:

- detector state
- cached hook/JSONL status
- prior workspace session status

Owns:

- call into `status_engine::reconcile`
- expire stale cached status
- update workspace session status
- task transition side effects
- context clear / compaction follow-up
- auto-assign follow-up on returning to idle

Start here when:

- a status badge is wrong even though raw detector/log inputs seem correct
- status transitions do not trigger the right task or compaction behavior

### `cli_pollers.rs`

Background JSONL and rollout polling for Codex and Gemini.

Owns:

- JSONL input collection
- process-tree CLI detection fallback
- Codex session-id and execution-mode inference
- ambiguity guards when multiple Codex sessions share a working directory
- cache updates and notifications from JSONL results

Start here when:

- Codex/Gemini status does not update
- execution mode inference is wrong
- multiple Codex sessions in one directory interfere with one another

### `hook_signals.rs`

Background hook-signal ingestion.

Owns:

- signal-file scanning
- process-start epoch guard
- stale-signal rejection
- CLI session-id backfill from hook metadata
- hook-derived status and CLI metadata updates

Start here when:

- Claude Code hook updates are missing
- a hook signal is applied to the wrong session
- hook-derived `cli_session_id` or execution mode is wrong

### `git_refresh.rs`

Background git refresh.

Owns:

- refresh scheduling
- applying refreshed git info to headers and cached sessions

### `terminal_input.rs`

Terminal follow-up helpers not directly tied to output draining.

Owns:

- deferred Enter handling
- VTE response forwarding
- compaction timeout cleanup

This is the place to look when the shell is waiting for an expected terminal
response or when post-command follow-up input timing is wrong.

## Status Data Sources

Status is not driven by one source. The system combines multiple hints:

- detector state from `InputDetector`
- semantic classification of the current visible terminal viewport
- hook-derived status for Claude Code
- JSONL-derived status for Codex/Gemini
- stale-cache handling rules

The terminal viewport is reconstructed from `TerminalRenderSnapshot` after
each PTY output batch and passed to `InputDetector::process_visible_screen`.
This is important for full-screen Agent TUIs: screen redraws replace old
permission menus, while raw PTY history would keep stale menu text. Semantic
rules are status-bearing (`NeedsAttention`, `ResponseReady`, and so on) and
match interaction concepts instead of requiring a known `CliType`. Legacy
custom input regexes remain compatible and still mean `NeedsAttention`.

The actual arbitration happens in:

- `status_engine.rs`
- `status_providers.rs`
- `impl_output_polling/status_reconcile.rs`

Practical rule:

- if the wrong raw data is entering the system, fix `hook_signals.rs` or
  `cli_pollers.rs`
- if the raw data is correct but the chosen status is wrong, fix
  `status_engine.rs` / `status_reconcile.rs`

## Output Flow

The normal output path is:

1. background session runtime marks output ready
2. `SessionUpdate` events are drained into `output_dispatcher`
3. ready sessions are prioritized, focused first
4. output is prepared in the background
5. terminal runtime snapshots are applied on the UI thread
6. status/header follow-up is synchronized

The legacy broad-scan path still exists behind `CODIRIGENT_LEGACY_PIPELINE` as
a transition fallback.

## Cross-Platform Notes

The polling layer has real platform sensitivity:

- Windows terminal behavior is more sensitive to missed PTY resizes and DSR
  responses.
- Hook signal and JSONL path handling must tolerate Windows and macOS path
  conventions.
- Process detection and child-pid behavior can vary between platforms.

When changing output or status behavior, validate:

- normal interactive output
- Claude hook-driven status
- Codex/Gemini JSONL status
- compaction follow-up
- terminal response forwarding on shells that expect DSR/DA replies

## Related Files Outside The Split

- `output_dispatcher.rs`
  - ready/in-flight session scheduling state

- `status_engine.rs`
  - pure-ish reconciliation logic

- `status_providers.rs`
  - status hint source types and stale actions

- `types.rs`
  - cached CLI status structures and shared polling types

- `project_state.rs`
  - working-directory and project-root behavior used by polling follow-up
