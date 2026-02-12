//! Clipboard state management for WorkspaceView.

use crate::clipboard_preview::ClipboardPreview;
use crate::smart_clipboard::SmartClipboardProvider;
use codirigent_session::clipboard_service::DefaultClipboardService;

/// Groups all clipboard-related state for the workspace.
pub(super) struct ClipboardState {
    /// Smart clipboard provider for cross-platform clipboard access.
    pub(super) smart_clipboard: Box<dyn SmartClipboardProvider>,
    /// Clipboard polling service.
    pub(super) clipboard_service: DefaultClipboardService,
    /// Preview panel for clipboard content.
    pub(super) clipboard_preview: ClipboardPreview,
    /// Timestamp when clipboard preview was last shown (for auto-hide).
    pub(super) clipboard_preview_shown_at: Option<std::time::Instant>,
}
