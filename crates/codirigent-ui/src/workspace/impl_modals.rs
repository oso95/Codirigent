//! Modal dialog handlers for WorkspaceView.
//!
//! This module contains all methods related to:
//! - Task creation modal (open, close, apply, edit)
//! - Session action modal (rename, assign group)
//! - Modal keyboard input handling

use super::gpui::WorkspaceView;
use super::types::{SessionActionKind, SessionActionModal, TaskCreationModal, GROUP_COLOR_PALETTE};
use codirigent_core::{SessionId, SessionManager, Task, TaskId};
use gpui::{Context, KeyDownEvent};
use std::path::Path;
use tracing::{info, warn};

impl WorkspaceView {
    pub(super) fn open_session_action_modal(&mut self, session_id: SessionId, kind: SessionActionKind) {
        let input = match kind {
            SessionActionKind::Rename => self
                .workspace
                .session(session_id)
                .map(|session| session.name.clone())
                .unwrap_or_default(),
            SessionActionKind::AssignGroup => self
                .workspace
                .session(session_id)
                .and_then(|session| session.group.clone())
                .unwrap_or_default(),
        };

        self.modals.session_action = Some(SessionActionModal {
            session_id,
            kind,
            input,
            error: None,
        });
    }

    pub(super) fn close_session_action_modal(&mut self) {
        self.modals.session_action = None;
    }

    /// Pick the next unused group color from the palette.
    pub(super) fn next_group_color(&self) -> String {
        let used_colors: std::collections::HashSet<&str> = self
            .workspace
            .sessions()
            .iter()
            .filter_map(|s| s.color.as_deref())
            .collect();
        GROUP_COLOR_PALETTE
            .iter()
            .find(|c| !used_colors.contains(**c))
            .unwrap_or(&GROUP_COLOR_PALETTE[0])
            .to_string()
    }

    pub(super) fn open_task_creation_modal(&mut self) {
        let project_dir = self.workspace.focused_session().and_then(|s| {
            s.git_info
                .as_ref()
                .map(|g| g.repo_root.clone())
                .or_else(|| Some(s.working_directory.clone()))
        });

        self.modals.task_creation = Some(TaskCreationModal {
            title: String::new(),
            description: String::new(),
            priority: codirigent_core::TaskPriority::Medium,
            focused_field: 0,
            error: None,
            project_dir,
            plan_file: String::new(),
            editing_task_id: None,
        });
    }

    /// Open the task creation modal pre-filled with a file's name and path.
    pub(super) fn open_task_creation_modal_for_file(&mut self, path: &Path) {
        let project_dir = self
            .file_tree_model
            .as_ref()
            .map(|t| t.root().to_path_buf());

        let relative_path = project_dir
            .as_ref()
            .and_then(|root| path.strip_prefix(root).ok())
            .unwrap_or(path);

        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        self.modals.task_creation = Some(TaskCreationModal {
            title: filename,
            description: String::new(),
            priority: codirigent_core::TaskPriority::Medium,
            focused_field: 0,
            error: None,
            project_dir,
            plan_file: relative_path.to_string_lossy().to_string(),
            editing_task_id: None,
        });
    }

    /// Open the task modal pre-filled with an existing task's data for editing.
    pub(super) fn open_task_edit_modal(&mut self, task_id: &TaskId) {
        let task = match self.task_manager.lock() {
            Ok(mgr) => mgr.get_task(task_id).cloned(),
            Err(_) => None,
        };

        let Some(task) = task else {
            warn!("Cannot edit task {}: not found", task_id);
            return;
        };

        self.modals.task_creation = Some(TaskCreationModal {
            title: task.title.clone(),
            description: task.description.clone(),
            priority: task.priority,
            focused_field: 0,
            error: None,
            project_dir: task.project_dir.clone(),
            plan_file: task.plan_file.clone().unwrap_or_default(),
            editing_task_id: Some(task_id.clone()),
        });
    }

    pub(super) fn close_task_creation_modal(&mut self) {
        self.modals.task_creation = None;
    }

    pub(super) fn apply_task_creation_modal(&mut self, cx: &mut Context<Self>) {
        let Some(modal) = self.modals.task_creation.clone() else {
            return;
        };

        let title = modal.title.trim().to_string();
        let description = modal.description.trim().to_string();

        // Validate title is not empty
        if title.is_empty() {
            if let Some(ref mut active) = self.modals.task_creation {
                active.error = Some("Title is required".to_string());
            }
            cx.notify();
            return;
        }

        let plan_file = if modal.plan_file.trim().is_empty() {
            None
        } else {
            Some(modal.plan_file.trim().to_string())
        };

        if let Some(existing_id) = &modal.editing_task_id {
            // Update existing task
            if let Ok(mut manager) = self.task_manager.lock() {
                if let Err(e) = manager.update_task(
                    existing_id,
                    title,
                    description,
                    modal.priority,
                    plan_file,
                    modal.project_dir.clone(),
                ) {
                    if let Some(ref mut active) = self.modals.task_creation {
                        active.error = Some(format!("Failed to update task: {}", e));
                    }
                    cx.notify();
                    return;
                }
                info!(%existing_id, "Task updated successfully from modal");
            } else {
                if let Some(ref mut active) = self.modals.task_creation {
                    active.error = Some("Failed to access task manager".to_string());
                }
                cx.notify();
                return;
            }
        } else {
            // Create new task
            let task_id = TaskId(format!("task-{}", self.next_session_id));
            self.next_session_id += 1;

            let mut task = Task::new(task_id.clone(), title, description);
            task.priority = modal.priority;
            task.project_dir = modal.project_dir.clone();
            task.plan_file = plan_file;

            if let Ok(mut manager) = self.task_manager.lock() {
                if let Err(e) = manager.create_task(task) {
                    if let Some(ref mut active) = self.modals.task_creation {
                        active.error = Some(format!("Failed to create task: {}", e));
                    }
                    cx.notify();
                    return;
                }
                info!(%task_id, "Task created successfully from modal");
            } else {
                if let Some(ref mut active) = self.modals.task_creation {
                    active.error = Some("Failed to access task manager".to_string());
                }
                cx.notify();
                return;
            }
        }

        self.close_task_creation_modal();
        cx.notify();
    }

    pub(super) fn apply_session_action_modal(&mut self, cx: &mut Context<Self>) {
        let Some(modal) = self.modals.session_action.clone() else {
            return;
        };

        let value = modal.input.trim().to_string();
        if value.is_empty() {
            if let Some(ref mut active) = self.modals.session_action {
                active.error = Some("Value is required".to_string());
            }
            cx.notify();
            return;
        }

        match modal.kind {
            SessionActionKind::Rename => {
                if let Ok(manager) = self.session_manager.lock() {
                    if let Err(e) = manager.rename_session(modal.session_id, value) {
                        warn!("Failed to rename session: {}", e);
                    }
                }
            }
            SessionActionKind::AssignGroup => {
                let color = self.next_group_color();
                if let Ok(manager) = self.session_manager.lock() {
                    if let Err(e) =
                        manager.set_session_group(modal.session_id, Some(value), Some(color))
                    {
                        warn!("Failed to set session group: {}", e);
                    }
                }
            }
        }

        // Sync workspace cache immediately so the UI reflects the change
        if let Ok(manager) = self.session_manager.lock() {
            self.workspace
                .sync_sessions_from_manager(&manager.list_sessions());
        }
        self.save_state_to_disk();
        self.close_session_action_modal();
        cx.notify();
    }

    pub(super) fn handle_session_action_key_down(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(modal) = self.modals.session_action.as_mut() else {
            return false;
        };

        let key = event.keystroke.key.to_lowercase();
        match key.as_str() {
            "escape" => {
                self.close_session_action_modal();
                cx.notify();
                return true;
            }
            "enter" => {
                self.apply_session_action_modal(cx);
                return true;
            }
            "backspace" => {
                modal.input.pop();
                cx.notify();
                return true;
            }
            "space" => {
                // GPUI on Windows reports space as key="space" with key_char=None
                modal.input.push(' ');
                cx.notify();
                return true;
            }
            _ => {}
        }

        // Ctrl+A selects all (clears input for easy replacement)
        if (event.keystroke.modifiers.control || event.keystroke.modifiers.platform) && key == "a" {
            modal.input.clear();
            cx.notify();
            return true;
        }

        // Ignore other modifier-based shortcuts inside the modal.
        if event.keystroke.modifiers.control
            || event.keystroke.modifiers.alt
            || event.keystroke.modifiers.platform
        {
            return true;
        }

        if let Some(ref key_char) = event.keystroke.key_char {
            if let Some(ch) = key_char.chars().next() {
                if ch.is_ascii_graphic() || ch == ' ' {
                    modal.input.push(ch);
                    cx.notify();
                }
            }
        }

        true
    }

    pub(super) fn handle_task_creation_key_down(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(modal) = self.modals.task_creation.as_mut() else {
            return false;
        };

        let key = event.keystroke.key.to_lowercase();
        match key.as_str() {
            "escape" => {
                self.close_task_creation_modal();
                cx.notify();
                return true;
            }
            "enter" => {
                // Submit from title or plan_file field, newline in description
                if modal.focused_field == 0 || modal.focused_field == 2 {
                    self.apply_task_creation_modal(cx);
                } else {
                    modal.description.push('\n');
                    cx.notify();
                }
                return true;
            }
            "tab" => {
                // Cycle: title(0) -> description(1) -> plan_file(2) -> title(0)
                modal.focused_field = (modal.focused_field + 1) % 3;
                cx.notify();
                return true;
            }
            "backspace" => {
                match modal.focused_field {
                    0 => {
                        modal.title.pop();
                    }
                    1 => {
                        modal.description.pop();
                    }
                    2 => {
                        modal.plan_file.pop();
                    }
                    _ => {}
                }
                modal.error = None;
                cx.notify();
                return true;
            }
            "space" => {
                match modal.focused_field {
                    0 => modal.title.push(' '),
                    1 => modal.description.push(' '),
                    2 => modal.plan_file.push(' '),
                    _ => {}
                }
                modal.error = None;
                cx.notify();
                return true;
            }
            _ => {}
        }

        // Ctrl+A selects all (clears focused field for easy replacement)
        if (event.keystroke.modifiers.control || event.keystroke.modifiers.platform) && key == "a" {
            match modal.focused_field {
                0 => modal.title.clear(),
                1 => modal.description.clear(),
                2 => modal.plan_file.clear(),
                _ => {}
            }
            modal.error = None;
            cx.notify();
            return true;
        }

        // Ctrl+V / Cmd+V — paste from system clipboard
        if (event.keystroke.modifiers.control || event.keystroke.modifiers.platform) && key == "v" {
            if let Ok(content) = self.smart_clipboard.read_content() {
                if let codirigent_core::ClipboardContent::Text(text) = content {
                    match modal.focused_field {
                        0 => modal.title.push_str(&text),
                        1 => modal.description.push_str(&text),
                        2 => modal.plan_file.push_str(&text),
                        _ => {}
                    }
                    modal.error = None;
                    cx.notify();
                }
            }
            return true;
        }

        // Ignore other modifier-based shortcuts inside the modal.
        if event.keystroke.modifiers.control
            || event.keystroke.modifiers.alt
            || event.keystroke.modifiers.platform
        {
            return true;
        }

        if let Some(ref key_char) = event.keystroke.key_char {
            if let Some(ch) = key_char.chars().next() {
                if ch.is_ascii_graphic() || ch == ' ' || ch == '\n' {
                    match modal.focused_field {
                        0 => modal.title.push(ch),
                        1 => modal.description.push(ch),
                        2 => modal.plan_file.push(ch),
                        _ => {}
                    }
                    modal.error = None;
                    cx.notify();
                }
            }
        }

        true
    }
}
