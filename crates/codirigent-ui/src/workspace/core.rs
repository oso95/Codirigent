//! Workspace view for Codirigent.
//!
//! This module provides the main workspace view that contains the grid
//! of session panes and manages layout switching.
//!
//! # Architecture
//!
//! The workspace is the root UI component that:
//! - Manages the current layout profile
//! - Tracks session assignments to grid cells
//! - Handles focus navigation between sessions
//! - Renders the grid of session panes
//!
//! # Example
//!
//! ```
//! use codirigent_ui::workspace::Workspace;
//! use codirigent_ui::layout::LayoutProfile;
//! use codirigent_core::{Session, SessionId};
//! use std::path::PathBuf;
//!
//! let mut workspace = Workspace::new();
//! workspace.set_layout(LayoutProfile::Grid2x2);
//!
//! let session = Session::new(SessionId(1), "Session 1".to_string(), PathBuf::from("/tmp"));
//! workspace.add_session(session);
//! ```

use crate::layout::{
    Bounds, FocusDirection, GridLayout, LayoutProfile, LayoutState, Point, TOP_BAR_HEIGHT,
};
use crate::theme::CodirigentTheme;
use codirigent_core::{Session, SessionId, SessionStatus};

/// Main workspace containing the grid of sessions.
///
/// The workspace is responsible for:
/// - Managing session layout and assignment
/// - Tracking which session is focused
/// - Handling layout profile changes
/// - Providing session navigation
#[derive(Debug)]
pub struct Workspace {
    /// Layout state (profile and assignments).
    layout_state: LayoutState,
    /// Sessions in the workspace.
    sessions: Vec<Session>,
    /// Theme configuration.
    theme: CodirigentTheme,
    /// Whether the sidebar is visible.
    show_sidebar: bool,
    /// Sidebar width in pixels.
    sidebar_width: f32,
    /// Right panel width in pixels (0.0 when closed).
    right_panel_width: f32,
    /// Current workspace bounds.
    bounds: Bounds,
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

impl Workspace {
    /// Create a new workspace with default settings.
    pub fn new() -> Self {
        Self {
            layout_state: LayoutState::new(),
            sessions: Vec::new(),
            theme: CodirigentTheme::default(),
            show_sidebar: true,
            sidebar_width: 56.0,
            right_panel_width: 0.0,
            bounds: Bounds::from_size(1280.0, 720.0),
        }
    }

    /// Create a new workspace with a specific layout profile.
    pub fn with_profile(profile: LayoutProfile) -> Self {
        Self {
            layout_state: LayoutState::with_profile(profile),
            sessions: Vec::new(),
            theme: CodirigentTheme::default(),
            show_sidebar: true,
            sidebar_width: 56.0,
            right_panel_width: 0.0,
            bounds: Bounds::from_size(1280.0, 720.0),
        }
    }

    // --- Layout Management ---

    /// Get the current layout profile.
    pub fn layout_profile(&self) -> LayoutProfile {
        self.layout_state.profile()
    }

    /// Set the layout profile.
    ///
    /// When switching to Single layout, moves the currently focused session
    /// to index 0 so it will be displayed in the single cell.
    pub fn set_layout(&mut self, profile: LayoutProfile) {
        // If switching to Single layout and we have a focused session,
        // move it to index 0 so it will be the one displayed
        if profile == LayoutProfile::Single {
            if let Some(focused_idx) = self.layout_state.focused_index() {
                if focused_idx > 0 {
                    // Move focused session to front of assignments
                    let mut assignments = self.layout_state.assignments().to_vec();
                    let focused_id = assignments[focused_idx];
                    assignments.remove(focused_idx);
                    assignments.insert(0, focused_id);
                    self.layout_state.set_assignments(assignments);
                    // Update focused index to reflect new position
                    self.layout_state.focus_index(0);
                }
            }
        }

        self.layout_state.set_profile(profile);
    }

    /// Cycle to the next layout profile.
    pub fn next_layout(&mut self) {
        self.layout_state.next_profile();
    }

    /// Cycle to the previous layout profile.
    pub fn previous_layout(&mut self) {
        self.layout_state.previous_profile();
    }

    /// Get the grid layout calculator for the current state.
    pub fn grid_layout(&self) -> GridLayout {
        let grid_bounds = self.grid_bounds();
        GridLayout::from_profile(
            self.layout_state.profile(),
            grid_bounds,
            self.theme.grid_gap,
        )
    }

    // --- Session Management ---

    /// Get all sessions.
    pub fn sessions(&self) -> &[Session] {
        &self.sessions
    }

    /// Get a session by ID.
    pub fn session(&self, id: SessionId) -> Option<&Session> {
        self.sessions.iter().find(|s| s.id == id)
    }

    /// Get a mutable reference to a session by ID.
    pub fn session_mut(&mut self, id: SessionId) -> Option<&mut Session> {
        self.sessions.iter_mut().find(|s| s.id == id)
    }

    /// Add a session to the workspace.
    ///
    /// The session is automatically assigned to the next available grid slot.
    ///
    /// # Returns
    ///
    /// `true` if the session was added successfully.
    pub fn add_session(&mut self, session: Session) -> bool {
        let id = session.id;

        // Check if session already exists
        if self.sessions.iter().any(|s| s.id == id) {
            return false;
        }

        // Try to add to layout
        if !self.layout_state.add_session(id) {
            return false;
        }

        self.sessions.push(session);

        // Focus the new session if it's the first one
        if self.layout_state.focused_index().is_none() {
            self.layout_state.focus_index(0);
        }

        true
    }

    /// Remove a session from the workspace.
    ///
    /// # Returns
    ///
    /// The removed session, if found.
    pub fn remove_session(&mut self, id: SessionId) -> Option<Session> {
        // Remove from layout
        self.layout_state.remove_session(id);

        // Remove from sessions list
        if let Some(pos) = self.sessions.iter().position(|s| s.id == id) {
            Some(self.sessions.remove(pos))
        } else {
            None
        }
    }

    /// Update a session's status.
    pub fn update_session_status(&mut self, id: SessionId, status: SessionStatus) {
        if let Some(session) = self.session_mut(id) {
            session.status = status;
        }
    }

    /// Sync session metadata from the authoritative source (SessionManager).
    ///
    /// Copies all fields from `manager_sessions` into the workspace's cached
    /// sessions, **except `status`** which is owned by the detector/UI side.
    /// This replaces the previous piecemeal dual-write pattern and ensures the
    /// workspace cache never drifts from the manager.
    pub fn sync_sessions_from_manager(&mut self, manager_sessions: &[Session]) {
        for src in manager_sessions {
            if let Some(dst) = self.session_mut(src.id) {
                dst.name = src.name.clone();
                dst.working_directory = src.working_directory.clone();
                dst.current_task = src.current_task.clone();
                dst.context_usage = src.context_usage;
                dst.group = src.group.clone();
                dst.color = src.color.clone();
                dst.git_info = src.git_info.clone();
                // `status` is NOT synced — the detector is the authority.
                // `id` and `created_at` are immutable.
            }
        }
    }

    /// Get the visible sessions (those assigned to grid cells).
    pub fn visible_sessions(&self) -> Vec<&Session> {
        self.layout_state
            .assignments()
            .iter()
            .filter_map(|&id| self.session(id))
            .collect()
    }

    /// Get the number of sessions that can still be added.
    pub fn available_slots(&self) -> usize {
        self.layout_state.profile().max_sessions() - self.layout_state.assignments().len()
    }

    // --- Focus Management ---

    /// Get the focused session ID.
    pub fn focused_session_id(&self) -> Option<SessionId> {
        self.layout_state.focused_session()
    }

    /// Get the focused session.
    pub fn focused_session(&self) -> Option<&Session> {
        self.focused_session_id().and_then(|id| self.session(id))
    }

    /// Focus a session by ID.
    ///
    /// # Returns
    ///
    /// `true` if the session was found and focused.
    pub fn focus_session(&mut self, id: SessionId) -> bool {
        self.layout_state.focus_session(id)
    }

    /// Focus a session by grid index (1-based, for keyboard shortcuts).
    ///
    /// # Returns
    ///
    /// `true` if the index is valid and session was focused.
    pub fn focus_session_number(&mut self, number: usize) -> bool {
        if number == 0 || number > self.layout_state.assignments().len() {
            return false;
        }
        self.layout_state.focus_index(number - 1);
        true
    }

    /// Focus the next session.
    pub fn focus_next(&mut self) {
        self.layout_state.focus_next();
    }

    /// Focus the previous session.
    pub fn focus_previous(&mut self) {
        self.layout_state.focus_previous();
    }

    /// Focus in a direction (for arrow key navigation).
    pub fn focus_direction(&mut self, direction: FocusDirection) {
        self.layout_state.focus_direction(direction);
    }

    // --- Sidebar ---

    /// Check if the sidebar is visible.
    pub fn is_sidebar_visible(&self) -> bool {
        self.show_sidebar
    }

    /// Toggle sidebar visibility.
    pub fn toggle_sidebar(&mut self) {
        self.show_sidebar = !self.show_sidebar;
    }

    /// Set sidebar visibility.
    pub fn set_sidebar_visible(&mut self, visible: bool) {
        self.show_sidebar = visible;
    }

    /// Get the sidebar width.
    pub fn sidebar_width(&self) -> f32 {
        self.sidebar_width
    }

    /// Set the sidebar width.
    pub fn set_sidebar_width(&mut self, width: f32) {
        self.sidebar_width = width.max(0.0);
    }

    /// Set the right panel width (0.0 when closed).
    pub fn set_right_panel_width(&mut self, width: f32) {
        self.right_panel_width = width.max(0.0);
    }

    // --- Theme ---

    /// Get the current theme.
    pub fn theme(&self) -> &CodirigentTheme {
        &self.theme
    }

    /// Set the theme.
    pub fn set_theme(&mut self, theme: CodirigentTheme) {
        self.theme = theme;
    }

    // --- Bounds ---

    /// Set the workspace bounds (called on window resize).
    pub fn set_bounds(&mut self, bounds: Bounds) {
        self.bounds = bounds;
    }

    /// Get the workspace bounds.
    pub fn bounds(&self) -> Bounds {
        self.bounds
    }

    /// Get the bounds available for the grid (excluding sidebar and chrome).
    ///
    /// Subtracts vertical chrome (title bar, top bar, grid padding) and
    /// horizontal chrome (sidebar, grid container padding) so that
    /// `cell_size()` and `cell_bounds_for_index()` return dimensions
    /// matching the actual GPUI flex-allocated space.
    pub fn grid_bounds(&self) -> Bounds {
        // Title bar (32px) + Top bar (48px) + grid container padding (gap * 2)
        let chrome_height = 32.0 + TOP_BAR_HEIGHT + self.theme.grid_gap * 2.0;
        // Grid container has .p(px(grid_gap)) padding on all sides
        let grid_padding_h = self.theme.grid_gap * 2.0;

        let x = if self.show_sidebar {
            self.sidebar_width
        } else {
            self.bounds.origin.x
        };

        let width = if self.show_sidebar {
            (self.bounds.size.width - self.sidebar_width - self.right_panel_width - grid_padding_h).max(0.0)
        } else {
            (self.bounds.size.width - self.right_panel_width - grid_padding_h).max(0.0)
        };

        // Subtract chrome heights from vertical space
        let y = self.bounds.origin.y + chrome_height;
        let height = (self.bounds.size.height - chrome_height).max(0.0);

        Bounds::new(x, y, width, height)
    }

    /// Get the sidebar bounds.
    pub fn sidebar_bounds(&self) -> Option<Bounds> {
        if self.show_sidebar {
            Some(Bounds::new(
                self.bounds.origin.x,
                self.bounds.origin.y,
                self.sidebar_width,
                self.bounds.size.height,
            ))
        } else {
            None
        }
    }

    // --- Cell Information ---

    /// Get the bounds for a session's cell.
    pub fn session_cell_bounds(&self, id: SessionId) -> Option<Bounds> {
        let index = self
            .layout_state
            .assignments()
            .iter()
            .position(|&s| s == id)?;
        self.grid_layout().cell_bounds_for_index(index)
    }

    /// Get the session at a point in the grid.
    pub fn session_at_point(&self, point: Point) -> Option<SessionId> {
        let grid_bounds = self.grid_bounds();

        // Check if point is in grid area
        if !grid_bounds.contains(point) {
            return None;
        }

        // Adjust point relative to grid origin
        let adjusted_point = Point::new(
            point.x - grid_bounds.origin.x,
            point.y - grid_bounds.origin.y,
        );

        let local_bounds = Bounds::from_size(grid_bounds.size.width, grid_bounds.size.height);
        let layout = GridLayout::from_profile(
            self.layout_state.profile(),
            local_bounds,
            self.theme.grid_gap,
        );

        let position = layout.cell_at_point(adjusted_point)?;
        self.layout_state.session_at_position(position)
    }

    // --- State for Rendering ---

    /// Get information about each visible cell for rendering.
    pub fn cell_info(&self) -> Vec<CellInfo> {
        let layout = self.grid_layout();
        let focused_id = self.focused_session_id();

        self.layout_state
            .assignments()
            .iter()
            .enumerate()
            .filter_map(|(index, &session_id)| {
                let bounds = layout.cell_bounds_for_index(index)?;
                let session = self.session(session_id)?;

                Some(CellInfo {
                    session_id,
                    index,
                    bounds,
                    name: session.name.clone(),
                    status: session.status,
                    is_focused: focused_id == Some(session_id),
                })
            })
            .collect()
    }
}

/// Information about a grid cell for rendering.
#[derive(Debug, Clone)]
pub struct CellInfo {
    /// Session ID.
    pub session_id: SessionId,
    /// Grid index (0-based).
    pub index: usize,
    /// Cell bounds.
    pub bounds: Bounds,
    /// Session name.
    pub name: String,
    /// Session status.
    pub status: SessionStatus,
    /// Whether this cell is focused.
    pub is_focused: bool,
}

