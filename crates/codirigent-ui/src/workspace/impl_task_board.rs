//! Task board event handlers for WorkspaceView.
//!
//! This module contains all methods related to:
//! - Task board event handling (add, start, review, complete, delete, assign)
//! - Task assignment to sessions
//! - Assignment confirmation and rejection

use super::cli_helpers::format_task_input;
use super::gpui::WorkspaceView;
use codirigent_core::{Session, SessionManager, TaskId};
use codirigent_session::clipboard_service::ClipboardService;
use gpui::Context;
use std::time::Instant;
use tracing::{info, warn};

impl WorkspaceView {
    pub(super) fn handle_task_board_event(
        &mut self,
        event: crate::task_board::TaskBoardEvent,
        cx: &mut Context<Self>,
    ) {
        use crate::task_board::TaskAction;

        match event {
            crate::task_board::TaskBoardEvent::TabSelected(tab) => {
                info!(?tab, "Task board tab selected");
            }
            crate::task_board::TaskBoardEvent::AutoAssignModeChanged(mode) => {
                info!(?mode, "Auto-assign mode changed");
                let (auto, confirm) = mode.to_config();
                if let Ok(mut manager) = self.task_manager.lock() {
                    manager.assignment_mut().set_auto_assign(auto);
                    manager.assignment_mut().set_confirm_before_assign(confirm);
                }
            }
            crate::task_board::TaskBoardEvent::AddTaskClicked => {
                info!("Add task clicked");
                self.open_task_creation_modal();
            }
            crate::task_board::TaskBoardEvent::TaskSelected(id) => {
                info!(%id, "Task selected");
            }
            crate::task_board::TaskBoardEvent::TaskAction { task_id, action } => {
                info!(%task_id, ?action, "Task action triggered");

                let task_id = TaskId::from(task_id);

                // Handle Edit outside the lock — it only needs to open a modal
                if matches!(action, TaskAction::Edit) {
                    info!("Edit action triggered for task {}", task_id);
                    self.open_task_edit_modal(&task_id);
                    cx.notify();
                    return;
                }

                if let Ok(mut manager) = self.task_manager.lock() {
                    let result = match action {
                        TaskAction::Start => {
                            info!("Starting task {}", task_id);
                            manager.start_task(&task_id)
                        }
                        TaskAction::Review => {
                            // Move to Review status and release from session
                            info!("Moving task {} to review", task_id);
                            let r = manager.move_to_review(&task_id);
                            if r.is_ok() {
                                let assigned_session_id = self
                                    .workspace
                                    .sessions()
                                    .iter()
                                    .find(|s| s.current_task.as_ref() == Some(&task_id))
                                    .map(|s| s.id);
                                if let Some(sid) = assigned_session_id {
                                    drop(manager);
                                    if let Ok(mgr) = self.session_manager.lock() {
                                        mgr.with_session_state_mut(sid, |state| {
                                            state.session.current_task = None;
                                        });
                                    }
                                    if let Some(session) = self.workspace.session_mut(sid) {
                                        session.current_task = None;
                                    }
                                    cx.notify();
                                    return;
                                }
                            }
                            r
                        }
                        TaskAction::Complete => {
                            // Actually approve and complete the task
                            info!("Approving task {}", task_id);
                            let r = manager.approve_task(&task_id);
                            if r.is_ok() {
                                // Find session that had this task and clear current_task
                                let assigned_session_id = self
                                    .workspace
                                    .sessions()
                                    .iter()
                                    .find(|s| s.current_task.as_ref() == Some(&task_id))
                                    .map(|s| s.id);

                                if let Some(sid) = assigned_session_id {
                                    // Release task_manager before session_manager
                                    drop(manager);
                                    if let Ok(mgr) = self.session_manager.lock() {
                                        mgr.with_session_state_mut(sid, |state| {
                                            state.session.current_task = None;
                                        });
                                    }
                                    if let Some(session) = self.workspace.session_mut(sid) {
                                        session.current_task = None;
                                    }
                                    cx.notify();
                                    return;
                                }
                            }
                            r
                        }
                        TaskAction::Delete => {
                            info!("Deleting task {}", task_id);
                            let r = manager.delete_task(&task_id);
                            if r.is_ok() {
                                // Clear current_task from any session that had this task
                                let assigned_session_id = self
                                    .workspace
                                    .sessions()
                                    .iter()
                                    .find(|s| s.current_task.as_ref() == Some(&task_id))
                                    .map(|s| s.id);
                                if let Some(sid) = assigned_session_id {
                                    drop(manager);
                                    if let Ok(mgr) = self.session_manager.lock() {
                                        mgr.with_session_state_mut(sid, |state| {
                                            state.session.current_task = None;
                                        });
                                    }
                                    if let Some(session) = self.workspace.session_mut(sid) {
                                        session.current_task = None;
                                    }
                                    cx.notify();
                                    return;
                                }
                            }
                            r
                        }
                        TaskAction::Assign => {
                            info!("Assign action triggered for task {}", task_id);
                            // Get task for directory matching
                            let task = manager.get_task(&task_id).cloned();
                            let target = task
                                .as_ref()
                                .and_then(|t| self.find_assignable_session_for_task(t));

                            if let Some(session) = target {
                                match manager.direct_assign(&task_id, session.id) {
                                    Ok(prompt) => {
                                        // Check CLI type to decide how to send the prompt
                                        let cli_type =
                                            self.clipboard.clipboard_service.get_session_cli_type(session.id);

                                        // Release task_manager before session_manager
                                        drop(manager);

                                        // Build the command to send to the PTY
                                        let input = format_task_input(&prompt, cli_type);

                                        if let Ok(mgr) = self.session_manager.lock() {
                                            mgr.with_session_state_mut(session.id, |state| {
                                                state.session.current_task = Some(task_id.clone());
                                            });
                                            if let Err(e) =
                                                mgr.send_input(session.id, input.as_bytes())
                                            {
                                                warn!("Failed to send task prompt: {}", e);
                                            }
                                        }
                                        self.polling.pending_enters
                                            .insert(session.id, (Instant::now(), false));
                                        if let Some(ws_session) =
                                            self.workspace.session_mut(session.id)
                                        {
                                            ws_session.current_task = Some(task_id.clone());
                                        }
                                        // Mark session as having received a manual assignment,
                                        // which unlocks auto-assign for future tasks.
                                        self.cache.manually_assigned_sessions.insert(session.id);
                                        info!(
                                            "Manually assigned task {} to session {}",
                                            task_id, session.id
                                        );
                                        cx.notify();
                                        return;
                                    }
                                    Err(e) => {
                                        warn!("Failed to assign task: {}", e);
                                        Err(e)
                                    }
                                }
                            } else {
                                warn!("No matching session available for assignment (check directory matching)");
                                Ok(())
                            }
                        }
                        TaskAction::Edit => {
                            // Handled above, before the lock
                            unreachable!()
                        }
                    };

                    if let Err(e) = result {
                        warn!("Task action failed: {}", e);
                    }
                }
            }
            crate::task_board::TaskBoardEvent::ConfirmAssignment { task_id } => {
                info!(%task_id, "Confirming pending assignment");
                let task_id = TaskId::from(task_id);

                // Confirm the pending assignment and get the prompt
                let (prompt, session_id) = {
                    let mut manager = match self.task_manager.lock() {
                        Ok(m) => m,
                        Err(_) => {
                            cx.notify();
                            return;
                        }
                    };
                    match manager.assignment_mut().confirm_assignment(&task_id) {
                        Ok(assignment) => {
                            // Also assign the task in the queue
                            if let Err(e) = manager
                                .queue_mut()
                                .assign_task(&task_id, assignment.session_id)
                            {
                                warn!("Failed to assign task in queue: {}", e);
                                cx.notify();
                                return;
                            }
                            (assignment.prompt, assignment.session_id)
                        }
                        Err(e) => {
                            warn!("Failed to confirm assignment: {}", e);
                            cx.notify();
                            return;
                        }
                    }
                };

                // Update session's current_task
                if let Ok(mgr) = self.session_manager.lock() {
                    mgr.with_session_state_mut(session_id, |state| {
                        state.session.current_task = Some(task_id.clone());
                    });
                }
                if let Some(ws_session) = self.workspace.session_mut(session_id) {
                    ws_session.current_task = Some(task_id.clone());
                }

                // Send prompt to PTY
                let cli_type = self.clipboard.clipboard_service.get_session_cli_type(session_id);
                let input = format_task_input(&prompt, cli_type);
                if let Ok(mgr) = self.session_manager.lock() {
                    if let Err(e) = mgr.send_input(session_id, input.as_bytes()) {
                        warn!(
                            "Failed to send confirmed task prompt to session {}: {}",
                            session_id, e
                        );
                    }
                }
                self.polling.pending_enters
                    .insert(session_id, (Instant::now(), false));

                info!(?task_id, ?session_id, "Confirmed and sent task to session");
            }
            crate::task_board::TaskBoardEvent::RejectAssignment { task_id } => {
                info!(%task_id, "Rejecting pending assignment");
                let task_id = TaskId::from(task_id);
                if let Ok(mut manager) = self.task_manager.lock() {
                    manager.assignment_mut().reject_assignment(&task_id);
                }
                info!(?task_id, "Rejected assignment — task remains queued");
            }
        }
        cx.notify();
    }

    fn find_assignable_session_for_task(&self, task: &codirigent_core::Task) -> Option<Session> {
        let candidates: Vec<_> = self
            .workspace
            .sessions()
            .iter()
            .filter(|s| {
                s.status == codirigent_core::SessionStatus::Idle
                    && s.current_task.is_none()
                    && self.clipboard.clipboard_service.get_session_cli_type(s.id)
                        != codirigent_core::CliType::GenericShell
                    && task.project_dir.as_ref().map_or(true, |pd| {
                        codirigent_core::session_matches_project(&s.working_directory, pd)
                    })
            })
            .cloned()
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // Prefer the focused session if it's among candidates
        if let Some(focused_id) = self.workspace.focused_session_id() {
            if let Some(session) = candidates.iter().find(|s| s.id == focused_id) {
                return Some(session.clone());
            }
        }

        // Among remaining, pick the session with lowest context_usage (freshest context window)
        candidates.into_iter().min_by(|a, b| {
            let usage_a = a.context_usage.unwrap_or(0.0);
            let usage_b = b.context_usage.unwrap_or(0.0);
            usage_a
                .partial_cmp(&usage_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}
