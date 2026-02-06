//! Expandable drawer panel (288px wide).
//!
//! Shows contextual content scoped to the selected session:
//! - Files panel: file tree for the session's project
//! - Worktrees panel: git worktrees grouped by repository

use codirigent_core::SessionId;

use crate::icon_rail::DrawerPanel;

/// Drawer panel state.
#[derive(Debug)]
pub struct Drawer {
    /// Width of the drawer in pixels.
    width: f32,
    /// Currently selected session (drives content scope).
    selected_session: Option<SessionId>,
    /// Which panel to display.
    active_panel: Option<DrawerPanel>,
}

impl Drawer {
    /// Default drawer width in pixels.
    pub const WIDTH: f32 = 288.0;

    /// Create a new drawer.
    pub fn new() -> Self {
        Self {
            width: Self::WIDTH,
            selected_session: None,
            active_panel: None,
        }
    }

    /// Update the active panel (synced from IconRail).
    pub fn set_active_panel(&mut self, panel: Option<DrawerPanel>) {
        self.active_panel = panel;
    }

    /// Get the active panel.
    pub fn active_panel(&self) -> Option<DrawerPanel> {
        self.active_panel
    }

    /// Update the selected session (drives file tree + git scope).
    pub fn set_selected_session(&mut self, session_id: Option<SessionId>) {
        self.selected_session = session_id;
    }

    /// Get the selected session.
    pub fn selected_session(&self) -> Option<SessionId> {
        self.selected_session
    }

    /// Whether the drawer is visible.
    pub fn is_open(&self) -> bool {
        self.active_panel.is_some()
    }

    /// Drawer width.
    pub fn width(&self) -> f32 {
        self.width
    }
}

impl Default for Drawer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawer_starts_closed() {
        let drawer = Drawer::new();
        assert!(!drawer.is_open());
        assert_eq!(drawer.selected_session(), None);
        assert_eq!(drawer.active_panel(), None);
    }

    #[test]
    fn set_active_panel_opens_drawer() {
        let mut drawer = Drawer::new();
        drawer.set_active_panel(Some(DrawerPanel::Files));
        assert!(drawer.is_open());
        assert_eq!(drawer.active_panel(), Some(DrawerPanel::Files));
    }

    #[test]
    fn close_drawer_by_setting_none() {
        let mut drawer = Drawer::new();
        drawer.set_active_panel(Some(DrawerPanel::Files));
        drawer.set_active_panel(None);
        assert!(!drawer.is_open());
    }

    #[test]
    fn selected_session_updates() {
        let mut drawer = Drawer::new();
        drawer.set_selected_session(Some(SessionId(42)));
        assert_eq!(drawer.selected_session(), Some(SessionId(42)));
    }

    #[test]
    fn width_is_default() {
        let drawer = Drawer::new();
        assert_eq!(drawer.width(), Drawer::WIDTH);
        assert_eq!(drawer.width(), 288.0);
    }

    #[test]
    fn default_is_same_as_new() {
        let default = Drawer::default();
        let new = Drawer::new();
        assert_eq!(default.is_open(), new.is_open());
        assert_eq!(default.selected_session(), new.selected_session());
    }
}
