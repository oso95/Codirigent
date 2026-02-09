//! Session sidebar component.
//!
//! This module provides the sidebar UI for managing sessions, including:
//! - Session list with status indicators
//! - Session grouping with collapsible sections
//! - Click-to-focus functionality
//! - Session renaming support
//! - File tree browser

mod file_tree;
mod types;
mod worktree_panel;

#[cfg(test)]
mod tests;

pub use file_tree::*;
pub use types::*;
pub use worktree_panel::*;

use codirigent_core::{Session, SessionId, SessionStatus};
use std::collections::HashMap;

/// Session sidebar component.
///
/// Displays a list of sessions with status indicators, grouping support,
/// and interactive controls for session management.
#[derive(Debug)]
pub struct SessionSidebar {
    sessions: Vec<Session>,
    focused_session: Option<SessionId>,
    editing_name: Option<SessionId>,
    edit_buffer: String,
    groups: HashMap<String, SessionGroup>,
    status_colors: StatusColors,
    width: f32,
    pending_events: Vec<SidebarEvent>,
}

impl Default for SessionSidebar {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionSidebar {
    /// Default sidebar width in pixels.
    pub const DEFAULT_WIDTH: f32 = 250.0;
    /// Minimum sidebar width in pixels.
    pub const MIN_WIDTH: f32 = 180.0;
    /// Maximum sidebar width in pixels.
    pub const MAX_WIDTH: f32 = 400.0;

    // Layout constants
    const HEADER_HEIGHT: f32 = 40.0;
    const ITEM_HEIGHT: f32 = 32.0;
    const GROUP_HEADER_HEIGHT: f32 = 28.0;

    /// Create a new sidebar.
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            focused_session: None,
            editing_name: None,
            edit_buffer: String::new(),
            groups: HashMap::new(),
            status_colors: StatusColors::default(),
            width: Self::DEFAULT_WIDTH,
            pending_events: Vec::new(),
        }
    }

    /// Create a new sidebar with custom status colors.
    pub fn with_status_colors(status_colors: StatusColors) -> Self {
        Self {
            status_colors,
            ..Self::new()
        }
    }

    /// Update the session list. Also updates group map from session assignments.
    pub fn update_sessions(&mut self, sessions: Vec<Session>) {
        for session in &sessions {
            if let Some(ref group_name) = session.group {
                self.groups
                    .entry(group_name.clone())
                    .or_insert_with(|| SessionGroup {
                        name: group_name.clone(),
                        color: session
                            .color
                            .clone()
                            .unwrap_or_else(|| "#6c7086".to_string()),
                        expanded: true,
                    });
            }
        }
        self.sessions = sessions;
    }

    /// Get all sessions.
    pub fn sessions(&self) -> &[Session] {
        &self.sessions
    }

    /// Set the focused session.
    pub fn set_focused(&mut self, id: SessionId) {
        self.focused_session = Some(id);
    }

    /// Get the currently focused session.
    pub fn focused_session(&self) -> Option<SessionId> {
        self.focused_session
    }

    /// Start renaming a session.
    pub fn start_renaming(&mut self, id: SessionId) {
        if let Some(session) = self.sessions.iter().find(|s| s.id == id) {
            self.editing_name = Some(id);
            self.edit_buffer = session.name.clone();
        }
    }

    /// Get the session currently being renamed.
    pub fn editing_session(&self) -> Option<SessionId> {
        self.editing_name
    }

    /// Update the edit buffer content.
    pub fn update_edit_buffer(&mut self, content: String) {
        self.edit_buffer = content;
    }

    /// Get the current edit buffer content.
    pub fn edit_buffer(&self) -> &str {
        &self.edit_buffer
    }

    /// Finish renaming a session.
    pub fn finish_renaming(&mut self) {
        if let Some(id) = self.editing_name.take() {
            let new_name = std::mem::take(&mut self.edit_buffer);
            if !new_name.is_empty() {
                self.pending_events
                    .push(SidebarEvent::RenameSession { id, new_name });
            }
        }
    }

    /// Cancel renaming.
    pub fn cancel_renaming(&mut self) {
        self.editing_name = None;
        self.edit_buffer.clear();
    }

    /// Click on a session to focus it.
    pub fn click_session(&mut self, id: SessionId) {
        self.focused_session = Some(id);
        self.pending_events.push(SidebarEvent::FocusSession(id));
    }

    /// Request to create a new session.
    pub fn request_new_session(&mut self) {
        self.pending_events.push(SidebarEvent::NewSession);
    }

    /// Request to close a session.
    pub fn request_close_session(&mut self, id: SessionId) {
        self.pending_events.push(SidebarEvent::CloseSession(id));
    }

    /// Toggle a group's expanded state.
    pub fn toggle_group(&mut self, group_name: &str) {
        if let Some(group) = self.groups.get_mut(group_name) {
            group.toggle();
        }
        self.pending_events
            .push(SidebarEvent::ToggleGroup(group_name.to_string()));
    }

    /// Check if a group is expanded.
    pub fn is_group_expanded(&self, group_name: &str) -> bool {
        self.groups
            .get(group_name)
            .map(|g| g.expanded)
            .unwrap_or(true)
    }

    /// Get the color for a session status.
    pub fn status_color(&self, status: SessionStatus) -> Color {
        self.status_colors.color_for(status)
    }

    /// Get sessions grouped by group name. None key = ungrouped sessions.
    pub fn sessions_by_group(&self) -> HashMap<Option<String>, Vec<&Session>> {
        let mut grouped: HashMap<Option<String>, Vec<&Session>> = HashMap::new();
        for session in &self.sessions {
            grouped
                .entry(session.group.clone())
                .or_default()
                .push(session);
        }
        grouped
    }

    /// Get all group names in alphabetical order.
    pub fn group_names(&self) -> Vec<&String> {
        let mut names: Vec<_> = self.groups.keys().collect();
        names.sort();
        names
    }

    /// Get a group by name.
    pub fn get_group(&self, name: &str) -> Option<&SessionGroup> {
        self.groups.get(name)
    }

    /// Get the sidebar width.
    pub fn width(&self) -> f32 {
        self.width
    }

    /// Set the sidebar width (clamped to min/max).
    pub fn set_width(&mut self, width: f32) {
        self.width = width.clamp(Self::MIN_WIDTH, Self::MAX_WIDTH);
    }

    /// Take all pending events. Returns and clears the pending event queue.
    pub fn take_events(&mut self) -> Vec<SidebarEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// Get the total session count.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Get the count of sessions in a specific group.
    pub fn group_session_count(&self, group_name: &str) -> usize {
        self.sessions
            .iter()
            .filter(|s| s.group.as_deref() == Some(group_name))
            .count()
    }

    /// Get the count of ungrouped sessions.
    pub fn ungrouped_session_count(&self) -> usize {
        self.sessions.iter().filter(|s| s.group.is_none()).count()
    }

    /// Generate rendering hints for the sidebar.
    pub fn render_hints(&self) -> SidebarRenderHints {
        let mut items = Vec::new();
        let grouped = self.sessions_by_group();

        // Ungrouped sessions first
        if let Some(ungrouped) = grouped.get(&None) {
            for session in ungrouped {
                items.push(self.create_session_item(session, None, 0));
            }
        }

        // Grouped sessions
        for group_name in self.group_names() {
            if let Some(group) = self.get_group(group_name) {
                let group_color = Color::from_hex(&group.color);
                items.push(SidebarItem::GroupHeader {
                    name: group.name.clone(),
                    color: group_color,
                    expanded: group.expanded,
                    session_count: self.group_session_count(group_name),
                });

                if group.expanded {
                    if let Some(sessions) = grouped.get(&Some(group_name.clone())) {
                        for session in sessions {
                            items.push(self.create_session_item(session, Some(group_color), 1));
                        }
                    }
                }
            }
        }

        // Calculate total height
        let mut total_height = Self::HEADER_HEIGHT;
        for item in &items {
            total_height += match item {
                SidebarItem::GroupHeader { .. } => Self::GROUP_HEADER_HEIGHT,
                SidebarItem::Session { .. } => Self::ITEM_HEIGHT,
            };
        }

        SidebarRenderHints {
            items,
            total_height,
            width: self.width,
        }
    }

    /// Create a SidebarItem::Session from a Session.
    fn create_session_item(
        &self,
        session: &Session,
        group_color: Option<Color>,
        indent_level: u8,
    ) -> SidebarItem {
        // Get task description from current_task if present
        let task = session.current_task.as_ref().map(|t| t.0.clone());

        SidebarItem::Session {
            id: session.id,
            name: session.name.clone(),
            status: session.status,
            status_color: self.status_color(session.status),
            is_focused: self.focused_session == Some(session.id),
            is_editing: self.editing_name == Some(session.id),
            indent_level,
            task,
            context_usage: session.context_usage,
            group_color,
        }
    }
}
