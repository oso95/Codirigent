//! UI operation handlers for WorkspaceView.

use super::editor_detection::{extra_editor_dirs, is_terminal_editor};
use super::gpui::WorkspaceView;
use super::types::SessionActionKind;
use codirigent_core::config_service::ConfigService;
use codirigent_core::{SessionId, SessionManager};
use gpui::Context;
use std::path::Path;
use tracing::{info, warn};

impl WorkspaceView {
    /// Open a file in the user's configured editor.
    pub(super) fn open_in_editor(&mut self, path: &Path) {
        let editor = self
            .config_service
            .as_ref()
            .and_then(|cs| cs.load_user_settings().ok())
            .map(|s| s.general.editor_command)
            .unwrap_or_else(|| "code".to_string());

        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else if let Some(root) = &self.project_root {
            root.join(path)
        } else {
            path.to_path_buf()
        };

        if is_terminal_editor(&editor) {
            if let Some(session_id) = self.workspace.focused_session_id() {
                let path_str = if let Some(tree) = &self.file_tree_model {
                    tree.path_for_terminal(path)
                } else {
                    path.to_string_lossy().to_string()
                };

                let command = format!("{} {}\n", editor, path_str);
                if let Ok(manager) = self.session_manager.lock() {
                    if let Err(e) = manager.send_input(session_id, command.as_bytes()) {
                        warn!("Failed to open file in editor: {}", e);
                    }
                }
            } else {
                warn!("No focused terminal session for terminal editor");
            }
        } else {
            let mut cmd = std::process::Command::new(&editor);
            cmd.arg(&absolute_path);
            let extra = extra_editor_dirs();
            if !extra.is_empty() {
                let mut path_val = std::env::var("PATH").unwrap_or_default();
                let sep = if cfg!(windows) { ";" } else { ":" };
                for dir in extra.iter().rev() {
                    path_val = format!("{}{}{}", dir.display(), sep, path_val);
                }
                cmd.env("PATH", path_val);
            }
            match cmd.spawn() {
                Ok(_) => {
                    info!(editor, ?absolute_path, "Opened file in GUI editor");
                }
                Err(e) => {
                    warn!(editor, ?e, "Failed to spawn editor, falling back to 'open'");
                }
            }
        }
    }

    /// Toggle the task board panel expanded/collapsed state.
    pub fn toggle_task_board(&mut self, cx: &mut Context<Self>) {
        self.task_board.toggle_expanded();
        cx.notify();
    }

    /// Open the session context menu for a specific session.
    pub fn open_session_menu(&mut self, session_id: SessionId, cx: &mut Context<Self>) {
        info!(?session_id, "Opening session menu");
        self.session_menu_open = Some(session_id);
        cx.notify();
    }

    /// Close the currently open session context menu.
    pub fn close_session_menu(&mut self, cx: &mut Context<Self>) {
        info!("Closing session menu");
        self.session_menu_open = None;
        cx.notify();
    }

    /// Handle a session menu action (rename, assign to group, end session, etc.).
    pub fn handle_session_menu_action(
        &mut self,
        session_id: SessionId,
        action: crate::workspace::render::SessionMenuAction,
        cx: &mut Context<Self>,
    ) {
        use crate::workspace::render::SessionMenuAction;

        info!(?session_id, ?action, "Handling session menu action");

        match action {
            SessionMenuAction::Rename => {
                info!(?session_id, "Rename action");
                self.close_session_menu(cx);
                self.open_session_action_modal(session_id, SessionActionKind::Rename);
            }
            SessionMenuAction::AssignToGroup(group_name) => {
                info!(?session_id, %group_name, "Assign to existing group");
                let color = self
                    .workspace
                    .sessions()
                    .iter()
                    .find(|s| s.group.as_deref() == Some(&group_name))
                    .and_then(|s| s.color.clone())
                    .unwrap_or_else(|| self.next_group_color());
                if let Ok(manager) = self.session_manager.lock() {
                    let _ = manager.set_session_group(session_id, Some(group_name), Some(color));
                }
                self.close_session_menu(cx);
            }
            SessionMenuAction::NewGroup => {
                info!(?session_id, "New group action");
                self.close_session_menu(cx);
                self.open_session_action_modal(session_id, SessionActionKind::AssignGroup);
            }
            SessionMenuAction::RemoveGroup => {
                if let Ok(manager) = self.session_manager.lock() {
                    let _ = manager.set_session_group(session_id, None, None);
                }
                self.close_session_menu(cx);
            }
            SessionMenuAction::EndSession => {
                self.close_session(session_id, cx);
                self.close_session_menu(cx);
            }
        }
        if let Ok(manager) = self.session_manager.lock() {
            self.workspace
                .sync_sessions_from_manager(&manager.list_sessions());
        }
        self.save_state_to_disk();
        cx.notify();
    }
}
