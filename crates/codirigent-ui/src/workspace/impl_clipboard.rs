//! Clipboard operations for WorkspaceView.
//!
//! This module contains all methods related to:
//! - Copy/paste handling
//! - Image clipboard formatting
//! - File path clipboard operations

use super::gpui::WorkspaceView;
use crate::app::{Copy, Paste};
use codirigent_core::{ClipboardContent, SessionManager};
use codirigent_session::clipboard_service::ClipboardService;
use gpui::{Context, Window};
use tracing::warn;

impl WorkspaceView {
    pub(super) fn handle_paste(
        &mut self,
        _action: &Paste,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(session_id) = self.workspace.focused_session_id() else {
            return;
        };

        // Read bracketed paste mode from terminal
        let bracketed = self
            .terminals
            .get(&session_id)
            .map(|tv| tv.terminal().bracketed_paste_mode())
            .unwrap_or(false);

        // Read clipboard content
        let content = match self.clipboard.smart_clipboard.read_content() {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to read clipboard: {}", e);
                return;
            }
        };

        match content {
            ClipboardContent::Text(text) => {
                if text.is_empty() {
                    return;
                }
                let sanitized = crate::clipboard::sanitize_paste(&text);
                let bytes = crate::clipboard::prepare_paste(&sanitized, bracketed);

                // Auto-scroll to bottom on paste
                if let Some(tv) = self.terminals.get_mut(&session_id) {
                    tv.scroll_to_bottom();
                }

                self.with_session_manager(|manager| {
                    if let Err(e) = manager.send_input(session_id, &bytes) {
                        warn!("Failed to paste to session {}: {}", session_id, e);
                    }
                });
            }
            ClipboardContent::Image(ref _image_data) => {
                // Get the CLI type for the focused session (defaults to ClaudeCode)
                let cli_type = self.clipboard.clipboard_service.get_session_cli_type(session_id);

                // Format for CLI: saves image to temp file and returns path string
                match self.clipboard.clipboard_service.format_for_cli(&content, cli_type) {
                    Ok(formatted_path) => {
                        if formatted_path.is_empty() {
                            return;
                        }
                        let sanitized = crate::clipboard::sanitize_paste(&formatted_path);
                        let bytes = crate::clipboard::prepare_paste(&sanitized, bracketed);

                        // Auto-scroll to bottom on paste
                        if let Some(tv) = self.terminals.get_mut(&session_id) {
                            tv.scroll_to_bottom();
                        }

                        self.with_session_manager(|manager| {
                            if let Err(e) = manager.send_input(session_id, &bytes) {
                                warn!(
                                    "Failed to paste image path to session {}: {}",
                                    session_id, e
                                );
                            }
                        });

                        // Hide clipboard preview on paste
                        self.clipboard.clipboard_preview.hide();
                        self.clipboard.clipboard_preview_shown_at = None;
                    }
                    Err(e) => {
                        warn!("Failed to format image for CLI: {:?}", e);
                    }
                }
            }
            ClipboardContent::Files(paths) => {
                if paths.is_empty() {
                    return;
                }
                let text: String = paths
                    .iter()
                    .map(|p| {
                        if let Some(tree) = &self.project.file_tree_model {
                            tree.path_for_terminal(p)
                        } else {
                            p.to_string_lossy().to_string()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                let bytes = crate::clipboard::prepare_paste(&text, bracketed);

                if let Some(tv) = self.terminals.get_mut(&session_id) {
                    tv.scroll_to_bottom();
                }

                self.with_session_manager(|manager| {
                    if let Err(e) = manager.send_input(session_id, &bytes) {
                        warn!("Failed to paste files to session {}: {}", session_id, e);
                    }
                });
            }
            ClipboardContent::Empty => {}
        }

        cx.notify();
    }

    /// Handle Copy action (Cmd+C / Ctrl+C).
    ///
    /// Dual behavior:
    /// - If a text selection is active in the focused terminal, copies the
    ///   selected text to the system clipboard and clears the selection.
    /// - If no selection is active, sends Ctrl+C (interrupt, `\x03`) to the PTY.
    pub(super) fn handle_copy(&mut self, _action: &Copy, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(session_id) = self.workspace.focused_session_id() else {
            return;
        };

        // Check if there's an active selection in the focused terminal
        let selected_text = self
            .terminals
            .get(&session_id)
            .and_then(|tv| tv.get_selected_text());

        if let Some(text) = selected_text {
            // Copy selected text to system clipboard
            if let Err(e) = self.clipboard.smart_clipboard.write_text(text) {
                warn!("Failed to copy selection to clipboard: {}", e);
            }
            // Clear the selection
            if let Some(tv) = self.terminals.get_mut(&session_id) {
                tv.clear_selection();
            }
        } else {
            // No selection: send Ctrl+C (interrupt) to the PTY
            self.with_session_manager(|manager| {
                if let Err(e) = manager.send_input(session_id, b"\x03") {
                    warn!("Failed to send interrupt to session {}: {}", session_id, e);
                }
            });
        }

        cx.notify();
    }
}
