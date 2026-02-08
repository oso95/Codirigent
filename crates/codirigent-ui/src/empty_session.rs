//! Empty session cell component.
//!
//! Displays a placeholder for grid cells that don't have an active session,
//! with a dashed border, plus icon, and "Idle - Ready for next task" text.

use crate::sidebar::Color;
use codirigent_core::GridPosition;
use std::collections::HashSet;

/// Empty session cell events.
#[derive(Debug, Clone, PartialEq)]
pub enum EmptySessionEvent {
    /// User clicked to create a new session in this cell.
    CreateSessionClicked {
        /// Grid position where the session should be created.
        position: GridPosition,
    },
}

/// Empty session cell component state.
#[derive(Debug)]
pub struct EmptySessionCell {
    /// Grid position of this cell.
    position: GridPosition,
    /// Whether the cell is hovered.
    is_hovered: bool,
    /// Pending events.
    pending_events: Vec<EmptySessionEvent>,
}

impl EmptySessionCell {
    /// Default cell height (matches terminal header height).
    pub const DEFAULT_HEIGHT: f32 = 32.0;
    /// Plus icon character.
    pub const PLUS_ICON: &'static str = "+";
    /// Idle text.
    pub const IDLE_TEXT: &'static str = "Idle - Ready for next task";

    /// Create a new empty session cell.
    pub fn new(position: GridPosition) -> Self {
        Self {
            position,
            is_hovered: false,
            pending_events: Vec::new(),
        }
    }

    /// Get the grid position.
    pub fn position(&self) -> GridPosition {
        self.position
    }

    /// Set the grid position.
    pub fn set_position(&mut self, position: GridPosition) {
        self.position = position;
    }

    /// Is the cell hovered?
    pub fn is_hovered(&self) -> bool {
        self.is_hovered
    }

    /// Set hover state.
    pub fn set_hovered(&mut self, hovered: bool) {
        self.is_hovered = hovered;
    }

    /// Handle click - triggers session creation.
    pub fn click(&mut self) {
        self.pending_events
            .push(EmptySessionEvent::CreateSessionClicked {
                position: self.position,
            });
    }

    /// Take pending events.
    pub fn take_events(&mut self) -> Vec<EmptySessionEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// Get the border style for rendering.
    pub fn border_style(&self) -> EmptySessionBorderStyle {
        EmptySessionBorderStyle {
            color: if self.is_hovered {
                Color::from_hex("#4ECDC4") // Primary teal on hover
            } else {
                Color::from_hex("#1a1a1f") // Border color
            },
            is_dashed: true,
            width: if self.is_hovered { 2.0 } else { 1.0 },
            radius: 8.0,
        }
    }
}

/// Border style configuration for empty session cell.
#[derive(Debug, Clone, Copy)]
pub struct EmptySessionBorderStyle {
    /// Border color.
    pub color: Color,
    /// Whether the border is dashed.
    pub is_dashed: bool,
    /// Border width.
    pub width: f32,
    /// Corner radius.
    pub radius: f32,
}

/// Rendering hints for the empty session cell.
#[derive(Debug, Clone)]
pub struct EmptySessionRenderHints {
    /// Grid position.
    pub position: GridPosition,
    /// Plus icon text.
    pub icon: &'static str,
    /// Idle message text.
    pub message: &'static str,
    /// Whether hovered.
    pub is_hovered: bool,
    /// Border style.
    pub border: EmptySessionBorderStyle,
    /// Background color.
    pub background: Color,
    /// Icon color.
    pub icon_color: Color,
    /// Text color.
    pub text_color: Color,
}

impl EmptySessionCell {
    /// Generate rendering hints.
    pub fn render_hints(&self) -> EmptySessionRenderHints {
        EmptySessionRenderHints {
            position: self.position,
            icon: Self::PLUS_ICON,
            message: Self::IDLE_TEXT,
            is_hovered: self.is_hovered,
            border: self.border_style(),
            background: if self.is_hovered {
                Color::from_hex("#151518") // Hover background
            } else {
                Color::from_hex("#0d0d10") // Panel background
            },
            icon_color: if self.is_hovered {
                Color::from_hex("#4ECDC4") // Primary teal
            } else {
                Color::from_hex("#555555") // Muted
            },
            text_color: Color::from_hex("#666666"), // Muted text
        }
    }
}

/// A pool of empty session cells for the grid.
#[derive(Debug, Default)]
pub struct EmptySessionPool {
    /// Empty cells by position.
    cells: Vec<EmptySessionCell>,
}

impl EmptySessionPool {
    /// Create a new empty pool.
    pub fn new() -> Self {
        Self { cells: Vec::new() }
    }

    /// Set up cells for a grid with given dimensions.
    ///
    /// Creates empty cells for all positions that don't have active sessions.
    pub fn setup_for_grid(&mut self, rows: u32, cols: u32, occupied: &[GridPosition]) {
        let occupied_set: HashSet<_> = occupied.iter().collect();
        self.cells.clear();

        for row in 0..rows {
            for col in 0..cols {
                let pos = GridPosition { row, col };
                if !occupied_set.contains(&pos) {
                    self.cells.push(EmptySessionCell::new(pos));
                }
            }
        }
    }

    /// Get all empty cells.
    pub fn cells(&self) -> &[EmptySessionCell] {
        &self.cells
    }

    /// Get mutable reference to cells.
    pub fn cells_mut(&mut self) -> &mut [EmptySessionCell] {
        &mut self.cells
    }

    /// Get cell at position.
    pub fn get(&self, position: GridPosition) -> Option<&EmptySessionCell> {
        self.cells.iter().find(|c| c.position == position)
    }

    /// Get mutable cell at position.
    pub fn get_mut(&mut self, position: GridPosition) -> Option<&mut EmptySessionCell> {
        self.cells.iter_mut().find(|c| c.position == position)
    }

    /// Set hover state for a position (clears hover on others).
    pub fn set_hovered(&mut self, position: Option<GridPosition>) {
        for cell in &mut self.cells {
            cell.is_hovered = position == Some(cell.position);
        }
    }

    /// Handle click at position.
    pub fn click(&mut self, position: GridPosition) {
        if let Some(cell) = self.get_mut(position) {
            cell.click();
        }
    }

    /// Take all pending events from all cells.
    pub fn take_events(&mut self) -> Vec<EmptySessionEvent> {
        self.cells
            .iter_mut()
            .flat_map(|c| c.take_events())
            .collect()
    }

    /// Check if a position is empty.
    pub fn is_empty_at(&self, position: GridPosition) -> bool {
        self.cells.iter().any(|c| c.position == position)
    }

    /// Get count of empty cells.
    pub fn count(&self) -> usize {
        self.cells.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_session_cell_new() {
        let pos = GridPosition { row: 0, col: 0 };
        let cell = EmptySessionCell::new(pos);
        assert_eq!(cell.position(), pos);
        assert!(!cell.is_hovered());
    }

    #[test]
    fn test_set_position() {
        let mut cell = EmptySessionCell::new(GridPosition { row: 0, col: 0 });
        cell.set_position(GridPosition { row: 1, col: 2 });
        assert_eq!(cell.position(), GridPosition { row: 1, col: 2 });
    }

    #[test]
    fn test_hover_state() {
        let mut cell = EmptySessionCell::new(GridPosition { row: 0, col: 0 });
        assert!(!cell.is_hovered());

        cell.set_hovered(true);
        assert!(cell.is_hovered());

        cell.set_hovered(false);
        assert!(!cell.is_hovered());
    }

    #[test]
    fn test_click_creates_event() {
        let pos = GridPosition { row: 1, col: 2 };
        let mut cell = EmptySessionCell::new(pos);
        cell.click();

        let events = cell.take_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            EmptySessionEvent::CreateSessionClicked { position } if *position == pos
        ));
    }

    #[test]
    fn test_take_events_clears() {
        let mut cell = EmptySessionCell::new(GridPosition { row: 0, col: 0 });
        cell.click();
        cell.click();

        let events = cell.take_events();
        assert_eq!(events.len(), 2);

        let events2 = cell.take_events();
        assert!(events2.is_empty());
    }

    #[test]
    fn test_border_style_normal() {
        let cell = EmptySessionCell::new(GridPosition { row: 0, col: 0 });
        let style = cell.border_style();

        assert!(style.is_dashed);
        assert_eq!(style.width, 1.0);
        assert_eq!(style.radius, 8.0);
    }

    #[test]
    fn test_border_style_hovered() {
        let mut cell = EmptySessionCell::new(GridPosition { row: 0, col: 0 });
        cell.set_hovered(true);
        let style = cell.border_style();

        assert!(style.is_dashed);
        assert_eq!(style.width, 2.0); // Thicker on hover
    }

    #[test]
    fn test_render_hints() {
        let cell = EmptySessionCell::new(GridPosition { row: 0, col: 0 });
        let hints = cell.render_hints();

        assert_eq!(hints.icon, EmptySessionCell::PLUS_ICON);
        assert_eq!(hints.message, EmptySessionCell::IDLE_TEXT);
        assert!(!hints.is_hovered);
    }

    #[test]
    fn test_render_hints_hovered() {
        let mut cell = EmptySessionCell::new(GridPosition { row: 0, col: 0 });
        cell.set_hovered(true);
        let hints = cell.render_hints();

        assert!(hints.is_hovered);
        // Hovered icon should be teal
        assert!(hints.icon_color.g > 0.7);
    }

    #[test]
    fn test_constants() {
        assert_eq!(EmptySessionCell::PLUS_ICON, "+");
        assert_eq!(EmptySessionCell::IDLE_TEXT, "Idle - Ready for next task");
        // Verify DEFAULT_HEIGHT is positive; use a let-binding to avoid
        // clippy::assertions_on_constants (the check guards against future edits).
        let height = EmptySessionCell::DEFAULT_HEIGHT;
        assert!(height > 0.0);
    }

    // === Pool Tests ===

    #[test]
    fn test_pool_new() {
        let pool = EmptySessionPool::new();
        assert_eq!(pool.count(), 0);
    }

    #[test]
    fn test_pool_default() {
        let pool = EmptySessionPool::default();
        assert_eq!(pool.count(), 0);
    }

    #[test]
    fn test_setup_for_grid_all_empty() {
        let mut pool = EmptySessionPool::new();
        pool.setup_for_grid(2, 2, &[]);

        assert_eq!(pool.count(), 4);
    }

    #[test]
    fn test_setup_for_grid_with_occupied() {
        let mut pool = EmptySessionPool::new();
        let occupied = vec![
            GridPosition { row: 0, col: 0 },
            GridPosition { row: 1, col: 1 },
        ];
        pool.setup_for_grid(2, 2, &occupied);

        assert_eq!(pool.count(), 2);
        assert!(pool.is_empty_at(GridPosition { row: 0, col: 1 }));
        assert!(pool.is_empty_at(GridPosition { row: 1, col: 0 }));
        assert!(!pool.is_empty_at(GridPosition { row: 0, col: 0 }));
    }

    #[test]
    fn test_pool_get() {
        let mut pool = EmptySessionPool::new();
        pool.setup_for_grid(2, 2, &[]);

        let cell = pool.get(GridPosition { row: 0, col: 0 });
        assert!(cell.is_some());

        let cell = pool.get(GridPosition { row: 5, col: 5 });
        assert!(cell.is_none());
    }

    #[test]
    fn test_pool_get_mut() {
        let mut pool = EmptySessionPool::new();
        pool.setup_for_grid(2, 2, &[]);

        let cell = pool.get_mut(GridPosition { row: 0, col: 0 });
        assert!(cell.is_some());
        cell.unwrap().set_hovered(true);

        let cell = pool.get(GridPosition { row: 0, col: 0 }).unwrap();
        assert!(cell.is_hovered());
    }

    #[test]
    fn test_pool_set_hovered() {
        let mut pool = EmptySessionPool::new();
        pool.setup_for_grid(2, 2, &[]);

        // Hover one cell
        pool.set_hovered(Some(GridPosition { row: 0, col: 0 }));

        let cells = pool.cells();
        let hovered_count = cells.iter().filter(|c| c.is_hovered()).count();
        assert_eq!(hovered_count, 1);

        // Clear hover
        pool.set_hovered(None);
        let hovered_count = pool.cells().iter().filter(|c| c.is_hovered()).count();
        assert_eq!(hovered_count, 0);
    }

    #[test]
    fn test_pool_click() {
        let mut pool = EmptySessionPool::new();
        pool.setup_for_grid(2, 2, &[]);

        pool.click(GridPosition { row: 0, col: 0 });

        let events = pool.take_events();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_pool_click_invalid_position() {
        let mut pool = EmptySessionPool::new();
        pool.setup_for_grid(2, 2, &[GridPosition { row: 0, col: 0 }]);

        // Click on occupied position (no cell there)
        pool.click(GridPosition { row: 0, col: 0 });

        let events = pool.take_events();
        assert!(events.is_empty());
    }

    #[test]
    fn test_pool_take_events_from_multiple() {
        let mut pool = EmptySessionPool::new();
        pool.setup_for_grid(2, 2, &[]);

        pool.click(GridPosition { row: 0, col: 0 });
        pool.click(GridPosition { row: 1, col: 1 });

        let events = pool.take_events();
        assert_eq!(events.len(), 2);

        // Second take should be empty
        let events2 = pool.take_events();
        assert!(events2.is_empty());
    }

    #[test]
    fn test_pool_cells_access() {
        let mut pool = EmptySessionPool::new();
        pool.setup_for_grid(2, 3, &[]);

        assert_eq!(pool.cells().len(), 6);

        // Mutable access
        for cell in pool.cells_mut() {
            cell.set_hovered(true);
        }

        let all_hovered = pool.cells().iter().all(|c| c.is_hovered());
        assert!(all_hovered);
    }

    #[test]
    fn test_setup_clears_previous() {
        let mut pool = EmptySessionPool::new();
        pool.setup_for_grid(3, 3, &[]);
        assert_eq!(pool.count(), 9);

        // Setup again with different size
        pool.setup_for_grid(2, 2, &[]);
        assert_eq!(pool.count(), 4);
    }

    #[test]
    fn test_is_empty_at() {
        let mut pool = EmptySessionPool::new();
        let occupied = vec![GridPosition { row: 0, col: 0 }];
        pool.setup_for_grid(2, 2, &occupied);

        assert!(!pool.is_empty_at(GridPosition { row: 0, col: 0 }));
        assert!(pool.is_empty_at(GridPosition { row: 0, col: 1 }));
        assert!(pool.is_empty_at(GridPosition { row: 1, col: 0 }));
        assert!(pool.is_empty_at(GridPosition { row: 1, col: 1 }));
    }
}
