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
    Bounds, FocusDirection, GridLayout, LayoutProfile, LayoutState, Point, SplitLayout,
    SplitLayoutState, WorkspaceLayoutState, TOP_BAR_HEIGHT,
};
use crate::theme::CodirigentTheme;
use codirigent_core::{LayoutNode, Session, SessionId, SessionStatus, SlotId, SplitDirection};

/// Main workspace containing the grid of sessions.
///
/// The workspace is responsible for:
/// - Managing session layout and assignment
/// - Tracking which session is focused
/// - Handling layout profile changes
/// - Providing session navigation
#[derive(Debug)]
pub struct Workspace {
    /// Unified layout state supporting both grid and split tree modes.
    layout_state: WorkspaceLayoutState,
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
            layout_state: WorkspaceLayoutState::default(),
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
            layout_state: WorkspaceLayoutState::with_profile(profile),
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
    ///
    /// Returns the grid profile if in grid mode, or `Grid2x2` as fallback
    /// if in split tree mode (split trees don't map to a single profile).
    pub fn layout_profile(&self) -> LayoutProfile {
        match &self.layout_state {
            WorkspaceLayoutState::Grid(s) => s.profile(),
            WorkspaceLayoutState::SplitTree(_) => LayoutProfile::Grid2x2,
        }
    }

    /// Set the layout profile, switching to grid mode.
    ///
    /// When switching from split tree to grid, sessions are re-assigned
    /// in their current order to the new grid.
    pub fn set_layout(&mut self, profile: LayoutProfile) {
        match &mut self.layout_state {
            WorkspaceLayoutState::Grid(s) => s.set_profile(profile),
            WorkspaceLayoutState::SplitTree(split_state) => {
                // Collect current sessions and focus
                let sessions = split_state.assigned_sessions();
                let focused = split_state.focused_session();

                let mut grid_state = LayoutState::with_profile(profile);
                for sess in &sessions {
                    grid_state.add_session(*sess);
                }
                if let Some(fid) = focused {
                    grid_state.focus_session(fid);
                }
                self.layout_state = WorkspaceLayoutState::Grid(grid_state);
            }
        }
    }

    /// Cycle to the next layout profile.
    pub fn next_layout(&mut self) {
        match &mut self.layout_state {
            WorkspaceLayoutState::Grid(s) => s.next_profile(),
            WorkspaceLayoutState::SplitTree(_) => {
                // Switch to Grid2x2
                self.set_layout(LayoutProfile::Grid2x2);
            }
        }
    }

    /// Cycle to the previous layout profile.
    pub fn previous_layout(&mut self) {
        match &mut self.layout_state {
            WorkspaceLayoutState::Grid(s) => s.previous_profile(),
            WorkspaceLayoutState::SplitTree(_) => {
                self.set_layout(LayoutProfile::Single);
            }
        }
    }

    /// Get the grid layout calculator for the current state.
    ///
    /// Only meaningful in grid mode; in split tree mode, returns a 1x1 grid.
    pub fn grid_layout(&self) -> GridLayout {
        let grid_bounds = self.grid_bounds();
        match &self.layout_state {
            WorkspaceLayoutState::Grid(s) => {
                GridLayout::from_profile(s.profile(), grid_bounds, self.theme.grid_gap)
            }
            WorkspaceLayoutState::SplitTree(_) => {
                GridLayout::from_profile(LayoutProfile::Single, grid_bounds, self.theme.grid_gap)
            }
        }
    }

    /// Get a split layout calculator, if in split tree mode.
    pub fn split_layout(&self) -> Option<SplitLayout> {
        match &self.layout_state {
            WorkspaceLayoutState::SplitTree(s) => Some(SplitLayout::new(
                s.tree().clone(),
                self.grid_bounds(),
                self.theme.grid_gap,
            )),
            _ => None,
        }
    }

    /// Check if the workspace is in split tree mode.
    pub fn is_split_tree_mode(&self) -> bool {
        self.layout_state.is_split_tree()
    }

    /// Get the underlying layout state.
    pub fn layout_state(&self) -> &WorkspaceLayoutState {
        &self.layout_state
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
    /// The session is automatically assigned to the next available slot.
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
        if self.layout_state.focused_session().is_none() {
            self.layout_state.focus_session(id);
        }

        true
    }

    /// Add a session to a specific slot in the split tree.
    pub fn add_session_to_slot(&mut self, session: Session, slot: SlotId) -> bool {
        let id = session.id;
        if self.sessions.iter().any(|s| s.id == id) {
            return false;
        }
        if !self.layout_state.add_session_to_slot(id, slot) {
            return false;
        }
        self.sessions.push(session);
        if self.layout_state.focused_session().is_none() {
            self.layout_state.focus_session(id);
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

    /// Get the visible sessions (those assigned to cells/slots).
    pub fn visible_sessions(&self) -> Vec<&Session> {
        self.layout_state
            .assigned_sessions()
            .iter()
            .filter_map(|id| self.session(*id))
            .collect()
    }

    /// Get the number of sessions that can still be added.
    pub fn available_slots(&self) -> usize {
        match &self.layout_state {
            WorkspaceLayoutState::Grid(s) => s.profile().max_sessions() - s.assignments().len(),
            WorkspaceLayoutState::SplitTree(s) => s.available_slots(),
        }
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
        match &mut self.layout_state {
            WorkspaceLayoutState::Grid(s) => {
                if number == 0 || number > s.assignments().len() {
                    return false;
                }
                s.focus_index(number - 1);
                true
            }
            WorkspaceLayoutState::SplitTree(s) => {
                let sessions = s.assigned_sessions();
                if number == 0 || number > sessions.len() {
                    return false;
                }
                s.focus_session(sessions[number - 1])
            }
        }
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
        // Extract bounds and gap before mutable borrow of layout_state
        let bounds = self.grid_bounds();
        let gap = self.theme.grid_gap;
        match &mut self.layout_state {
            WorkspaceLayoutState::Grid(s) => s.focus_direction(direction),
            WorkspaceLayoutState::SplitTree(s) => {
                let layout = SplitLayout::new(
                    s.tree().clone(),
                    bounds,
                    gap,
                );
                s.focus_direction(direction, &layout);
            }
        }
    }

    /// Apply a split tree layout directly.
    ///
    /// Transfers current sessions and focus to the new tree.
    pub fn set_split_tree(&mut self, tree: LayoutNode) {
        let sessions = self.layout_state.assigned_sessions();
        let focused = self.layout_state.focused_session();
        let mut split_state = SplitLayoutState::new(tree);
        for sid in &sessions {
            split_state.add_session(*sid);
        }
        if let Some(fid) = focused {
            split_state.focus_session(fid);
        }
        self.layout_state = WorkspaceLayoutState::SplitTree(split_state);
    }

    // --- Split Pane Operations ---

    /// Split the focused pane into two.
    ///
    /// Switches to split tree mode if currently in grid mode.
    /// Returns the new slot ID, or None if no pane is focused.
    pub fn split_pane(&mut self, direction: SplitDirection, ratio: f32) -> Option<SlotId> {
        // Ensure we're in split tree mode
        if self.layout_state.is_grid() {
            self.convert_to_split_tree();
        }

        if let WorkspaceLayoutState::SplitTree(s) = &mut self.layout_state {
            let target = s.focused_slot()?;
            s.split_slot(target, direction, ratio)
        } else {
            None
        }
    }

    /// Close a pane (slot), promoting its sibling.
    ///
    /// If only one pane remains, switches back to grid mode.
    pub fn close_pane(&mut self) -> bool {
        if let WorkspaceLayoutState::SplitTree(s) = &mut self.layout_state {
            if let Some(target) = s.focused_slot() {
                let result = s.close_slot(target);
                // If only one slot remains, consider switching back to grid
                if result && s.slot_count() == 1 {
                    let sessions = s.assigned_sessions();
                    let focused = s.focused_session();
                    let mut grid_state = LayoutState::with_profile(LayoutProfile::Single);
                    for sess in &sessions {
                        grid_state.add_session(*sess);
                    }
                    if let Some(fid) = focused {
                        grid_state.focus_session(fid);
                    }
                    self.layout_state = WorkspaceLayoutState::Grid(grid_state);
                }
                return result;
            }
        }
        false
    }

    /// Resize a split by updating the ratio for the parent of the focused slot.
    pub fn resize_split(&mut self, new_ratio: f32) -> bool {
        if let WorkspaceLayoutState::SplitTree(s) = &mut self.layout_state {
            if let Some(target) = s.focused_slot() {
                return s.resize_split(target, new_ratio);
            }
        }
        false
    }

    /// Convert the current grid layout to an equivalent split tree.
    fn convert_to_split_tree(&mut self) {
        if let WorkspaceLayoutState::Grid(grid_state) = &self.layout_state {
            let (rows, cols) = grid_state.profile().dimensions();
            let tree = LayoutNode::from_grid(rows, cols);
            let mut split_state = SplitLayoutState::new(tree);

            // Transfer session assignments
            for &session_id in grid_state.assignments() {
                split_state.add_session(session_id);
            }

            // Transfer focus
            if let Some(focused) = grid_state.focused_session() {
                split_state.focus_session(focused);
            }

            self.layout_state = WorkspaceLayoutState::SplitTree(split_state);
        }
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

    /// Get a mutable reference to the theme for live settings updates.
    pub fn theme_mut(&mut self) -> &mut CodirigentTheme {
        &mut self.theme
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
            (self.bounds.size.width - self.sidebar_width - self.right_panel_width - grid_padding_h)
                .max(0.0)
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
        match &self.layout_state {
            WorkspaceLayoutState::Grid(s) => {
                let index = s.assignments().iter().position(|&sid| sid == id)?;
                self.grid_layout().cell_bounds_for_index(index)
            }
            WorkspaceLayoutState::SplitTree(s) => {
                let slot = s.slot_for_session(id)?;
                let layout = self.split_layout()?;
                layout
                    .leaf_bounds()
                    .into_iter()
                    .find(|(sid, _)| *sid == slot)
                    .map(|(_, b)| b)
            }
        }
    }

    /// Get the session at a point in the grid.
    pub fn session_at_point(&self, point: Point) -> Option<SessionId> {
        let grid_bounds = self.grid_bounds();

        // Check if point is in grid area
        if !grid_bounds.contains(point) {
            return None;
        }

        match &self.layout_state {
            WorkspaceLayoutState::Grid(s) => {
                // Adjust point relative to grid origin
                let adjusted_point = Point::new(
                    point.x - grid_bounds.origin.x,
                    point.y - grid_bounds.origin.y,
                );
                let local_bounds =
                    Bounds::from_size(grid_bounds.size.width, grid_bounds.size.height);
                let layout =
                    GridLayout::from_profile(s.profile(), local_bounds, self.theme.grid_gap);
                let position = layout.cell_at_point(adjusted_point)?;
                s.session_at_position(position)
            }
            WorkspaceLayoutState::SplitTree(s) => {
                let layout = self.split_layout()?;
                let slot = layout.slot_at_point(point)?;
                s.session_at_slot(slot)
            }
        }
    }

    // --- State for Rendering ---

    /// Get information about each visible cell for rendering.
    ///
    /// In Single layout mode, only returns the focused session.
    /// This ensures that when switching to Single layout, the currently
    /// focused session is displayed, not just the first session.
    pub fn cell_info(&self) -> Vec<CellInfo> {
        match &self.layout_state {
            WorkspaceLayoutState::Grid(s) => self.grid_cell_info(s),
            WorkspaceLayoutState::SplitTree(s) => self.split_cell_info(s),
        }
    }

    fn grid_cell_info(&self, state: &LayoutState) -> Vec<CellInfo> {
        let layout = self.grid_layout();
        let focused_id = state.focused_session();

        // Special handling for Single layout: only show the focused session
        if state.profile() == LayoutProfile::Single {
            if let Some(focused_session_id) = focused_id {
                if let Some(session) = self.session(focused_session_id) {
                    // Get bounds for the single cell (index 0)
                    if let Some(bounds) = layout.cell_bounds_for_index(0) {
                        return vec![CellInfo {
                            session_id: focused_session_id,
                            index: 0,
                            bounds,
                            name: session.name.clone(),
                            status: session.status,
                            is_focused: true,
                        }];
                    }
                }
            }
            // If no focused session, return empty (shouldn't happen in practice)
            return vec![];
        }

        // For other layouts, show all assigned sessions
        state
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

    fn split_cell_info(&self, state: &SplitLayoutState) -> Vec<CellInfo> {
        let Some(layout) = self.split_layout() else {
            return vec![];
        };
        let focused_id = state.focused_session();
        let leaf_bounds = layout.leaf_bounds();

        leaf_bounds
            .into_iter()
            .enumerate()
            .filter_map(|(index, (slot, bounds))| {
                let session_id = state.session_at_slot(slot)?;
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
