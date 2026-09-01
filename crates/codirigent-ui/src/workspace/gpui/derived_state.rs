//! Derived UI state reducers and refresh helpers.

use super::session_metadata::{pending_git_file_counts, session_project_name};
use super::WorkspaceView;
use codirigent_core::SessionId;
use std::collections::HashMap;

impl WorkspaceView {
    fn task_title_for_session(
        &self,
        session: &codirigent_core::Session,
        task_titles: Option<&HashMap<codirigent_core::TaskId, String>>,
    ) -> Option<String> {
        let task_id = session.current_task.as_ref()?;
        if let Some(task_titles) = task_titles {
            return Some(super::session_metadata::resolved_task_title(
                task_id,
                Some(task_titles),
            ));
        }

        if let Ok(manager) = self.task_manager.lock() {
            return Some(
                manager
                    .get_task(task_id)
                    .map(|task| task.title.clone())
                    .unwrap_or_else(|| task_id.0.to_string()),
            );
        }

        Some(task_id.0.to_string())
    }

    fn sync_task_board_state(&mut self) -> HashMap<codirigent_core::TaskId, String> {
        let Ok(manager) = self.task_manager.lock() else {
            return HashMap::new();
        };

        let mut titles = HashMap::new();
        let all_tasks = manager.list_tasks();
        let counts =
            all_tasks
                .iter()
                .fold((0usize, 0usize, 0usize, 0usize), |(q, ip, r, d), task| {
                    titles.insert(task.id.clone(), task.title.clone());
                    match task.status {
                        codirigent_core::TaskStatus::Queued
                        | codirigent_core::TaskStatus::Blocked => (q + 1, ip, r, d),
                        codirigent_core::TaskStatus::Assigned
                        | codirigent_core::TaskStatus::Working => (q, ip + 1, r, d),
                        codirigent_core::TaskStatus::Verifying
                        | codirigent_core::TaskStatus::Review => (q, ip, r + 1, d),
                        codirigent_core::TaskStatus::Done => (q, ip, r, d + 1),
                    }
                });
        let running_items = all_tasks
            .iter()
            .filter(|t| {
                matches!(
                    t.status,
                    codirigent_core::TaskStatus::Assigned | codirigent_core::TaskStatus::Working
                )
            })
            .map(|t| self.core_task_to_ui_item(t))
            .collect();
        let queued_items = all_tasks
            .iter()
            .filter(|t| {
                matches!(
                    t.status,
                    codirigent_core::TaskStatus::Queued | codirigent_core::TaskStatus::Blocked
                )
            })
            .map(|t| self.core_task_to_ui_item(t))
            .collect();
        let review_items = all_tasks
            .iter()
            .filter(|t| {
                matches!(
                    t.status,
                    codirigent_core::TaskStatus::Verifying | codirigent_core::TaskStatus::Review
                )
            })
            .map(|t| self.core_task_to_ui_item(t))
            .collect();
        let done_items = all_tasks
            .iter()
            .filter(|t| t.status == codirigent_core::TaskStatus::Done)
            .map(|t| self.core_task_to_ui_item(t))
            .collect();
        let config = manager.assignment().config();
        let auto_assign_mode = crate::task_board::AutoAssignMode::from_config(
            config.auto_assign,
            config.confirm_before_assign,
        );
        let pending_assignments = manager
            .assignment()
            .pending_assignments()
            .iter()
            .map(|p| crate::task_board::PendingAssignmentSummary {
                task_id: p.task_id.to_string(),
                session_number: p.session_id.0,
                task_title: all_tasks
                    .iter()
                    .find(|t| t.id == p.task_id)
                    .map(|t| t.title.clone())
                    .unwrap_or_else(|| p.task_id.to_string()),
            })
            .collect();

        self.task_board
            .set_task_counts(counts.0, counts.1, counts.2, counts.3);
        self.task_board
            .set_snapshot(crate::task_board::TaskBoardSnapshot {
                running_items,
                queued_items,
                review_items,
                done_items,
                auto_assign_mode,
                pending_assignments,
            });

        titles
    }

    fn sync_all_session_headers(
        &mut self,
        task_titles: Option<&HashMap<codirigent_core::TaskId, String>>,
    ) {
        let sessions = self.workspace.sessions();
        let focused_id = self.workspace.focused_session_id();
        for session in sessions {
            let project_name = session_project_name(session);
            let cli_name = self.session_cli_display_name(session.id);
            let git_branch = session.git_info.as_ref().map(|gi| gi.branch.clone());
            let (git_pending_additions, git_pending_deletions) = session
                .git_info
                .as_ref()
                .map(pending_git_file_counts)
                .map(|(added, deleted)| (Some(added), Some(deleted)))
                .unwrap_or((None, None));
            let session_color = session
                .color
                .as_deref()
                .map(crate::sidebar::Color::from_hex)
                .unwrap_or_else(|| crate::sidebar::Color::from_hex("#6366f1"));
            let task = self.task_title_for_session(session, task_titles);
            let (shell_label, shell_warning) =
                self.session_shell_display(session.id, session.shell.as_deref());
            if let Some(header) = self.terminal_headers.get_mut(&session.id) {
                if header.session_name != session.name {
                    header.session_name = session.name.clone();
                }
                if header.group_name != session.group {
                    header.group_name = session.group.clone();
                }
                header.status = session.status;
                header.context_usage = session.context_usage;
                header.is_focused = focused_id == Some(session.id);
                if header.project_name != project_name {
                    header.project_name = project_name;
                }
                if header.cli_name != cli_name {
                    header.cli_name = cli_name.clone();
                }
                if header.git_branch != git_branch {
                    header.git_branch = git_branch;
                }
                if header.git_pending_additions != git_pending_additions {
                    header.git_pending_additions = git_pending_additions;
                }
                if header.git_pending_deletions != git_pending_deletions {
                    header.git_pending_deletions = git_pending_deletions;
                }
                if header.session_color != session_color {
                    header.session_color = session_color;
                }
                if header.task != task {
                    header.task = task;
                }
                if header.shell_label.as_deref() != Some(shell_label.as_str()) {
                    header.shell_label = Some(shell_label.clone());
                }
                if header.shell_warning != shell_warning {
                    header.shell_warning = shell_warning.clone();
                }
            }
        }
    }

    fn sync_empty_cells_state(&mut self) {
        let (rows, cols) = self.workspace.layout_profile().dimensions();
        let occupied: Vec<codirigent_core::GridPosition> = self
            .workspace
            .layout_state()
            .as_grid()
            .map(|state| {
                state
                    .assignments()
                    .iter()
                    .enumerate()
                    .filter_map(|(index, session_id)| {
                        session_id.map(|_| codirigent_core::GridPosition {
                            row: index as u32 / cols,
                            col: index as u32 % cols,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.empty_cells.setup_for_grid(rows, cols, &occupied);
    }

    pub(in crate::workspace) fn sync_layout_derived_state(&mut self) {
        self.sync_all_session_headers(None);
        self.sync_empty_cells_state();
    }

    pub(in crate::workspace) fn sync_task_derived_state(&mut self) {
        let task_titles = self.sync_task_board_state();
        self.sync_all_session_headers(Some(&task_titles));
    }

    /// Synchronize all derived UI state from canonical workspace/task state.
    ///
    /// This must only run from explicit mutation paths, never as a render fallback.
    pub(in crate::workspace) fn refresh_derived_ui_state(&mut self) {
        let task_titles = self.sync_task_board_state();
        self.sync_all_session_headers(Some(&task_titles));
        self.sync_empty_cells_state();

        // Update shutdown guard: block system shutdown/logout while sessions exist.
        crate::platform::shutdown_guard::set_shutdown_blocked(
            !self.workspace.sessions().is_empty(),
        );
    }

    /// Sync a single session's terminal header from workspace state.
    ///
    /// This is a targeted delta update for the common case where only one
    /// session's status changed. Avoids the O(all sessions) cost of
    /// `refresh_derived_ui_state()` for each output poll.
    pub(in crate::workspace) fn sync_session_header(&mut self, session_id: SessionId) {
        let Some(session) = self.workspace.session(session_id) else {
            return;
        };
        let focused_id = self.workspace.focused_session_id();
        let project_name = session_project_name(session);
        let cli_name = self.session_cli_display_name(session.id);
        let git_branch = session.git_info.as_ref().map(|gi| gi.branch.clone());
        let (git_pending_additions, git_pending_deletions) = session
            .git_info
            .as_ref()
            .map(pending_git_file_counts)
            .map(|(added, deleted)| (Some(added), Some(deleted)))
            .unwrap_or((None, None));
        let session_color = session
            .color
            .as_deref()
            .map(crate::sidebar::Color::from_hex)
            .unwrap_or_else(|| crate::sidebar::Color::from_hex("#6366f1"));
        let task = self.task_title_for_session(session, None);
        let (shell_label, shell_warning) =
            self.session_shell_display(session.id, session.shell.as_deref());

        if let Some(header) = self.terminal_headers.get_mut(&session_id) {
            header.status = session.status;
            header.context_usage = session.context_usage;
            header.is_focused = focused_id == Some(session_id);

            if header.session_name != session.name {
                header.session_name = session.name.clone();
            }
            if header.group_name != session.group {
                header.group_name = session.group.clone();
            }

            if header.project_name != project_name {
                header.project_name = project_name;
            }
            if header.cli_name != cli_name {
                header.cli_name = cli_name;
            }

            if header.git_branch != git_branch {
                header.git_branch = git_branch;
            }
            if header.git_pending_additions != git_pending_additions {
                header.git_pending_additions = git_pending_additions;
            }
            if header.git_pending_deletions != git_pending_deletions {
                header.git_pending_deletions = git_pending_deletions;
            }
            if header.session_color != session_color {
                header.session_color = session_color;
            }
            if header.task != task {
                header.task = task;
            }
            if header.shell_label.as_deref() != Some(shell_label.as_str()) {
                header.shell_label = Some(shell_label);
            }
            if header.shell_warning != shell_warning {
                header.shell_warning = shell_warning;
            }
        }
    }
}
