//! Modal dialog handlers for WorkspaceView.
//!
//! This module contains all methods related to:
//! - Task creation modal (open, close, apply, edit)
//! - Session action modal (rename, assign group)
//! - Modal keyboard input handling

use super::gpui::WorkspaceView;
use super::types::{
    SessionActionKind, SessionActionModal, SessionCreationModal, TaskCreationModal,
    GROUP_COLOR_PALETTE,
};
use codirigent_core::{PaneId, SessionId, SessionManager, Task, TaskId};
use gpui::{Context, KeyDownEvent};
use std::path::Path;
use tracing::{info, warn};

impl WorkspaceView {
    pub(super) fn open_session_action_modal(
        &mut self,
        session_id: SessionId,
        kind: SessionActionKind,
    ) {
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

        let cursor_position = input.chars().count();
        self.modals.session_action = Some(SessionActionModal {
            session_id,
            kind,
            input,
            cursor_position,
            error: None,
        });
    }

    pub(super) fn close_session_action_modal(&mut self) {
        self.modals.session_action = None;
    }

    pub(super) fn open_session_creation_modal(&mut self, target_pane: Option<PaneId>) {
        self.modals.session_creation = Some(SessionCreationModal {
            target_pane,
            shell_options: self.detected_shell_options(),
            selected_shell_index: 0,
            pending: false,
            error: None,
        });
    }

    pub(super) fn close_session_creation_modal(&mut self) {
        self.modals.session_creation = None;
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
            cursor_positions: [0, 0, 0],
            error: None,
            project_dir,
            plan_file: String::new(),
            editing_task_id: None,
        });
    }

    /// Open the task creation modal pre-filled with a file's name and path.
    pub(super) fn open_task_creation_modal_for_file(&mut self, path: &Path) {
        let project_dir = self
            .project
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
            cursor_positions: [
                path.file_name()
                    .map(|n| n.to_string_lossy().chars().count())
                    .unwrap_or(0),
                0,
                relative_path.to_string_lossy().chars().count(),
            ],
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
            cursor_positions: [
                task.title.chars().count(),
                task.description.chars().count(),
                task.plan_file
                    .as_ref()
                    .map(|s| s.chars().count())
                    .unwrap_or(0),
            ],
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
            let task_id = TaskId::from(format!("task-{}", self.next_session_id));
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

        self.sync_task_derived_state();
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
        self.refresh_derived_ui_state();
        self.save_state_to_disk(cx);
        self.close_session_action_modal();
        cx.notify();
    }

    pub(super) fn apply_session_creation_modal(&mut self, cx: &mut Context<Self>) {
        let Some(modal) = self.modals.session_creation.clone() else {
            return;
        };
        if modal.pending {
            return;
        }

        let requested_shell = modal
            .shell_options
            .get(modal.selected_shell_index)
            .cloned()
            .filter(|shell| !shell.is_empty());
        let target_pane = modal.target_pane.clone();

        if let Some(active) = self.modals.session_creation.as_mut() {
            active.pending = true;
            active.error = None;
        }
        self.create_session_with_shell(target_pane, requested_shell, cx);
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
        modal.cursor_position = modal.cursor_position.min(Self::char_count(&modal.input));

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
                Self::backspace_at_cursor(&mut modal.input, &mut modal.cursor_position);
                modal.error = None;
                cx.notify();
                return true;
            }
            "delete" => {
                Self::delete_at_cursor(&mut modal.input, &mut modal.cursor_position);
                modal.error = None;
                cx.notify();
                return true;
            }
            "left" | "arrowleft" => {
                Self::move_cursor_left(&modal.input, &mut modal.cursor_position);
                cx.notify();
                return true;
            }
            "right" | "arrowright" => {
                Self::move_cursor_right(&modal.input, &mut modal.cursor_position);
                cx.notify();
                return true;
            }
            "home" => {
                Self::move_cursor_home(&mut modal.cursor_position);
                cx.notify();
                return true;
            }
            "end" => {
                Self::move_cursor_end(&modal.input, &mut modal.cursor_position);
                cx.notify();
                return true;
            }
            "space" => {
                // GPUI on Windows reports space as key="space" with key_char=None
                Self::insert_at_cursor(&mut modal.input, &mut modal.cursor_position, " ");
                modal.error = None;
                cx.notify();
                return true;
            }
            _ => {}
        }

        // Ctrl+A selects all (clears input for easy replacement)
        if (event.keystroke.modifiers.control || event.keystroke.modifiers.platform) && key == "a" {
            modal.input.clear();
            modal.cursor_position = 0;
            cx.notify();
            return true;
        }

        // Ctrl+V / Cmd+V — paste from system clipboard
        if (event.keystroke.modifiers.control || event.keystroke.modifiers.platform) && key == "v" {
            if let Ok(codirigent_core::ClipboardContent::Text(text)) =
                self.clipboard.smart_clipboard.read_content()
            {
                if let Some(modal) = self.modals.session_action.as_mut() {
                    Self::insert_at_cursor(&mut modal.input, &mut modal.cursor_position, &text);
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
            if !key_char.is_empty() {
                Self::insert_at_cursor(&mut modal.input, &mut modal.cursor_position, key_char);
                modal.error = None;
                cx.notify();
            }
        }

        true
    }

    pub(super) fn handle_session_creation_key_down(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(modal) = self.modals.session_creation.as_ref() else {
            return false;
        };
        let modal_pending = modal.pending;
        let selected_shell_index = modal.selected_shell_index;
        let visible_shell_order = self.shell_picker_option_order(&modal.shell_options);

        let key = event.keystroke.key.to_lowercase();
        match key.as_str() {
            "escape" => {
                if modal_pending {
                    cx.notify();
                    return true;
                }
                self.close_session_creation_modal();
                cx.notify();
                return true;
            }
            "enter" => {
                self.apply_session_creation_modal(cx);
                return true;
            }
            "up" | "left" | "k" => {
                if modal_pending {
                    return true;
                }
                if !visible_shell_order.is_empty() {
                    let current_position = visible_shell_order
                        .iter()
                        .position(|&index| index == selected_shell_index)
                        .unwrap_or(0);
                    let previous_position = current_position
                        .checked_sub(1)
                        .unwrap_or(visible_shell_order.len().saturating_sub(1));
                    if let Some(modal) = self.modals.session_creation.as_mut() {
                        modal.selected_shell_index = visible_shell_order[previous_position];
                    }
                    cx.notify();
                }
                return true;
            }
            "down" | "right" | "j" => {
                if modal_pending {
                    return true;
                }
                if !visible_shell_order.is_empty() {
                    let current_position = visible_shell_order
                        .iter()
                        .position(|&index| index == selected_shell_index)
                        .unwrap_or(0);
                    let next_position = (current_position + 1) % visible_shell_order.len();
                    if let Some(modal) = self.modals.session_creation.as_mut() {
                        modal.selected_shell_index = visible_shell_order[next_position];
                    }
                    cx.notify();
                }
                return true;
            }
            "tab" => {
                if modal_pending {
                    return true;
                }
                if !visible_shell_order.is_empty() {
                    let current_position = visible_shell_order
                        .iter()
                        .position(|&index| index == selected_shell_index)
                        .unwrap_or(0);
                    let len = visible_shell_order.len();
                    let step = if event.keystroke.modifiers.shift {
                        len.saturating_sub(1)
                    } else {
                        1
                    };
                    let next_position = (current_position + step) % len;
                    if let Some(modal) = self.modals.session_creation.as_mut() {
                        modal.selected_shell_index = visible_shell_order[next_position];
                    }
                    cx.notify();
                }
                return true;
            }
            _ => {}
        }

        true
    }

    fn char_count(text: &str) -> usize {
        text.chars().count()
    }

    fn byte_index_for_char(text: &str, char_index: usize) -> usize {
        if char_index == 0 {
            return 0;
        }
        text.char_indices()
            .nth(char_index)
            .map(|(i, _)| i)
            .unwrap_or(text.len())
    }

    fn focused_field_and_cursor_mut(
        modal: &mut TaskCreationModal,
    ) -> Option<(&mut String, &mut usize)> {
        match modal.focused_field {
            0 => Some((&mut modal.title, &mut modal.cursor_positions[0])),
            1 => Some((&mut modal.description, &mut modal.cursor_positions[1])),
            2 => Some((&mut modal.plan_file, &mut modal.cursor_positions[2])),
            _ => None,
        }
    }

    fn clamp_task_modal_cursor(modal: &mut TaskCreationModal) {
        let title_len = Self::char_count(&modal.title);
        let desc_len = Self::char_count(&modal.description);
        let plan_len = Self::char_count(&modal.plan_file);
        modal.cursor_positions[0] = modal.cursor_positions[0].min(title_len);
        modal.cursor_positions[1] = modal.cursor_positions[1].min(desc_len);
        modal.cursor_positions[2] = modal.cursor_positions[2].min(plan_len);
    }

    fn insert_at_cursor(field: &mut String, cursor: &mut usize, text: &str) {
        let cursor_byte = Self::byte_index_for_char(field, *cursor);
        field.insert_str(cursor_byte, text);
        *cursor += text.chars().count();
    }

    fn insert_text_into_task_modal(modal: &mut TaskCreationModal, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }

        Self::clamp_task_modal_cursor(modal);
        if let Some((field, cursor)) = Self::focused_field_and_cursor_mut(modal) {
            Self::insert_at_cursor(field, cursor, text);
            modal.error = None;
            true
        } else {
            false
        }
    }

    pub(super) fn insert_task_creation_text(&mut self, text: &str) -> bool {
        let Some(modal) = self.modals.task_creation.as_mut() else {
            return false;
        };
        Self::insert_text_into_task_modal(modal, text)
    }

    fn backspace_at_cursor(field: &mut String, cursor: &mut usize) {
        if *cursor == 0 {
            return;
        }
        let end = Self::byte_index_for_char(field, *cursor);
        let start = Self::byte_index_for_char(field, *cursor - 1);
        field.replace_range(start..end, "");
        *cursor -= 1;
    }

    fn delete_at_cursor(field: &mut String, cursor: &mut usize) {
        let len = Self::char_count(field);
        if *cursor >= len {
            return;
        }
        let start = Self::byte_index_for_char(field, *cursor);
        let end = Self::byte_index_for_char(field, *cursor + 1);
        field.replace_range(start..end, "");
    }

    fn move_cursor_left(field: &str, cursor: &mut usize) {
        let len = Self::char_count(field);
        *cursor = (*cursor).min(len);
        if *cursor > 0 {
            *cursor -= 1;
        }
    }

    fn move_cursor_right(field: &str, cursor: &mut usize) {
        let len = Self::char_count(field);
        if *cursor < len {
            *cursor += 1;
        }
    }

    fn move_cursor_home(cursor: &mut usize) {
        *cursor = 0;
    }

    fn move_cursor_end(field: &str, cursor: &mut usize) {
        *cursor = Self::char_count(field);
    }

    pub(super) fn handle_task_creation_key_down(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let ime_composing = self.ime_marked_range.is_some() || self.ime_preedit_text.is_some();
        let Some(modal) = self.modals.task_creation.as_mut() else {
            return false;
        };
        Self::clamp_task_modal_cursor(modal);

        let key = event.keystroke.key.to_lowercase();
        // While an IME composition is active, keys such as Space, Enter, arrows,
        // and digits belong to the IME candidate UI. The committed text arrives
        // through EntityInputHandler::replace_text_in_range().
        if ime_composing {
            return true;
        }

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
                    if let Some((field, cursor)) = Self::focused_field_and_cursor_mut(modal) {
                        Self::insert_at_cursor(field, cursor, "\n");
                    }
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
                if let Some((field, cursor)) = Self::focused_field_and_cursor_mut(modal) {
                    Self::backspace_at_cursor(field, cursor);
                }
                modal.error = None;
                cx.notify();
                return true;
            }
            "delete" => {
                if let Some((field, cursor)) = Self::focused_field_and_cursor_mut(modal) {
                    Self::delete_at_cursor(field, cursor);
                }
                modal.error = None;
                cx.notify();
                return true;
            }
            "left" | "arrowleft" => {
                if let Some((field, cursor)) = Self::focused_field_and_cursor_mut(modal) {
                    Self::move_cursor_left(field, cursor);
                }
                cx.notify();
                return true;
            }
            "right" | "arrowright" => {
                if let Some((field, cursor)) = Self::focused_field_and_cursor_mut(modal) {
                    Self::move_cursor_right(field, cursor);
                }
                cx.notify();
                return true;
            }
            "home" => {
                if let Some((_, cursor)) = Self::focused_field_and_cursor_mut(modal) {
                    Self::move_cursor_home(cursor);
                }
                cx.notify();
                return true;
            }
            "end" => {
                if let Some((field, cursor)) = Self::focused_field_and_cursor_mut(modal) {
                    Self::move_cursor_end(field, cursor);
                }
                cx.notify();
                return true;
            }
            _ => {}
        }

        // Ctrl+A selects all (clears focused field for easy replacement)
        if (event.keystroke.modifiers.control || event.keystroke.modifiers.platform) && key == "a" {
            if let Some((field, cursor)) = Self::focused_field_and_cursor_mut(modal) {
                field.clear();
                *cursor = 0;
            }
            modal.error = None;
            cx.notify();
            return true;
        }

        // Ctrl+V / Cmd+V — paste from system clipboard
        if (event.keystroke.modifiers.control || event.keystroke.modifiers.platform) && key == "v" {
            if let Ok(codirigent_core::ClipboardContent::Text(text)) =
                self.clipboard.smart_clipboard.read_content()
            {
                if let Some((field, cursor)) = Self::focused_field_and_cursor_mut(modal) {
                    Self::insert_at_cursor(field, cursor, &text);
                }
                modal.error = None;
                cx.notify();
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

        // Printable characters (including plain Space) must keep propagating so
        // the platform can deliver them through EntityInputHandler. The root
        // keyboard handler already prevents these keys from reaching the PTY.
        // During IME composition we returned early above, so candidate-selection
        // digits and Space remain owned by the IME instead.
        !Self::keystroke_is_text_input(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task_modal(focused_field: usize) -> TaskCreationModal {
        TaskCreationModal {
            title: String::new(),
            description: String::new(),
            priority: codirigent_core::TaskPriority::Medium,
            focused_field,
            cursor_positions: [0, 0, 0],
            error: Some("stale error".to_string()),
            project_dir: None,
            plan_file: String::new(),
            editing_task_id: None,
        }
    }

    #[test]
    fn committed_chinese_text_is_inserted_into_every_task_field() {
        for focused_field in 0..3 {
            let mut modal = task_modal(focused_field);

            assert!(WorkspaceView::insert_text_into_task_modal(
                &mut modal,
                "中文输入"
            ));

            let values = [&modal.title, &modal.description, &modal.plan_file];
            assert_eq!(values[focused_field], "中文输入");
            assert_eq!(modal.cursor_positions[focused_field], 4);
            assert!(modal.error.is_none());
        }
    }

    #[test]
    fn committed_text_uses_character_cursor_without_splitting_unicode() {
        let mut modal = task_modal(0);
        modal.title = "甲乙".to_string();
        modal.cursor_positions[0] = 1;

        assert!(WorkspaceView::insert_text_into_task_modal(
            &mut modal, "任务"
        ));

        assert_eq!(modal.title, "甲任务乙");
        assert_eq!(modal.cursor_positions[0], 3);
    }

    #[test]
    fn empty_ime_commit_does_not_change_task_field() {
        let mut modal = task_modal(1);
        modal.description = "已有内容".to_string();
        modal.cursor_positions[1] = 4;

        assert!(!WorkspaceView::insert_text_into_task_modal(&mut modal, ""));

        assert_eq!(modal.description, "已有内容");
        assert_eq!(modal.cursor_positions[1], 4);
        assert!(modal.error.is_some());
    }
}
