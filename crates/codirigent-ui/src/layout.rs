//! Grid layout system for Codirigent.
//!
//! This module provides the layout profile and grid calculation system
//! for organizing multiple session panes in the workspace.
//!
//! # Layout Profiles
//!
//! The system supports several predefined layout configurations:
//! - 2x2 Grid: 4 sessions in a square layout (default)
//! - 1x4 Stack: 4 sessions in a vertical column
//! - 2x3 Grid: 6 sessions in 2 rows, 3 columns
//! - 3x3 Grid: 9 sessions in a 3x3 grid
//! - Single: One session takes the full workspace
//! - Custom: User-defined grid with any rows x columns (1-10 each)
//!
//! # Example
//!
//! ```
//! use codirigent_ui::layout::{LayoutProfile, GridLayout, Bounds, Size, Point};
//! use codirigent_core::LayoutMode;
//!
//! let profile = LayoutProfile::Grid2x2;
//! assert_eq!(profile.max_sessions(), 4);
//!
//! // Custom layout example
//! let custom = LayoutProfile::custom(4, 3).unwrap();
//! assert_eq!(custom.max_sessions(), 12);
//!
//! let bounds = Bounds::new(0.0, 0.0, 1000.0, 800.0);
//! let layout = GridLayout::new(profile.to_mode(), bounds, 4.0);
//! assert_eq!(layout.cell_count(), 4);
//! ```

use codirigent_core::{GridPosition, LayoutMode, SessionId};

/// Recommended minimum cell width in pixels for comfortable terminal display.
/// This is a soft limit - cells can be smaller, but will log warnings.
pub const RECOMMENDED_MIN_CELL_WIDTH: f32 = 400.0;

/// Recommended minimum cell height in pixels for comfortable terminal display.
/// This is a soft limit - cells can be smaller, but will log warnings.
pub const RECOMMENDED_MIN_CELL_HEIGHT: f32 = 300.0;

/// Absolute minimum cell width in pixels for functional terminal display.
/// This is a hard limit - cells will not be smaller than this.
pub const ABSOLUTE_MIN_CELL_WIDTH: f32 = 200.0;

/// Absolute minimum cell height in pixels for functional terminal display.
/// This is a hard limit - cells will not be smaller than this.
pub const ABSOLUTE_MIN_CELL_HEIGHT: f32 = 150.0;

/// Legacy alias for recommended minimum width (deprecated).
#[deprecated(since = "0.1.0", note = "Use RECOMMENDED_MIN_CELL_WIDTH instead")]
pub const MIN_CELL_WIDTH: f32 = RECOMMENDED_MIN_CELL_WIDTH;

/// Legacy alias for recommended minimum height (deprecated).
#[deprecated(since = "0.1.0", note = "Use RECOMMENDED_MIN_CELL_HEIGHT instead")]
pub const MIN_CELL_HEIGHT: f32 = RECOMMENDED_MIN_CELL_HEIGHT;

/// Width of the old sidebar in pixels (deprecated: replaced by IconRail 56px + Drawer 288px).
#[deprecated(since = "0.1.0", note = "Use icon_rail::IconRail::WIDTH + drawer::Drawer::WIDTH instead")]
pub const SIDEBAR_WIDTH: f32 = 260.0;

/// Height of the old title bar in pixels (deprecated: replaced by TopBar 48px).
#[deprecated(since = "0.1.0", note = "Use TOP_BAR_HEIGHT instead")]
pub const TITLE_BAR_HEIGHT: f32 = 32.0;

/// Height of the old toolbar in pixels (deprecated: merged into TopBar 48px).
#[deprecated(since = "0.1.0", note = "Use TOP_BAR_HEIGHT instead")]
pub const TOOLBAR_HEIGHT: f32 = 48.0;

/// Height of the top bar in pixels (replaces TITLE_BAR_HEIGHT + TOOLBAR_HEIGHT).
pub const TOP_BAR_HEIGHT: f32 = 48.0;

/// Height of the old status bar in pixels (deprecated: removed, info moved to TopBar).
#[deprecated(since = "0.1.0", note = "Status bar has been removed; info is now in TopBar")]
pub const STATUS_BAR_HEIGHT: f32 = 24.0;

/// Width of the right task board panel in pixels.
pub const RIGHT_PANEL_WIDTH: f32 = 288.0;

/// A point in 2D space with pixel coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// X coordinate in pixels.
    pub x: f32,
    /// Y coordinate in pixels.
    pub y: f32,
}

impl Point {
    /// Create a new point.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Origin point (0, 0).
    pub const fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

/// A size with width and height in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    /// Width in pixels.
    pub width: f32,
    /// Height in pixels.
    pub height: f32,
}

impl Size {
    /// Create a new size.
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// Zero size.
    pub const fn zero() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
        }
    }
}

/// A rectangular bounds with origin and size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    /// Top-left origin point.
    pub origin: Point,
    /// Size of the bounds.
    pub size: Size,
}

impl Bounds {
    /// Create new bounds from origin and size.
    pub const fn from_origin_size(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    /// Create new bounds from coordinates.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }

    /// Create bounds at origin with given size.
    pub fn from_size(width: f32, height: f32) -> Self {
        Self::new(0.0, 0.0, width, height)
    }

    /// Get the right edge x coordinate.
    pub fn right(&self) -> f32 {
        self.origin.x + self.size.width
    }

    /// Get the bottom edge y coordinate.
    pub fn bottom(&self) -> f32 {
        self.origin.y + self.size.height
    }

    /// Check if this bounds contains a point.
    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.origin.x
            && point.x <= self.right()
            && point.y >= self.origin.y
            && point.y <= self.bottom()
    }
}

/// Maximum rows/columns allowed for custom layouts.
pub const MAX_GRID_DIMENSION: u32 = 10;

/// Minimum rows/columns for custom layouts.
pub const MIN_GRID_DIMENSION: u32 = 1;

/// Layout profiles for workspace organization.
///
/// Each profile defines a specific grid configuration that determines
/// how sessions are arranged in the workspace. Includes both predefined
/// profiles and custom user-defined layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum LayoutProfile {
    /// 2x2 grid layout (4 sessions).
    #[default]
    Grid2x2,
    /// 1x4 vertical stack layout (4 sessions).
    Stack1x4,
    /// 2x3 grid layout (6 sessions).
    Grid2x3,
    /// 3x3 grid layout (9 sessions).
    Grid3x3,
    /// Single session taking full workspace.
    Single,
    /// Custom grid layout with user-defined dimensions.
    /// Rows and columns must be between 1 and 10.
    Custom {
        /// Number of rows (1-10).
        rows: u32,
        /// Number of columns (1-10).
        cols: u32,
    },
}

impl LayoutProfile {
    /// All predefined profiles in cycling order (excludes Custom).
    pub const PREDEFINED: &'static [LayoutProfile] = &[
        LayoutProfile::Grid2x2,
        LayoutProfile::Stack1x4,
        LayoutProfile::Grid2x3,
        LayoutProfile::Grid3x3,
        LayoutProfile::Single,
    ];

    /// Create a custom layout profile with the given dimensions.
    ///
    /// # Arguments
    ///
    /// * `rows` - Number of rows (1-10)
    /// * `cols` - Number of columns (1-10)
    ///
    /// # Returns
    ///
    /// `Some(LayoutProfile::Custom)` if dimensions are valid, `None` otherwise.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::layout::LayoutProfile;
    ///
    /// let custom = LayoutProfile::custom(4, 3).unwrap();
    /// assert_eq!(custom.max_sessions(), 12);
    ///
    /// // Invalid dimensions return None
    /// assert!(LayoutProfile::custom(0, 3).is_none());
    /// assert!(LayoutProfile::custom(11, 3).is_none());
    /// ```
    pub fn custom(rows: u32, cols: u32) -> Option<Self> {
        if rows >= MIN_GRID_DIMENSION
            && rows <= MAX_GRID_DIMENSION
            && cols >= MIN_GRID_DIMENSION
            && cols <= MAX_GRID_DIMENSION
        {
            Some(LayoutProfile::Custom { rows, cols })
        } else {
            None
        }
    }

    /// Check if this is a custom layout.
    pub fn is_custom(self) -> bool {
        matches!(self, LayoutProfile::Custom { .. })
    }

    /// Convert this profile to a [`LayoutMode`].
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::layout::LayoutProfile;
    /// use codirigent_core::LayoutMode;
    ///
    /// let mode = LayoutProfile::Grid2x2.to_mode();
    /// assert!(matches!(mode, LayoutMode::Grid { rows: 2, cols: 2 }));
    ///
    /// let custom = LayoutProfile::custom(4, 3).unwrap();
    /// let mode = custom.to_mode();
    /// assert!(matches!(mode, LayoutMode::Grid { rows: 4, cols: 3 }));
    /// ```
    pub fn to_mode(self) -> LayoutMode {
        match self {
            LayoutProfile::Grid2x2 => LayoutMode::Grid { rows: 2, cols: 2 },
            LayoutProfile::Stack1x4 => LayoutMode::Grid { rows: 4, cols: 1 },
            LayoutProfile::Grid2x3 => LayoutMode::Grid { rows: 2, cols: 3 },
            LayoutProfile::Grid3x3 => LayoutMode::Grid { rows: 3, cols: 3 },
            LayoutProfile::Single => LayoutMode::Single,
            LayoutProfile::Custom { rows, cols } => LayoutMode::Grid { rows, cols },
        }
    }

    /// Get the next profile in the cycling order.
    ///
    /// Custom layouts cycle back to Grid2x2.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::layout::LayoutProfile;
    ///
    /// let mut profile = LayoutProfile::Grid2x2;
    /// profile = profile.next();
    /// assert_eq!(profile, LayoutProfile::Stack1x4);
    ///
    /// // Custom cycles back to Grid2x2
    /// let custom = LayoutProfile::custom(4, 3).unwrap();
    /// assert_eq!(custom.next(), LayoutProfile::Grid2x2);
    /// ```
    pub fn next(self) -> Self {
        match self {
            LayoutProfile::Custom { .. } => LayoutProfile::Grid2x2,
            _ => {
                let profiles = Self::PREDEFINED;
                let current_idx = profiles.iter().position(|&p| p == self).unwrap_or(0);
                let next_idx = (current_idx + 1) % profiles.len();
                profiles[next_idx]
            }
        }
    }

    /// Get the previous profile in the cycling order.
    ///
    /// Custom layouts cycle back to Single.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::layout::LayoutProfile;
    ///
    /// let mut profile = LayoutProfile::Grid2x2;
    /// profile = profile.previous();
    /// assert_eq!(profile, LayoutProfile::Single);
    ///
    /// // Custom cycles back to Single
    /// let custom = LayoutProfile::custom(4, 3).unwrap();
    /// assert_eq!(custom.previous(), LayoutProfile::Single);
    /// ```
    pub fn previous(self) -> Self {
        match self {
            LayoutProfile::Custom { .. } => LayoutProfile::Single,
            _ => {
                let profiles = Self::PREDEFINED;
                let current_idx = profiles.iter().position(|&p| p == self).unwrap_or(0);
                let prev_idx = if current_idx == 0 {
                    profiles.len() - 1
                } else {
                    current_idx - 1
                };
                profiles[prev_idx]
            }
        }
    }

    /// Get the display name for this layout.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::layout::LayoutProfile;
    ///
    /// assert_eq!(LayoutProfile::Grid2x2.display_name(), "2x2");
    /// ```
    pub fn display_name(self) -> String {
        match self {
            LayoutProfile::Grid2x2 => "2x2".to_string(),
            LayoutProfile::Stack1x4 => "1x4".to_string(),
            LayoutProfile::Grid2x3 => "2x3".to_string(),
            LayoutProfile::Grid3x3 => "3x3".to_string(),
            LayoutProfile::Single => "Single".to_string(),
            LayoutProfile::Custom { rows, cols } => format!("{}x{}", rows, cols),
        }
    }

    /// Get the maximum number of sessions this layout can display.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::layout::LayoutProfile;
    ///
    /// assert_eq!(LayoutProfile::Grid2x2.max_sessions(), 4);
    /// assert_eq!(LayoutProfile::Grid3x3.max_sessions(), 9);
    ///
    /// let custom = LayoutProfile::custom(4, 3).unwrap();
    /// assert_eq!(custom.max_sessions(), 12);
    /// ```
    pub fn max_sessions(self) -> usize {
        match self {
            LayoutProfile::Grid2x2 => 4,
            LayoutProfile::Stack1x4 => 4,
            LayoutProfile::Grid2x3 => 6,
            LayoutProfile::Grid3x3 => 9,
            LayoutProfile::Single => 1,
            LayoutProfile::Custom { rows, cols } => (rows * cols) as usize,
        }
    }

    /// Calculate recommended minimum window size for comfortable terminal display.
    ///
    /// Returns (width, height) in pixels based on recommended cell sizes and UI elements.
    /// The window can be made smaller than this, but terminals may be cramped.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::layout::LayoutProfile;
    ///
    /// let (width, height) = LayoutProfile::Grid2x2.recommended_window_size();
    /// assert!(width > 800.0);  // At least 2 columns of 400px + sidebar
    /// assert!(height > 600.0); // At least 2 rows of 300px + UI chrome
    /// ```
    pub fn recommended_window_size(self) -> (f32, f32) {
        let (rows, cols) = self.dimensions();
        let gap = 4.0;

        // IconRail (56px) is the minimum left chrome; drawer is optional.
        let icon_rail_width = 56.0;
        let min_width = icon_rail_width
            + (RECOMMENDED_MIN_CELL_WIDTH * cols as f32)
            + (gap * (cols - 1) as f32);

        let min_height = TOP_BAR_HEIGHT
            + (RECOMMENDED_MIN_CELL_HEIGHT * rows as f32)
            + (gap * (rows - 1) as f32);

        (min_width, min_height)
    }

    /// Calculate absolute minimum window size for functional (though cramped) display.
    ///
    /// Returns (width, height) in pixels based on absolute minimum cell sizes.
    /// The window should not be made smaller than this.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::layout::LayoutProfile;
    ///
    /// let (width, height) = LayoutProfile::Grid3x3.minimum_window_size();
    /// // Minimum allows 200×150px cells - functional but very cramped
    /// assert!(width > 600.0);  // At least 3 columns of 200px + sidebar
    /// assert!(height > 450.0); // At least 3 rows of 150px + UI chrome
    /// ```
    pub fn minimum_window_size(self) -> (f32, f32) {
        let (rows, cols) = self.dimensions();
        let gap = 4.0;

        // IconRail (56px) is the minimum left chrome; drawer is optional.
        let icon_rail_width = 56.0;
        let min_width = icon_rail_width
            + (ABSOLUTE_MIN_CELL_WIDTH * cols as f32)
            + (gap * (cols - 1) as f32);

        let min_height = TOP_BAR_HEIGHT
            + (ABSOLUTE_MIN_CELL_HEIGHT * rows as f32)
            + (gap * (rows - 1) as f32);

        (min_width, min_height)
    }

    /// Get the grid dimensions (rows, cols) for this layout.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::layout::LayoutProfile;
    ///
    /// assert_eq!(LayoutProfile::Grid2x3.dimensions(), (2, 3));
    ///
    /// let custom = LayoutProfile::custom(4, 3).unwrap();
    /// assert_eq!(custom.dimensions(), (4, 3));
    /// ```
    pub fn dimensions(self) -> (u32, u32) {
        match self {
            LayoutProfile::Grid2x2 => (2, 2),
            LayoutProfile::Stack1x4 => (4, 1),
            LayoutProfile::Grid2x3 => (2, 3),
            LayoutProfile::Grid3x3 => (3, 3),
            LayoutProfile::Single => (1, 1),
            LayoutProfile::Custom { rows, cols } => (rows, cols),
        }
    }

    /// Try to create a profile from a LayoutMode.
    ///
    /// Returns a predefined profile if the mode matches one, otherwise
    /// returns a Custom profile for valid Grid modes, or None for
    /// Custom LayoutModes (which use session-specific positioning).
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::layout::LayoutProfile;
    /// use codirigent_core::LayoutMode;
    ///
    /// let mode = LayoutMode::Grid { rows: 2, cols: 2 };
    /// assert_eq!(LayoutProfile::from_mode(&mode), Some(LayoutProfile::Grid2x2));
    ///
    /// let mode = LayoutMode::Single;
    /// assert_eq!(LayoutProfile::from_mode(&mode), Some(LayoutProfile::Single));
    ///
    /// // Non-predefined grids become Custom
    /// let mode = LayoutMode::Grid { rows: 4, cols: 3 };
    /// let profile = LayoutProfile::from_mode(&mode).unwrap();
    /// assert!(profile.is_custom());
    /// assert_eq!(profile.dimensions(), (4, 3));
    /// ```
    pub fn from_mode(mode: &LayoutMode) -> Option<Self> {
        match mode {
            LayoutMode::Grid { rows: 2, cols: 2 } => Some(LayoutProfile::Grid2x2),
            LayoutMode::Grid { rows: 4, cols: 1 } => Some(LayoutProfile::Stack1x4),
            LayoutMode::Grid { rows: 2, cols: 3 } => Some(LayoutProfile::Grid2x3),
            LayoutMode::Grid { rows: 3, cols: 3 } => Some(LayoutProfile::Grid3x3),
            LayoutMode::Single => Some(LayoutProfile::Single),
            LayoutMode::Grid { rows, cols } => LayoutProfile::custom(*rows, *cols),
            LayoutMode::Custom { .. } => None, // Custom positions not supported
        }
    }
}

/// Grid layout calculator for positioning session cells.
///
/// This struct calculates cell positions and sizes based on the layout mode
/// and available workspace bounds.
///
/// # Example
///
/// ```
/// use codirigent_ui::layout::{GridLayout, Bounds};
/// use codirigent_core::LayoutMode;
///
/// let bounds = Bounds::from_size(1000.0, 800.0);
/// let layout = GridLayout::new(
///     LayoutMode::Grid { rows: 2, cols: 2 },
///     bounds,
///     4.0,
/// );
///
/// assert_eq!(layout.cell_count(), 4);
/// let cell = layout.cell_bounds(0, 0);
/// assert!(cell.size.width > 0.0);
/// ```
#[derive(Debug, Clone)]
pub struct GridLayout {
    /// Number of rows in the grid.
    rows: u32,
    /// Number of columns in the grid.
    cols: u32,
    /// Total bounds available for the grid.
    bounds: Bounds,
    /// Gap between cells in pixels.
    gap: f32,
}

impl GridLayout {
    /// Create a new grid layout.
    ///
    /// # Arguments
    ///
    /// * `mode` - The layout mode (Grid, Single, or Custom)
    /// * `bounds` - The available bounds for the entire grid
    /// * `gap` - Gap between cells in pixels
    pub fn new(mode: LayoutMode, bounds: Bounds, gap: f32) -> Self {
        let (rows, cols) = match mode {
            LayoutMode::Grid { rows, cols } => (rows, cols),
            LayoutMode::Single => (1, 1),
            LayoutMode::Custom { ref positions } => {
                // Calculate bounds from positions
                let max_row = positions.iter().map(|(_, p)| p.row).max().unwrap_or(0);
                let max_col = positions.iter().map(|(_, p)| p.col).max().unwrap_or(0);
                (max_row + 1, max_col + 1)
            }
        };

        Self {
            rows,
            cols,
            bounds,
            gap,
        }
    }

    /// Create a grid layout from a profile.
    ///
    /// # Arguments
    ///
    /// * `profile` - The layout profile
    /// * `bounds` - The available bounds for the entire grid
    /// * `gap` - Gap between cells in pixels
    pub fn from_profile(profile: LayoutProfile, bounds: Bounds, gap: f32) -> Self {
        Self::new(profile.to_mode(), bounds, gap)
    }

    /// Calculate the size of a single cell.
    ///
    /// The cell size is calculated by dividing the available space
    /// (after subtracting gaps) evenly among all cells.
    ///
    /// Enforces absolute minimum cell dimensions and warns if below recommended sizes.
    /// This allows the layout to be flexible while still providing feedback about
    /// potentially cramped terminal displays.
    pub fn cell_size(&self) -> Size {
        if self.rows == 0 || self.cols == 0 {
            return Size::zero();
        }

        let total_gap_x = self.gap * (self.cols.saturating_sub(1) as f32);
        let total_gap_y = self.gap * (self.rows.saturating_sub(1) as f32);

        let calculated_width = (self.bounds.size.width - total_gap_x) / self.cols as f32;
        let calculated_height = (self.bounds.size.height - total_gap_y) / self.rows as f32;

        // Enforce absolute minimums only (hard constraint)
        let cell_width = calculated_width.max(ABSOLUTE_MIN_CELL_WIDTH);
        let cell_height = calculated_height.max(ABSOLUTE_MIN_CELL_HEIGHT);

        // Log warnings if below recommended sizes (soft constraint)
        if cell_width < RECOMMENDED_MIN_CELL_WIDTH {
            tracing::warn!(
                "Terminal cell width ({:.0}px) is below recommended minimum ({:.0}px). \
                 Display may be cramped. Consider using a smaller grid or larger window.",
                cell_width,
                RECOMMENDED_MIN_CELL_WIDTH
            );
        }
        if cell_height < RECOMMENDED_MIN_CELL_HEIGHT {
            tracing::warn!(
                "Terminal cell height ({:.0}px) is below recommended minimum ({:.0}px). \
                 Display may be cramped. Consider using a smaller grid or larger window.",
                cell_height,
                RECOMMENDED_MIN_CELL_HEIGHT
            );
        }

        Size::new(cell_width, cell_height)
    }

    /// Calculate the bounds for a cell at the given row and column.
    ///
    /// # Arguments
    ///
    /// * `row` - Row index (0-based)
    /// * `col` - Column index (0-based)
    pub fn cell_bounds(&self, row: u32, col: u32) -> Bounds {
        let cell_size = self.cell_size();

        let x = self.bounds.origin.x + (cell_size.width + self.gap) * col as f32;
        let y = self.bounds.origin.y + (cell_size.height + self.gap) * row as f32;

        Bounds::from_origin_size(Point::new(x, y), cell_size)
    }

    /// Calculate cell bounds for a session at the given index.
    ///
    /// Sessions are laid out left-to-right, top-to-bottom.
    ///
    /// # Returns
    ///
    /// `None` if the index is out of bounds for this grid.
    pub fn cell_bounds_for_index(&self, index: usize) -> Option<Bounds> {
        let position = self.index_to_position(index)?;
        Some(self.cell_bounds(position.row, position.col))
    }

    /// Convert a linear index to a grid position.
    ///
    /// Sessions are indexed left-to-right, top-to-bottom.
    ///
    /// # Returns
    ///
    /// `None` if the index is out of bounds for this grid.
    pub fn index_to_position(&self, index: usize) -> Option<GridPosition> {
        let max_cells = self.cell_count();
        if index >= max_cells {
            return None;
        }

        let row = (index / self.cols as usize) as u32;
        let col = (index % self.cols as usize) as u32;

        Some(GridPosition { row, col })
    }

    /// Convert a grid position to a linear index.
    ///
    /// # Returns
    ///
    /// `None` if the position is out of bounds for this grid.
    pub fn position_to_index(&self, position: GridPosition) -> Option<usize> {
        if position.row >= self.rows || position.col >= self.cols {
            return None;
        }

        Some((position.row * self.cols + position.col) as usize)
    }

    /// Get the total number of cells in this grid.
    pub fn cell_count(&self) -> usize {
        (self.rows * self.cols) as usize
    }

    /// Get the grid dimensions (rows, cols).
    pub fn dimensions(&self) -> (u32, u32) {
        (self.rows, self.cols)
    }

    /// Get the gap between cells.
    pub fn gap(&self) -> f32 {
        self.gap
    }

    /// Get the total bounds of the grid.
    pub fn bounds(&self) -> Bounds {
        self.bounds
    }

    /// Find which cell contains a point.
    ///
    /// # Returns
    ///
    /// The grid position of the cell containing the point, or `None`
    /// if the point is outside all cells (e.g., in a gap).
    pub fn cell_at_point(&self, point: Point) -> Option<GridPosition> {
        for row in 0..self.rows {
            for col in 0..self.cols {
                let cell = self.cell_bounds(row, col);
                if cell.contains(point) {
                    return Some(GridPosition { row, col });
                }
            }
        }
        None
    }

    /// Update the bounds of this layout.
    ///
    /// This is useful when the window is resized.
    pub fn update_bounds(&mut self, bounds: Bounds) {
        self.bounds = bounds;
    }

    /// Update the gap between cells.
    pub fn update_gap(&mut self, gap: f32) {
        self.gap = gap;
    }
}

/// Layout state manager for workspace.
///
/// Tracks the current layout profile and provides session-to-cell mapping.
#[derive(Debug, Clone)]
pub struct LayoutState {
    /// Current layout profile.
    profile: LayoutProfile,
    /// Session assignments to grid positions.
    assignments: Vec<SessionId>,
    /// Currently focused session index.
    focused_index: Option<usize>,
}

impl Default for LayoutState {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutState {
    /// Create a new layout state with default profile.
    pub fn new() -> Self {
        Self {
            profile: LayoutProfile::default(),
            assignments: Vec::new(),
            focused_index: None,
        }
    }

    /// Create a new layout state with a specific profile.
    pub fn with_profile(profile: LayoutProfile) -> Self {
        Self {
            profile,
            assignments: Vec::new(),
            focused_index: None,
        }
    }

    /// Get the current layout profile.
    pub fn profile(&self) -> LayoutProfile {
        self.profile
    }

    /// Set the layout profile.
    pub fn set_profile(&mut self, profile: LayoutProfile) {
        self.profile = profile;
    }

    /// Cycle to the next layout profile.
    pub fn next_profile(&mut self) {
        self.profile = self.profile.next();
    }

    /// Cycle to the previous layout profile.
    pub fn previous_profile(&mut self) {
        self.profile = self.profile.previous();
    }

    /// Get the session assignments.
    pub fn assignments(&self) -> &[SessionId] {
        &self.assignments
    }

    /// Set the session assignments.
    pub fn set_assignments(&mut self, assignments: Vec<SessionId>) {
        self.assignments = assignments;
    }

    /// Add a session to the layout.
    ///
    /// The session is added to the next available slot.
    ///
    /// # Returns
    ///
    /// `true` if the session was added, `false` if the layout is full.
    pub fn add_session(&mut self, session_id: SessionId) -> bool {
        if self.assignments.len() < self.profile.max_sessions() {
            self.assignments.push(session_id);
            true
        } else {
            false
        }
    }

    /// Remove a session from the layout.
    ///
    /// # Returns
    ///
    /// `true` if the session was removed, `false` if not found.
    pub fn remove_session(&mut self, session_id: SessionId) -> bool {
        if let Some(pos) = self.assignments.iter().position(|&id| id == session_id) {
            self.assignments.remove(pos);
            // Adjust focused index if needed
            if let Some(focused) = self.focused_index {
                if focused == pos {
                    self.focused_index = self.assignments.first().map(|_| 0);
                } else if focused > pos {
                    self.focused_index = Some(focused - 1);
                }
            }
            true
        } else {
            false
        }
    }

    /// Get the session at a given index.
    pub fn session_at(&self, index: usize) -> Option<SessionId> {
        self.assignments.get(index).copied()
    }

    /// Get the session at a given grid position.
    pub fn session_at_position(&self, position: GridPosition) -> Option<SessionId> {
        let (rows, cols) = self.profile.dimensions();
        if position.row >= rows || position.col >= cols {
            return None;
        }
        let index = (position.row * cols + position.col) as usize;
        self.session_at(index)
    }

    /// Get the focused session index.
    pub fn focused_index(&self) -> Option<usize> {
        self.focused_index
    }

    /// Get the focused session ID.
    pub fn focused_session(&self) -> Option<SessionId> {
        self.focused_index.and_then(|i| self.session_at(i))
    }

    /// Set the focused session by index.
    pub fn focus_index(&mut self, index: usize) {
        if index < self.assignments.len() {
            self.focused_index = Some(index);
        }
    }

    /// Set the focused session by ID.
    ///
    /// # Returns
    ///
    /// `true` if the session was found and focused.
    pub fn focus_session(&mut self, session_id: SessionId) -> bool {
        if let Some(index) = self.assignments.iter().position(|&id| id == session_id) {
            self.focused_index = Some(index);
            true
        } else {
            false
        }
    }

    /// Focus the next session in the layout.
    pub fn focus_next(&mut self) {
        if self.assignments.is_empty() {
            return;
        }
        let next = match self.focused_index {
            Some(i) => (i + 1) % self.assignments.len(),
            None => 0,
        };
        self.focused_index = Some(next);
    }

    /// Focus the previous session in the layout.
    pub fn focus_previous(&mut self) {
        if self.assignments.is_empty() {
            return;
        }
        let prev = match self.focused_index {
            Some(i) if i > 0 => i - 1,
            Some(_) => self.assignments.len() - 1,
            None => 0,
        };
        self.focused_index = Some(prev);
    }

    /// Focus the session in the given direction from current focus.
    ///
    /// # Arguments
    ///
    /// * `direction` - Direction to move focus (Up, Down, Left, Right)
    pub fn focus_direction(&mut self, direction: FocusDirection) {
        let Some(current_index) = self.focused_index else {
            if !self.assignments.is_empty() {
                self.focused_index = Some(0);
            }
            return;
        };

        let (rows, cols) = self.profile.dimensions();
        let current_row = current_index as u32 / cols;
        let current_col = current_index as u32 % cols;

        let (new_row, new_col) = match direction {
            FocusDirection::Up => {
                if current_row > 0 {
                    (current_row - 1, current_col)
                } else {
                    (rows - 1, current_col)
                }
            }
            FocusDirection::Down => {
                if current_row < rows - 1 {
                    (current_row + 1, current_col)
                } else {
                    (0, current_col)
                }
            }
            FocusDirection::Left => {
                if current_col > 0 {
                    (current_row, current_col - 1)
                } else {
                    (current_row, cols - 1)
                }
            }
            FocusDirection::Right => {
                if current_col < cols - 1 {
                    (current_row, current_col + 1)
                } else {
                    (current_row, 0)
                }
            }
        };

        let new_index = (new_row * cols + new_col) as usize;
        if new_index < self.assignments.len() {
            self.focused_index = Some(new_index);
        }
    }
}

/// Direction for focus navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDirection {
    /// Move focus up.
    Up,
    /// Move focus down.
    Down,
    /// Move focus left.
    Left,
    /// Move focus right.
    Right,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Point tests
    #[test]
    fn test_point_new() {
        let p = Point::new(10.0, 20.0);
        assert_eq!(p.x, 10.0);
        assert_eq!(p.y, 20.0);
    }

    #[test]
    fn test_point_zero() {
        let p = Point::zero();
        assert_eq!(p.x, 0.0);
        assert_eq!(p.y, 0.0);
    }

    // Size tests
    #[test]
    fn test_size_new() {
        let s = Size::new(100.0, 200.0);
        assert_eq!(s.width, 100.0);
        assert_eq!(s.height, 200.0);
    }

    #[test]
    fn test_size_zero() {
        let s = Size::zero();
        assert_eq!(s.width, 0.0);
        assert_eq!(s.height, 0.0);
    }

    // Bounds tests
    #[test]
    fn test_bounds_new() {
        let b = Bounds::new(10.0, 20.0, 100.0, 200.0);
        assert_eq!(b.origin.x, 10.0);
        assert_eq!(b.origin.y, 20.0);
        assert_eq!(b.size.width, 100.0);
        assert_eq!(b.size.height, 200.0);
    }

    #[test]
    fn test_bounds_from_size() {
        let b = Bounds::from_size(100.0, 200.0);
        assert_eq!(b.origin.x, 0.0);
        assert_eq!(b.origin.y, 0.0);
        assert_eq!(b.size.width, 100.0);
        assert_eq!(b.size.height, 200.0);
    }

    #[test]
    fn test_bounds_edges() {
        let b = Bounds::new(10.0, 20.0, 100.0, 200.0);
        assert_eq!(b.right(), 110.0);
        assert_eq!(b.bottom(), 220.0);
    }

    #[test]
    fn test_bounds_contains() {
        let b = Bounds::new(10.0, 20.0, 100.0, 200.0);
        assert!(b.contains(Point::new(50.0, 100.0)));
        assert!(b.contains(Point::new(10.0, 20.0)));
        assert!(b.contains(Point::new(110.0, 220.0)));
        assert!(!b.contains(Point::new(5.0, 100.0)));
        assert!(!b.contains(Point::new(50.0, 250.0)));
    }

    // LayoutProfile tests
    #[test]
    fn test_layout_profile_default() {
        assert_eq!(LayoutProfile::default(), LayoutProfile::Grid2x2);
    }

    #[test]
    fn test_layout_profile_to_mode() {
        assert!(matches!(
            LayoutProfile::Grid2x2.to_mode(),
            LayoutMode::Grid { rows: 2, cols: 2 }
        ));
        assert!(matches!(
            LayoutProfile::Stack1x4.to_mode(),
            LayoutMode::Grid { rows: 4, cols: 1 }
        ));
        assert!(matches!(
            LayoutProfile::Grid2x3.to_mode(),
            LayoutMode::Grid { rows: 2, cols: 3 }
        ));
        assert!(matches!(
            LayoutProfile::Grid3x3.to_mode(),
            LayoutMode::Grid { rows: 3, cols: 3 }
        ));
        assert!(matches!(
            LayoutProfile::Single.to_mode(),
            LayoutMode::Single
        ));
    }

    #[test]
    fn test_layout_profile_next() {
        let mut profile = LayoutProfile::Grid2x2;
        profile = profile.next();
        assert_eq!(profile, LayoutProfile::Stack1x4);
        profile = profile.next();
        assert_eq!(profile, LayoutProfile::Grid2x3);
        profile = profile.next();
        assert_eq!(profile, LayoutProfile::Grid3x3);
        profile = profile.next();
        assert_eq!(profile, LayoutProfile::Single);
        profile = profile.next();
        assert_eq!(profile, LayoutProfile::Grid2x2); // wrap around
    }

    #[test]
    fn test_layout_profile_previous() {
        let mut profile = LayoutProfile::Grid2x2;
        profile = profile.previous();
        assert_eq!(profile, LayoutProfile::Single);
        profile = profile.previous();
        assert_eq!(profile, LayoutProfile::Grid3x3);
    }

    #[test]
    fn test_layout_profile_display_name() {
        assert_eq!(LayoutProfile::Grid2x2.display_name(), "2x2");
        assert_eq!(LayoutProfile::Stack1x4.display_name(), "1x4");
        assert_eq!(LayoutProfile::Grid2x3.display_name(), "2x3");
        assert_eq!(LayoutProfile::Grid3x3.display_name(), "3x3");
        assert_eq!(LayoutProfile::Single.display_name(), "Single");

        // Custom layout display name
        let custom = LayoutProfile::custom(4, 3).unwrap();
        assert_eq!(custom.display_name(), "4x3");
    }

    #[test]
    fn test_layout_profile_max_sessions() {
        assert_eq!(LayoutProfile::Grid2x2.max_sessions(), 4);
        assert_eq!(LayoutProfile::Stack1x4.max_sessions(), 4);
        assert_eq!(LayoutProfile::Grid2x3.max_sessions(), 6);
        assert_eq!(LayoutProfile::Grid3x3.max_sessions(), 9);
        assert_eq!(LayoutProfile::Single.max_sessions(), 1);
    }

    #[test]
    fn test_layout_profile_dimensions() {
        assert_eq!(LayoutProfile::Grid2x2.dimensions(), (2, 2));
        assert_eq!(LayoutProfile::Stack1x4.dimensions(), (4, 1));
        assert_eq!(LayoutProfile::Grid2x3.dimensions(), (2, 3));
        assert_eq!(LayoutProfile::Grid3x3.dimensions(), (3, 3));
        assert_eq!(LayoutProfile::Single.dimensions(), (1, 1));
    }

    #[test]
    fn test_layout_profile_from_mode() {
        assert_eq!(
            LayoutProfile::from_mode(&LayoutMode::Grid { rows: 2, cols: 2 }),
            Some(LayoutProfile::Grid2x2)
        );
        assert_eq!(
            LayoutProfile::from_mode(&LayoutMode::Single),
            Some(LayoutProfile::Single)
        );

        // Non-predefined grid becomes Custom
        let profile = LayoutProfile::from_mode(&LayoutMode::Grid { rows: 5, cols: 5 }).unwrap();
        assert!(profile.is_custom());
        assert_eq!(profile.dimensions(), (5, 5));

        // Invalid dimensions return None
        assert_eq!(
            LayoutProfile::from_mode(&LayoutMode::Grid { rows: 11, cols: 5 }),
            None
        );
    }

    // Custom layout tests
    #[test]
    fn test_layout_profile_custom_creation() {
        // Valid custom layouts
        let custom = LayoutProfile::custom(4, 3).unwrap();
        assert!(custom.is_custom());
        assert_eq!(custom.dimensions(), (4, 3));
        assert_eq!(custom.max_sessions(), 12);

        // Boundary values
        assert!(LayoutProfile::custom(1, 1).is_some());
        assert!(LayoutProfile::custom(10, 10).is_some());

        // Invalid dimensions
        assert!(LayoutProfile::custom(0, 3).is_none());
        assert!(LayoutProfile::custom(3, 0).is_none());
        assert!(LayoutProfile::custom(11, 3).is_none());
        assert!(LayoutProfile::custom(3, 11).is_none());
    }

    #[test]
    fn test_layout_profile_custom_to_mode() {
        let custom = LayoutProfile::custom(4, 3).unwrap();
        let mode = custom.to_mode();
        assert!(matches!(mode, LayoutMode::Grid { rows: 4, cols: 3 }));
    }

    #[test]
    fn test_layout_profile_custom_cycling() {
        let custom = LayoutProfile::custom(4, 3).unwrap();

        // Custom cycles to Grid2x2 on next
        assert_eq!(custom.next(), LayoutProfile::Grid2x2);

        // Custom cycles to Single on previous
        assert_eq!(custom.previous(), LayoutProfile::Single);
    }

    #[test]
    fn test_layout_profile_is_custom() {
        assert!(!LayoutProfile::Grid2x2.is_custom());
        assert!(!LayoutProfile::Single.is_custom());
        assert!(LayoutProfile::custom(4, 3).unwrap().is_custom());
    }

    // GridLayout tests
    fn test_bounds() -> Bounds {
        Bounds::from_size(1000.0, 800.0)
    }

    #[test]
    fn test_grid_layout_new_grid() {
        let layout = GridLayout::new(LayoutMode::Grid { rows: 2, cols: 2 }, test_bounds(), 4.0);
        assert_eq!(layout.dimensions(), (2, 2));
        assert_eq!(layout.cell_count(), 4);
        assert_eq!(layout.gap(), 4.0);
    }

    #[test]
    fn test_grid_layout_new_single() {
        let layout = GridLayout::new(LayoutMode::Single, test_bounds(), 4.0);
        assert_eq!(layout.dimensions(), (1, 1));
        assert_eq!(layout.cell_count(), 1);
    }

    #[test]
    fn test_grid_layout_new_custom() {
        let positions = vec![
            (SessionId(1), GridPosition { row: 0, col: 0 }),
            (SessionId(2), GridPosition { row: 0, col: 2 }),
            (SessionId(3), GridPosition { row: 1, col: 1 }),
        ];
        let layout = GridLayout::new(LayoutMode::Custom { positions }, test_bounds(), 4.0);
        assert_eq!(layout.dimensions(), (2, 3));
    }

    #[test]
    fn test_grid_layout_from_profile() {
        let layout = GridLayout::from_profile(LayoutProfile::Grid3x3, test_bounds(), 4.0);
        assert_eq!(layout.dimensions(), (3, 3));
        assert_eq!(layout.cell_count(), 9);
    }

    #[test]
    fn test_grid_layout_cell_size_2x2() {
        let layout = GridLayout::new(LayoutMode::Grid { rows: 2, cols: 2 }, test_bounds(), 4.0);
        let size = layout.cell_size();
        // (1000 - 4) / 2 = 498
        assert!((size.width - 498.0).abs() < 0.01);
        // (800 - 4) / 2 = 398
        assert!((size.height - 398.0).abs() < 0.01);
    }

    #[test]
    fn test_grid_layout_cell_size_single() {
        let layout = GridLayout::new(LayoutMode::Single, test_bounds(), 4.0);
        let size = layout.cell_size();
        assert_eq!(size.width, 1000.0);
        assert_eq!(size.height, 800.0);
    }

    #[test]
    fn test_grid_layout_cell_bounds() {
        let layout = GridLayout::new(LayoutMode::Grid { rows: 2, cols: 2 }, test_bounds(), 4.0);

        let cell00 = layout.cell_bounds(0, 0);
        assert_eq!(cell00.origin.x, 0.0);
        assert_eq!(cell00.origin.y, 0.0);

        let cell01 = layout.cell_bounds(0, 1);
        assert!((cell01.origin.x - 502.0).abs() < 0.01); // 498 + 4

        let cell10 = layout.cell_bounds(1, 0);
        assert_eq!(cell10.origin.x, 0.0);
        assert!((cell10.origin.y - 402.0).abs() < 0.01); // 398 + 4
    }

    #[test]
    fn test_grid_layout_index_to_position() {
        let layout = GridLayout::new(LayoutMode::Grid { rows: 2, cols: 3 }, test_bounds(), 4.0);

        assert_eq!(
            layout.index_to_position(0),
            Some(GridPosition { row: 0, col: 0 })
        );
        assert_eq!(
            layout.index_to_position(2),
            Some(GridPosition { row: 0, col: 2 })
        );
        assert_eq!(
            layout.index_to_position(3),
            Some(GridPosition { row: 1, col: 0 })
        );
        assert_eq!(
            layout.index_to_position(5),
            Some(GridPosition { row: 1, col: 2 })
        );
        assert_eq!(layout.index_to_position(6), None);
    }

    #[test]
    fn test_grid_layout_position_to_index() {
        let layout = GridLayout::new(LayoutMode::Grid { rows: 2, cols: 3 }, test_bounds(), 4.0);

        assert_eq!(
            layout.position_to_index(GridPosition { row: 0, col: 0 }),
            Some(0)
        );
        assert_eq!(
            layout.position_to_index(GridPosition { row: 0, col: 2 }),
            Some(2)
        );
        assert_eq!(
            layout.position_to_index(GridPosition { row: 1, col: 0 }),
            Some(3)
        );
        assert_eq!(
            layout.position_to_index(GridPosition { row: 2, col: 0 }),
            None
        );
    }

    #[test]
    fn test_grid_layout_cell_bounds_for_index() {
        let layout = GridLayout::new(LayoutMode::Grid { rows: 2, cols: 2 }, test_bounds(), 4.0);

        let cell = layout.cell_bounds_for_index(0).unwrap();
        assert_eq!(cell.origin.x, 0.0);
        assert_eq!(cell.origin.y, 0.0);

        let cell = layout.cell_bounds_for_index(3).unwrap();
        assert!(cell.origin.x > 0.0);
        assert!(cell.origin.y > 0.0);

        assert!(layout.cell_bounds_for_index(4).is_none());
    }

    #[test]
    fn test_grid_layout_cell_at_point() {
        let layout = GridLayout::new(LayoutMode::Grid { rows: 2, cols: 2 }, test_bounds(), 4.0);

        // Point in cell (0, 0)
        let pos = layout.cell_at_point(Point::new(100.0, 100.0));
        assert_eq!(pos, Some(GridPosition { row: 0, col: 0 }));

        // Point in cell (0, 1)
        let pos = layout.cell_at_point(Point::new(600.0, 100.0));
        assert_eq!(pos, Some(GridPosition { row: 0, col: 1 }));

        // Point in cell (1, 0)
        let pos = layout.cell_at_point(Point::new(100.0, 500.0));
        assert_eq!(pos, Some(GridPosition { row: 1, col: 0 }));

        // Point in gap between cells
        let cell_size = layout.cell_size();
        let gap_point = Point::new(cell_size.width + 2.0, 100.0);
        let pos = layout.cell_at_point(gap_point);
        // This should be in the gap, so no cell
        assert!(pos.is_none() || pos == Some(GridPosition { row: 0, col: 0 }));
    }

    #[test]
    fn test_grid_layout_update_bounds() {
        let mut layout = GridLayout::new(LayoutMode::Grid { rows: 2, cols: 2 }, test_bounds(), 4.0);
        let new_bounds = Bounds::from_size(2000.0, 1600.0);
        layout.update_bounds(new_bounds);
        assert_eq!(layout.bounds().size.width, 2000.0);
    }

    #[test]
    fn test_grid_layout_update_gap() {
        let mut layout = GridLayout::new(LayoutMode::Grid { rows: 2, cols: 2 }, test_bounds(), 4.0);
        layout.update_gap(8.0);
        assert_eq!(layout.gap(), 8.0);
    }

    // LayoutState tests
    #[test]
    fn test_layout_state_new() {
        let state = LayoutState::new();
        assert_eq!(state.profile(), LayoutProfile::Grid2x2);
        assert!(state.assignments().is_empty());
        assert!(state.focused_index().is_none());
    }

    #[test]
    fn test_layout_state_with_profile() {
        let state = LayoutState::with_profile(LayoutProfile::Grid3x3);
        assert_eq!(state.profile(), LayoutProfile::Grid3x3);
    }

    #[test]
    fn test_layout_state_set_profile() {
        let mut state = LayoutState::new();
        state.set_profile(LayoutProfile::Single);
        assert_eq!(state.profile(), LayoutProfile::Single);
    }

    #[test]
    fn test_layout_state_next_previous_profile() {
        let mut state = LayoutState::new();
        state.next_profile();
        assert_eq!(state.profile(), LayoutProfile::Stack1x4);
        state.previous_profile();
        assert_eq!(state.profile(), LayoutProfile::Grid2x2);
    }

    #[test]
    fn test_layout_state_add_session() {
        let mut state = LayoutState::with_profile(LayoutProfile::Grid2x2);
        assert!(state.add_session(SessionId(1)));
        assert!(state.add_session(SessionId(2)));
        assert!(state.add_session(SessionId(3)));
        assert!(state.add_session(SessionId(4)));
        assert!(!state.add_session(SessionId(5))); // Full

        assert_eq!(state.assignments().len(), 4);
    }

    #[test]
    fn test_layout_state_remove_session() {
        let mut state = LayoutState::new();
        state.add_session(SessionId(1));
        state.add_session(SessionId(2));
        state.focus_index(1);

        assert!(state.remove_session(SessionId(1)));
        assert_eq!(state.assignments().len(), 1);
        assert_eq!(state.focused_index(), Some(0)); // Adjusted

        assert!(!state.remove_session(SessionId(99))); // Not found
    }

    #[test]
    fn test_layout_state_session_at() {
        let mut state = LayoutState::new();
        state.add_session(SessionId(1));
        state.add_session(SessionId(2));

        assert_eq!(state.session_at(0), Some(SessionId(1)));
        assert_eq!(state.session_at(1), Some(SessionId(2)));
        assert_eq!(state.session_at(2), None);
    }

    #[test]
    fn test_layout_state_session_at_position() {
        let mut state = LayoutState::with_profile(LayoutProfile::Grid2x2);
        state.add_session(SessionId(1));
        state.add_session(SessionId(2));
        state.add_session(SessionId(3));

        assert_eq!(
            state.session_at_position(GridPosition { row: 0, col: 0 }),
            Some(SessionId(1))
        );
        assert_eq!(
            state.session_at_position(GridPosition { row: 0, col: 1 }),
            Some(SessionId(2))
        );
        assert_eq!(
            state.session_at_position(GridPosition { row: 1, col: 0 }),
            Some(SessionId(3))
        );
        assert_eq!(
            state.session_at_position(GridPosition { row: 1, col: 1 }),
            None
        );
    }

    #[test]
    fn test_layout_state_focus_index() {
        let mut state = LayoutState::new();
        state.add_session(SessionId(1));
        state.add_session(SessionId(2));

        state.focus_index(0);
        assert_eq!(state.focused_index(), Some(0));
        assert_eq!(state.focused_session(), Some(SessionId(1)));

        state.focus_index(1);
        assert_eq!(state.focused_index(), Some(1));
        assert_eq!(state.focused_session(), Some(SessionId(2)));

        // Out of bounds does nothing
        state.focus_index(10);
        assert_eq!(state.focused_index(), Some(1));
    }

    #[test]
    fn test_layout_state_focus_session() {
        let mut state = LayoutState::new();
        state.add_session(SessionId(1));
        state.add_session(SessionId(2));

        assert!(state.focus_session(SessionId(2)));
        assert_eq!(state.focused_index(), Some(1));

        assert!(!state.focus_session(SessionId(99)));
        assert_eq!(state.focused_index(), Some(1)); // Unchanged
    }

    #[test]
    fn test_layout_state_focus_next() {
        let mut state = LayoutState::new();
        state.add_session(SessionId(1));
        state.add_session(SessionId(2));
        state.add_session(SessionId(3));

        // No focus initially
        state.focus_next();
        assert_eq!(state.focused_index(), Some(0));

        state.focus_next();
        assert_eq!(state.focused_index(), Some(1));

        state.focus_next();
        assert_eq!(state.focused_index(), Some(2));

        // Wrap around
        state.focus_next();
        assert_eq!(state.focused_index(), Some(0));
    }

    #[test]
    fn test_layout_state_focus_previous() {
        let mut state = LayoutState::new();
        state.add_session(SessionId(1));
        state.add_session(SessionId(2));
        state.add_session(SessionId(3));

        state.focus_index(0);
        state.focus_previous();
        assert_eq!(state.focused_index(), Some(2)); // Wrap around
    }

    #[test]
    fn test_layout_state_focus_direction_2x2() {
        let mut state = LayoutState::with_profile(LayoutProfile::Grid2x2);
        state.add_session(SessionId(1));
        state.add_session(SessionId(2));
        state.add_session(SessionId(3));
        state.add_session(SessionId(4));
        state.focus_index(0); // Top-left

        // Move right
        state.focus_direction(FocusDirection::Right);
        assert_eq!(state.focused_index(), Some(1));

        // Move down
        state.focus_direction(FocusDirection::Down);
        assert_eq!(state.focused_index(), Some(3));

        // Move left
        state.focus_direction(FocusDirection::Left);
        assert_eq!(state.focused_index(), Some(2));

        // Move up
        state.focus_direction(FocusDirection::Up);
        assert_eq!(state.focused_index(), Some(0));
    }

    #[test]
    fn test_layout_state_focus_direction_wraps() {
        let mut state = LayoutState::with_profile(LayoutProfile::Grid2x2);
        state.add_session(SessionId(1));
        state.add_session(SessionId(2));
        state.add_session(SessionId(3));
        state.add_session(SessionId(4));

        // Start at top-left, go up (wraps to bottom)
        state.focus_index(0);
        state.focus_direction(FocusDirection::Up);
        assert_eq!(state.focused_index(), Some(2));

        // Start at top-left, go left (wraps to right)
        state.focus_index(0);
        state.focus_direction(FocusDirection::Left);
        assert_eq!(state.focused_index(), Some(1));
    }

    #[test]
    fn test_focus_direction_equality() {
        assert_eq!(FocusDirection::Up, FocusDirection::Up);
        assert_ne!(FocusDirection::Up, FocusDirection::Down);
    }

    #[test]
    fn test_grid_layout_zero_dimensions() {
        let layout = GridLayout::new(LayoutMode::Grid { rows: 0, cols: 0 }, test_bounds(), 4.0);
        let size = layout.cell_size();
        assert_eq!(size.width, 0.0);
        assert_eq!(size.height, 0.0);
    }

    #[test]
    fn test_layout_state_empty_focus_operations() {
        let mut state = LayoutState::new();

        // Focus operations on empty state should do nothing
        state.focus_next();
        assert!(state.focused_index().is_none());

        state.focus_previous();
        assert!(state.focused_index().is_none());

        state.focus_direction(FocusDirection::Up);
        assert!(state.focused_index().is_none());
    }

    #[test]
    fn test_layout_state_focus_direction_no_current_focus() {
        let mut state = LayoutState::new();
        state.add_session(SessionId(1));
        state.add_session(SessionId(2));

        // No focus, should set to first session
        state.focus_direction(FocusDirection::Right);
        assert_eq!(state.focused_index(), Some(0));
    }

    // Soft minimums tests
    #[test]
    fn test_cell_size_allows_below_recommended() {
        // Simulate 3x3 grid on a tight but realistic window
        // Use a window size where 3x3 results in cells below recommended but above absolute
        // Example: 1200×900 window with 3x3 grid
        // IconRail = 56px, TopBar = 48px
        let available_height = 900.0 - TOP_BAR_HEIGHT;
        let available_width = 1200.0 - 56.0; // icon rail
        let bounds = Bounds::from_size(available_width, available_height);
        let layout = GridLayout::from_profile(LayoutProfile::Grid3x3, bounds, 4.0);

        let size = layout.cell_size();

        // Calculate expected: (900 - 48 - 8) / 3 = 844 / 3 = ~281px height
        // This is below 300px recommended but above 150px absolute

        assert!(
            size.height >= ABSOLUTE_MIN_CELL_HEIGHT,
            "Height {} should be >= absolute min {}",
            size.height,
            ABSOLUTE_MIN_CELL_HEIGHT
        );
        assert!(
            size.height < RECOMMENDED_MIN_CELL_HEIGHT,
            "Height {} should be < recommended min {}",
            size.height,
            RECOMMENDED_MIN_CELL_HEIGHT
        );

        // Width should also be below recommended for this tight scenario
        // (1200 - 56 - 8) / 3 = 1136 / 3 = ~378px width
        assert!(
            size.width >= ABSOLUTE_MIN_CELL_WIDTH,
            "Width {} should be >= absolute min {}",
            size.width,
            ABSOLUTE_MIN_CELL_WIDTH
        );
        assert!(
            size.width < RECOMMENDED_MIN_CELL_WIDTH,
            "Width {} should be < recommended min {}",
            size.width,
            RECOMMENDED_MIN_CELL_WIDTH
        );
    }

    #[test]
    fn test_cell_size_enforces_absolute_minimum() {
        // Very small window that would result in tiny cells
        let bounds = Bounds::from_size(600.0, 450.0);
        let layout = GridLayout::from_profile(LayoutProfile::Grid3x3, bounds, 4.0);

        let size = layout.cell_size();

        // Should enforce absolute minimums
        assert!(
            size.width >= ABSOLUTE_MIN_CELL_WIDTH,
            "Width {} should be >= absolute min {}",
            size.width,
            ABSOLUTE_MIN_CELL_WIDTH
        );
        assert!(
            size.height >= ABSOLUTE_MIN_CELL_HEIGHT,
            "Height {} should be >= absolute min {}",
            size.height,
            ABSOLUTE_MIN_CELL_HEIGHT
        );

        // Will be at the minimums since window is too small
        assert_eq!(size.width, ABSOLUTE_MIN_CELL_WIDTH);
        assert_eq!(size.height, ABSOLUTE_MIN_CELL_HEIGHT);
    }

    #[test]
    fn test_cell_size_comfortable_on_large_monitor() {
        // 4K monitor (3840×2160) with 2x2 grid should be very comfortable
        let _available_height = 2160.0 - TOP_BAR_HEIGHT;
        let available_width = 3840.0 - 56.0; // icon rail
        // Note: Using width for both dimensions to ensure very large cells
        let bounds = Bounds::from_size(available_width, available_width);
        let layout = GridLayout::from_profile(LayoutProfile::Grid2x2, bounds, 4.0);

        let size = layout.cell_size();

        // Should be well above recommended minimums
        assert!(size.width > RECOMMENDED_MIN_CELL_WIDTH * 2.0);
        assert!(size.height > RECOMMENDED_MIN_CELL_HEIGHT * 2.0);
    }

    #[test]
    fn test_minimum_window_size_uses_absolute() {
        let (width, height) = LayoutProfile::Grid3x3.minimum_window_size();

        // Should use absolute minimums (200×150) with icon rail (56px)
        let icon_rail_width = 56.0;
        let expected_width =
            icon_rail_width + (ABSOLUTE_MIN_CELL_WIDTH * 3.0) + (4.0 * 2.0);
        let expected_height = TOP_BAR_HEIGHT
            + (ABSOLUTE_MIN_CELL_HEIGHT * 3.0)
            + (4.0 * 2.0);

        assert!((width - expected_width).abs() < 0.01);
        assert!((height - expected_height).abs() < 0.01);

        // 3x3 at absolute minimum should fit on even small laptops
        assert!(width < 1000.0);
        assert!(height < 600.0);
    }

    #[test]
    fn test_recommended_window_size_uses_recommended() {
        let (width, height) = LayoutProfile::Grid3x3.recommended_window_size();

        // Should use recommended minimums (400×300) with icon rail (56px)
        let icon_rail_width = 56.0;
        let expected_width =
            icon_rail_width + (RECOMMENDED_MIN_CELL_WIDTH * 3.0) + (4.0 * 2.0);
        let expected_height = TOP_BAR_HEIGHT
            + (RECOMMENDED_MIN_CELL_HEIGHT * 3.0)
            + (4.0 * 2.0);

        assert!((width - expected_width).abs() < 0.01);
        assert!((height - expected_height).abs() < 0.01);

        // 3x3 at recommended should need ~1264×956 window (smaller than before due to less chrome)
        assert!(width > 1200.0);
        assert!(height > 900.0);
    }

    #[test]
    fn test_single_layout_comfortable_at_any_size() {
        // Even at small window, single layout should be >= recommended
        let bounds = Bounds::from_size(800.0, 600.0);
        let layout = GridLayout::from_profile(LayoutProfile::Single, bounds, 4.0);

        let size = layout.cell_size();

        assert!(size.width >= RECOMMENDED_MIN_CELL_WIDTH);
        assert!(size.height >= RECOMMENDED_MIN_CELL_HEIGHT);
    }

    #[test]
    fn test_dynamic_gap_sizing() {
        // Test that smaller gap helps when space is tight
        let tight_bounds = Bounds::from_size(1300.0, 1000.0);

        // With 4px gap
        let layout_4px = GridLayout::from_profile(LayoutProfile::Grid3x3, tight_bounds, 4.0);
        let size_4px = layout_4px.cell_size();

        // With 2px gap (more space for cells)
        let layout_2px = GridLayout::from_profile(LayoutProfile::Grid3x3, tight_bounds, 2.0);
        let size_2px = layout_2px.cell_size();

        // Smaller gap should give slightly larger cells
        assert!(size_2px.width > size_4px.width);
        assert!(size_2px.height > size_4px.height);
    }

    #[test]
    fn test_cell_size_with_zero_gap() {
        let bounds = Bounds::from_size(1000.0, 800.0);
        let layout = GridLayout::new(LayoutMode::Grid { rows: 2, cols: 2 }, bounds, 0.0);

        let size = layout.cell_size();

        // With no gap, should get exactly 1/2 of bounds
        assert_eq!(size.width, 500.0);
        assert_eq!(size.height, 400.0);
    }

    #[test]
    fn test_constants_are_consistent() {
        // Absolute minimums should be less than recommended
        assert!(ABSOLUTE_MIN_CELL_WIDTH < RECOMMENDED_MIN_CELL_WIDTH);
        assert!(ABSOLUTE_MIN_CELL_HEIGHT < RECOMMENDED_MIN_CELL_HEIGHT);

        // Absolute minimums should be reasonable (not too small)
        assert!(ABSOLUTE_MIN_CELL_WIDTH >= 100.0);
        assert!(ABSOLUTE_MIN_CELL_HEIGHT >= 100.0);

        // Recommended minimums should be reasonable
        assert!(RECOMMENDED_MIN_CELL_WIDTH >= 300.0);
        assert!(RECOMMENDED_MIN_CELL_HEIGHT >= 200.0);
    }
}
