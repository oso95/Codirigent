//! Project and file tree state for WorkspaceView.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::sidebar::{FileTreePanel, WorktreePanel};
use codirigent_filetree::FileTree;

/// Groups all project/file-tree state for the workspace.
pub(super) struct ProjectState {
    /// File tree sidebar panel.
    pub(super) file_tree: FileTreePanel,
    /// In-memory file tree model.
    pub(super) file_tree_model: Option<FileTree>,
    /// Root directory of the current project.
    pub(super) project_root: Option<PathBuf>,
    /// Worktree management panel.
    pub(super) worktree_panel: WorktreePanel,
    /// Shared worktree manager for git worktree operations.
    pub(super) worktree_manager: Option<Arc<Mutex<codirigent_session::WorktreeManager>>>,
    /// Current git branch name.
    pub(super) current_branch: Option<String>,
}
