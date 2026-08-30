use crate::clipboard;
use crate::terminal::{Terminal, TerminalSize};
use crate::terminal_colors::{convert_color, dim_color};
use crate::terminal_search::{self, SearchMatch};
use crate::terminal_view::{CachedTerminalRow, Selection, TextRunSegment};
use crate::theme::{CodirigentTheme, Rgba};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags as CellFlags;
use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::{Color as TermColor, NamedColor};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub(crate) struct TerminalRenderSnapshot {
    pub(crate) generation: u64,
    pub(crate) rows: u16,
    pub(crate) cols: u16,
    pub(crate) mode: TermMode,
    pub(crate) history_size: usize,
    pub(crate) display_offset: usize,
    pub(crate) cached_rows: Vec<CachedTerminalRow>,
    pub(crate) dirty_rows: Option<Vec<usize>>,
    pub(crate) cursor_viewport_cell: Option<(usize, usize)>,
}

impl TerminalRenderSnapshot {
    /// Reconstruct the current visible terminal viewport as plain text.
    pub(crate) fn visible_text(&self) -> String {
        let mut lines = Vec::with_capacity(self.cached_rows.len());
        for row in &self.cached_rows {
            let mut runs = row.text_runs_hsla.iter().collect::<Vec<_>>();
            runs.sort_by_key(|(run, _)| run.start_col);
            let mut line = String::new();
            let mut cell_cursor = 0usize;
            for (run, _) in runs {
                if run.start_col > cell_cursor {
                    line.push_str(&" ".repeat(run.start_col - cell_cursor));
                }
                line.push_str(&run.text);
                cell_cursor = run.start_col + run.cell_count;
            }
            lines.push(line.trim_end().to_string());
        }
        while lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }
        lines.join("\n")
    }
}

struct TerminalRuntime {
    terminal: Terminal,
    theme: CodirigentTheme,
    generation: u64,
    last_snapshot_mode: TermMode,
    cached_rows: Option<Vec<CachedTerminalRow>>,
    cached_search_snapshot: Option<Arc<terminal_search::SearchSnapshot>>,
}

#[derive(Clone)]
pub(crate) struct TerminalRuntimeHandle {
    inner: Arc<Mutex<TerminalRuntime>>,
}

impl TerminalRuntimeHandle {
    pub(crate) fn new(
        mut terminal: Terminal,
        theme: CodirigentTheme,
        initial_size: TerminalSize,
    ) -> (Self, TerminalRenderSnapshot) {
        terminal.resize_with_cells(initial_size);
        let last_snapshot_mode = terminal.mode();
        let mut runtime = TerminalRuntime {
            terminal,
            theme,
            generation: 0,
            last_snapshot_mode,
            cached_rows: None,
            cached_search_snapshot: None,
        };
        let snapshot = runtime.snapshot_full();
        (
            Self {
                inner: Arc::new(Mutex::new(runtime)),
            },
            snapshot,
        )
    }

    pub(crate) fn apply_output(&self, data: &[u8]) -> Option<TerminalRenderSnapshot> {
        self.with_runtime_mut(|runtime| runtime.apply_output(data))
    }

    pub(crate) fn resize_with_cells(&self, size: TerminalSize) -> Option<TerminalRenderSnapshot> {
        self.with_runtime_mut(|runtime| runtime.resize_with_cells(size))
    }

    pub(crate) fn scroll_up(&self, lines: usize) -> Option<TerminalRenderSnapshot> {
        self.with_runtime_mut(|runtime| runtime.scroll_display(Scroll::Delta(lines as i32)))
    }

    pub(crate) fn scroll_down(&self, lines: usize) -> Option<TerminalRenderSnapshot> {
        self.with_runtime_mut(|runtime| runtime.scroll_display(Scroll::Delta(-(lines as i32))))
    }

    pub(crate) fn scroll_to_bottom(&self) -> Option<TerminalRenderSnapshot> {
        self.with_runtime_mut(|runtime| runtime.scroll_display(Scroll::Bottom))
    }

    pub(crate) fn scroll_to_offset(&self, target: usize) -> Option<TerminalRenderSnapshot> {
        self.with_runtime_mut(|runtime| runtime.scroll_to_offset(target))
    }

    pub(crate) fn clear(&self) -> Option<TerminalRenderSnapshot> {
        self.with_runtime_mut(TerminalRuntime::clear)
    }

    pub(crate) fn set_theme(&self, theme: CodirigentTheme) -> Option<TerminalRenderSnapshot> {
        self.with_runtime_mut(|runtime| runtime.set_theme(theme))
    }

    pub(crate) fn get_selected_text(&self, selection: &Selection) -> Option<String> {
        let (start, end) = selection.normalized()?;
        self.with_runtime(|runtime| {
            let text = clipboard::copy_selection(runtime.terminal.term(), start, end);
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        })
        .flatten()
    }

    pub(crate) fn search(&self, query: &str) -> Vec<SearchMatch> {
        let snapshot = self
            .with_runtime_mut(TerminalRuntime::cached_search_snapshot)
            .unwrap_or_default();
        terminal_search::search_snapshot(&snapshot, query)
    }

    pub(crate) fn match_still_matches(&self, query: &str, search_match: &SearchMatch) -> bool {
        self.with_runtime(|runtime| {
            terminal_search::match_still_matches(runtime.terminal.term(), query, search_match)
        })
        .unwrap_or(false)
    }

    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> Option<TerminalRenderSnapshot> {
        self.with_runtime_mut(|runtime| runtime.snapshot_full())
    }

    fn with_runtime<R>(&self, f: impl FnOnce(&TerminalRuntime) -> R) -> Option<R> {
        match self.inner.lock() {
            Ok(runtime) => Some(f(&runtime)),
            Err(_) => None,
        }
    }

    fn with_runtime_mut<R>(&self, f: impl FnOnce(&mut TerminalRuntime) -> R) -> Option<R> {
        match self.inner.lock() {
            Ok(mut runtime) => Some(f(&mut runtime)),
            Err(_) => None,
        }
    }
}

impl TerminalRuntime {
    fn invalidate_search_snapshot(&mut self) {
        self.cached_search_snapshot = None;
    }

    fn cached_search_snapshot(&mut self) -> Arc<terminal_search::SearchSnapshot> {
        if let Some(snapshot) = &self.cached_search_snapshot {
            return Arc::clone(snapshot);
        }

        let snapshot = Arc::new(terminal_search::snapshot(self.terminal.term()));
        self.cached_search_snapshot = Some(Arc::clone(&snapshot));
        snapshot
    }

    fn apply_output(&mut self, data: &[u8]) -> TerminalRenderSnapshot {
        self.terminal.process_output(data);
        self.generation += 1;
        self.invalidate_search_snapshot();
        self.snapshot_from_damage()
    }

    fn resize_with_cells(&mut self, size: TerminalSize) -> TerminalRenderSnapshot {
        self.terminal.resize_with_cells(size);
        self.generation += 1;
        self.invalidate_search_snapshot();
        self.snapshot_full()
    }

    fn scroll_display(&mut self, scroll: Scroll) -> TerminalRenderSnapshot {
        self.terminal.term_mut().scroll_display(scroll);
        self.generation += 1;
        // Scrolling changes the viewport, not the underlying terminal content,
        // so the cached search snapshot remains valid.
        self.snapshot_full()
    }

    fn scroll_to_offset(&mut self, target: usize) -> TerminalRenderSnapshot {
        let current = self.terminal.term().grid().display_offset();
        let delta =
            ((target as i64) - (current as i64)).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        self.scroll_display(Scroll::Delta(delta))
    }

    fn clear(&mut self) -> TerminalRenderSnapshot {
        self.terminal.clear();
        self.generation += 1;
        self.invalidate_search_snapshot();
        self.snapshot_full()
    }

    fn set_theme(&mut self, theme: CodirigentTheme) -> TerminalRenderSnapshot {
        self.theme = theme;
        self.generation += 1;
        self.snapshot_full()
    }

    fn snapshot_from_damage(&mut self) -> TerminalRenderSnapshot {
        let rows = self.terminal.rows() as usize;
        let cols = self.terminal.cols() as usize;
        let scrolled_back = self.terminal.term().grid().display_offset() > 0;
        let alternate_screen_changed = self.last_snapshot_mode.contains(TermMode::ALT_SCREEN)
            != self.terminal.mode().contains(TermMode::ALT_SCREEN);
        let damage = if !alternate_screen_changed
            && !scrolled_back
            && self
                .cached_rows
                .as_ref()
                .is_some_and(|cached_rows| cached_rows.len() == rows)
        {
            let term = self.terminal.term_mut();
            let damage = match term.damage() {
                alacritty_terminal::term::TermDamage::Full => None,
                alacritty_terminal::term::TermDamage::Partial(lines) => {
                    Some(lines.map(|line| line.line).collect::<Vec<_>>())
                }
            };
            term.reset_damage();
            damage
        } else {
            self.terminal.term_mut().reset_damage();
            None
        };

        let dirty_rows = if let Some(dirty_rows) = damage {
            if let Some(cached_rows) = self.cached_rows.as_mut() {
                for row in dirty_rows.iter().copied().filter(|row| *row < rows) {
                    cached_rows[row] = build_row_cache(&self.terminal, &self.theme, row, cols);
                }
                Some(dirty_rows)
            } else {
                None
            }
        } else {
            None
        };

        if dirty_rows.is_none() {
            self.cached_rows = Some(
                (0..rows)
                    .map(|row| build_row_cache(&self.terminal, &self.theme, row, cols))
                    .collect(),
            );
        }

        self.build_snapshot(dirty_rows)
    }

    fn snapshot_full(&mut self) -> TerminalRenderSnapshot {
        self.terminal.term_mut().reset_damage();
        self.cached_rows = Some(
            (0..self.terminal.rows() as usize)
                .map(|row| {
                    build_row_cache(
                        &self.terminal,
                        &self.theme,
                        row,
                        self.terminal.cols() as usize,
                    )
                })
                .collect(),
        );
        self.build_snapshot(None)
    }

    fn build_snapshot(&mut self, dirty_rows: Option<Vec<usize>>) -> TerminalRenderSnapshot {
        let rows = self.terminal.rows();
        let cols = self.terminal.cols();
        let mode = self.terminal.mode();
        let term = self.terminal.term();
        let content = term.renderable_content();
        let display_offset = content.display_offset;
        let history_size = term.topmost_line().0.unsigned_abs() as usize;
        let viewport_line = content.cursor.point.line.0 + display_offset as i32;
        let cursor_viewport_cell = if viewport_line >= 0 && (viewport_line as usize) < rows as usize
        {
            Some((viewport_line as usize, content.cursor.point.column.0))
        } else {
            None
        };
        self.terminal.mark_clean();
        self.last_snapshot_mode = mode;

        TerminalRenderSnapshot {
            generation: self.generation,
            rows,
            cols,
            mode,
            history_size,
            display_offset,
            cached_rows: self.cached_rows.clone().unwrap_or_default(),
            dirty_rows,
            cursor_viewport_cell,
        }
    }
}

fn build_row_cache(
    terminal: &Terminal,
    theme: &CodirigentTheme,
    row: usize,
    cols: usize,
) -> CachedTerminalRow {
    let display_offset = terminal.term().grid().display_offset();
    let grid_line = Line(row as i32) - display_offset;
    let grid = terminal.term().grid();

    let mut text_runs: Vec<TextRunSegment> = Vec::new();
    let mut background_rects: Vec<(usize, usize, usize, Rgba)> = Vec::new();
    let mut current_run: Option<TextRunSegment> = None;

    for col in 0..cols {
        let cell = &grid[grid_line][Column(col)];
        let c = cell.c;

        if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
            let bg = convert_color(cell.bg, theme);
            if bg != theme.terminal_background {
                let merged = background_rects.last_mut().and_then(
                    |last: &mut (usize, usize, usize, Rgba)| {
                        if last.0 == row && last.2 == col && last.3 == bg {
                            last.2 = col + 1;
                            Some(())
                        } else {
                            None
                        }
                    },
                );
                if merged.is_none() {
                    background_rects.push((row, col, col + 1, bg));
                }
            }
            continue;
        }

        if c == ' ' && cell.bg == TermColor::Named(NamedColor::Background) {
            continue;
        }

        let mut foreground = convert_color(cell.fg, theme);
        let mut background = convert_color(cell.bg, theme);

        if cell.flags.contains(CellFlags::INVERSE) {
            std::mem::swap(&mut foreground, &mut background);
        }
        if cell.flags.contains(CellFlags::DIM) {
            foreground = dim_color(foreground);
        }

        let same_style = current_run.as_ref().is_some_and(|run| {
            run.row == row
                && run.foreground == foreground
                && run.bold == cell.flags.contains(CellFlags::BOLD)
                && run.italic == cell.flags.contains(CellFlags::ITALIC)
                && run.underline == cell.flags.contains(CellFlags::UNDERLINE)
                && run.strikethrough == cell.flags.contains(CellFlags::STRIKEOUT)
                && run.start_col + run.cell_count == col
        });

        if same_style {
            if let Some(run) = current_run.as_mut() {
                run.text.push(c);
                run.cell_count += 1;
            }
        } else {
            if let Some(run) = current_run.take() {
                text_runs.push(run);
            }
            current_run = Some(TextRunSegment {
                text: String::from(c),
                foreground,
                bold: cell.flags.contains(CellFlags::BOLD),
                italic: cell.flags.contains(CellFlags::ITALIC),
                underline: cell.flags.contains(CellFlags::UNDERLINE),
                strikethrough: cell.flags.contains(CellFlags::STRIKEOUT),
                row,
                start_col: col,
                cell_count: 1,
            });
        }

        if background != theme.terminal_background {
            let merged =
                background_rects
                    .last_mut()
                    .and_then(|last: &mut (usize, usize, usize, Rgba)| {
                        if last.0 == row && last.2 == col && last.3 == background {
                            last.2 = col + 1;
                            Some(())
                        } else {
                            None
                        }
                    });
            if merged.is_none() {
                background_rects.push((row, col, col + 1, background));
            }
        }
    }

    if let Some(run) = current_run.take() {
        text_runs.push(run);
    }

    CachedTerminalRow {
        bg_rects_hsla: Arc::new(
            background_rects
                .into_iter()
                .map(|(r, start, end, color)| (r, start, end, color.into()))
                .collect(),
        ),
        text_runs_hsla: Arc::new(
            text_runs
                .into_iter()
                .map(|run| {
                    let fg: gpui::Hsla = run.foreground.into();
                    (run, fg)
                })
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codirigent_core::SessionId;

    fn create_runtime() -> TerminalRuntimeHandle {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let terminal = Terminal::new(4, 8, SessionId(1), tx);
        let (runtime, _snapshot) = TerminalRuntimeHandle::new(
            terminal,
            CodirigentTheme::dark(),
            TerminalSize::new(4, 8, 8.0, 16.0),
        );
        runtime
    }

    #[test]
    fn runtime_snapshots_advance_generation() {
        let runtime = create_runtime();
        let initial = runtime.snapshot().expect("runtime snapshot");
        let next = runtime
            .apply_output(b"hello")
            .expect("runtime output snapshot");

        assert!(next.generation > initial.generation);
        assert_eq!(next.rows, 4);
        assert_eq!(next.cols, 8);
        assert!(next.dirty_rows.is_some());
    }

    #[test]
    fn runtime_alt_screen_exit_rebuilds_all_cached_rows() {
        let runtime = create_runtime();
        let entered = runtime
            .apply_output(b"\x1b[?1049h\x1b[2J\x1b[4;1HKIMI")
            .expect("alternate-screen snapshot");
        assert!(entered.mode.contains(TermMode::ALT_SCREEN));

        let exited = runtime
            .apply_output(b"\x1b[?1049lPS> ")
            .expect("primary-screen snapshot");
        let visible_text = exited
            .cached_rows
            .iter()
            .flat_map(|row| row.text_runs_hsla.iter())
            .map(|(run, _)| run.text.as_str())
            .collect::<String>();

        assert!(!exited.mode.contains(TermMode::ALT_SCREEN));
        assert_eq!(exited.dirty_rows, None);
        assert!(!visible_text.contains("KIMI"));
    }

    #[test]
    fn snapshot_visible_text_preserves_row_and_column_spacing() {
        let runtime = create_runtime();
        let snapshot = runtime
            .apply_output(b"left  ok\r\nnext")
            .expect("runtime output snapshot");

        let visible_text = snapshot.visible_text();

        assert!(visible_text.contains("left  ok\nnext"));
    }

    #[test]
    fn runtime_resize_updates_dimensions() {
        let runtime = create_runtime();
        let snapshot = runtime
            .resize_with_cells(TerminalSize::new(6, 10, 8.0, 16.0))
            .expect("runtime resize snapshot");

        assert_eq!(snapshot.rows, 6);
        assert_eq!(snapshot.cols, 10);
    }

    #[test]
    fn runtime_selection_reads_from_background_terminal() {
        let runtime = create_runtime();
        let _ = runtime.apply_output(b"hello");
        let mut selection = Selection::default();
        selection.set_start(0, 0);
        selection.set_end(0, 4);

        assert_eq!(
            runtime.get_selected_text(&selection),
            Some("hello".to_string())
        );
    }

    #[test]
    fn runtime_search_reuses_cached_snapshot_until_content_changes() {
        let runtime = create_runtime();
        let _ = runtime.apply_output(b"alpha beta gamma");

        assert!(!runtime
            .with_runtime(|runtime| runtime.cached_search_snapshot.is_some())
            .unwrap_or(false));

        let first = runtime.search("alpha");
        assert_eq!(first.len(), 1);
        assert!(runtime
            .with_runtime(|runtime| runtime.cached_search_snapshot.is_some())
            .unwrap_or(false));

        let second = runtime.search("beta");
        assert_eq!(second.len(), 1);
        assert!(runtime
            .with_runtime(|runtime| runtime.cached_search_snapshot.is_some())
            .unwrap_or(false));

        let _ = runtime.apply_output(b"\ndelta");
        assert!(!runtime
            .with_runtime(|runtime| runtime.cached_search_snapshot.is_some())
            .unwrap_or(false));

        let third = runtime.search("delta");
        assert_eq!(third.len(), 1);
    }

    #[test]
    fn runtime_scroll_to_offset_clamps_large_deltas() {
        let runtime = create_runtime();

        let snapshot = runtime
            .scroll_to_offset(usize::MAX)
            .expect("runtime scroll snapshot");

        assert_eq!(snapshot.display_offset, snapshot.history_size);
    }
}
