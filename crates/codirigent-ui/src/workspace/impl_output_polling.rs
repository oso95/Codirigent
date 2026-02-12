//! Output polling and session status management for WorkspaceView.
//!
//! This module contains the main output polling loop that:
//! - Drains PTY output and feeds it to terminal emulators
//! - Detects CLI types from output banners
//! - Processes shell state markers (OSC 133) and working directory changes (OSC 7)
//! - Overlays JSONL-derived session status over pattern-based detection
//! - Manages automatic task assignment and context compaction
//! - Handles clipboard preview auto-show/hide

use super::cli_helpers::{clear_command, detect_cli_from_output, format_task_input};
use super::gpui::WorkspaceView;
use codirigent_core::{AssignmentAction, CodirigentEvent, EventBus, ProcessMonitor, SessionId, SessionManager, SessionStatus, TaskStatus};
use codirigent_session::cli_detector::CliDetector;
use codirigent_session::clipboard_service::ClipboardService;
use gpui::Context;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

impl WorkspaceView {
    pub(super) fn poll_output(&mut self, cx: &mut Context<Self>) {
        // Send deferred Enter keypresses (text was sent earlier; Enter comes
        // as a separate write so ink treats it as a distinct stdin event).
        // Phase 1: entries where Enter hasn't been sent yet and 100ms elapsed → send \r.
        let need_enter: Vec<SessionId> = self
            .polling.pending_enters
            .iter()
            .filter(|(_, (when, sent))| !sent && when.elapsed() >= Duration::from_millis(100))
            .map(|(id, _)| *id)
            .collect();
        for session_id in need_enter {
            if let Ok(mgr) = self.session_manager.lock() {
                let _ = mgr.send_input(session_id, b"\r");
            }
            // Flip to phase 2: keep entry for a grace period so the CLI can
            // process the command before auto-assign considers this session.
            self.polling.pending_enters
                .insert(session_id, (Instant::now(), true));
        }
        // Phase 2: remove entries where Enter was already sent and grace period elapsed.
        let expired: Vec<SessionId> = self
            .polling.pending_enters
            .iter()
            .filter(|(_, (when, sent))| *sent && when.elapsed() >= Duration::from_millis(500))
            .map(|(id, _)| *id)
            .collect();
        for session_id in expired {
            self.polling.pending_enters.remove(&session_id);
        }

        let session_ids: Vec<SessionId> = self.terminals.keys().copied().collect();
        let mut any_dirty = false;

        // Decide once whether to run JSONL checks this cycle (throttled to ~1/second)
        let has_any_reader = self.cli_readers.claude.is_some()
            || self.cli_readers.codex.is_some()
            || self.cli_readers.gemini.is_some();
        let check_jsonl =
            has_any_reader && self.polling.last_jsonl_check.elapsed() >= Duration::from_secs(1);
        if check_jsonl {
            self.polling.last_jsonl_check = Instant::now();
        }

        for session_id in session_ids {
            // Try to drain output from the session manager
            let output = self.with_session_manager(|manager| {
                manager.try_drain_output(session_id)
            });

            if let Some(data) = output {
                // Feed output to terminal emulator
                if let Some(terminal_view) = self.terminals.get_mut(&session_id) {
                    terminal_view.terminal_mut().process_output(&data);
                    any_dirty = true;
                }

                // Detect CLI type from output banners
                if let Some(cli_type) = detect_cli_from_output(&data) {
                    let current = self.clipboard.clipboard_service.get_session_cli_type(session_id);
                    if current == codirigent_core::CliType::GenericShell {
                        self.clipboard.clipboard_service
                            .set_session_cli_type(session_id, cli_type);
                        info!(?session_id, ?cli_type, "Detected CLI type from output");
                    }
                }

                // Feed output to detector for status detection
                self.with_detector(|detector| {
                    detector.process_output(session_id, &data);

                    // Parse OSC 133 shell state markers for reliable idle detection
                    let shell_events = codirigent_session::extract_osc133_events(&data);
                    for event in shell_events {
                        detector.set_shell_state(session_id, event);
                    }
                });

                // Check for OSC 7 (working directory change) sequences
                if let Some(new_cwd) = codirigent_session::extract_osc7_path(&data) {
                    let changed = self.with_session_manager(|manager| {
                        manager.update_working_directory(session_id, new_cwd)
                    });
                    if changed {
                        // Force immediate git refresh (updates the manager's copy)
                        let git_info = self.with_session_manager(|manager| {
                            manager.refresh_git_status(session_id)
                        });

                        // Update terminal header (UI-only state, not part of Session)
                        if let Some((_, header)) = self
                            .terminal_headers
                            .iter_mut()
                            .find(|(sid, _)| *sid == session_id)
                        {
                            if let Some(ref info) = git_info {
                                header.git_branch = Some(info.branch.clone());
                                header.git_dirty_count = Some(info.dirty_count);
                            } else {
                                header.git_branch = None;
                                header.git_dirty_count = None;
                            }
                        }

                        // Sync workspace cache so file tree sees the new CWD
                        let manager_sessions = self.with_session_manager(|manager| {
                            manager.list_sessions()
                        });
                        self.workspace.sync_sessions_from_manager(&manager_sessions);

                        // Update file tree panel if this is the focused session
                        if self.workspace.focused_session_id() == Some(session_id) {
                            if let Some(session) = self.workspace.session(session_id) {
                                self.set_project_root(session.working_directory.clone());
                            }
                        }
                    }
                }
            }

            // Update session status from detector
            let (mut status, idle_time) = self.with_detector(|detector| {
                (
                    detector.get_status(session_id),
                    detector.get_idle_time(session_id),
                )
            });

            // Overlay CLI-specific session status (throttled to ~1/second)
            if check_jsonl {
                if let Some(session) = self.workspace.session(session_id) {
                    let working_dir = session.working_directory.clone();
                    let cli_type = self.clipboard.clipboard_service.get_session_cli_type(session_id);

                    // Get child PID for PID-based JSONL matching
                    let child_pid = self.with_session_manager(|manager| {
                        manager.get_child_pid(session_id)
                    });

                    let cli_status = match cli_type {
                        codirigent_core::CliType::ClaudeCode => {
                            self.cli_readers.claude.as_mut().and_then(|r| {
                                r.get_status(&working_dir, child_pid).to_session_status()
                            })
                        }
                        codirigent_core::CliType::CodexCli => {
                            self.cli_readers.codex.as_mut().and_then(|r| {
                                r.get_status(&working_dir, child_pid).to_session_status()
                            })
                        }
                        codirigent_core::CliType::GeminiCli => {
                            self.cli_readers.gemini.as_mut().and_then(|r| {
                                r.get_status(&working_dir, child_pid).to_session_status()
                            })
                        }
                        codirigent_core::CliType::GenericShell => {
                            // Use process tree to detect CLI type before reading JSONL.
                            // Prevents stale JSONL from previous sessions causing false promotion.
                            if let Some(pid) = child_pid {
                                let detected = self.cli_readers.detector.detect_cli_type(pid);
                                if detected != codirigent_core::CliType::GenericShell {
                                    self.clipboard.clipboard_service
                                        .set_session_cli_type(session_id, detected);
                                    match detected {
                                        codirigent_core::CliType::ClaudeCode => {
                                            self.cli_readers.claude.as_mut().and_then(|r| {
                                                r.get_status(&working_dir, child_pid)
                                                    .to_session_status()
                                            })
                                        }
                                        codirigent_core::CliType::CodexCli => {
                                            self.cli_readers.codex.as_mut().and_then(|r| {
                                                r.get_status(&working_dir, child_pid)
                                                    .to_session_status()
                                            })
                                        }
                                        codirigent_core::CliType::GeminiCli => {
                                            self.cli_readers.gemini.as_mut().and_then(|r| {
                                                r.get_status(&working_dir, child_pid)
                                                    .to_session_status()
                                            })
                                        }
                                        _ => None,
                                    }
                                } else {
                                    debug!(?session_id, pid, "Process tree detection returned GenericShell — JSONL reader skipped");
                                    None
                                }
                            } else {
                                None
                            }
                        }
                    };

                    debug!(?session_id, ?cli_status, "JSONL reader result");

                    if let Some((new_status, tool_name)) = cli_status.clone() {
                        // Cache the JSONL result so it persists between checks
                        self.cli_readers.cached_status
                            .insert(session_id, (new_status, tool_name.clone()));

                        if new_status == SessionStatus::NeedsAttention {
                            // Only fire event + notification on transition, not every poll
                            if session.status != SessionStatus::NeedsAttention {
                                self.event_bus.publish(CodirigentEvent::AttentionRequired {
                                    session_id,
                                    detail: tool_name.clone(),
                                });
                            }
                        }
                        status = Some(new_status);
                    } else {
                        // JSONL returned Unknown/None — clear cache so detector takes over
                        self.cli_readers.cached_status.remove(&session_id);
                    }
                }
            } else if let Some((cached_status, ref _tool_name)) =
                self.cli_readers.cached_status.get(&session_id).cloned()
            {
                // Non-JSONL cycle: reuse cached JSONL status instead of detector
                status = Some(cached_status);
            }

            if let Some(status) = status {
                if self.polling.idle_poll_count % 120 == 0 {
                    info!(?session_id, ?status, ?idle_time, "Session status poll");
                }
                let old_status = self.workspace.session(session_id).map(|s| s.status);
                let mut just_started_compaction = false;
                if self.workspace.update_session_status(session_id, status) {
                    any_dirty = true;
                    // Sync task board with the canonical (JSONL-corrected) status
                    if let Some(old) = old_status {
                        // Check if task transitioned to Review
                        let task_transitioned_to_review = if let Ok(mut task_mgr) =
                            self.task_manager.lock()
                        {
                            let tid = task_mgr.on_session_status_changed(session_id, old, status);
                            if let Some(ref task_id) = tid {
                                task_mgr
                                    .get_task(task_id)
                                    .map_or(false, |t| t.status == TaskStatus::Review)
                            } else {
                                false
                            }
                        } else {
                            false
                        };

                        // When task auto-transitions to Review:
                        // 1. Clear current_task so auto-assign can work later
                        // 2. Send /clear to reset context for next task
                        if task_transitioned_to_review {
                            // Clear current_task
                            if let Ok(mgr) = self.session_manager.lock() {
                                mgr.with_session_state_mut(session_id, |state| {
                                    state.session.current_task = None;
                                });
                            }
                            if let Some(session) = self.workspace.session_mut(session_id) {
                                session.current_task = None;
                            }
                            // Start context clear — reuse compaction infrastructure
                            let cli_type = self.clipboard.clipboard_service.get_session_cli_type(session_id);
                            let clear_cmd = clear_command(cli_type);
                            if let Ok(mut svc) = self.persistence.compaction.lock() {
                                if svc.begin_compaction(session_id) {
                                    if let Ok(mgr) = self.session_manager.lock() {
                                        let _ = mgr.send_input(session_id, clear_cmd.as_bytes());
                                    }
                                    self.polling.pending_enters
                                        .insert(session_id, (Instant::now(), false));
                                    self.cache.compaction_start_times
                                        .insert(session_id, Instant::now());
                                    just_started_compaction = true;
                                }
                            }
                        }
                    }
                }

                // NeedsAttention is NOT treated as idle — session is blocked
                // Skip if we just started compaction — wait for /clear to finish
                // Skip if a deferred Enter is pending — text hasn't been submitted yet
                if matches!(status, SessionStatus::Idle)
                    && !just_started_compaction
                    && !self.polling.pending_enters.contains_key(&session_id)
                {
                    let is_compacting = self
                        .persistence.compaction
                        .lock()
                        .map(|svc| svc.is_compacting(session_id))
                        .unwrap_or(false);

                    if is_compacting {
                        // Compaction just finished — session returned to Idle
                        if let Ok(mut svc) = self.persistence.compaction.lock() {
                            svc.end_compaction(session_id);
                        }
                        self.cache.compaction_start_times.remove(&session_id);
                        self.event_bus
                            .publish(CodirigentEvent::CompactionCompleted {
                                session_id,
                                success: true,
                            });
                        info!(?session_id, "Compaction completed successfully");
                        // Fall through to try_auto_assign
                    } else {
                        // Not compacting — check if we should compact before proceeding
                        let has_task = self
                            .workspace
                            .session(session_id)
                            .map_or(false, |s| s.current_task.is_some());
                        if has_task && self.try_compact(session_id) {
                            // Compaction started — skip auto-assign this cycle
                            continue;
                        }
                    }

                    self.try_auto_assign(session_id);
                }
            }
        }

        // Compaction timeout: end compaction for sessions that exceeded the limit
        let timeout_secs = self
            .persistence.compaction
            .lock()
            .map(|svc| svc.timeout_secs())
            .unwrap_or(120);
        let timed_out: Vec<SessionId> = self
            .cache.compaction_start_times
            .iter()
            .filter(|(_, start)| start.elapsed() > Duration::from_secs(timeout_secs))
            .map(|(id, _)| *id)
            .collect();
        for session_id in timed_out {
            if let Ok(mut svc) = self.persistence.compaction.lock() {
                svc.end_compaction(session_id);
            }
            self.cache.compaction_start_times.remove(&session_id);
            self.event_bus
                .publish(CodirigentEvent::CompactionCompleted {
                    session_id,
                    success: false,
                });
            warn!(?session_id, "Compaction timed out");
        }

        // Stale proposal cleanup: reject pending assignments whose target session
        // now has a current_task (became busy), and clear proposals older than 5 min.
        if let Ok(mut manager) = self.task_manager.lock() {
            let stale_task_ids: Vec<_> = manager
                .assignment()
                .pending_assignments()
                .iter()
                .filter(|p| {
                    self.workspace
                        .session(p.session_id)
                        .map_or(true, |s| s.current_task.is_some())
                })
                .map(|p| p.task_id.clone())
                .collect();
            for tid in stale_task_ids {
                manager.assignment_mut().reject_assignment(&tid);
            }
            manager.assignment_mut().clear_expired(300);
        }

        // Refresh git status every 3 seconds
        if self.polling.last_git_refresh.elapsed() >= Duration::from_secs(3) {
            self.polling.last_git_refresh = Instant::now();
            let session_ids: Vec<SessionId> =
                self.workspace.sessions().iter().map(|s| s.id).collect();

            // Collect git info from manager
            let git_infos = self.with_session_manager(|manager| {
                session_ids.iter().filter_map(|id| {
                    manager.refresh_git_status(*id).map(|info| (*id, info))
                }).collect::<Vec<_>>()
            });

            // Update terminal headers with git info
            for (id, git_info) in git_infos {
                if let Some((_, header)) = self.terminal_headers.iter_mut().find(|(sid, _)| *sid == id) {
                    header.git_branch = Some(git_info.branch.clone());
                    header.git_dirty_count = Some(git_info.dirty_count);
                }
            }
            // Bulk-sync git_info (and all other fields) from manager
            let manager_sessions = self.with_session_manager(|manager| {
                manager.list_sessions()
            });
            self.workspace.sync_sessions_from_manager(&manager_sessions);
            any_dirty = true;
        }

        // Clipboard preview: show for 4 seconds whenever clipboard content changes and has an image.
        // Uses platform clipboard sequence number (has_changed) to detect new content.
        if self.polling.idle_poll_count % 60 == 0 {
            let changed = self.clipboard.smart_clipboard.has_changed();
            if changed && self.clipboard.smart_clipboard.has_image() {
                // Clipboard changed and has an image — show preview
                if let Ok(content) = self.clipboard.smart_clipboard.read_content() {
                    if let codirigent_core::ClipboardContent::Image(ref image_data) = content {
                        let path = self
                            .clipboard.clipboard_service
                            .save_image(image_data)
                            .unwrap_or_default();
                        let file_size = image_data.bytes.len() as u64;
                        let preview = crate::clipboard_preview::ClipboardPreview::create_preview(
                            image_data, path, file_size,
                        );
                        self.clipboard.clipboard_preview.show(preview);
                        self.clipboard.clipboard_preview_shown_at = Some(std::time::Instant::now());
                        any_dirty = true;
                    }
                }
            }

            // Auto-dismiss after 4 seconds
            if self.clipboard.clipboard_preview.is_visible() {
                if let Some(shown_at) = self.clipboard.clipboard_preview_shown_at {
                    if shown_at.elapsed() > std::time::Duration::from_secs(4) {
                        self.clipboard.clipboard_preview.hide();
                        self.clipboard.clipboard_preview_shown_at = None;
                        any_dirty = true;
                    }
                }
            }
        }

        // Track output activity for adaptive polling
        self.polling.last_poll_had_output = any_dirty;

        if any_dirty {
            cx.notify();
        }
    }

    /// Try to compact a session before verification.
    /// Returns true if compaction was started, false if skipped.
    fn try_compact(&mut self, session_id: SessionId) -> bool {
        let context_usage = self
            .workspace
            .session(session_id)
            .and_then(|s| s.context_usage);

        let command = {
            let mut svc = match self.persistence.compaction.lock() {
                Ok(s) => s,
                Err(_) => return false,
            };
            if !svc.should_compact(session_id, context_usage) {
                return false;
            }
            if !svc.begin_compaction(session_id) {
                return false;
            }
            svc.compact_command()
        };

        // Send /compact via PTY stdin
        if let Ok(mgr) = self.session_manager.lock() {
            if let Err(e) = mgr.send_input(session_id, command.as_bytes()) {
                warn!(?session_id, error = %e, "Failed to send /compact command");
                if let Ok(mut svc) = self.persistence.compaction.lock() {
                    svc.end_compaction(session_id);
                }
                return false;
            }
        }

        self.cache.compaction_start_times
            .insert(session_id, Instant::now());

        let focus = self
            .persistence.compaction
            .lock()
            .ok()
            .and_then(|svc| svc.config().focus_instructions.clone());
        self.event_bus
            .publish(CodirigentEvent::CompactionStarted { session_id, focus });

        info!(?session_id, "Compaction started");
        true
    }

    /// Try to auto-assign a queued task to a session that just became idle.
    ///
    /// Checks whether auto-assign is enabled and a task is available, then
    /// confirms the assignment, updates the session's `current_task`, and
    /// sends the generated prompt to the session's PTY.
    fn try_auto_assign(&mut self, session_id: SessionId) {
        let session = match self.workspace.session(session_id) {
            Some(s) => s.clone(),
            None => return,
        };

        // Skip if session already has a task assigned
        if session.current_task.is_some() {
            return;
        }

        // Never auto-assign to bare shell sessions — CLI must be detected first
        let cli_type = self.clipboard.clipboard_service.get_session_cli_type(session_id);
        if cli_type == codirigent_core::CliType::GenericShell {
            return;
        }

        // Block auto-assign until the user has manually assigned at least once.
        // A freshly-started CLI may need auth, config, or other user input first.
        if !self.cache.manually_assigned_sessions.contains(&session_id) {
            return;
        }

        let action = {
            let mut manager = match self.task_manager.lock() {
                Ok(m) => m,
                Err(_) => return,
            };
            manager.on_session_idle(&session)
        };

        match action {
            Some(AssignmentAction::AssignNow {
                task_id,
                session_id: target_id,
                prompt,
            }) => {
                // AssignNow already has the prompt — directly assign via queue
                {
                    let mut manager = match self.task_manager.lock() {
                        Ok(m) => m,
                        Err(_) => return,
                    };
                    if let Err(e) = manager.queue_mut().assign_task(&task_id, target_id) {
                        warn!("Failed to assign task in queue: {}", e);
                        return;
                    }
                }

                // Update session's current_task in the session manager
                if let Ok(mgr) = self.session_manager.lock() {
                    mgr.with_session_state_mut(target_id, |state| {
                        state.session.current_task = Some(task_id.clone());
                    });
                }

                // Update workspace's cached copy
                if let Some(ws_session) = self.workspace.session_mut(target_id) {
                    ws_session.current_task = Some(task_id.clone());
                }

                // Send prompt to PTY (format based on CLI type)
                let cli_type = self.clipboard.clipboard_service.get_session_cli_type(target_id);
                let input = format_task_input(&prompt, cli_type);
                if let Ok(mgr) = self.session_manager.lock() {
                    if let Err(e) = mgr.send_input(target_id, input.as_bytes()) {
                        warn!("Failed to send task prompt to session {}: {}", target_id, e);
                    }
                }
                self.polling.pending_enters
                    .insert(target_id, (Instant::now(), false));

                info!(?task_id, ?target_id, "Auto-assigned task to session");
            }
            Some(AssignmentAction::AwaitConfirmation {
                task_id,
                session_id: target_id,
            }) => {
                // Pending assignment is stored in AssignmentManager.pending;
                // the UI will render the confirmation banner on next frame.
                info!(
                    ?task_id,
                    ?target_id,
                    "Task proposed — awaiting user confirmation"
                );
            }
            Some(AssignmentAction::NoTask) | None => {}
        }
    }
}
