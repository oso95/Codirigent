//! File tree and worktree event handlers for WorkspaceView.
//!
//! This module contains all methods related to:
//! - File tree panel refresh and synchronization
//! - File tree event handling (selection, activation, drag-drop)
//! - Worktree panel management
//! - File tree context menu operations
//! - Path insertion and clipboard operations

use super::gpui::WorkspaceView;
use super::types::FileTreeContextMenu;
use crate::sidebar::{FileTreeEntryData, FileTreeEvent, WorktreeEvent};
use codirigent_core::{SessionId, SessionManager};
use codirigent_filetree::FileTree;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

impl WorkspaceView {
    pub(super) fn refresh_file_tree_panel(&mut self) {
        let entries = if let Some(tree) = &self.file_tree_model {
            let tree: &FileTree = tree;
            tree.visible_entries()
                .into_iter()
                .map(|(depth, entry)| {
                    (
                        depth,
                        FileTreeEntryData {
                            path: entry.path.clone(),
                            name: entry.name.clone(),
                            is_dir: entry.is_dir,
                            expanded: entry.expanded,
                        },
                    )
                })
                .collect()
        } else {
            Vec::new()
        };

        self.file_tree.update_from_entries(entries);
    }

    /// Refresh worktree panel from the worktree manager.
    pub(super) fn refresh_worktree_panel(&mut self) {
        if let Some(ref manager) = self.worktree_manager {
            if let Ok(mut mgr) = manager.lock() {
                let _: Result<(), anyhow::Error> = mgr.refresh();
                self.worktree_panel.set_worktrees(mgr.list().to_vec());
                if let Ok(branches) = mgr.list_local_branches() {
                    self.worktree_panel.set_available_branches(branches);
                }
                return;
            }
        }

        self.worktree_panel.set_worktrees(Vec::new());
        self.worktree_panel.set_available_branches(Vec::new());
    }

    /// Set the current project root and update dependent UI.
    pub(super) fn set_project_root(&mut self, path: PathBuf) {
        self.project_root = Some(path.clone());
        self.file_tree.set_root(path.clone());

        match FileTree::new(path.clone()) {
            Ok(tree) => {
                self.file_tree_model = Some(tree);
            }
            Err(e) => {
                warn!("Failed to initialize file tree for {:?}: {}", path, e);
                self.file_tree_model = None;
            }
        }

        self.refresh_file_tree_panel();

        self.worktree_manager = codirigent_session::WorktreeManager::new(&path)
            .ok()
            .map(|manager| Arc::new(Mutex::new(manager)));
        self.refresh_worktree_panel();
    }

    /// Sync the file tree panel to show the focused session's working directory.
    ///
    /// Called when focus switches between sessions so the file tree always
    /// reflects the active session's CWD.
    pub(super) fn sync_file_tree_to_focused_session(&mut self) {
        let cwd = self
            .workspace
            .focused_session()
            .map(|s| s.working_directory.clone());
        if let Some(cwd) = cwd {
            // Only update if the directory actually differs from the current root
            if self.project_root.as_ref() != Some(&cwd) {
                self.set_project_root(cwd);
            }
        }
    }

    pub(super) fn handle_file_tree_event(&mut self, event: FileTreeEvent, cx: &mut gpui::Context<Self>) {
        match event {
            FileTreeEvent::FileSelected(path) => {
                info!(?path, "File selected");
                self.file_tree.select(&path);
                if let Some(tree) = self.file_tree_model.as_mut() {
                    let tree: &mut FileTree = tree;
                    tree.select(&path);
                }
                self.refresh_file_tree_panel();
            }
            FileTreeEvent::FileActivated(path) => {
                info!(?path, "File activated");
                self.open_in_editor(&path);
            }
            FileTreeEvent::DirectoryToggled(path) => {
                info!(?path, "Directory toggled");
                if let Some(tree) = self.file_tree_model.as_mut() {
                    let tree: &mut FileTree = tree;
                    if let Err(e) = tree.toggle(&path) {
                        warn!("Failed to toggle directory {:?}: {}", path, e);
                    }
                    self.refresh_file_tree_panel();
                } else {
                    self.file_tree.toggle_directory(&path);
                }
            }
            FileTreeEvent::PathDraggedToTerminal { path, session_id } => {
                info!(?path, ?session_id, "Path dragged to terminal");
                // C3 implementation: insert path into terminal
                let path_str = if let Some(tree) = &self.file_tree_model {
                    let tree: &FileTree = tree;
                    tree.path_for_terminal(&path)
                } else {
                    path.to_string_lossy().to_string()
                };
                let input = format!("{} ", path_str); // Add space after path
                let session_id = SessionId(session_id);
                if let Ok(manager) = self.session_manager.lock() {
                    if let Err(e) = manager.send_input(session_id, input.as_bytes()) {
                        warn!("Failed to send path to terminal: {}", e);
                    }
                }
            }
        }
        cx.notify();
    }

    /// Handle worktree panel events.
    pub(super) fn handle_worktree_event(&mut self, event: WorktreeEvent, cx: &mut gpui::Context<Self>) {
        match event {
            WorktreeEvent::RemoveRequested(path) => {
                info!(?path, "Remove worktree requested");
                if let Some(ref manager) = self.worktree_manager {
                    if let Ok(mut mgr) = manager.lock() {
                        if let Err(e) = mgr.remove(&path, false) {
                            warn!("Failed to remove worktree: {}", e);
                        } else {
                            // Refresh the list
                            if let Ok(()) = mgr.refresh() {
                                self.worktree_panel.set_worktrees(mgr.list().to_vec());
                            }
                        }
                    }
                }
            }
            WorktreeEvent::BindSession {
                worktree_path,
                session_id,
            } => {
                info!(?worktree_path, ?session_id, "Bind session to worktree");
                if let Some(ref manager) = self.worktree_manager {
                    if let Ok(mut mgr) = manager.lock() {
                        if mgr.bind_session(&worktree_path, session_id).is_ok() {
                            // Refresh the list
                            mgr.refresh().ok();
                            self.worktree_panel.set_worktrees(mgr.list().to_vec());
                        }
                    }
                }
            }
            WorktreeEvent::UnbindSession(session_id) => {
                info!(?session_id, "Unbind session from worktree");
                if let Some(ref manager) = self.worktree_manager {
                    if let Ok(mut mgr) = manager.lock() {
                        if mgr.unbind_session(session_id).is_ok() {
                            // Refresh the list
                            mgr.refresh().ok();
                            self.worktree_panel.set_worktrees(mgr.list().to_vec());
                        }
                    }
                }
            }
            WorktreeEvent::CleanupMerged => {
                info!("Cleanup merged worktrees");
                if let Some(ref manager) = self.worktree_manager {
                    if let Ok(mut mgr) = manager.lock() {
                        if let Ok(removed) = mgr.cleanup_merged("main") {
                            info!("Removed {} merged worktrees", removed.len());
                            // Refresh the list
                            let _: Result<(), anyhow::Error> = mgr.refresh();
                            self.worktree_panel.set_worktrees(mgr.list().to_vec());
                        }
                    }
                }
            }
            WorktreeEvent::Refresh => {
                info!("Refresh worktree list");
                if let Some(ref manager) = self.worktree_manager {
                    if let Ok(mut mgr) = manager.lock() {
                        let _: Result<(), anyhow::Error> = mgr.refresh();
                        self.worktree_panel.set_worktrees(mgr.list().to_vec());
                    }
                }
            }
        }
        cx.notify();
    }

    pub(super) fn open_file_tree_context_menu(
        &mut self,
        path: PathBuf,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut gpui::Context<Self>,
    ) {
        self.selection.file_tree_context_menu = Some(FileTreeContextMenu { path, position });
        cx.notify();
    }

    /// Close the file tree context menu.
    pub(super) fn close_file_tree_context_menu(&mut self, cx: &mut gpui::Context<Self>) {
        self.selection.file_tree_context_menu = None;
        cx.notify();
    }

    /// Insert a file path into the focused terminal session.
    pub(super) fn insert_path_to_terminal(&mut self, path: &std::path::Path) {
        if let Some(session_id) = self.workspace.focused_session_id() {
            let path_str = if let Some(tree) = &self.file_tree_model {
                tree.path_for_terminal(path)
            } else {
                path.to_string_lossy().to_string()
            };
            let input = format!("{} ", path_str);
            if let Ok(manager) = self.session_manager.lock() {
                if let Err(e) = manager.send_input(session_id, input.as_bytes()) {
                    warn!("Failed to insert path into terminal: {}", e);
                }
            }
        }
    }

    /// Copy a file path to the system clipboard.
    pub(super) fn copy_path_to_clipboard(&self, path: &std::path::Path) {
        let path_str = if let Some(tree) = &self.file_tree_model {
            tree.path_for_terminal(path)
        } else {
            path.to_string_lossy().to_string()
        };
        if let Err(e) = self.smart_clipboard.write_text(path_str) {
            warn!("Failed to copy path to clipboard: {}", e);
        }
    }
}
