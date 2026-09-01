//! Output scheduling and prepared-output application helpers.

use super::WorkspaceView;
use crate::terminal_runtime::TerminalRenderSnapshot;
use codirigent_core::{CliType, SessionId, SessionManager, SessionUpdate};
use codirigent_session::clipboard_service::ClipboardService;
use codirigent_session::{detect_cli_from_output, ShellState};
use gpui::Context;
use std::time::Instant;
use tracing::{info, trace};

#[derive(Debug)]
struct PreparedSessionOutput {
    session_id: SessionId,
    bytes_drained: usize,
    has_more: bool,
    render_snapshot: Option<TerminalRenderSnapshot>,
    detected_cli_type: Option<CliType>,
    revert_cli_to_shell: bool,
    cwd_session: Option<codirigent_core::Session>,
}

fn shell_prompt_event_reverts_cli(events: &[ShellState]) -> bool {
    events
        .iter()
        .any(|event| matches!(event, ShellState::PromptStart))
}

fn prioritize_and_partition_output_sessions<F>(
    mut session_ids: Vec<SessionId>,
    focused_id: Option<SessionId>,
    mut can_schedule: F,
) -> (Vec<SessionId>, Vec<SessionId>)
where
    F: FnMut(SessionId) -> bool,
{
    if let Some(focused_id) = focused_id {
        if let Some(index) = session_ids.iter().position(|id| *id == focused_id) {
            session_ids.swap(0, index);
        }
    }

    let mut ready = Vec::with_capacity(session_ids.len());
    let mut deferred = Vec::new();
    for session_id in session_ids {
        if can_schedule(session_id) {
            ready.push(session_id);
        } else {
            deferred.push(session_id);
        }
    }

    (ready, deferred)
}

impl WorkspaceView {
    pub(in crate::workspace) fn poll_output(&mut self, cx: &mut Context<Self>) {
        self.process_deferred_enters();
        self.drain_vte_responses();

        let had_output_activity = self.schedule_output_preparation(cx);

        // Safety-net: dispatch resume commands that have been waiting longer
        // than the fallback timeout (shell never produced output).
        if !self.polling.pending_resume_commands.is_empty() {
            self.dispatch_timed_out_resume_commands();
        }

        // Track output activity for adaptive polling
        //
        // Sessions that actually produced output are synchronized in
        // `apply_prepared_session_output()`. Detector-based status decay stays
        // on the slower maintenance cadence to avoid O(all sessions) work on
        // every active 16 ms poll.
        self.polling.last_poll_had_output = had_output_activity;
    }

    fn schedule_output_preparation(&mut self, cx: &mut Context<Self>) -> bool {
        if super::is_legacy_pipeline() {
            return self.schedule_output_preparation_legacy(cx);
        }

        // Phase 1: Drain the event-driven mpsc channel into the dispatcher.
        if let Some(ref mut rx) = self.update_rx {
            let other_events = self.output_dispatcher.drain_updates(rx);
            for event in other_events {
                match event {
                    SessionUpdate::ChildProcessExited { session_id } => {
                        // PTY child exited; mark session ready so it gets a
                        // final output drain and status re-evaluation.
                        trace!(
                            ?session_id,
                            "ChildProcessExited: marking ready for final drain"
                        );
                        self.output_dispatcher.mark_ready(session_id);
                    }
                    SessionUpdate::OutputReady { .. } => {
                        // Consumed by drain_updates into the ready set; should
                        // not appear here, but handle gracefully.
                    }
                    SessionUpdate::ShellStateChanged { session_id, .. }
                    | SessionUpdate::WorkingDirectoryChanged { session_id, .. } => {
                        // Phase-2: handled inline during output preparation
                        // (dual-path). Channel copies are informational only
                        // until phase-2 routing replaces the inline path.
                        trace!(?session_id, "phase-2 event received (not yet routed)");
                    }
                }
            }
        }

        // Phase 2: Low-frequency legacy safety net; drain the
        // pending_output_sessions set at ~1s intervals to catch any sessions
        // that bypass the mpsc channel (e.g., manual mark_output_pending
        // calls). This is NOT the hot path; the dispatcher handles that.
        if self.polling.last_legacy_fallback.elapsed() >= Self::LEGACY_FALLBACK_INTERVAL {
            self.polling.last_legacy_fallback = Instant::now();
            let legacy_ids =
                self.with_session_manager(|manager| manager.sessions_with_pending_output());
            if !legacy_ids.is_empty() {
                trace!(
                    count = legacy_ids.len(),
                    "legacy fallback drain (safety net)"
                );
                for id in &legacy_ids {
                    let was_new = self.output_dispatcher.mark_ready(*id);
                    // Shadow mode: log only genuinely missed events; sessions
                    // the mpsc channel didn't deliver to the dispatcher.
                    if super::is_shadow_status() && was_new {
                        info!(
                            ?id,
                            "shadow: legacy fallback discovered session not in dispatcher"
                        );
                    }
                }
            }
        }

        // Phase 3: Take ready sessions from the dispatcher (focused first).
        let session_ids = self
            .output_dispatcher
            .take_ready_sessions(self.workspace.focused_session_id());

        // Filter: only schedule sessions that have a terminal view.
        // Sessions without a terminal yet (gap between create_session and
        // terminals.insert) are re-queued so the next poll cycle picks them
        // up, avoiding a ~1s delay waiting for the legacy fallback.
        let mut schedulable = Vec::with_capacity(session_ids.len());
        for id in session_ids {
            if self.terminals.contains_key(&id) {
                schedulable.push(id);
            } else {
                self.output_dispatcher.mark_ready(id);
            }
        }

        let in_flight_count = self.output_dispatcher.in_flight_count();
        if !schedulable.is_empty() || in_flight_count > 0 {
            trace!(
                discovered_count = schedulable.len(),
                in_flight_count,
                "schedule_output_preparation"
            );
        }

        let had_output_activity = !schedulable.is_empty() || self.output_dispatcher.has_activity();

        for session_id in schedulable {
            self.schedule_session_output_preparation(session_id, cx);
        }

        had_output_activity
    }

    /// Legacy output preparation path; uses the broad
    /// `sessions_with_pending_output()` scan without the event-driven
    /// dispatcher. Activated by `CODIRIGENT_LEGACY_PIPELINE=1`.
    fn schedule_output_preparation_legacy(&mut self, cx: &mut Context<Self>) -> bool {
        let session_ids =
            self.with_session_manager(|manager| manager.sessions_with_pending_output());
        let (session_ids, deferred_ids) = prioritize_and_partition_output_sessions(
            session_ids,
            self.workspace.focused_session_id(),
            |id| {
                self.terminals.contains_key(&id)
                    && !self.polling.output_prepare_in_flight.contains(&id)
            },
        );

        if !deferred_ids.is_empty() {
            self.with_session_manager(|manager| {
                for session_id in deferred_ids {
                    manager.mark_output_pending(session_id);
                }
            });
        }

        let had_output_activity =
            !session_ids.is_empty() || !self.polling.output_prepare_in_flight.is_empty();

        for session_id in session_ids {
            self.schedule_session_output_preparation(session_id, cx);
        }

        had_output_activity
    }

    fn schedule_session_output_preparation(
        &mut self,
        session_id: SessionId,
        cx: &mut Context<Self>,
    ) {
        trace!(?session_id, "schedule_session_output_preparation");
        let Some(runtime) = self
            .terminals
            .get(&session_id)
            .map(|tv| tv.runtime_handle())
        else {
            trace!(
                ?session_id,
                "deferring output preparation until terminal runtime attaches"
            );
            self.output_dispatcher.mark_ready(session_id);
            self.with_session_manager(|manager| manager.mark_output_pending(session_id));
            return;
        };

        // Guard: prevent double-dispatch via the dispatcher's in-flight set.
        if !self.output_dispatcher.mark_in_flight(session_id) {
            return;
        }
        // TRANSITION: Legacy in-flight set kept in sync until
        // CODIRIGENT_LEGACY_PIPELINE and schedule_output_preparation_legacy
        // are removed. Both sets are always updated together.
        self.polling.output_prepare_in_flight.insert(session_id);
        debug_assert_eq!(
            self.polling.output_prepare_in_flight.len(),
            self.output_dispatcher.in_flight_count(),
            "dual in-flight sets desynchronized after marking session {} in-flight",
            session_id.0,
        );

        let session_manager = self.session_manager.clone();
        let detector = self.detector.clone();
        let update_tx = self.update_tx.clone();

        cx.spawn(async move |this: gpui::WeakEntity<Self>, cx| {
            let prepared = cx
                .background_executor()
                .spawn(async move {
                    let drained = {
                        let manager = session_manager.lock().ok()?;
                        manager.try_drain_output_bounded(
                            session_id,
                            Self::MAX_OUTPUT_CHUNKS_PER_POLL,
                            Self::MAX_OUTPUT_BYTES_PER_POLL,
                        )
                    }?;

                    let data = drained.data;
                    let bytes_drained = data.len();
                    let render_snapshot = runtime.apply_output(&data);
                    let visible_screen = render_snapshot
                        .as_ref()
                        .map(TerminalRenderSnapshot::visible_text);
                    let detected_cli_type = detect_cli_from_output(&data);

                    let shell_events = codirigent_session::extract_osc133_events(&data);
                    let revert_cli_to_shell = shell_prompt_event_reverts_cli(&shell_events);

                    {
                        let mut detector = detector.lock().ok()?;
                        detector.process_output(session_id, &data);
                        if let Some(screen) = visible_screen.as_deref() {
                            detector.process_visible_screen(session_id, screen);
                        }
                        for event in shell_events {
                            // DUAL-PATH: Emitted to channel for phase-2 event routing.
                            // Also applied directly below via set_shell_state() for correctness now.
                            if let Some(tx) = &update_tx {
                                if let Err(e) = tx.try_send(SessionUpdate::ShellStateChanged {
                                    session_id,
                                    state: event.clone(),
                                }) {
                                    trace!("ShellStateChanged try_send for {}: {e}", session_id.0);
                                }
                            }
                            detector.set_shell_state(session_id, event);
                        }
                    }

                    let cwd_session =
                        codirigent_session::extract_osc7_path(&data).and_then(|new_cwd| {
                            // DUAL-PATH: Emitted to channel for phase-2 event routing.
                            // Also applied directly below via update_working_directory() for correctness now.
                            if let Some(tx) = &update_tx {
                                if let Err(e) =
                                    tx.try_send(SessionUpdate::WorkingDirectoryChanged {
                                        session_id,
                                        cwd: new_cwd.clone(),
                                    })
                                {
                                    trace!(
                                        "WorkingDirectoryChanged try_send for {}: {e}",
                                        session_id.0
                                    );
                                }
                            }
                            let manager = session_manager.lock().ok()?;
                            let changed = manager.update_working_directory(session_id, new_cwd);
                            if changed {
                                manager.get_session(session_id)
                            } else {
                                None
                            }
                        });

                    Some(PreparedSessionOutput {
                        session_id,
                        bytes_drained,
                        has_more: drained.has_more,
                        render_snapshot,
                        detected_cli_type,
                        revert_cli_to_shell,
                        cwd_session,
                    })
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                this.polling.output_prepare_in_flight.remove(&session_id);
                this.output_dispatcher.complete_in_flight(session_id);
                debug_assert_eq!(
                    this.polling.output_prepare_in_flight.len(),
                    this.output_dispatcher.in_flight_count(),
                    "dual in-flight sets desynchronized after completing session {}",
                    session_id.0,
                );
                if let Some(prepared) = prepared {
                    this.apply_prepared_session_output(prepared, cx);
                } else {
                    // No output to drain (e.g. ChildProcessExited with no
                    // trailing bytes). Still run status reconciliation so
                    // OSC133-driven sessions don't stick in Working after
                    // the PTY exits.
                    if this.sync_session_status(session_id) {
                        this.sync_session_header(session_id);
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    fn apply_prepared_session_output(
        &mut self,
        prepared: PreparedSessionOutput,
        cx: &mut Context<Self>,
    ) {
        let PreparedSessionOutput {
            session_id,
            bytes_drained,
            has_more,
            render_snapshot,
            detected_cli_type,
            revert_cli_to_shell,
            cwd_session,
        } = prepared;
        trace!(
            ?session_id,
            bytes_drained,
            has_more,
            "apply_prepared_session_output"
        );

        // Prompt-aware resume dispatch: the shell has produced real output,
        // so it is alive and can accept input. Flush any queued resume
        // commands for this session now.
        if bytes_drained > 0 {
            self.polling.resume_shell_ready.insert(session_id);
            self.dispatch_pending_resume_commands_for_session(session_id);
        }

        let mut any_dirty = false;

        if let Some(snapshot) = render_snapshot {
            if let Some(terminal_view) = self.terminals.get_mut(&session_id) {
                any_dirty |= terminal_view.apply_snapshot(snapshot);
            }
        }

        if let Some(cli_type) = detected_cli_type {
            let current = self
                .clipboard
                .clipboard_service
                .get_session_cli_type(session_id);
            if current == CliType::GenericShell {
                self.clipboard
                    .clipboard_service
                    .set_session_cli_type(session_id, cli_type);
                any_dirty = true;
                info!(?session_id, ?cli_type, "Detected CLI type from output");
            }
        }

        if revert_cli_to_shell {
            let current = self
                .clipboard
                .clipboard_service
                .get_session_cli_type(session_id);
            if current != CliType::GenericShell {
                self.clipboard
                    .clipboard_service
                    .set_session_cli_type(session_id, CliType::GenericShell);
                any_dirty = true;
                info!(
                    ?session_id,
                    "Reverted CLI badge to shell after prompt return"
                );
            }
        }

        if let Some(mgr_session) = cwd_session {
            if let Some(header) = self.terminal_headers.get_mut(&session_id) {
                header.git_branch = None;
                header.git_pending_additions = None;
                header.git_pending_deletions = None;
            }

            if let Some(ws_session) = self.workspace.session_mut(session_id) {
                super::git_refresh::apply_cwd_session_update_from_manager(ws_session, &mgr_session);
            }

            if self.workspace.focused_session_id() == Some(session_id) {
                self.sync_file_tree_to_focused_session(cx);
            }

            self.spawn_session_git_refresh(session_id, mgr_session.working_directory.clone(), cx);
            self.save_state_to_disk(cx);
            any_dirty = true;
        }

        any_dirty |= self.sync_session_status(session_id);

        // Targeted delta: sync only this session's header instead of
        // dirtying the full UI sync path for every output poll.
        if any_dirty {
            self.sync_session_header(session_id);
            cx.notify();
        }
        if has_more {
            // Re-queue through the dispatcher so other sessions get fair
            // scheduling in the next poll cycle (16ms), instead of immediately
            // re-entering schedule_session_output_preparation which bypasses
            // the dispatcher's focused-first prioritization.
            self.output_dispatcher.mark_ready(session_id);
            // Also mark in the legacy pending set so the legacy path picks
            // it up when CODIRIGENT_LEGACY_PIPELINE=1 is active.
            self.with_session_manager(|manager| manager.mark_output_pending(session_id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn focused_schedulable_output_is_prioritized() {
        let session_ids = vec![SessionId(1), SessionId(2), SessionId(3)];
        let schedulable = HashSet::from([SessionId(2), SessionId(3)]);

        let (ready, deferred) =
            prioritize_and_partition_output_sessions(session_ids, Some(SessionId(2)), |id| {
                schedulable.contains(&id)
            });

        assert_eq!(ready, vec![SessionId(2), SessionId(3)]);
        assert_eq!(deferred, vec![SessionId(1)]);
    }

    #[test]
    fn unschedulable_output_sessions_are_deferred_instead_of_dropped() {
        let session_ids = vec![SessionId(1), SessionId(2), SessionId(3)];
        let schedulable = HashSet::from([SessionId(3)]);

        let (ready, deferred) =
            prioritize_and_partition_output_sessions(session_ids, Some(SessionId(2)), |id| {
                schedulable.contains(&id)
            });

        assert_eq!(ready, vec![SessionId(3)]);
        assert_eq!(deferred, vec![SessionId(2), SessionId(1)]);
    }

    #[test]
    fn shell_prompt_events_revert_cli_to_shell() {
        assert!(shell_prompt_event_reverts_cli(&[ShellState::PromptStart]));
        assert!(!shell_prompt_event_reverts_cli(&[
            ShellState::CommandExecuted,
            ShellState::CommandInputStart,
        ]));
        assert!(!shell_prompt_event_reverts_cli(&[
            ShellState::CommandExecuted
        ]));
        assert!(!shell_prompt_event_reverts_cli(&[
            ShellState::CommandFinished { exit_code: Some(0) }
        ]));
    }
}
