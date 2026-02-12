//! GPUI action handlers for WorkspaceView.
//!
//! This module contains keyboard shortcut action handlers for:
//! - Session management (New, Close, Focus)
//! - Layout operations (Split, ClosePane, NextLayout)
//! - Sidebar toggle

use super::gpui::WorkspaceView;
use crate::app::*;
use codirigent_core::SplitDirection;
use gpui::{Context, Window};
use tracing::info;

impl WorkspaceView {
    /// Handle NewSession action (Cmd+N).
    pub(super) fn handle_new_session(
        &mut self,
        _action: &NewSession,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        info!("NewSession action triggered");
        self.create_session(cx);
    }

    /// Handle CloseSession action (Cmd+W).
    pub(super) fn handle_close_session(
        &mut self,
        _action: &CloseSession,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        info!("CloseSession action triggered");
        self.close_focused_session(cx);
    }

    /// Handle SplitHorizontal action (Cmd+D).
    pub(super) fn handle_split_horizontal(
        &mut self,
        _action: &SplitHorizontal,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        info!("SplitHorizontal action triggered");
        if let Some(slot) = self.workspace.split_pane(SplitDirection::Horizontal, 0.5) {
            info!(?slot, "Split pane horizontally, new slot created");
            cx.notify();
        }
    }

    /// Handle SplitVertical action (Cmd+Shift+D).
    pub(super) fn handle_split_vertical(
        &mut self,
        _action: &SplitVertical,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        info!("SplitVertical action triggered");
        if let Some(slot) = self.workspace.split_pane(SplitDirection::Vertical, 0.5) {
            info!(?slot, "Split pane vertically, new slot created");
            cx.notify();
        }
    }

    /// Handle ClosePane action (Cmd+Shift+W).
    pub(super) fn handle_close_pane(
        &mut self,
        _action: &ClosePane,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        info!("ClosePane action triggered");
        // Get the session in the focused slot BEFORE closing the pane
        let session_to_close = self.workspace.focused_session_id();

        if self.workspace.close_pane() {
            info!("Closed focused pane");
            // If the closed pane had a session, clean it up fully
            if let Some(id) = session_to_close {
                self.close_session(id, cx);
            } else {
                cx.notify();
            }
        }
    }

    /// Handle NextLayout action (Cmd+\).
    pub(super) fn handle_next_layout(
        &mut self,
        _action: &NextLayout,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        info!("NextLayout action triggered");
        self.next_layout(cx);
    }

    /// Handle ToggleSidebar action (Cmd+B).
    pub(super) fn handle_toggle_sidebar(
        &mut self,
        _action: &ToggleSidebar,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        info!("ToggleSidebar action triggered");
        self.toggle_sidebar(cx);
    }

    /// Handle FocusSession1 action (Cmd+1).
    pub(super) fn handle_focus_session1(
        &mut self,
        _action: &FocusSession1,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_session_number(1, cx);
    }

    /// Handle FocusSession2 action (Cmd+2).
    pub(super) fn handle_focus_session2(
        &mut self,
        _action: &FocusSession2,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_session_number(2, cx);
    }

    /// Handle FocusSession3 action (Cmd+3).
    pub(super) fn handle_focus_session3(
        &mut self,
        _action: &FocusSession3,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_session_number(3, cx);
    }

    /// Handle FocusSession4 action (Cmd+4).
    pub(super) fn handle_focus_session4(
        &mut self,
        _action: &FocusSession4,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_session_number(4, cx);
    }

    /// Handle FocusSession5 action (Cmd+5).
    pub(super) fn handle_focus_session5(
        &mut self,
        _action: &FocusSession5,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_session_number(5, cx);
    }

    /// Handle FocusSession6 action (Cmd+6).
    pub(super) fn handle_focus_session6(
        &mut self,
        _action: &FocusSession6,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_session_number(6, cx);
    }

    /// Handle FocusSession7 action (Cmd+7).
    pub(super) fn handle_focus_session7(
        &mut self,
        _action: &FocusSession7,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_session_number(7, cx);
    }

    /// Handle FocusSession8 action (Cmd+8).
    pub(super) fn handle_focus_session8(
        &mut self,
        _action: &FocusSession8,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_session_number(8, cx);
    }

    /// Handle FocusSession9 action (Cmd+9).
    pub(super) fn handle_focus_session9(
        &mut self,
        _action: &FocusSession9,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_session_number(9, cx);
    }
}
