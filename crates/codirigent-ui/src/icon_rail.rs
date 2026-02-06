//! Narrow icon rail component (56px wide).
//!
//! Sits on the far left. Contains icon buttons that toggle
//! the expandable drawer panel for files, worktrees, and settings.

/// Which drawer panel is currently open (if any).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawerPanel {
    /// Session list panel.
    Sessions,
    /// File tree explorer.
    Files,
    /// Git worktree management.
    Worktrees,
}

/// Events emitted by the icon rail.
#[derive(Debug, Clone)]
pub enum IconRailEvent {
    /// A drawer panel was toggled (Some = opened, None = closed).
    DrawerToggled(Option<DrawerPanel>),
    /// Settings button clicked.
    SettingsRequested,
}

/// Icon rail state.
#[derive(Debug)]
pub struct IconRail {
    /// Currently active drawer panel (None = all closed).
    active_panel: Option<DrawerPanel>,
    /// Whether the worktree panel has a notification dot.
    worktree_has_notification: bool,
    /// Pending events.
    pending_events: Vec<IconRailEvent>,
}

impl IconRail {
    /// Icon rail width in pixels.
    pub const WIDTH: f32 = 56.0;

    /// Create a new icon rail.
    pub fn new() -> Self {
        Self {
            active_panel: None,
            worktree_has_notification: false,
            pending_events: Vec::new(),
        }
    }

    /// Get the currently active drawer panel.
    pub fn active_panel(&self) -> Option<DrawerPanel> {
        self.active_panel
    }

    /// Toggle a drawer panel. If it's already active, close it.
    pub fn toggle_panel(&mut self, panel: DrawerPanel) {
        if self.active_panel == Some(panel) {
            self.active_panel = None;
        } else {
            self.active_panel = Some(panel);
        }
        self.pending_events
            .push(IconRailEvent::DrawerToggled(self.active_panel));
    }

    /// Close any open drawer.
    pub fn close_drawer(&mut self) {
        if self.active_panel.is_some() {
            self.active_panel = None;
            self.pending_events
                .push(IconRailEvent::DrawerToggled(None));
        }
    }

    /// Set whether worktree icon shows a notification dot.
    pub fn set_worktree_notification(&mut self, has_notification: bool) {
        self.worktree_has_notification = has_notification;
    }

    /// Check if worktree has notification.
    pub fn has_worktree_notification(&self) -> bool {
        self.worktree_has_notification
    }

    /// Drain pending events.
    pub fn drain_events(&mut self) -> Vec<IconRailEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

impl Default for IconRail {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_icon_rail_starts_closed() {
        let rail = IconRail::new();
        assert_eq!(rail.active_panel(), None);
        assert!(!rail.has_worktree_notification());
    }

    #[test]
    fn toggle_panel_opens_and_closes() {
        let mut rail = IconRail::new();
        assert_eq!(rail.active_panel(), None);
        rail.toggle_panel(DrawerPanel::Files);
        assert_eq!(rail.active_panel(), Some(DrawerPanel::Files));
        rail.toggle_panel(DrawerPanel::Files);
        assert_eq!(rail.active_panel(), None);
    }

    #[test]
    fn toggle_different_panel_switches() {
        let mut rail = IconRail::new();
        rail.toggle_panel(DrawerPanel::Files);
        rail.toggle_panel(DrawerPanel::Worktrees);
        assert_eq!(rail.active_panel(), Some(DrawerPanel::Worktrees));
    }

    #[test]
    fn close_drawer() {
        let mut rail = IconRail::new();
        rail.toggle_panel(DrawerPanel::Files);
        rail.close_drawer();
        assert_eq!(rail.active_panel(), None);
    }

    #[test]
    fn close_drawer_when_already_closed_is_noop() {
        let mut rail = IconRail::new();
        rail.close_drawer();
        assert_eq!(rail.drain_events().len(), 0);
    }

    #[test]
    fn notification_dot() {
        let mut rail = IconRail::new();
        assert!(!rail.has_worktree_notification());
        rail.set_worktree_notification(true);
        assert!(rail.has_worktree_notification());
    }

    #[test]
    fn drain_events() {
        let mut rail = IconRail::new();
        rail.toggle_panel(DrawerPanel::Files);
        rail.toggle_panel(DrawerPanel::Worktrees);
        let events = rail.drain_events();
        assert_eq!(events.len(), 2);
        assert_eq!(rail.drain_events().len(), 0);
    }

    #[test]
    fn default_is_same_as_new() {
        let default = IconRail::default();
        let new = IconRail::new();
        assert_eq!(default.active_panel(), new.active_panel());
    }

    #[test]
    fn toggle_sessions_panel() {
        let mut rail = IconRail::new();
        rail.toggle_panel(DrawerPanel::Sessions);
        assert_eq!(rail.active_panel(), Some(DrawerPanel::Sessions));
        rail.toggle_panel(DrawerPanel::Sessions);
        assert_eq!(rail.active_panel(), None);
    }

    #[test]
    fn toggle_from_sessions_to_files() {
        let mut rail = IconRail::new();
        rail.toggle_panel(DrawerPanel::Sessions);
        assert_eq!(rail.active_panel(), Some(DrawerPanel::Sessions));
        rail.toggle_panel(DrawerPanel::Files);
        assert_eq!(rail.active_panel(), Some(DrawerPanel::Files));
    }
}
