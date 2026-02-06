//! Terminal view component for rendering terminal content.
//!
//! This module provides the `TerminalView` component that renders terminal cells,
//! cursor, and selection using GPUI. It integrates with alacritty_terminal's
//! `RenderableContent` to efficiently display terminal output.
//!
//! # Architecture
//!
//! The terminal view consists of:
//! - Cell rendering with proper color conversion
//! - Cursor rendering with multiple styles (block, beam, underline)
//! - Text selection support
//! - Scrollback buffer navigation
//!
//! # Performance
//!
//! The view is optimized for 120fps rendering through:
//! - Dirty state tracking (only render when content changes)
//! - Batched cell rendering (group cells by style)
//! - Efficient color conversion
//!
//! # Example
//!
//! ```rust,ignore
//! use codirigent_ui::{Terminal, TerminalView, CodirigentTheme};
//! use codirigent_core::SessionId;
//!
//! let terminal = Terminal::new(24, 80, SessionId(1));
//! let theme = CodirigentTheme::dark();
//! let mut view = TerminalView::new(terminal, theme);
//!
//! // Process output and render
//! view.terminal_mut().process_output(b"Hello, World!");
//! ```

use crate::terminal::Terminal;
use crate::terminal::TerminalSize;
use crate::terminal_colors::{convert_color, dim_color};
use crate::theme::{CodirigentTheme, Rgba};
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::term::cell::Flags as CellFlags;
use alacritty_terminal::vte::ansi::{Color as TermColor, NamedColor};
#[allow(unused_imports)]
use codirigent_core::SessionId;

/// Line height multiplier applied to font metrics (ascent + |descent|).
///
/// GPUI's Win metrics already include extra ascent space for accented
/// characters, so the base glyph height (ascent + |descent|) is larger
/// than font_size. A 1.0x factor gives natural terminal line spacing.
/// If lines appear too tight, increase slightly (e.g. 1.05).
const TERMINAL_LINE_HEIGHT_FACTOR: f32 = 1.0;

/// A run of text with uniform style for efficient canvas painting.
#[derive(Debug, Clone)]
pub struct TextRunSegment {
    /// Concatenated characters in this run.
    pub text: String,
    /// Foreground color for the run.
    pub foreground: Rgba,
    /// Background color for the run.
    pub background: Rgba,
    /// Whether the run is bold.
    pub bold: bool,
    /// Whether the run is italic.
    pub italic: bool,
    /// Whether the run is underlined.
    pub underline: bool,
    /// Whether the run has strikethrough.
    pub strikethrough: bool,
    /// Row of the run.
    pub row: usize,
    /// Starting column of the run.
    pub start_col: usize,
    /// Number of cells in the run.
    pub cell_count: usize,
}

/// Pre-computed terminal content for canvas rendering.
#[derive(Debug, Clone)]
pub struct CachedTerminalContent {
    /// Background rectangles grouped by color (row, start_col, end_col, color).
    pub background_rects: Vec<(usize, usize, usize, Rgba)>,
    /// Text runs batched by style for efficient painting.
    pub text_runs: Vec<TextRunSegment>,
    /// Terminal rows at time of caching.
    pub rows: usize,
    /// Terminal columns at time of caching.
    pub cols: usize,
}

/// Cursor shape for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorShape {
    /// Filled block cursor (default).
    #[default]
    Block,
    /// Hollow block (unfocused).
    HollowBlock,
    /// Vertical beam cursor.
    Beam,
    /// Horizontal underline cursor.
    Underline,
}

/// Selection state for text selection.
///
/// Tracks the start and end positions of the current selection,
/// if any.
#[derive(Debug, Clone, Default)]
pub struct Selection {
    /// Start position (row, column), if selection is active.
    pub start: Option<(usize, usize)>,
    /// End position (row, column), if selection is active.
    pub end: Option<(usize, usize)>,
}

impl Selection {
    /// Create a new empty selection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if the selection is active (has both start and end).
    pub fn is_active(&self) -> bool {
        self.start.is_some() && self.end.is_some()
    }

    /// Clear the selection.
    pub fn clear(&mut self) {
        self.start = None;
        self.end = None;
    }

    /// Set the selection start position.
    pub fn set_start(&mut self, row: usize, col: usize) {
        self.start = Some((row, col));
    }

    /// Set the selection end position.
    pub fn set_end(&mut self, row: usize, col: usize) {
        self.end = Some((row, col));
    }

    /// Check if a cell position is within the selection.
    ///
    /// Returns `true` if the given (row, column) is selected.
    pub fn contains(&self, row: usize, col: usize) -> bool {
        match (self.start, self.end) {
            (Some(start), Some(end)) => {
                // Normalize so start <= end
                let (start, end) = if start <= end {
                    (start, end)
                } else {
                    (end, start)
                };

                let pos = (row, col);
                pos >= start && pos <= end
            }
            _ => false,
        }
    }

    /// Get the normalized selection range (start <= end).
    ///
    /// Returns `None` if selection is not active.
    pub fn normalized(&self) -> Option<((usize, usize), (usize, usize))> {
        match (self.start, self.end) {
            (Some(start), Some(end)) => {
                if start <= end {
                    Some((start, end))
                } else {
                    Some((end, start))
                }
            }
            _ => None,
        }
    }
}

/// Rendered cell with resolved colors.
///
/// Represents a single terminal cell ready for rendering,
/// with colors resolved from the terminal and theme.
#[derive(Debug, Clone)]
pub struct RenderedCell {
    /// The character to display.
    pub character: char,
    /// Foreground color.
    pub foreground: Rgba,
    /// Background color.
    pub background: Rgba,
    /// Whether the cell is bold.
    pub bold: bool,
    /// Whether the cell is italic.
    pub italic: bool,
    /// Whether the cell is underlined.
    pub underline: bool,
    /// Whether the cell is strikethrough.
    pub strikethrough: bool,
    /// Row position (0-indexed).
    pub row: usize,
    /// Column position (0-indexed).
    pub column: usize,
}

/// Terminal view component.
///
/// Renders terminal content to the screen, handling cells, cursor,
/// and selection display.
pub struct TerminalView {
    /// The underlying terminal emulator.
    terminal: Terminal,
    /// Theme for color resolution.
    theme: CodirigentTheme,
    /// Width of a single character cell in pixels.
    cell_width: f32,
    /// Height of a single character cell in pixels.
    cell_height: f32,
    /// Font size in pixels.
    font_size: f32,
    /// Current text selection.
    selection: Selection,
    /// Cursor shape to render.
    cursor_shape: CursorShape,
    /// Whether the terminal view is focused.
    focused: bool,
    /// Cached terminal content for canvas rendering.
    cached_content: Option<CachedTerminalContent>,
    /// Whether the content needs to be recomputed.
    content_dirty: bool,
    /// Whether cell dimensions have been initialized from font metrics.
    dimensions_initialized: bool,
}

impl TerminalView {
    /// Create a new terminal view.
    pub fn new(terminal: Terminal, theme: CodirigentTheme) -> Self {
        let font_size = theme.terminal_font_size;
        // Approximate cell dimensions until real font metrics arrive via
        // compute_cell_dimensions() on first render. Using conservative
        // ratios that slightly overestimate so the initial grid doesn't
        // allocate more rows/cols than will fit after correction.
        let cell_width = (font_size * 0.55).max(7.0);
        let cell_height = (font_size * TERMINAL_LINE_HEIGHT_FACTOR).max(14.0);

        let mut terminal = terminal;
        terminal.resize_with_cells(TerminalSize::new(
            terminal.rows(),
            terminal.cols(),
            cell_width,
            cell_height,
        ));

        Self {
            terminal,
            theme,
            cell_width,
            cell_height,
            font_size,
            selection: Selection::new(),
            cursor_shape: CursorShape::Block,
            focused: true,
            cached_content: None,
            content_dirty: true,
            dimensions_initialized: false,
        }
    }

    /// Get a reference to the terminal.
    pub fn terminal(&self) -> &Terminal {
        &self.terminal
    }

    /// Get a mutable reference to the terminal.
    ///
    /// Marks content as dirty since callers typically modify terminal state.
    pub fn terminal_mut(&mut self) -> &mut Terminal {
        self.content_dirty = true;
        &mut self.terminal
    }

    /// Get the theme.
    pub fn theme(&self) -> &CodirigentTheme {
        &self.theme
    }

    /// Set the theme.
    pub fn set_theme(&mut self, theme: CodirigentTheme) {
        self.theme = theme;
        self.content_dirty = true;
    }

    /// Get the cell width in pixels.
    pub fn cell_width(&self) -> f32 {
        self.cell_width
    }

    /// Get the cell height in pixels.
    pub fn cell_height(&self) -> f32 {
        self.cell_height
    }

    /// Set the cell dimensions.
    ///
    /// Marks the view as having initialized dimensions and triggers a
    /// terminal resize with the new cell metrics.
    pub fn set_cell_dimensions(&mut self, width: f32, height: f32) {
        self.cell_width = width;
        self.cell_height = height;
        self.dimensions_initialized = true;
        self.content_dirty = true;
        self.terminal.resize_with_cells(TerminalSize::new(
            self.terminal.rows(),
            self.terminal.cols(),
            width,
            height,
        ));
    }

    /// Check if cell dimensions have been initialized from font metrics.
    pub fn dimensions_initialized(&self) -> bool {
        self.dimensions_initialized
    }

    /// Get the font size.
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Set the font size.
    pub fn set_font_size(&mut self, size: f32) {
        self.font_size = size;
        self.content_dirty = true;
    }

    /// Get the current selection.
    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    /// Get mutable access to the selection.
    pub fn selection_mut(&mut self) -> &mut Selection {
        &mut self.selection
    }

    /// Set the cursor shape.
    pub fn set_cursor_shape(&mut self, shape: CursorShape) {
        self.cursor_shape = shape;
    }

    /// Get the cursor shape.
    pub fn cursor_shape(&self) -> CursorShape {
        self.cursor_shape
    }

    /// Set the focused state.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Check if the view is focused.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Get the session ID.
    pub fn session_id(&self) -> SessionId {
        self.terminal.session_id()
    }

    /// Scroll up by the specified number of lines.
    pub fn scroll_up(&mut self, lines: usize) {
        self.content_dirty = true;
        self.terminal
            .term_mut()
            .scroll_display(Scroll::Delta(-(lines as i32)));
    }

    /// Scroll down by the specified number of lines.
    pub fn scroll_down(&mut self, lines: usize) {
        self.content_dirty = true;
        self.terminal
            .term_mut()
            .scroll_display(Scroll::Delta(lines as i32));
    }

    /// Scroll to the bottom (most recent output).
    pub fn scroll_to_bottom(&mut self) {
        self.content_dirty = true;
        self.terminal.term_mut().scroll_display(Scroll::Bottom);
    }

    /// Scroll to the top (oldest output in scrollback).
    pub fn scroll_to_top(&mut self) {
        self.content_dirty = true;
        self.terminal.term_mut().scroll_display(Scroll::Top);
    }

    /// Scroll up by one page (viewport height).
    pub fn page_up(&mut self) {
        let lines = self.terminal.rows() as usize;
        self.scroll_up(lines);
    }

    /// Scroll down by one page (viewport height).
    pub fn page_down(&mut self) {
        let lines = self.terminal.rows() as usize;
        self.scroll_down(lines);
    }

    /// Clear the terminal screen while preserving the current line (prompt).
    ///
    /// This clears the scrollback and visible content while keeping
    /// the current line (typically the shell prompt) at the top.
    pub fn clear(&mut self) {
        self.content_dirty = true;
        self.terminal.clear();
    }

    /// Get all visible cells for rendering.
    pub fn visible_cells(&self) -> Vec<RenderedCell> {
        let content = self.terminal.term().renderable_content();
        let display_offset = content.display_offset;
        let rows = self.terminal.rows() as usize;
        let mut cells = Vec::new();

        for indexed in content.display_iter {
            let cell = &indexed.cell;
            let point = indexed.point;

            // Convert grid-relative line to viewport-relative row.
            // Grid lines are negative for scrollback, 0+ for active screen.
            // Viewport row = grid_line + display_offset.
            let viewport_line = point.line.0 + display_offset as i32;
            if viewport_line < 0 || viewport_line as usize >= rows {
                continue;
            }
            let row = viewport_line as usize;
            let col = point.column.0;

            // Skip empty cells (optimization)
            let c = cell.c;
            if c == ' ' && cell.bg == TermColor::Named(NamedColor::Background) {
                continue;
            }

            // Resolve colors
            let mut foreground = convert_color(cell.fg, &self.theme, true);
            let mut background = convert_color(cell.bg, &self.theme, false);

            // Handle inverse/reverse video
            if cell.flags.contains(CellFlags::INVERSE) {
                std::mem::swap(&mut foreground, &mut background);
            }

            // Handle selection (invert colors if selected)
            if self.selection.contains(row, col) {
                foreground = self.theme.terminal_selection_fg;
                background = self.theme.terminal_selection_bg;
            }

            // Handle dim attribute
            if cell.flags.contains(CellFlags::DIM) {
                foreground = dim_color(foreground);
            }

            cells.push(RenderedCell {
                character: c,
                foreground,
                background,
                bold: cell.flags.contains(CellFlags::BOLD),
                italic: cell.flags.contains(CellFlags::ITALIC),
                underline: cell.flags.contains(CellFlags::UNDERLINE),
                strikethrough: cell.flags.contains(CellFlags::STRIKEOUT),
                row,
                column: col,
            });
        }

        cells
    }

    /// Get cells grouped by row for efficient rendering.
    pub fn cells_by_row(&self) -> Vec<Vec<RenderedCell>> {
        let cells = self.visible_cells();
        let rows = self.terminal.rows() as usize;

        let mut by_row: Vec<Vec<RenderedCell>> = vec![Vec::new(); rows];

        for cell in cells {
            if cell.row < rows {
                by_row[cell.row].push(cell);
            }
        }

        // Sort each row by column
        for row in &mut by_row {
            row.sort_by_key(|c| c.column);
        }

        by_row
    }

    /// Get cursor rendering information.
    ///
    /// Returns `None` if the cursor is not visible or is scrolled off-screen.
    /// Uses viewport-relative coordinates from `renderable_content()` so the
    /// cursor position is correct when the terminal is scrolled.
    pub fn cursor_rect(&self) -> Option<CursorRect> {
        if !self.terminal.cursor_visible() {
            return None;
        }

        let content = self.terminal.term().renderable_content();
        let display_offset = content.display_offset;
        let cursor_point = content.cursor.point;

        // Convert grid-relative cursor line to viewport-relative row.
        let viewport_line = cursor_point.line.0 + display_offset as i32;
        let rows = self.terminal.rows() as usize;
        if viewport_line < 0 || viewport_line as usize >= rows {
            return None; // Cursor is off-screen
        }

        let row = viewport_line as usize;
        let col = cursor_point.column.0;
        let x = col as f32 * self.cell_width;
        let y = row as f32 * self.cell_height;

        let shape = if self.focused {
            self.cursor_shape
        } else {
            CursorShape::HollowBlock
        };

        Some(CursorRect {
            x,
            y,
            width: self.cell_width,
            height: self.cell_height,
            color: self.theme.terminal_cursor,
            shape,
        })
    }

    /// Calculate pixel dimensions for the current terminal size.
    pub fn pixel_size(&self) -> (f32, f32) {
        let width = self.terminal.cols() as f32 * self.cell_width;
        let height = self.terminal.rows() as f32 * self.cell_height;
        (width, height)
    }

    /// Calculate terminal dimensions from pixel size.
    pub fn dimensions_from_pixels(&self, width: f32, height: f32) -> (u16, u16) {
        let cols = (width / self.cell_width).floor() as u16;
        let rows = (height / self.cell_height).floor() as u16;
        (rows.max(1), cols.max(1))
    }

    /// Resize the terminal to fit within the given pixel dimensions.
    pub fn resize_to_fit(&mut self, width: f32, height: f32) {
        let (rows, cols) = self.dimensions_from_pixels(width, height);
        self.content_dirty = true;
        self.terminal.resize(rows, cols);
    }

    /// Start a new text selection at the given cell position.
    ///
    /// Sets the selection start, clears any previous end, and marks dirty.
    pub fn start_selection(&mut self, row: usize, col: usize) {
        self.selection.set_start(row, col);
        self.selection.end = None;
        self.content_dirty = true;
    }

    /// Update the selection end position during a drag.
    ///
    /// Sets the selection end and marks dirty for re-rendering.
    pub fn update_selection(&mut self, row: usize, col: usize) {
        self.selection.set_end(row, col);
        self.content_dirty = true;
    }

    /// End the selection (no-op — selection stays active until explicitly cleared).
    pub fn end_selection(&mut self) {
        // Selection remains active until cleared by next click or copy
    }

    /// Clear the current selection.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.content_dirty = true;
    }

    /// Get the currently selected text, if any.
    ///
    /// Returns `None` if no selection is active. Extracts text from the
    /// terminal grid between the normalized selection start and end.
    pub fn get_selected_text(&self) -> Option<String> {
        let (start, end) = self.selection.normalized()?;
        let text = crate::clipboard::copy_selection(self.terminal.term(), start, end);
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    /// Mark content as dirty, forcing recomputation on next access.
    pub fn mark_dirty(&mut self) {
        self.content_dirty = true;
    }

    /// Check if content is dirty (needs recomputation).
    pub fn is_dirty(&self) -> bool {
        self.content_dirty
    }

    /// Get cached terminal content, recomputing only if dirty.
    ///
    /// Returns pre-computed text runs and background rects for efficient
    /// canvas-based rendering.
    pub fn cached_content(&mut self) -> &CachedTerminalContent {
        if self.content_dirty || self.cached_content.is_none() {
            let cells = self.visible_cells();
            let rows = self.terminal.rows() as usize;
            let cols = self.terminal.cols() as usize;
            let content = Self::build_cached_content(cells, rows, cols, &self.theme);
            self.cached_content = Some(content);
            self.content_dirty = false;
        }
        self.cached_content.as_ref().unwrap()
    }

    /// Build cached content from visible cells.
    ///
    /// Groups cells into text runs (adjacent cells with same style) and
    /// background rectangles for efficient canvas painting.
    fn build_cached_content(
        cells: Vec<RenderedCell>,
        rows: usize,
        cols: usize,
        theme: &CodirigentTheme,
    ) -> CachedTerminalContent {
        let mut text_runs = Vec::new();
        let mut background_rects = Vec::new();

        // Group cells by row
        let mut by_row: Vec<Vec<&RenderedCell>> = vec![Vec::new(); rows];
        for cell in &cells {
            if cell.row < rows {
                by_row[cell.row].push(cell);
            }
        }

        for (row_idx, row_cells) in by_row.iter_mut().enumerate() {
            row_cells.sort_by_key(|c| c.column);

            // Build text runs: merge adjacent cells with same styling
            let mut current_run: Option<TextRunSegment> = None;

            for cell in row_cells.iter() {
                let same_style = current_run.as_ref().is_some_and(|run| {
                    run.foreground == cell.foreground
                        && run.bold == cell.bold
                        && run.italic == cell.italic
                        && run.underline == cell.underline
                        && run.strikethrough == cell.strikethrough
                        && run.start_col + run.cell_count == cell.column
                });

                if same_style {
                    let run = current_run.as_mut().unwrap();
                    run.text.push(cell.character);
                    run.cell_count += 1;
                } else {
                    if let Some(run) = current_run.take() {
                        text_runs.push(run);
                    }
                    current_run = Some(TextRunSegment {
                        text: String::from(cell.character),
                        foreground: cell.foreground,
                        background: cell.background,
                        bold: cell.bold,
                        italic: cell.italic,
                        underline: cell.underline,
                        strikethrough: cell.strikethrough,
                        row: row_idx,
                        start_col: cell.column,
                        cell_count: 1,
                    });
                }

                // Collect non-default background rects
                if cell.background != theme.terminal_background {
                    // Check if we can merge with the previous background rect
                    let merged = background_rects.last_mut().and_then(
                        |last: &mut (usize, usize, usize, Rgba)| {
                            if last.0 == row_idx && last.2 == cell.column && last.3 == cell.background {
                                last.2 = cell.column + 1;
                                Some(())
                            } else {
                                None
                            }
                        },
                    );
                    if merged.is_none() {
                        background_rects.push((row_idx, cell.column, cell.column + 1, cell.background));
                    }
                }
            }

            if let Some(run) = current_run.take() {
                text_runs.push(run);
            }
        }

        CachedTerminalContent {
            background_rects,
            text_runs,
            rows,
            cols,
        }
    }

    /// Convert pixel coordinates to terminal cell position.
    pub fn pixel_to_cell(&self, x: f32, y: f32) -> Option<(usize, usize)> {
        if x < 0.0 || y < 0.0 {
            return None;
        }

        let col = (x / self.cell_width).floor() as usize;
        let row = (y / self.cell_height).floor() as usize;

        let max_row = self.terminal.rows() as usize;
        let max_col = self.terminal.cols() as usize;

        if row < max_row && col < max_col {
            Some((row, col))
        } else {
            None
        }
    }
}

/// Default monospace font family for terminals per platform.
pub fn default_terminal_font_family() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "Consolas"
    }
    #[cfg(target_os = "macos")]
    {
        "Menlo"
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        "DejaVu Sans Mono"
    }
    #[cfg(not(any(windows, unix)))]
    {
        "monospace"
    }
}

/// Compute cell dimensions from actual font metrics using the text system.
///
/// Uses `text_system.advance('m')` to get the actual character width and
/// `(ascent + descent) * TERMINAL_LINE_HEIGHT_FACTOR` to get the line height
/// with comfortable inter-line spacing for the monospace font.
///
/// Returns `(cell_width, cell_height)` in pixels.
pub fn compute_cell_dimensions(
    text_system: &gpui::TextSystem,
    font_family: &'static str,
    font_size: f32,
) -> (f32, f32) {
    use gpui::{Font, FontFeatures, FontStyle, FontWeight, px};

    let font = Font {
        family: font_family.into(),
        features: FontFeatures::default(),
        fallbacks: None,
        weight: FontWeight::NORMAL,
        style: FontStyle::Normal,
    };

    let font_id = text_system.resolve_font(&font);
    let font_size_px = px(font_size);

    let cell_width = text_system
        .advance(font_id, font_size_px, 'm')
        .map(|adv| f32::from(adv.width))
        .unwrap_or(font_size * 0.6);

    // Use font ascent + |descent| as the base line height, then apply a
    // leading factor for readability. GPUI returns descent as a negative
    // value (below baseline), so we use abs() to get the true glyph height.
    // Without the leading factor, rows are packed with zero inter-line space.
    // The previous 1.3x factor on font_size caused double-spacing.
    // TERMINAL_LINE_HEIGHT_FACTOR (1.0x) gives natural terminal spacing
    // since GPUI's ascent already includes room for accented characters.
    let ascent: f32 = text_system.ascent(font_id, font_size_px).into();
    let descent: f32 = text_system.descent(font_id, font_size_px).into();
    let glyph_height = ascent + descent.abs();
    let cell_height = glyph_height * TERMINAL_LINE_HEIGHT_FACTOR;

    (cell_width, cell_height)
}

/// Cursor rendering information.
#[derive(Debug, Clone, Copy)]
pub struct CursorRect {
    /// X position in pixels.
    pub x: f32,
    /// Y position in pixels.
    pub y: f32,
    /// Width in pixels.
    pub width: f32,
    /// Height in pixels.
    pub height: f32,
    /// Cursor color.
    pub color: Rgba,
    /// Cursor shape.
    pub shape: CursorShape,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_colors::brighten_color;

    fn create_test_view() -> TerminalView {
        let terminal = Terminal::new(24, 80, SessionId(1));
        let theme = CodirigentTheme::dark();
        TerminalView::new(terminal, theme)
    }

    #[test]
    fn test_selection_new() {
        let selection = Selection::new();
        assert!(!selection.is_active());
        assert_eq!(selection.start, None);
        assert_eq!(selection.end, None);
    }

    #[test]
    fn test_selection_set_and_clear() {
        let mut selection = Selection::new();
        selection.set_start(5, 10);
        selection.set_end(10, 20);
        assert!(selection.is_active());
        selection.clear();
        assert!(!selection.is_active());
    }

    #[test]
    fn test_selection_contains() {
        let mut selection = Selection::new();
        selection.set_start(5, 0);
        selection.set_end(10, 80);

        assert!(selection.contains(5, 0));
        assert!(selection.contains(7, 40));
        assert!(selection.contains(10, 80));
        assert!(!selection.contains(4, 0));
        assert!(!selection.contains(11, 0));
    }

    #[test]
    fn test_selection_contains_reversed() {
        let mut selection = Selection::new();
        selection.set_start(10, 80);
        selection.set_end(5, 0);
        assert!(selection.contains(5, 0));
        assert!(selection.contains(7, 40));
    }

    #[test]
    fn test_selection_normalized() {
        let mut selection = Selection::new();
        selection.set_start(10, 80);
        selection.set_end(5, 0);

        let normalized = selection.normalized();
        assert!(normalized.is_some());
        let ((sr, sc), (er, ec)) = normalized.unwrap();
        assert_eq!((sr, sc), (5, 0));
        assert_eq!((er, ec), (10, 80));
    }

    #[test]
    fn test_terminal_view_creation() {
        let view = create_test_view();
        assert_eq!(view.terminal().rows(), 24);
        assert_eq!(view.terminal().cols(), 80);
        assert!(view.is_focused());
    }

    #[test]
    fn test_terminal_view_cell_dimensions() {
        let mut view = create_test_view();
        view.set_cell_dimensions(10.0, 20.0);
        assert_eq!(view.cell_width(), 10.0);
        assert_eq!(view.cell_height(), 20.0);
    }

    #[test]
    fn test_terminal_view_font_size() {
        let mut view = create_test_view();
        view.set_font_size(16.0);
        assert_eq!(view.font_size(), 16.0);
    }

    #[test]
    fn test_terminal_view_cursor_shape() {
        let mut view = create_test_view();
        assert_eq!(view.cursor_shape(), CursorShape::Block);
        view.set_cursor_shape(CursorShape::Beam);
        assert_eq!(view.cursor_shape(), CursorShape::Beam);
    }

    #[test]
    fn test_terminal_view_focused() {
        let mut view = create_test_view();
        assert!(view.is_focused());
        view.set_focused(false);
        assert!(!view.is_focused());
    }

    #[test]
    fn test_pixel_size() {
        let view = create_test_view();
        let (width, height) = view.pixel_size();
        assert_eq!(width, view.cell_width() * view.terminal().cols() as f32);
        assert_eq!(height, view.cell_height() * view.terminal().rows() as f32);
    }

    #[test]
    fn test_dimensions_from_pixels() {
        let view = create_test_view();
        let (rows, cols) = view.dimensions_from_pixels(800.0, 600.0);
        assert_eq!(cols, (800.0 / view.cell_width()).floor() as u16);
        assert_eq!(rows, (600.0 / view.cell_height()).floor() as u16);
    }

    #[test]
    fn test_pixel_to_cell() {
        let view = create_test_view();
        assert_eq!(view.pixel_to_cell(40.0, 32.0), Some((2, 5)));
        assert_eq!(view.pixel_to_cell(-1.0, 0.0), None);
        assert_eq!(view.pixel_to_cell(1000.0, 1000.0), None);
    }

    #[test]
    fn test_cursor_rect_unfocused() {
        let mut view = create_test_view();
        view.set_focused(false);
        let cursor = view.cursor_rect();
        assert!(cursor.is_some());
        assert_eq!(cursor.unwrap().shape, CursorShape::HollowBlock);
    }

    #[test]
    fn test_cursor_rect_focused() {
        let view = create_test_view();
        let cursor = view.cursor_rect();
        assert!(cursor.is_some());
        assert_eq!(cursor.unwrap().shape, CursorShape::Block);
    }

    #[test]
    fn test_visible_cells_empty() {
        let view = create_test_view();
        let cells = view.visible_cells();
        assert!(cells.is_empty());
    }

    #[test]
    fn test_visible_cells_with_content() {
        let mut view = create_test_view();
        view.terminal_mut().process_output(b"Hello");
        let cells = view.visible_cells();
        assert!(!cells.is_empty());
        let h_cell = cells.iter().find(|c| c.character == 'H');
        assert!(h_cell.is_some());
    }

    #[test]
    fn test_cells_by_row() {
        let mut view = create_test_view();
        view.terminal_mut().process_output(b"Line 1\nLine 2");
        let by_row = view.cells_by_row();
        assert_eq!(by_row.len(), 24);
        assert!(!by_row[0].is_empty());
    }

    #[test]
    fn test_resize_to_fit() {
        let mut view = create_test_view();
        view.resize_to_fit(400.0, 200.0);
        assert_eq!(view.terminal().cols(), (400.0 / view.cell_width()).floor() as u16);
        assert_eq!(view.terminal().rows(), (200.0 / view.cell_height()).floor() as u16);
    }

    #[test]
    fn test_color_functions() {
        let original = Rgba::rgb(100, 100, 100);
        let dimmed = dim_color(original);
        let brightened = brighten_color(original);
        assert_eq!(dimmed.r, 70);
        assert_eq!(brightened.r, 120);
    }

    #[test]
    fn test_visible_cells_consecutive_rows() {
        let mut view = create_test_view();
        // Simulate multi-line output with Windows-style \r\n endings
        // This mimics what ConPTY sends for a simple dir/ls listing
        view.terminal_mut().process_output(
            b"file1.txt\x1b[K\r\nfile2.txt\x1b[K\r\nfile3.txt\x1b[K\r\nfile4.txt\x1b[K\r\n"
        );
        let cells = view.visible_cells();

        // Collect unique sorted row indices
        let mut rows: Vec<usize> = cells.iter().map(|c| c.row).collect();
        rows.sort();
        rows.dedup();

        println!("Content rows: {:?}", rows);
        assert!(rows.len() >= 4, "Expected at least 4 content rows, got {:?}", rows);

        // Verify rows are consecutive (no gaps)
        for i in 1..rows.len() {
            assert_eq!(
                rows[i], rows[i - 1] + 1,
                "Rows must be consecutive but found gap: {:?}", rows
            );
        }
    }

    #[test]
    fn test_start_selection() {
        let mut view = create_test_view();
        view.start_selection(5, 10);
        assert!(view.selection().start.is_some());
        assert_eq!(view.selection().start.unwrap(), (5, 10));
        assert!(view.selection().end.is_none());
    }

    #[test]
    fn test_update_selection() {
        let mut view = create_test_view();
        view.start_selection(5, 10);
        view.update_selection(10, 20);
        assert!(view.selection().is_active());
        assert_eq!(view.selection().end.unwrap(), (10, 20));
    }

    #[test]
    fn test_clear_selection() {
        let mut view = create_test_view();
        view.start_selection(5, 10);
        view.update_selection(10, 20);
        assert!(view.selection().is_active());
        view.clear_selection();
        assert!(!view.selection().is_active());
    }

    #[test]
    fn test_end_selection_keeps_active() {
        let mut view = create_test_view();
        view.start_selection(5, 10);
        view.update_selection(10, 20);
        view.end_selection();
        // Selection should remain active after end_selection
        assert!(view.selection().is_active());
    }

    #[test]
    fn test_get_selected_text_no_selection() {
        let view = create_test_view();
        assert!(view.get_selected_text().is_none());
    }

    #[test]
    fn test_get_selected_text_with_content() {
        let mut view = create_test_view();
        view.terminal_mut().process_output(b"Hello, World!");
        view.start_selection(0, 0);
        view.update_selection(0, 4);
        let text = view.get_selected_text();
        assert!(text.is_some());
        assert_eq!(text.unwrap(), "Hello");
    }

    #[test]
    fn test_get_selected_text_empty_region() {
        let view = create_test_view();
        // No content rendered, so even with selection, text should be empty/None
        let mut view = view;
        view.start_selection(0, 0);
        view.update_selection(0, 0);
        // Single cell with no content = None (empty string filtered)
        let text = view.get_selected_text();
        assert!(text.is_none());
    }

    #[test]
    fn test_visible_cells_row_indices_with_ansi() {
        let mut view = create_test_view();
        // More realistic ConPTY output with ANSI sequences
        view.terminal_mut().process_output(
            b"\x1b[?25l\x1b[2J\x1b[H\
            Row0 text here\x1b[K\r\n\
            Row1 text here\x1b[K\r\n\
            Row2 text here\x1b[K\r\n\
            Row3 text here\x1b[K\r\n\
            Row4 text here\x1b[K\r\n\
            \x1b[?25h"
        );
        let cells = view.visible_cells();

        let mut rows: Vec<usize> = cells.iter().map(|c| c.row).collect();
        rows.sort();
        rows.dedup();

        println!("ANSI content rows: {:?}", rows);
        assert!(rows.len() >= 5, "Expected at least 5 content rows, got {:?}", rows);

        for i in 1..rows.len() {
            assert_eq!(
                rows[i], rows[i - 1] + 1,
                "Rows must be consecutive with ANSI output: {:?}", rows
            );
        }
    }
}
