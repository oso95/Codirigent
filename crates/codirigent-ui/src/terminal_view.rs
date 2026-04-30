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
//! let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
//! let terminal = Terminal::new(24, 80, SessionId(1), tx);
//! let theme = CodirigentTheme::dark();
//! let mut view = TerminalView::new(terminal, theme);
//!
//! // Simulate output and render
//! view.apply_output_for_test(b"Hello, World!");
//! ```

use std::sync::Arc;
use std::time::Instant;

/// Ratio of font_size used as a conservative initial cell width estimate
/// before real font metrics arrive from `compute_cell_dimensions`.
/// Slightly underestimates typical monospace advance to avoid over-allocating cols.
const APPROX_CELL_WIDTH_RATIO: f32 = 0.55;

/// Ratio of font_size used as fallback cell width when GPUI font advance fails.
const FALLBACK_CELL_WIDTH_RATIO: f32 = 0.6;

/// Minimum initial cell width in pixels to stay sane at tiny font sizes.
const MIN_CELL_WIDTH_PX: f32 = 7.0;

use crate::terminal::Terminal;
use crate::terminal::TerminalSize;
use crate::terminal_runtime::{TerminalRenderSnapshot, TerminalRuntimeHandle};
use crate::terminal_search::SearchMatch;
use crate::theme::{CodirigentTheme, Rgba};
use alacritty_terminal::term::TermMode;
use codirigent_core::SessionId;
use unicode_width::UnicodeWidthChar;

/// A run of text with uniform style for efficient canvas painting.
#[derive(Debug, Clone)]
pub struct TextRunSegment {
    /// Concatenated characters in this run.
    pub text: String,
    /// Foreground color for the run.
    pub foreground: Rgba,
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
#[derive(Debug)]
pub struct CachedTerminalContent {
    /// Pre-converted background rects with GPUI Hsla colors (avoids per-frame conversion).
    /// Wrapped in Arc for cheap per-frame cloning into canvas closures.
    pub bg_rects_hsla: Arc<Vec<(usize, usize, usize, gpui::Hsla)>>,
    /// Pre-converted text runs with GPUI Hsla foreground colors (avoids per-frame conversion).
    /// Wrapped in Arc for cheap per-frame cloning into canvas closures.
    pub text_runs_hsla: Arc<Vec<(TextRunSegment, gpui::Hsla)>>,
    /// Terminal rows at time of caching.
    pub rows: usize,
    /// Terminal columns at time of caching.
    pub cols: usize,
}

/// Cached content for a single visible terminal row.
#[derive(Debug, Clone, Default)]
pub(crate) struct CachedTerminalRow {
    pub(crate) bg_rects_hsla: Arc<Vec<(usize, usize, usize, gpui::Hsla)>>,
    pub(crate) text_runs_hsla: Arc<Vec<(TextRunSegment, gpui::Hsla)>>,
}

type ShapedTerminalRow = Arc<Vec<(usize, usize, gpui::ShapedLine)>>;
type SelectionRange = ((i32, usize), (i32, usize));

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
    /// Start position (grid line, column), if selection is active.
    pub start: Option<(i32, usize)>,
    /// End position (grid line, column), if selection is active.
    pub end: Option<(i32, usize)>,
}

impl Selection {
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
    pub fn set_start(&mut self, line: i32, col: usize) {
        self.start = Some((line, col));
    }

    /// Set the selection end position.
    pub fn set_end(&mut self, line: i32, col: usize) {
        self.end = Some((line, col));
    }

    /// Check if a cell position is within the selection.
    ///
    /// Returns `true` if the given (grid line, column) is selected.
    ///
    /// Comparison uses **lexicographic (row-major) order**: `(row, col)` tuples
    /// compare by row first, then column. This means the selection spans full
    /// rows between `start.row` and `end.row`, and only partial rows at the
    /// endpoints — which is standard terminal selection behaviour.
    pub fn contains(&self, line: i32, col: usize) -> bool {
        match (self.start, self.end) {
            (Some(start), Some(end)) => {
                // Normalize so start <= end
                let (start, end) = if start <= end {
                    (start, end)
                } else {
                    (end, start)
                };

                let pos = (line, col);
                pos >= start && pos <= end
            }
            _ => false,
        }
    }

    /// Get the normalized selection range (start <= end).
    ///
    /// Returns `None` if selection is not active.
    pub fn normalized(&self) -> Option<((i32, usize), (i32, usize))> {
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

/// Scrollbar interaction state for a terminal pane.
#[derive(Debug, Clone)]
pub struct ScrollbarState {
    /// Current scrollbar opacity.
    pub opacity: f32,
    /// Whether the pointer is over the scrollbar.
    pub hovered: bool,
    /// Thumb drag offset from the thumb top in pixels.
    pub dragging: Option<f32>,
    /// Last time scroll activity occurred.
    pub last_scroll_activity: Instant,
}

impl Default for ScrollbarState {
    fn default() -> Self {
        Self {
            opacity: 0.0,
            hovered: false,
            dragging: None,
            last_scroll_activity: Instant::now(),
        }
    }
}

/// Keyboard-focused control within the terminal search overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchFocusControl {
    /// Search query input.
    #[default]
    Input,
    /// Previous match button.
    Previous,
    /// Next match button.
    Next,
    /// Close search button.
    Close,
}

impl SearchFocusControl {
    fn cycle(self, reverse: bool) -> Self {
        match (self, reverse) {
            (Self::Input, false) => Self::Previous,
            (Self::Previous, false) => Self::Next,
            (Self::Next, false) => Self::Close,
            (Self::Close, false) => Self::Input,
            (Self::Input, true) => Self::Close,
            (Self::Previous, true) => Self::Input,
            (Self::Next, true) => Self::Previous,
            (Self::Close, true) => Self::Next,
        }
    }
}

/// Terminal search overlay state.
#[derive(Debug, Clone, Default)]
pub struct SearchState {
    /// Whether the search overlay is currently open.
    pub active: bool,
    /// Current search query.
    pub query: String,
    /// Cached matches for the current query.
    pub matches: Vec<SearchMatch>,
    /// Focused match index.
    pub current_match: Option<usize>,
    /// Monotonic debounce generation.
    pub generation: u64,
    /// Keyboard-focused control inside the search overlay.
    pub focus_control: SearchFocusControl,
}

/// Terminal view component.
///
/// Renders terminal content to the screen, handling cells, cursor,
/// and selection display.
pub struct TerminalView {
    /// Session associated with this terminal view.
    session_id: SessionId,
    /// Background runtime that owns the live terminal state.
    runtime: TerminalRuntimeHandle,
    /// Theme for color resolution.
    theme: CodirigentTheme,
    /// Width of a single character cell in pixels.
    cell_width: f32,
    /// Height of a single character cell in pixels.
    cell_height: f32,
    /// Vertical shift to center visible glyphs within cells.
    /// Compensates for DirectWrite internal leading (invisible space in ascent).
    /// Zero when `ascent + |descent| <= font_size`.
    centering_adjust: f32,
    /// Font size in pixels.
    font_size: f32,
    /// Font family name.
    font_family: String,
    /// Current text selection.
    selection: Selection,
    /// Cursor shape to render.
    cursor_shape: CursorShape,
    /// Whether the terminal view is focused.
    focused: bool,
    /// Cached terminal dimensions from the latest runtime snapshot.
    rows: u16,
    /// Cached terminal dimensions from the latest runtime snapshot.
    cols: u16,
    /// Cached terminal mode flags from the latest runtime snapshot.
    mode: TermMode,
    /// Cached scrollback history size from the latest runtime snapshot.
    history_size: usize,
    /// Cached viewport display offset from the latest runtime snapshot.
    display_offset: usize,
    /// Generation of the latest applied runtime snapshot.
    snapshot_generation: u64,
    /// Cached terminal content for canvas rendering.
    cached_content: Option<CachedTerminalContent>,
    /// Per-row cached terminal content from the latest runtime snapshot.
    cached_rows: Vec<CachedTerminalRow>,
    /// Font family used to build `cached_shaped_rows`.
    cached_shaped_font_family: Option<String>,
    /// Font size used to build `cached_shaped_rows`.
    cached_shaped_font_size: Option<f32>,
    /// Per-row shaped text derived from `cached_rows`.
    cached_shaped_rows: Option<Vec<ShapedTerminalRow>>,
    /// Selection range used to build `cached_shaped_rows`.
    cached_shaped_selection: Option<SelectionRange>,
    /// Dirty viewport rows after a partial content rebuild.
    dirty_rows: Option<Vec<usize>>,
    /// Whether cell dimensions have been initialized from font metrics.
    dimensions_initialized: bool,
    /// Cached GPUI Hsla for terminal background (avoids per-frame conversion).
    cached_terminal_bg: gpui::Hsla,
    /// Cached GPUI Hsla for terminal foreground (avoids per-frame conversion).
    cached_terminal_fg: gpui::Hsla,
    /// Cached viewport-relative cursor (x, y) in pixels.
    /// Updated alongside row caches in `ensure_row_caches()` so the render
    /// pass never calls `renderable_content()` for cursor/IME positioning.
    cached_cursor_viewport_pos: Option<(f32, f32)>,
    /// Scrollbar interaction state.
    scrollbar: ScrollbarState,
    /// Search overlay state.
    search: SearchState,
}

impl TerminalView {
    /// Create a new terminal view.
    pub fn new(terminal: Terminal, theme: CodirigentTheme) -> Self {
        let font_size = theme.terminal_font_size;
        let font_family = theme.terminal_font_family.clone();
        // Approximate cell dimensions until real font metrics arrive via
        // compute_cell_dimensions() on first render. Using conservative
        // ratios that slightly overestimate so the initial grid doesn't
        // allocate more rows/cols than will fit after correction.
        let cell_width = (font_size * APPROX_CELL_WIDTH_RATIO).max(MIN_CELL_WIDTH_PX);
        let cell_height = font_size.max(14.0);
        let session_id = terminal.session_id();
        let initial_size =
            TerminalSize::new(terminal.rows(), terminal.cols(), cell_width, cell_height);
        let (runtime, snapshot) = TerminalRuntimeHandle::new(terminal, theme.clone(), initial_size);

        let cached_terminal_bg: gpui::Hsla = theme.terminal_background.into();
        let cached_terminal_fg: gpui::Hsla = theme.terminal_foreground.into();

        let mut view = Self {
            session_id,
            runtime,
            theme,
            cell_width,
            cell_height,
            centering_adjust: 0.0,
            font_size,
            font_family,
            selection: Selection::default(),
            cursor_shape: CursorShape::Block,
            focused: true,
            rows: 0,
            cols: 0,
            mode: TermMode::empty(),
            history_size: 0,
            display_offset: 0,
            snapshot_generation: 0,
            cached_content: None,
            cached_rows: Vec::new(),
            cached_shaped_font_family: None,
            cached_shaped_font_size: None,
            cached_shaped_rows: None,
            cached_shaped_selection: None,
            dirty_rows: None,
            dimensions_initialized: false,
            cached_terminal_bg,
            cached_terminal_fg,
            cached_cursor_viewport_pos: None,
            scrollbar: ScrollbarState::default(),
            search: SearchState::default(),
        };
        let _ = view.apply_snapshot(snapshot);
        view
    }

    /// Get a clone of the terminal runtime handle.
    pub(crate) fn runtime_handle(&self) -> TerminalRuntimeHandle {
        self.runtime.clone()
    }

    /// Apply a terminal snapshot from the background runtime.
    ///
    /// Returns `true` when the snapshot was accepted. Stale snapshots are
    /// dropped to avoid regressing the visible terminal after newer resize,
    /// scroll, or output updates have already been applied.
    pub(crate) fn apply_snapshot(&mut self, snapshot: TerminalRenderSnapshot) -> bool {
        if snapshot.generation < self.snapshot_generation {
            return false;
        }

        let requires_full_shaped_rebuild = snapshot.dirty_rows.is_none()
            || self.rows != snapshot.rows
            || self.cols != snapshot.cols
            || self.cached_rows.len() != snapshot.cached_rows.len();

        let display_offset_changed = self.display_offset != snapshot.display_offset;
        self.rows = snapshot.rows;
        self.cols = snapshot.cols;
        self.mode = snapshot.mode;
        self.history_size = snapshot.history_size;
        self.display_offset = snapshot.display_offset;
        self.snapshot_generation = snapshot.generation;
        self.cached_rows = snapshot.cached_rows;
        self.cached_content = None;
        self.refresh_cursor_cache(snapshot.cursor_viewport_cell);

        if display_offset_changed {
            self.note_scroll_activity();
        }

        if requires_full_shaped_rebuild {
            self.cached_shaped_font_family = None;
            self.cached_shaped_font_size = None;
            self.cached_shaped_rows = None;
            self.dirty_rows = None;
        } else {
            self.dirty_rows = snapshot.dirty_rows;
        }

        true
    }

    /// Get the theme.
    pub fn theme(&self) -> &CodirigentTheme {
        &self.theme
    }

    /// Set the theme.
    pub fn set_theme(&mut self, theme: CodirigentTheme) {
        self.cached_terminal_bg = theme.terminal_background.into();
        self.cached_terminal_fg = theme.terminal_foreground.into();
        self.theme = theme.clone();
        if let Some(snapshot) = self.runtime.set_theme(theme) {
            let _ = self.apply_snapshot(snapshot);
        } else {
            self.mark_dirty();
        }
    }

    /// Get the cell width in pixels.
    pub fn cell_width(&self) -> f32 {
        self.cell_width
    }

    /// Get the cell height in pixels.
    pub fn cell_height(&self) -> f32 {
        self.cell_height
    }

    /// Get the vertical centering adjustment in pixels.
    pub fn centering_adjust(&self) -> f32 {
        self.centering_adjust
    }

    /// Set the cell dimensions.
    ///
    /// Marks the view as having initialized dimensions and triggers a
    /// terminal resize with the new cell metrics.
    pub fn set_cell_dimensions(&mut self, width: f32, height: f32, centering_adjust: f32) {
        self.cell_width = width;
        self.cell_height = height;
        self.centering_adjust = centering_adjust;
        self.dimensions_initialized = true;
        if let Some(snapshot) = self
            .runtime
            .resize_with_cells(TerminalSize::new(self.rows, self.cols, width, height))
        {
            let _ = self.apply_snapshot(snapshot);
        } else {
            self.mark_dirty();
            self.refresh_cursor_cache(None);
        }
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
        self.cached_shaped_font_size = None;
    }

    /// Get the font family.
    pub fn font_family(&self) -> &str {
        &self.font_family
    }

    /// Get cached terminal background color as GPUI Hsla.
    pub fn terminal_bg_hsla(&self) -> gpui::Hsla {
        self.cached_terminal_bg
    }

    /// Get cached terminal foreground color as GPUI Hsla.
    pub fn terminal_fg_hsla(&self) -> gpui::Hsla {
        self.cached_terminal_fg
    }

    /// Set the font family.
    pub fn set_font_family(&mut self, family: String) {
        self.font_family = family;
        self.cached_shaped_font_family = None;
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
        self.session_id
    }

    /// Get the current terminal row count.
    pub fn rows(&self) -> u16 {
        self.rows
    }

    /// Get the total number of scrollback lines currently retained.
    pub fn total_scrollback_lines(&self) -> usize {
        self.history_size
    }

    /// Get the current terminal column count.
    pub fn cols(&self) -> u16 {
        self.cols
    }

    /// Get the current terminal mode flags.
    pub fn mode(&self) -> TermMode {
        self.mode
    }

    /// Check whether bracketed paste mode is enabled.
    pub fn bracketed_paste_mode(&self) -> bool {
        self.mode.contains(TermMode::BRACKETED_PASTE)
    }

    /// Scroll up by the specified number of lines (show older content).
    ///
    /// Positive `Scroll::Delta` increases `display_offset`, moving the
    /// viewport up into the scrollback buffer.
    pub fn scroll_up(&mut self, lines: usize) {
        if let Some(snapshot) = self.runtime.scroll_up(lines) {
            let _ = self.apply_snapshot(snapshot);
        }
    }

    /// Scroll down by the specified number of lines (show newer content).
    ///
    /// Negative `Scroll::Delta` decreases `display_offset`, moving the
    /// viewport down toward the most recent output.
    pub fn scroll_down(&mut self, lines: usize) {
        if let Some(snapshot) = self.runtime.scroll_down(lines) {
            let _ = self.apply_snapshot(snapshot);
        }
    }

    /// Scroll to the bottom (most recent output).
    pub fn scroll_to_bottom(&mut self) {
        let _ = self.scroll_to_bottom_if_needed();
    }

    /// Scroll to an absolute scrollback offset.
    pub fn scroll_to_offset(&mut self, target: usize) {
        let target = target.min(self.history_size);
        if target == self.display_offset {
            return;
        }
        if let Some(snapshot) = self.runtime.scroll_to_offset(target) {
            let _ = self.apply_snapshot(snapshot);
        }
    }

    /// Scroll to the bottom only when the viewport is currently in scrollback.
    ///
    /// Returns `true` if the viewport changed.
    pub fn scroll_to_bottom_if_needed(&mut self) -> bool {
        if !self.is_scrolled_back() {
            return false;
        }

        self.runtime
            .scroll_to_bottom()
            .is_some_and(|snapshot| self.apply_snapshot(snapshot))
    }

    /// Get the current viewport scroll offset (lines above the live view).
    ///
    /// Returns 0 when the user is at the bottom (live terminal output).
    pub fn display_offset(&self) -> usize {
        self.display_offset
    }

    /// Check whether the viewport is showing scrollback instead of the live prompt.
    pub fn is_scrolled_back(&self) -> bool {
        self.display_offset != 0
    }

    fn viewport_row_to_grid_line(&self, row: usize) -> i32 {
        row as i32 - self.display_offset as i32
    }

    /// Clear the terminal screen while preserving the current line (prompt).
    ///
    /// This clears the scrollback and visible content while keeping
    /// the current line (typically the shell prompt) at the top.
    pub fn clear(&mut self) {
        if let Some(snapshot) = self.runtime.clear() {
            let _ = self.apply_snapshot(snapshot);
        }
    }

    /// Get cursor rendering information.
    ///
    /// Returns `None` if the cursor is not visible or is scrolled off-screen.
    /// Uses viewport-relative coordinates from `renderable_content()` so the
    /// cursor position is correct when the terminal is scrolled.
    /// Get cursor rendering information from cached state.
    ///
    /// Returns `None` if the cursor is hidden (`\e[?25l`) or off-screen.
    /// The underlying position is cached in `ensure_row_caches()` so this
    /// is a pure field read — no terminal state access in the render pass.
    pub fn cursor_rect(&self) -> Option<CursorRect> {
        if !self.mode.contains(TermMode::SHOW_CURSOR) {
            return None;
        }

        let (x, y) = self.cached_cursor_viewport_pos?;

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

    /// Returns the cached cursor (x, y) for IME preedit anchoring.
    ///
    /// Unlike `cursor_rect`, this ignores `\e[?25l` visibility so the
    /// preedit overlay tracks the real cursor location even during
    /// Claude Code / Ink redraw cycles.
    pub fn ime_anchor_pos(&self) -> Option<(f32, f32)> {
        self.cached_cursor_viewport_pos
    }

    /// Calculate pixel dimensions for the current terminal size.
    #[cfg(test)]
    pub(crate) fn pixel_size(&self) -> (f32, f32) {
        let width = self.cols as f32 * self.cell_width;
        let height = self.rows as f32 * self.cell_height;
        (width, height)
    }

    /// Calculate terminal dimensions from pixel size.
    pub fn dimensions_from_pixels(&self, width: f32, height: f32) -> (u16, u16) {
        let cols = (width / self.cell_width).floor() as u16;
        let rows = (height / self.cell_height).floor() as u16;
        (rows.max(1), cols.max(1))
    }

    /// Resize the terminal to fit within the given pixel dimensions.
    ///
    /// Returns `true` if the terminal was actually resized, `false` if
    /// dimensions were already at the target size (no-op).
    pub fn resize_to_fit(&mut self, width: f32, height: f32) -> bool {
        let (rows, cols) = self.dimensions_from_pixels(width, height);
        if rows == self.rows && cols == self.cols {
            return false;
        }
        if let Some(snapshot) = self.runtime.resize_with_cells(TerminalSize::new(
            rows,
            cols,
            self.cell_width,
            self.cell_height,
        )) {
            let _ = self.apply_snapshot(snapshot);
        } else {
            self.mark_dirty();
        }
        true
    }

    /// Start a new text selection at the given cell position.
    ///
    /// Converts the viewport row into a stable grid line, clears any previous
    /// end, and marks dirty.
    pub fn start_selection(&mut self, row: usize, col: usize) {
        self.selection
            .set_start(self.viewport_row_to_grid_line(row), col);
        self.selection.end = None;
    }

    /// Update the selection end position during a drag.
    ///
    /// Converts the viewport row into a stable grid line and marks dirty for
    /// re-rendering.
    pub fn update_selection(&mut self, row: usize, col: usize) {
        self.selection
            .set_end(self.viewport_row_to_grid_line(row), col);
    }

    /// Clear the current selection.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    /// Get the currently selected text, if any.
    ///
    /// Returns `None` if no selection is active. Extracts text from the
    /// terminal grid between the normalized selection start and end.
    pub fn get_selected_text(&self) -> Option<String> {
        self.runtime.get_selected_text(&self.selection)
    }

    /// Mark content as dirty, forcing recomputation on next access.
    pub fn mark_dirty(&mut self) {
        self.cached_content = None;
        self.cached_shaped_font_family = None;
        self.cached_shaped_font_size = None;
        self.cached_shaped_rows = None;
        self.cached_shaped_selection = None;
        self.dirty_rows = None;
    }

    /// Get cached terminal content, flattening row caches only when requested.
    ///
    /// Rendering uses row caches directly; this flattened view is retained for
    /// tests and any future callers that need a contiguous snapshot.
    pub fn cached_content(&mut self) -> &CachedTerminalContent {
        self.cached_content.get_or_insert_with(|| {
            let rows = self.rows as usize;
            let cols = self.cols as usize;
            Self::flatten_cached_rows(&self.cached_rows, rows, cols)
        })
    }

    /// Get per-row cached terminal content for rendering.
    pub(crate) fn render_rows(&self) -> Vec<CachedTerminalRow> {
        self.cached_rows.clone()
    }

    /// Get per-row shaped text, rebuilding only dirty rows when content changes.
    pub(crate) fn shaped_rows(
        &mut self,
        text_system: &gpui::WindowTextSystem,
    ) -> Vec<ShapedTerminalRow> {
        let font_family = self.font_family.clone();
        let font_size = self.font_size;
        let selection_range = self.selection.normalized();
        let selection_fg: gpui::Hsla = self.theme.terminal_selection_fg.into();

        let font_changed = self.cached_shaped_font_family.as_ref() != Some(&font_family)
            || self
                .cached_shaped_font_size
                .map_or(true, |size| (size - font_size).abs() > 0.01);
        let selection_changed = self.cached_shaped_selection != selection_range;
        let row_shapes_need_full_rebuild = font_changed
            || selection_changed
            || self.cached_shaped_rows.is_none()
            || self.cached_rows.len()
                != self
                    .cached_shaped_rows
                    .as_ref()
                    .map_or(0, |row_shapes| row_shapes.len());

        if row_shapes_need_full_rebuild {
            let row_shapes = self
                .cached_rows
                .iter()
                .enumerate()
                .map(|(row_index, row)| {
                    shape_text_runs(
                        text_system,
                        row.text_runs_hsla.as_ref(),
                        &font_family,
                        font_size,
                        self.selection_range_for_viewport_row(row_index),
                        selection_fg,
                    )
                })
                .collect::<Vec<_>>();
            self.cached_shaped_font_family = Some(font_family);
            self.cached_shaped_font_size = Some(font_size);
            self.cached_shaped_selection = selection_range;
            self.cached_shaped_rows = Some(row_shapes);
            self.dirty_rows = None;
        } else if let Some(dirty_rows) = self.dirty_rows.take() {
            let dirty_selection_ranges = dirty_rows
                .iter()
                .map(|row| (*row, self.selection_range_for_viewport_row(*row)))
                .collect::<Vec<_>>();
            if let Some(row_shapes) = self.cached_shaped_rows.as_mut() {
                for (row, selection_range) in dirty_selection_ranges {
                    if row >= self.cached_rows.len() || row >= row_shapes.len() {
                        continue;
                    }
                    row_shapes[row] = shape_text_runs(
                        text_system,
                        self.cached_rows[row].text_runs_hsla.as_ref(),
                        &font_family,
                        font_size,
                        selection_range,
                        selection_fg,
                    );
                }
            }
        }
        self.cached_shaped_rows.clone().unwrap_or_default()
    }

    fn flatten_cached_rows(
        cached_rows: &[CachedTerminalRow],
        rows: usize,
        cols: usize,
    ) -> CachedTerminalContent {
        let bg_capacity = cached_rows.iter().map(|row| row.bg_rects_hsla.len()).sum();
        let text_capacity = cached_rows.iter().map(|row| row.text_runs_hsla.len()).sum();

        let mut bg_rects = Vec::with_capacity(bg_capacity);
        let mut text_runs = Vec::with_capacity(text_capacity);
        for row in cached_rows {
            bg_rects.extend(row.bg_rects_hsla.iter().copied());
            text_runs.extend(row.text_runs_hsla.iter().cloned());
        }

        CachedTerminalContent {
            bg_rects_hsla: Arc::new(bg_rects),
            text_runs_hsla: Arc::new(text_runs),
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

        let max_row = self.rows as usize;
        let max_col = self.cols as usize;

        if row < max_row && col < max_col {
            Some((row, col))
        } else {
            None
        }
    }

    /// Convert pixel coordinates to a clamped terminal cell position.
    ///
    /// Unlike `pixel_to_cell`, this never returns `None` — coordinates are
    /// clamped to the viewport bounds. The returned `scroll_dir` indicates
    /// whether the position was above (`-1`), within (`0`), or below (`1`)
    /// the viewport, allowing callers to trigger auto-scroll during selection.
    pub fn pixel_to_cell_clamped(&self, x: f32, y: f32) -> (usize, usize, i32) {
        let max_row = self.rows as usize;
        let max_col = self.cols as usize;

        if max_row == 0 || max_col == 0 {
            return (0, 0, 0);
        }

        let scroll_dir = if y < 0.0 {
            -1
        } else if y >= (max_row as f32) * self.cell_height {
            1
        } else {
            0
        };

        let row = if y < 0.0 {
            0
        } else {
            let r = (y / self.cell_height).floor() as usize;
            r.min(max_row - 1)
        };

        let col = if x < 0.0 {
            0
        } else {
            let c = (x / self.cell_width).floor() as usize;
            c.min(max_col - 1)
        };

        (row, col, scroll_dir)
    }

    /// Get selection background rectangles for the current viewport.
    pub(crate) fn selection_rects_hsla(&self) -> Vec<(usize, usize, usize, gpui::Hsla)> {
        let Some(((start_line, start_col), (end_line, end_col))) = self.selection.normalized()
        else {
            return Vec::new();
        };

        let max_col = self.cols as usize;
        if max_col == 0 {
            return Vec::new();
        }

        let viewport_start_line = -(self.display_offset as i32);
        let viewport_end_line = viewport_start_line + self.rows as i32 - 1;
        let visible_start = start_line.max(viewport_start_line);
        let visible_end = end_line.min(viewport_end_line);
        if visible_start > visible_end {
            return Vec::new();
        }

        let selection_bg: gpui::Hsla = self.theme.terminal_selection_bg.into();
        let mut rects = Vec::new();
        for line in visible_start..=visible_end {
            let row = (line + self.display_offset as i32) as usize;
            let start = if line == start_line {
                start_col.min(max_col)
            } else {
                0
            };
            let end = if line == end_line {
                end_col.saturating_add(1).min(max_col)
            } else {
                max_col
            };
            if start < end {
                rects.push((row, start, end, selection_bg));
            }
        }

        rects
    }

    /// Get scrollbar interaction state.
    pub fn scrollbar(&self) -> &ScrollbarState {
        &self.scrollbar
    }

    /// Mark recent scrollbar activity and show it immediately.
    pub fn note_scroll_activity(&mut self) {
        self.scrollbar.opacity = 1.0;
        self.scrollbar.last_scroll_activity = Instant::now();
    }

    /// Update scrollbar hover state.
    pub fn set_scrollbar_hovered(&mut self, hovered: bool) {
        self.scrollbar.hovered = hovered;
        if hovered {
            self.note_scroll_activity();
        }
    }

    /// Begin thumb dragging.
    pub fn start_scrollbar_drag(&mut self, thumb_offset: f32) {
        self.scrollbar.dragging = Some(thumb_offset.max(0.0));
        self.note_scroll_activity();
    }

    /// End thumb dragging.
    pub fn stop_scrollbar_drag(&mut self) {
        self.scrollbar.dragging = None;
    }

    /// Current thumb drag offset, if any.
    pub fn scrollbar_drag_offset(&self) -> Option<f32> {
        self.scrollbar.dragging
    }

    /// Update the scrollbar opacity when inactivity has elapsed.
    pub fn fade_scrollbar_if_idle(&mut self, now: Instant) -> bool {
        if self.scrollbar.hovered || self.scrollbar.dragging.is_some() {
            if self.scrollbar.opacity != 1.0 {
                self.scrollbar.opacity = 1.0;
                return true;
            }
            return false;
        }

        if now
            .duration_since(self.scrollbar.last_scroll_activity)
            .as_millis()
            >= 1500
            && self.scrollbar.opacity != 0.0
        {
            self.scrollbar.opacity = 0.0;
            return true;
        }

        false
    }

    /// Compute scrollbar thumb height and top offset for a given track height.
    pub fn scrollbar_thumb_metrics(&self, track_height: f32) -> (f32, f32) {
        if track_height <= 0.0 {
            return (0.0, 0.0);
        }

        let total_lines = (self.history_size + self.rows as usize).max(1) as f32;
        let thumb_height = (track_height * (self.rows as f32 / total_lines))
            .max(30.0)
            .min(track_height);
        let max_thumb_top = (track_height - thumb_height).max(0.0);
        let thumb_top = if self.history_size == 0 || max_thumb_top == 0.0 {
            max_thumb_top
        } else {
            max_thumb_top * (1.0 - (self.display_offset as f32 / self.history_size as f32))
        };

        (thumb_height, thumb_top)
    }

    /// Convert a track-relative Y position into a scrollback offset.
    pub fn scrollbar_offset_for_pointer(
        &self,
        pointer_y: f32,
        track_height: f32,
        drag_offset: Option<f32>,
    ) -> usize {
        if self.history_size == 0 || track_height <= 0.0 {
            return 0;
        }

        if drag_offset.is_none() {
            let fraction = (pointer_y.clamp(0.0, track_height) / track_height).clamp(0.0, 1.0);
            ((1.0 - fraction) * self.history_size as f32).round() as usize
        } else {
            let (thumb_height, _) = self.scrollbar_thumb_metrics(track_height);
            let max_thumb_top = (track_height - thumb_height).max(0.0);
            let thumb_top = (pointer_y - drag_offset.unwrap_or(0.0)).clamp(0.0, max_thumb_top);

            if max_thumb_top == 0.0 {
                0
            } else {
                ((1.0 - (thumb_top / max_thumb_top)) * self.history_size as f32).round() as usize
            }
        }
    }

    /// Get search overlay state.
    pub fn search(&self) -> &SearchState {
        &self.search
    }

    /// Open search for this terminal.
    pub fn open_search(&mut self) {
        self.search.active = true;
        self.search.focus_control = SearchFocusControl::Input;
        if self.search.current_match.is_none() && !self.search.matches.is_empty() {
            self.search.current_match = Some(0);
        }
    }

    /// Close search and clear its matches.
    pub fn close_search(&mut self) {
        self.search = SearchState::default();
    }

    /// Replace the current search query.
    pub fn set_search_query(&mut self, query: String) {
        self.search.query = query;
        self.search.generation = self.search.generation.saturating_add(1);
        if self.search.query.is_empty() {
            self.clear_search_matches();
        } else {
            self.search.matches.clear();
            self.search.current_match = None;
        }
    }

    /// Append committed text to the search query.
    pub fn append_search_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        self.search.query.push_str(text);
        self.search.generation = self.search.generation.saturating_add(1);
        self.search.matches.clear();
        self.search.current_match = None;
    }

    /// Remove the last search character.
    pub fn pop_search_char(&mut self) {
        if self.search.query.pop().is_some() {
            self.search.generation = self.search.generation.saturating_add(1);
            if self.search.query.is_empty() {
                self.clear_search_matches();
            } else {
                self.search.matches.clear();
                self.search.current_match = None;
            }
        }
    }

    /// Current search debounce generation.
    pub fn search_generation(&self) -> u64 {
        self.search.generation
    }

    /// Current search query text.
    pub fn search_query(&self) -> &str {
        &self.search.query
    }

    /// Keyboard-focused control for the search overlay.
    pub fn search_focus_control(&self) -> SearchFocusControl {
        self.search.focus_control
    }

    /// Update the keyboard-focused control for the search overlay.
    pub fn set_search_focus_control(&mut self, control: SearchFocusControl) {
        self.search.focus_control = control;
    }

    /// Move keyboard focus to the next or previous search control.
    pub fn cycle_search_focus_control(&mut self, reverse: bool) {
        self.search.focus_control = self.search.focus_control.cycle(reverse);
    }

    /// Apply search results for the current query.
    pub fn set_search_matches(&mut self, matches: Vec<SearchMatch>) {
        self.search.matches = matches;
        self.search.current_match = if self.search.matches.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    /// Clear cached search matches.
    pub fn clear_search_matches(&mut self) {
        self.search.matches.clear();
        self.search.current_match = None;
    }

    /// Update the active search match index.
    pub fn set_current_search_match(&mut self, index: Option<usize>) {
        self.search.current_match = index;
    }

    /// Search highlight rects for the current viewport.
    pub(crate) fn search_highlight_rects_hsla(&self) -> Vec<(usize, usize, usize, gpui::Hsla)> {
        if !self.search.active || self.search.matches.is_empty() {
            return Vec::new();
        }

        let viewport_start_line = -(self.display_offset as i32);
        let viewport_end_line = viewport_start_line + self.rows as i32 - 1;
        let cols = self.cols as usize;
        let inactive: gpui::Hsla = self.theme.primary.into();
        let active: gpui::Hsla = self.theme.orange.into();
        let mut rects = Vec::new();

        for (index, search_match) in self.search.matches.iter().enumerate() {
            let visible_start = search_match.grid_line.max(viewport_start_line);
            let visible_end = search_match.end_grid_line.min(viewport_end_line);
            if visible_start > visible_end {
                continue;
            }

            let color = if self.search.current_match == Some(index) {
                active.opacity(0.45)
            } else {
                inactive.opacity(0.28)
            };

            for line in visible_start..=visible_end {
                let row = (line + self.display_offset as i32) as usize;
                let start = if line == search_match.grid_line {
                    search_match.start_col.min(cols)
                } else {
                    0
                };
                let end = if line == search_match.end_grid_line {
                    search_match.end_col.min(cols)
                } else {
                    cols
                };
                if start < end {
                    rects.push((row, start, end, color));
                }
            }
        }

        rects
    }

    /// Proportional scrollbar marker positions for current search matches.
    pub fn search_marker_fractions(&self) -> Vec<f32> {
        if !self.search.active || self.search.matches.is_empty() {
            return Vec::new();
        }

        let total_lines = (self.history_size + self.rows as usize).max(1) as f32;
        self.search
            .matches
            .iter()
            .map(|search_match| {
                ((self.history_size as i32 + search_match.grid_line) as f32 / total_lines)
                    .clamp(0.0, 1.0)
            })
            .collect()
    }

    /// Scroll the viewport so the focused match is centered when possible.
    pub fn scroll_to_search_match(&mut self, index: usize) {
        let Some(search_match) = self.search.matches.get(index) else {
            return;
        };

        let center_row = self.rows as i32 / 2;
        let target = (center_row - search_match.grid_line).max(0) as usize;
        self.scroll_to_offset(target.min(self.history_size));
        self.search.current_match = Some(index);
    }

    fn selection_range_for_viewport_row(&self, row: usize) -> Option<(usize, usize)> {
        let ((start_line, start_col), (end_line, end_col)) = self.selection.normalized()?;
        let grid_line = self.viewport_row_to_grid_line(row);
        if grid_line < start_line || grid_line > end_line {
            return None;
        }

        let max_col = self.cols as usize;
        if max_col == 0 {
            return None;
        }

        let start = if grid_line == start_line {
            start_col.min(max_col)
        } else {
            0
        };
        let end = if grid_line == end_line {
            end_col.saturating_add(1).min(max_col)
        } else {
            max_col
        };

        (start < end).then_some((start, end))
    }

    /// Snapshot the cursor viewport position into `cached_cursor_viewport_pos`.
    fn refresh_cursor_cache(&mut self, cursor_viewport_cell: Option<(usize, usize)>) {
        if let Some((row, col)) = cursor_viewport_cell {
            self.cached_cursor_viewport_pos =
                Some((col as f32 * self.cell_width, row as f32 * self.cell_height));
        } else {
            self.cached_cursor_viewport_pos = None;
        }
    }

    #[cfg(test)]
    pub(crate) fn apply_output_for_test(&mut self, data: &[u8]) {
        if let Some(snapshot) = self.runtime.apply_output(data) {
            let _ = self.apply_snapshot(snapshot);
        }
    }
}

fn shape_text_runs(
    text_system: &gpui::WindowTextSystem,
    text_runs: &[(TextRunSegment, gpui::Hsla)],
    font_family: &str,
    font_size: f32,
    selection_columns: Option<(usize, usize)>,
    selection_fg: gpui::Hsla,
) -> Arc<Vec<(usize, usize, gpui::ShapedLine)>> {
    use gpui::{px, Font, FontFeatures, FontStyle, FontWeight, TextRun};

    let font_size_px = px(font_size);
    let font_family: gpui::SharedString = font_family.to_owned().into();
    let mut shaped_runs = Vec::with_capacity(text_runs.len());

    for (run, fg_color) in text_runs.iter() {
        for (split_run, split_fg) in
            split_text_run_by_selection(run, *fg_color, selection_columns, selection_fg)
        {
            let weight = if split_run.bold {
                FontWeight::BOLD
            } else {
                FontWeight::NORMAL
            };
            let style = if split_run.italic {
                FontStyle::Italic
            } else {
                FontStyle::Normal
            };

            let font = Font {
                family: font_family.clone(),
                features: FontFeatures::default(),
                fallbacks: None,
                weight,
                style,
            };

            let underline = if split_run.underline {
                Some(gpui::UnderlineStyle {
                    thickness: px(1.0),
                    color: Some(split_fg),
                    wavy: false,
                })
            } else {
                None
            };

            let strikethrough = if split_run.strikethrough {
                Some(gpui::StrikethroughStyle {
                    thickness: px(1.0),
                    color: Some(split_fg),
                })
            } else {
                None
            };

            let text: gpui::SharedString = split_run.text.clone().into();
            let text_run = TextRun {
                len: text.len(),
                font,
                color: split_fg,
                background_color: None,
                underline,
                strikethrough,
            };

            let shaped = text_system.shape_line(text, font_size_px, &[text_run], None);
            shaped_runs.push((split_run.row, split_run.start_col, shaped));
        }
    }

    Arc::new(shaped_runs)
}

fn split_text_run_by_selection(
    run: &TextRunSegment,
    default_fg: gpui::Hsla,
    selection_columns: Option<(usize, usize)>,
    selection_fg: gpui::Hsla,
) -> Vec<(TextRunSegment, gpui::Hsla)> {
    let Some((selection_start, selection_end)) = selection_columns else {
        return vec![(run.clone(), default_fg)];
    };

    let run_end = run.start_col + run.cell_count;
    if selection_end <= run.start_col || selection_start >= run_end {
        return vec![(run.clone(), default_fg)];
    }

    let mut segments = Vec::new();
    let mut segment_text = String::new();
    let mut segment_start_col = run.start_col;
    let mut segment_cell_count = 0usize;
    let mut segment_fg = default_fg;
    let mut current_col = run.start_col;
    let mut has_segment = false;

    for character in run.text.chars() {
        let cell_width = terminal_char_width(character);
        let character_fg =
            if current_col < selection_end && current_col + cell_width > selection_start {
                selection_fg
            } else {
                default_fg
            };

        if !has_segment {
            segment_start_col = current_col;
            segment_fg = character_fg;
            has_segment = true;
        } else if character_fg != segment_fg {
            segments.push((
                clone_text_run_segment(
                    run,
                    segment_text.clone(),
                    segment_start_col,
                    segment_cell_count,
                ),
                segment_fg,
            ));
            segment_text.clear();
            segment_start_col = current_col;
            segment_cell_count = 0;
            segment_fg = character_fg;
        }

        segment_text.push(character);
        segment_cell_count += cell_width;
        current_col += cell_width;
    }

    if has_segment {
        segments.push((
            clone_text_run_segment(run, segment_text, segment_start_col, segment_cell_count),
            segment_fg,
        ));
    }

    segments
}

fn clone_text_run_segment(
    run: &TextRunSegment,
    text: String,
    start_col: usize,
    cell_count: usize,
) -> TextRunSegment {
    TextRunSegment {
        text,
        foreground: run.foreground,
        bold: run.bold,
        italic: run.italic,
        underline: run.underline,
        strikethrough: run.strikethrough,
        row: run.row,
        start_col,
        cell_count,
    }
}

fn terminal_char_width(character: char) -> usize {
    UnicodeWidthChar::width(character).unwrap_or(1).max(1)
}

/// Compute cell dimensions from actual font metrics using the text system.
///
/// Uses `text_system.advance('m')` to get the actual character width and
/// `ascent + |descent|` to get the line height. GPUI's ascent already includes
/// room for accented characters, so no leading factor is needed.
///
/// Returns `(cell_width, cell_height, centering_adjust)` in pixels.
/// `centering_adjust` compensates for font internal leading (invisible space
/// in the ascent metric) so visible glyphs are centered within cells.
pub fn compute_cell_dimensions(
    text_system: &gpui::TextSystem,
    font_family: &str,
    font_size: f32,
    line_height: f32,
) -> (f32, f32, f32) {
    use gpui::{px, Font, FontFeatures, FontStyle, FontWeight};

    let font = Font {
        family: font_family.to_owned().into(),
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
        .unwrap_or(font_size * FALLBACK_CELL_WIDTH_RATIO)
        .max(MIN_CELL_WIDTH_PX);

    let ascent: f32 = text_system.ascent(font_id, font_size_px).into();
    let descent: f32 = text_system.descent(font_id, font_size_px).into();
    let raw_cell_height = ascent + descent.abs();
    let cell_height = raw_cell_height.max(font_size) * line_height.max(1.0);
    let centering_adjust = (raw_cell_height - font_size).max(0.0) / 2.0;

    (cell_width, cell_height, centering_adjust)
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
    use crate::terminal_colors::dim_color;

    fn create_test_view() -> TerminalView {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let terminal = Terminal::new(24, 80, SessionId(1), tx);
        let theme = CodirigentTheme::dark();
        TerminalView::new(terminal, theme)
    }

    #[test]
    fn test_selection_new() {
        let selection = Selection::default();
        assert!(!selection.is_active());
        assert_eq!(selection.start, None);
        assert_eq!(selection.end, None);
    }

    #[test]
    fn test_selection_set_and_clear() {
        let mut selection = Selection::default();
        selection.set_start(5, 10);
        selection.set_end(10, 20);
        assert!(selection.is_active());
        selection.clear();
        assert!(!selection.is_active());
    }

    #[test]
    fn test_selection_contains() {
        let mut selection = Selection::default();
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
        let mut selection = Selection::default();
        selection.set_start(10, 80);
        selection.set_end(5, 0);
        assert!(selection.contains(5, 0));
        assert!(selection.contains(7, 40));
    }

    #[test]
    fn test_split_text_run_by_selection_uses_selection_foreground() {
        let theme = CodirigentTheme::dark();
        let default_fg: gpui::Hsla = Rgba::rgb(255, 255, 255).into();
        let selection_fg: gpui::Hsla = Rgba::rgb(255, 0, 0).into();
        let run = TextRunSegment {
            text: "hello".to_string(),
            foreground: theme.terminal_foreground,
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            row: 0,
            start_col: 0,
            cell_count: 5,
        };

        let segments = split_text_run_by_selection(&run, default_fg, Some((1, 4)), selection_fg);

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].0.text, "h");
        assert_eq!(segments[0].1, default_fg);
        assert_eq!(segments[1].0.text, "ell");
        assert_eq!(segments[1].1, selection_fg);
        assert_eq!(segments[2].0.text, "o");
        assert_eq!(segments[2].1, default_fg);
    }

    #[test]
    fn test_split_text_run_by_selection_keeps_wide_chars_whole() {
        let theme = CodirigentTheme::dark();
        let default_fg: gpui::Hsla = Rgba::rgb(255, 255, 255).into();
        let selection_fg: gpui::Hsla = Rgba::rgb(255, 0, 0).into();
        let run = TextRunSegment {
            text: "中a".to_string(),
            foreground: theme.terminal_foreground,
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            row: 0,
            start_col: 0,
            cell_count: 3,
        };

        let segments = split_text_run_by_selection(&run, default_fg, Some((1, 3)), selection_fg);

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].0.text, "中a");
        assert_eq!(segments[0].1, selection_fg);
    }

    /// Verifies lexicographic row-major ordering: the column at the end of
    /// the start-row IS included, but a column that is AFTER the end-col on
    /// the end-row is NOT included.
    #[test]
    fn test_selection_contains_row_major_boundary() {
        let mut selection = Selection::default();
        // Select from (2, 5) to (4, 3) in forward order
        selection.set_start(2, 5);
        selection.set_end(4, 3);

        // Exact endpoints are included
        assert!(selection.contains(2, 5));
        assert!(selection.contains(4, 3));

        // Middle row is fully included
        assert!(selection.contains(3, 0));
        assert!(selection.contains(3, 79));

        // On start-row, columns BEFORE start-col are NOT selected
        assert!(!selection.contains(2, 4));
        // On end-row, columns AFTER end-col are NOT selected
        assert!(!selection.contains(4, 4));

        // Rows outside the range are not selected
        assert!(!selection.contains(1, 99));
        assert!(!selection.contains(5, 0));

        // Single-cell selection
        let mut sel2 = Selection::default();
        sel2.set_start(3, 7);
        sel2.set_end(3, 7);
        assert!(sel2.contains(3, 7));
        assert!(!sel2.contains(3, 6));
        assert!(!sel2.contains(3, 8));
    }

    #[test]
    fn test_selection_normalized() {
        let mut selection = Selection::default();
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
        assert_eq!(view.rows(), 24);
        assert_eq!(view.cols(), 80);
        assert!(view.is_focused());
    }

    #[test]
    fn test_terminal_view_cell_dimensions() {
        let mut view = create_test_view();
        view.set_cell_dimensions(10.0, 20.0, 0.0);
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
    fn test_open_search_focuses_input() {
        let mut view = create_test_view();
        view.open_search();
        assert_eq!(view.search_focus_control(), SearchFocusControl::Input);
    }

    #[test]
    fn test_cycle_search_focus_control_wraps() {
        let mut view = create_test_view();
        view.open_search();

        view.cycle_search_focus_control(false);
        assert_eq!(view.search_focus_control(), SearchFocusControl::Previous);

        view.cycle_search_focus_control(false);
        assert_eq!(view.search_focus_control(), SearchFocusControl::Next);

        view.cycle_search_focus_control(false);
        assert_eq!(view.search_focus_control(), SearchFocusControl::Close);

        view.cycle_search_focus_control(false);
        assert_eq!(view.search_focus_control(), SearchFocusControl::Input);

        view.cycle_search_focus_control(true);
        assert_eq!(view.search_focus_control(), SearchFocusControl::Close);
    }

    #[test]
    fn test_search_query_edits_clear_stale_matches() {
        let mut view = create_test_view();
        view.open_search();
        view.set_search_matches(vec![SearchMatch {
            grid_line: 0,
            start_col: 0,
            end_grid_line: 0,
            end_col: 1,
        }]);

        view.append_search_text("a");

        assert!(view.search().matches.is_empty());
        assert_eq!(view.search().current_match, None);
    }

    #[test]
    fn test_scrollbar_live_view_renders_at_bottom() {
        let mut view = create_test_view();
        view.history_size = 100;
        view.display_offset = 0;

        let (_, thumb_top) = view.scrollbar_thumb_metrics(200.0);

        assert!(thumb_top > 0.0);
    }

    #[test]
    fn test_scrollbar_pointer_mapping_uses_bottom_as_live_view() {
        let mut view = create_test_view();
        view.history_size = 100;
        view.display_offset = 0;

        let top_target = view.scrollbar_offset_for_pointer(0.0, 200.0, None);
        let bottom_target = view.scrollbar_offset_for_pointer(200.0, 200.0, None);

        assert_eq!(top_target, 100);
        assert_eq!(bottom_target, 0);
    }

    #[test]
    fn test_pixel_size() {
        let view = create_test_view();
        let (width, height) = view.pixel_size();
        assert_eq!(width, view.cell_width() * view.cols() as f32);
        assert_eq!(height, view.cell_height() * view.rows() as f32);
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
    fn test_pixel_to_cell_clamped() {
        let view = create_test_view();
        assert_eq!(view.pixel_to_cell_clamped(-1.0, -1.0), (0, 0, -1));
        assert_eq!(view.pixel_to_cell_clamped(40.0, 32.0), (2, 5, 0));
        assert_eq!(
            view.pixel_to_cell_clamped(10_000.0, 10_000.0),
            (view.rows() as usize - 1, view.cols() as usize - 1, 1)
        );
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
    fn test_cached_content_empty() {
        let mut view = create_test_view();
        let content = view.cached_content();
        assert!(
            content.text_runs_hsla.is_empty() && content.bg_rects_hsla.is_empty(),
            "Expected empty terminal to produce no text runs or background rects"
        );
    }

    #[test]
    fn test_cached_content_with_content() {
        let mut view = create_test_view();
        view.apply_output_for_test(b"Hello");
        let content = view.cached_content();
        assert!(
            !content.text_runs_hsla.is_empty(),
            "Expected non-empty text runs after writing 'Hello'"
        );
        let has_h = content
            .text_runs_hsla
            .iter()
            .any(|(run, _)| run.text.contains('H'));
        assert!(has_h, "Expected a text run containing 'H'");
    }

    #[test]
    fn test_resize_to_fit() {
        let mut view = create_test_view();
        view.resize_to_fit(400.0, 200.0);
        assert_eq!(view.cols(), (400.0 / view.cell_width()).floor() as u16);
        assert_eq!(view.rows(), (200.0 / view.cell_height()).floor() as u16);
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
    fn test_cached_content_consecutive_rows() {
        let mut view = create_test_view();
        // Simulate multi-line output with Windows-style \r\n endings
        // This mimics what ConPTY sends for a simple dir/ls listing
        view.apply_output_for_test(
            b"file1.txt\x1b[K\r\nfile2.txt\x1b[K\r\nfile3.txt\x1b[K\r\nfile4.txt\x1b[K\r\n",
        );
        let content = view.cached_content();

        // Collect unique sorted row indices from text runs
        let mut rows: Vec<usize> = content
            .text_runs_hsla
            .iter()
            .map(|(run, _)| run.row)
            .collect();
        rows.sort();
        rows.dedup();

        assert!(
            rows.len() >= 4,
            "Expected at least 4 content rows, got {:?}",
            rows
        );

        // Verify rows are consecutive (no gaps)
        for i in 1..rows.len() {
            assert_eq!(
                rows[i],
                rows[i - 1] + 1,
                "Rows must be consecutive but found gap: {:?}",
                rows
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
    fn test_start_selection_uses_grid_line_when_scrolled_back() {
        let mut view = create_test_view();

        let mut output = String::new();
        for i in 0..40 {
            output.push_str(&format!("row{i:02}\r\n"));
        }
        view.apply_output_for_test(output.as_bytes());
        view.scroll_up(3);

        view.start_selection(0, 1);
        assert_eq!(view.selection().start.unwrap(), (-3, 1));
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
    fn test_selection_stays_active_after_update() {
        let mut view = create_test_view();
        view.start_selection(5, 10);
        view.update_selection(10, 20);
        // Selection stays active until cleared — no separate end_selection needed
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
        view.apply_output_for_test(b"Hello, World!");
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
    fn test_get_selected_text_remains_bound_to_buffer_while_scrolling() {
        let mut view = create_test_view();

        let mut output = String::new();
        for i in 0..40 {
            output.push_str(&format!("row{i:02}\r\n"));
        }
        view.apply_output_for_test(output.as_bytes());
        view.scroll_up(5);

        view.start_selection(0, 0);
        view.update_selection(0, 4);
        let before = view.get_selected_text();
        assert_eq!(before.as_deref(), Some("row12"));

        view.scroll_up(2);
        let after_scroll_up = view.get_selected_text();
        assert_eq!(after_scroll_up, before);

        view.scroll_down(1);
        let after_scroll_down = view.get_selected_text();
        assert_eq!(after_scroll_down, before);
    }

    #[test]
    fn test_cached_content_row_indices_with_ansi() {
        let mut view = create_test_view();
        // More realistic ConPTY output with ANSI sequences
        view.apply_output_for_test(
            b"\x1b[?25l\x1b[2J\x1b[H\
            Row0 text here\x1b[K\r\n\
            Row1 text here\x1b[K\r\n\
            Row2 text here\x1b[K\r\n\
            Row3 text here\x1b[K\r\n\
            Row4 text here\x1b[K\r\n\
            \x1b[?25h",
        );
        let content = view.cached_content();

        let mut rows: Vec<usize> = content
            .text_runs_hsla
            .iter()
            .map(|(run, _)| run.row)
            .collect();
        rows.sort();
        rows.dedup();

        assert!(
            rows.len() >= 5,
            "Expected at least 5 content rows, got {:?}",
            rows
        );

        for i in 1..rows.len() {
            assert_eq!(
                rows[i],
                rows[i - 1] + 1,
                "Rows must be consecutive with ANSI output: {:?}",
                rows
            );
        }
    }

    #[test]
    fn test_cached_content_wide_char_background_rect() {
        let mut view = create_test_view();
        // '中' (U+4E2D) is a CJK character that occupies 2 columns.
        // \x1b[41m sets red background; \x1b[0m resets.
        view.apply_output_for_test("\x1b[41m中\x1b[0m".as_bytes());
        let content = view.cached_content();
        // The background rect must span both columns of the wide char (start=0, end=2).
        let has_two_col_rect = content
            .bg_rects_hsla
            .iter()
            .any(|(_, start, end, _)| *end - *start >= 2);
        assert!(
            has_two_col_rect,
            "Expected a 2-column-wide background rect for wide char '中', got: {:?}",
            content
                .bg_rects_hsla
                .iter()
                .map(|(r, s, e, _)| (r, s, e))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_apply_snapshot_ignores_stale_generation() {
        let mut view = create_test_view();
        view.apply_output_for_test(b"new");
        let current = view.rows();
        let stale = TerminalRenderSnapshot {
            generation: 0,
            rows: 2,
            cols: 2,
            mode: TermMode::empty(),
            history_size: 0,
            display_offset: 0,
            cached_rows: Vec::new(),
            dirty_rows: None,
            cursor_viewport_cell: None,
        };

        assert!(!view.apply_snapshot(stale));
        assert_eq!(view.rows(), current);
    }

    #[test]
    fn test_selection_rects_follow_scrollback() {
        let mut view = create_test_view();

        let mut output = String::new();
        for i in 0..40 {
            output.push_str(&format!("row{i:02}\r\n"));
        }
        view.apply_output_for_test(output.as_bytes());
        view.scroll_up(3);
        view.start_selection(0, 0);
        view.update_selection(0, 4);

        let rects = view.selection_rects_hsla();
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].0, 0);
        assert_eq!(rects[0].1, 0);
        assert_eq!(rects[0].2, 5);
    }
}
