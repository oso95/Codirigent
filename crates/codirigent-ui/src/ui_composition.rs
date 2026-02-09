//! UI Composition Guide
//!
//! This module provides documentation and helper types for composing
//! all the UI redesign components together into a complete application.
//!
//! # Component Overview
//!
//! The redesigned UI consists of the following components:
//!
//! ## Layout Structure
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                        TitleBar                                  │
//! │  [●●●] CODIRIGENT            (drag area)          [─] [□] [✕]  │
//! ├─────────────┬───────────────────────────────────────────────────┤
//! │  Sidebar    │              SessionsToolbar                       │
//! │             │  [2×2] [3×2] [3×3] [Custom]    [Broadcast] [+ New] │
//! │  Sessions   ├───────────────────────────────────────────────────┤
//! │  ● Sess 1   │     ┌──────────┬──────────┬──────────┐            │
//! │  ● Sess 2   │     │TermHeader│TermHeader│ Terminal │            │
//! │  ● Sess 3   │     │ Terminal │ Terminal │  Header  │            │
//! │             │     │          │          │          │            │
//! │  Files      │     ├──────────┼──────────┼──────────┤            │
//! │  📁 src/    │     │ Empty    │TermHeader│ Terminal │            │
//! │    📄 main  │     │ Session  │ Terminal │  Header  │            │
//! │             │     │  Cell    │          │          │            │
//! │             │     └──────────┴──────────┴──────────┘            │
//! ├─────────────┴───────────────────────────────────────────────────┤
//! │                     TaskBoardPanel                               │
//! │  [Queue] [In Progress] [Review] [Done]     [Auto-assign] [+ Add]│
//! │  ┌─────────────────────────────────────────────────────────────┐│
//! │  │ TaskItem | TaskItem | TaskItem                              ││
//! │  └─────────────────────────────────────────────────────────────┘│
//! ├─────────────────────────────────────────────────────────────────┤
//! │ StatusBar: ● 4 sessions | Working: 2 | Waiting: 1    v0.1.0    │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Components
//!
//! - [`title_bar::TitleBar`] - Logo and drag area
//! - [`sidebar::SessionSidebar`] - Session list with status indicators
//! - [`sidebar::FileTreePanel`] - File tree with expandable folders
//! - [`toolbar::SessionsToolbar`] - Layout tabs and new session button
//! - [`terminal_header::TerminalHeader`] - Session header with status and context
//! - [`empty_session::EmptySessionCell`] - Placeholder for empty grid slots
//! - [`task_board::TaskBoardPanel`] - Task queue management panel
//! - [`status_bar::StatusBar`] - Status information bar
//!
//! # Usage Example
//!
//! ```ignore
//! use codirigent_ui::{
//!     title_bar::TitleBar,
//!     sidebar::SessionSidebar,
//!     toolbar::SessionsToolbar,
//!     terminal_header::TerminalHeader,
//!     empty_session::EmptySessionPool,
//!     task_board::TaskBoardPanel,
//!     status_bar::StatusBar,
//!     layout::LayoutProfile,
//! };
//!
//! // Create all UI components
//! let title_bar = TitleBar::new();
//!
//! let mut sidebar = SessionSidebar::new();
//! // Add sessions...
//!
//! let mut toolbar = SessionsToolbar::new();
//! toolbar.set_active_layout(LayoutProfile::Grid2x2);
//!
//! let mut task_board = TaskBoardPanel::new();
//! task_board.set_task_counts(5, 2, 1, 10);
//!
//! let mut status_bar = StatusBar::new();
//! status_bar.set_session_counts(4, 2, 1);
//!
//! // For each terminal cell:
//! let header = TerminalHeader::new(session_id, "Session 1", SessionStatus::Working);
//!
//! // For empty cells:
//! let mut empty_cells = EmptySessionPool::new();
//! empty_cells.setup_for_grid(2, 2, &occupied_positions);
//! ```
//!
//! # Event Handling
//!
//! Each component emits events that should be handled by the parent:
//!
//! - `SidebarEvent` - Session selection, new session
//! - `ToolbarEvent` - Layout changes, broadcast toggle
//! - `TerminalHeaderEvent` - Header actions
//! - `EmptySessionEvent` - Create session in slot
//! - `TaskBoardEvent` - Tab selection, task actions
//!
//! ```ignore
//! // Example event handling loop
//! for event in sidebar.take_events() {
//!     // Handle sidebar events...
//! }
//! ```
//!
//! # Rendering
//!
//! Each component provides a `render_hints()` method that returns
//! all the data needed for GPUI rendering:
//!
//! ```ignore
//! // Get render hints
//! let sidebar_hints = sidebar.render_hints();
//! let toolbar_hints = toolbar.render_hints();
//!
//! // Use hints in GPUI render function
//! div()
//!     .child(render_title_bar(title_bar.height()))
//!     .child(
//!         div().flex_row()
//!             .child(render_sidebar(&sidebar_hints))
//!             .child(render_main_area(&toolbar_hints, &cells))
//!     )
//! ```

use crate::empty_session::{EmptySessionEvent, EmptySessionPool};
use crate::layout::LayoutProfile;
use crate::sidebar::{SessionSidebar, SidebarEvent};
use crate::status_bar::StatusBar;
use crate::task_board::{TaskBoardEvent, TaskBoardPanel};
use crate::terminal_header::TerminalHeader;
use crate::title_bar::TitleBar;
use crate::toolbar::{SessionsToolbar, ToolbarEvent};
use codirigent_core::GridPosition;
use codirigent_core::{SessionId, SessionStatus};

/// Complete UI state container.
///
/// This struct holds all the UI components for easy management.
/// Each component maintains its own state and emits events.
#[derive(Debug)]
pub struct AppUiState {
    /// Title bar with window controls.
    pub title_bar: TitleBar,
    /// Session sidebar.
    pub sidebar: SessionSidebar,
    /// Sessions toolbar with layout tabs.
    pub toolbar: SessionsToolbar,
    /// Task board panel.
    pub task_board: TaskBoardPanel,
    /// Status bar.
    pub status_bar: StatusBar,
    /// Empty session cells pool.
    pub empty_cells: EmptySessionPool,
    /// Terminal headers by session ID.
    terminal_headers: Vec<(SessionId, TerminalHeader)>,
}

impl Default for AppUiState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppUiState {
    /// Create a new UI state with default components.
    pub fn new() -> Self {
        Self {
            title_bar: TitleBar::new(),
            sidebar: SessionSidebar::new(),
            toolbar: SessionsToolbar::new(),
            task_board: TaskBoardPanel::new(),
            status_bar: StatusBar::new(),
            empty_cells: EmptySessionPool::new(),
            terminal_headers: Vec::new(),
        }
    }

    /// Set the active layout profile.
    pub fn set_layout(&mut self, profile: LayoutProfile) {
        self.toolbar.set_active_layout(profile);
    }

    /// Update session counts in status bar.
    pub fn update_session_counts(&mut self, total: usize, working: usize, waiting: usize) {
        self.status_bar.set_session_counts(total, working, waiting);
    }

    /// Update task counts in status bar and task board.
    pub fn update_task_counts(
        &mut self,
        queue: usize,
        in_progress: usize,
        review: usize,
        done: usize,
    ) {
        self.status_bar.set_task_counts(queue, in_progress);
        self.task_board
            .set_task_counts(queue, in_progress, review, done);
    }

    /// Add or update a terminal header for a session.
    pub fn set_terminal_header(&mut self, id: SessionId, name: &str, status: SessionStatus) {
        if let Some((_, header)) = self.terminal_headers.iter_mut().find(|(sid, _)| *sid == id) {
            header.session_name = name.to_string();
            header.status = status;
        } else {
            self.terminal_headers
                .push((id, TerminalHeader::new(name, status)));
        }
    }

    /// Get a terminal header for a session.
    pub fn terminal_header(&self, id: SessionId) -> Option<&TerminalHeader> {
        self.terminal_headers
            .iter()
            .find(|(sid, _)| *sid == id)
            .map(|(_, h)| h)
    }

    /// Get a mutable terminal header for a session.
    pub fn terminal_header_mut(&mut self, id: SessionId) -> Option<&mut TerminalHeader> {
        self.terminal_headers
            .iter_mut()
            .find(|(sid, _)| *sid == id)
            .map(|(_, h)| h)
    }

    /// Remove a terminal header.
    pub fn remove_terminal_header(&mut self, id: SessionId) {
        self.terminal_headers.retain(|(sid, _)| *sid != id);
    }

    /// Set up empty cells for a grid layout.
    pub fn setup_empty_cells(&mut self, rows: u32, cols: u32, occupied: &[GridPosition]) {
        self.empty_cells.setup_for_grid(rows, cols, occupied);
    }

    /// Take all pending events from all components.
    pub fn take_events(&mut self) -> AppUiEvents {
        AppUiEvents {
            sidebar: self.sidebar.take_events(),
            toolbar: self.toolbar.take_events(),
            task_board: self.task_board.take_events(),
            empty_session: self.empty_cells.take_events(),
        }
    }
}

/// Collected events from all UI components.
#[derive(Debug, Default)]
pub struct AppUiEvents {
    /// Events from sidebar.
    pub sidebar: Vec<SidebarEvent>,
    /// Events from toolbar.
    pub toolbar: Vec<ToolbarEvent>,
    /// Events from task board.
    pub task_board: Vec<TaskBoardEvent>,
    /// Events from empty session cells.
    pub empty_session: Vec<EmptySessionEvent>,
}

impl AppUiEvents {
    /// Check if there are any pending events.
    pub fn is_empty(&self) -> bool {
        self.sidebar.is_empty()
            && self.toolbar.is_empty()
            && self.task_board.is_empty()
            && self.empty_session.is_empty()
    }
}

/// Color palette from the mockup design.
///
/// Use these colors for consistency across all components.
pub mod colors {
    /// Darkest background color.
    pub const BACKGROUND: &str = "#0a0a0c";
    /// Panel background color.
    pub const PANEL_BG: &str = "#0d0d10";
    /// Header background color.
    pub const HEADER_BG: &str = "#141418";
    /// Border color.
    pub const BORDER: &str = "#1a1a1f";
    /// Hover state color.
    pub const HOVER: &str = "#151518";
    /// Active state color.
    pub const ACTIVE: &str = "#1a1a22";

    /// Primary accent color (GitHub commit green).
    pub const PRIMARY: &str = "#39d353";
    /// Secondary accent color (blue).
    pub const SECONDARY: &str = "#5B8DEF";
    /// Purple accent.
    pub const PURPLE: &str = "#A78BFA";
    /// Orange accent.
    pub const ORANGE: &str = "#F59E0B";
    /// Error/waiting color (red).
    pub const ERROR: &str = "#FF6B6B";
    /// Warning color (yellow).
    pub const WARNING: &str = "#febc2e";

    /// Primary text color.
    pub const TEXT_PRIMARY: &str = "#e0e0e0";
    /// Secondary text color.
    pub const TEXT_SECONDARY: &str = "#888888";
    /// Muted text color.
    pub const TEXT_MUTED: &str = "#666666";
}

/// Layout constants for the UI.
pub mod layout {
    /// Default title bar height.
    pub const TITLE_BAR_HEIGHT: f32 = 32.0;
    /// Default sidebar width.
    pub const SIDEBAR_WIDTH: f32 = 220.0;
    /// Default status bar height.
    pub const STATUS_BAR_HEIGHT: f32 = 24.0;
    /// Default task board collapsed height.
    pub const TASK_BOARD_COLLAPSED: f32 = 40.0;
    /// Default task board expanded height.
    pub const TASK_BOARD_EXPANDED: f32 = 200.0;
    /// Default terminal header height.
    pub const TERMINAL_HEADER_HEIGHT: f32 = 32.0;
    /// Default grid gap.
    pub const GRID_GAP: f32 = 4.0;
    /// Default toolbar height.
    pub const TOOLBAR_HEIGHT: f32 = 44.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_ui_state_new() {
        let state = AppUiState::new();
        assert!(state.terminal_headers.is_empty());
    }

    #[test]
    fn test_app_ui_state_default() {
        let state = AppUiState::default();
        assert!(state.terminal_headers.is_empty());
    }

    #[test]
    fn test_set_layout() {
        let mut state = AppUiState::new();
        state.set_layout(LayoutProfile::Grid3x3);
        assert_eq!(state.toolbar.active_layout(), LayoutProfile::Grid3x3);
    }

    #[test]
    fn test_update_session_counts() {
        let mut state = AppUiState::new();
        state.update_session_counts(5, 3, 1);
        assert_eq!(state.status_bar.total_sessions(), 5);
        assert_eq!(state.status_bar.working_sessions(), 3);
        assert_eq!(state.status_bar.waiting_sessions(), 1);
    }

    #[test]
    fn test_update_task_counts() {
        let mut state = AppUiState::new();
        state.update_task_counts(5, 2, 1, 10);
        assert_eq!(state.status_bar.tasks_in_queue(), 5);
        assert_eq!(state.status_bar.tasks_in_progress(), 2);
    }

    #[test]
    fn test_terminal_header_lifecycle() {
        let mut state = AppUiState::new();
        let id = SessionId(1);

        // Add header
        state.set_terminal_header(id, "Session 1", SessionStatus::Working);
        assert!(state.terminal_header(id).is_some());

        // Update header
        state.set_terminal_header(id, "Renamed", SessionStatus::Idle);
        let header = state.terminal_header(id).unwrap();
        assert_eq!(header.session_name, "Renamed");

        // Remove header
        state.remove_terminal_header(id);
        assert!(state.terminal_header(id).is_none());
    }

    #[test]
    fn test_terminal_header_mut() {
        let mut state = AppUiState::new();
        let id = SessionId(1);
        state.set_terminal_header(id, "Session 1", SessionStatus::Idle);

        let header = state.terminal_header_mut(id).unwrap();
        header.context_usage = Some(0.5);
    }

    #[test]
    fn test_setup_empty_cells() {
        let mut state = AppUiState::new();
        let occupied = vec![GridPosition { row: 0, col: 0 }];
        state.setup_empty_cells(2, 2, &occupied);
        assert_eq!(state.empty_cells.count(), 3);
    }

    #[test]
    fn test_take_events_empty() {
        let mut state = AppUiState::new();
        let events = state.take_events();
        assert!(events.is_empty());
    }

    #[test]
    fn test_app_ui_events_is_empty() {
        let events = AppUiEvents::default();
        assert!(events.is_empty());
    }

    #[test]
    fn test_color_constants() {
        assert_eq!(colors::BACKGROUND, "#0a0a0c");
        assert_eq!(colors::PRIMARY, "#39d353");
        assert_eq!(colors::TEXT_PRIMARY, "#e0e0e0");
    }

    #[test]
    fn test_layout_constants() {
        let title_bar = layout::TITLE_BAR_HEIGHT;
        let sidebar = layout::SIDEBAR_WIDTH;
        let status_bar = layout::STATUS_BAR_HEIGHT;
        let grid_gap = layout::GRID_GAP;
        assert!(title_bar > 0.0);
        assert!(sidebar > 0.0);
        assert!(status_bar > 0.0);
        assert!(grid_gap > 0.0);
    }
}
