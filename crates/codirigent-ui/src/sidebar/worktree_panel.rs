//! Git worktree panel component.
//!
//! Provides a read-only UI for viewing git worktrees and branches:
//! - View existing worktrees with branch names
//! - See session bindings
//! - View all local branches

use codirigent_core::{SessionId, Worktree};
use std::path::PathBuf;

/// Worktree panel events.
#[derive(Debug, Clone, PartialEq)]
pub enum WorktreeEvent {
    /// Remove worktree requested.
    RemoveRequested(PathBuf),
    /// Bind session to worktree.
    BindSession {
        /// Worktree path.
        worktree_path: PathBuf,
        /// Session ID to bind.
        session_id: SessionId,
    },
    /// Unbind session from worktree.
    UnbindSession(SessionId),
    /// Cleanup merged worktrees.
    CleanupMerged,
    /// Refresh worktree list.
    Refresh,
}

/// Worktree panel state.
#[derive(Debug)]
pub struct WorktreePanel {
    /// List of worktrees.
    worktrees: Vec<Worktree>,
    /// Available branches for display.
    available_branches: Vec<String>,
    /// Panel height.
    height: f32,
}

impl Default for WorktreePanel {
    fn default() -> Self {
        Self::new()
    }
}

impl WorktreePanel {
    /// Default panel height.
    pub const DEFAULT_HEIGHT: f32 = 300.0;
    /// Item height in pixels.
    pub const ITEM_HEIGHT: f32 = 40.0;
    /// Header height in pixels.
    pub const HEADER_HEIGHT: f32 = 36.0;

    /// Create a new worktree panel.
    pub fn new() -> Self {
        Self {
            worktrees: Vec::new(),
            available_branches: Vec::new(),
            height: Self::DEFAULT_HEIGHT,
        }
    }

    /// Update the worktree list.
    pub fn set_worktrees(&mut self, worktrees: Vec<Worktree>) {
        self.worktrees = worktrees;
    }

    /// Get the current worktree list.
    pub fn worktrees(&self) -> &[Worktree] {
        &self.worktrees
    }

    /// Set available branches for display.
    pub fn set_available_branches(&mut self, branches: Vec<String>) {
        self.available_branches = branches;
    }

    /// Get available branches.
    pub fn available_branches(&self) -> &[String] {
        &self.available_branches
    }

    /// Generate rendering hints for GPUI.
    pub fn render_hints(&self) -> WorktreeRenderHints {
        WorktreeRenderHints {
            worktrees: self.worktrees.clone(),
            available_branches: self.available_branches.clone(),
            height: self.height,
            header_height: Self::HEADER_HEIGHT,
            item_height: Self::ITEM_HEIGHT,
        }
    }
}

/// Rendering hints for the worktree panel.
#[derive(Debug, Clone)]
pub struct WorktreeRenderHints {
    /// List of worktrees.
    pub worktrees: Vec<Worktree>,
    /// Available branches.
    pub available_branches: Vec<String>,
    /// Panel height.
    pub height: f32,
    /// Header height.
    pub header_height: f32,
    /// Item height.
    pub item_height: f32,
}

/// Worktree item for rendering.
#[derive(Debug, Clone)]
pub struct WorktreeItem {
    /// Worktree path.
    pub path: PathBuf,
    /// Branch name.
    pub branch: String,
    /// Short commit SHA.
    pub head_sha: Option<String>,
    /// Whether this is the main worktree.
    pub is_main: bool,
    /// Bound session ID.
    pub bound_session: Option<SessionId>,
    /// Whether this item is hovered.
    pub is_hovered: bool,
}

impl From<&Worktree> for WorktreeItem {
    fn from(wt: &Worktree) -> Self {
        Self {
            path: wt.path.clone(),
            branch: wt.branch.clone(),
            head_sha: wt.head_sha.clone(),
            is_main: wt.is_main,
            bound_session: wt.bound_session,
            is_hovered: false,
        }
    }
}
