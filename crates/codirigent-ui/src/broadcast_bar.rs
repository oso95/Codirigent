//! Broadcast input bar component.
//!
//! Slides in below the top bar when broadcast mode is active.
//! Allows typing a command that gets sent to all active sessions.

/// Events emitted by the broadcast bar.
#[derive(Debug, Clone)]
pub enum BroadcastBarEvent {
    /// User submitted input to broadcast to all sessions.
    BroadcastSubmitted(String),
}

/// Broadcast input bar state.
#[derive(Debug)]
pub struct BroadcastBar {
    /// Current input text.
    input: String,
    /// Whether the bar is visible.
    visible: bool,
    /// Pending events.
    pending_events: Vec<BroadcastBarEvent>,
}

impl BroadcastBar {
    /// Bar height in pixels.
    pub const HEIGHT: f32 = 52.0;

    /// Placeholder text for the input field.
    pub const PLACEHOLDER: &'static str =
        "Ex: Stop current task and check for security updates...";

    /// Create a new broadcast bar.
    pub fn new() -> Self {
        Self {
            input: String::new(),
            visible: false,
            pending_events: Vec::new(),
        }
    }

    /// Show or hide the bar.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        if !visible {
            self.input.clear();
        }
    }

    /// Whether the bar is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Get the current input text.
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Set the input text.
    pub fn set_input(&mut self, text: String) {
        self.input = text;
    }

    /// Append a character to input.
    pub fn push_char(&mut self, c: char) {
        self.input.push(c);
    }

    /// Remove last character from input.
    pub fn backspace(&mut self) {
        self.input.pop();
    }

    /// Submit the current input. Returns the submitted text and clears input.
    pub fn submit(&mut self) -> Option<String> {
        if self.input.trim().is_empty() {
            return None;
        }
        let text = std::mem::take(&mut self.input);
        self.pending_events
            .push(BroadcastBarEvent::BroadcastSubmitted(text.clone()));
        Some(text)
    }

    /// Drain pending events.
    pub fn drain_events(&mut self) -> Vec<BroadcastBarEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

impl Default for BroadcastBar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_hidden_with_empty_input() {
        let bar = BroadcastBar::new();
        assert!(!bar.is_visible());
        assert_eq!(bar.input(), "");
    }

    #[test]
    fn set_visible_clears_input_on_hide() {
        let mut bar = BroadcastBar::new();
        bar.set_visible(true);
        bar.set_input("hello".to_string());
        assert_eq!(bar.input(), "hello");
        bar.set_visible(false);
        assert_eq!(bar.input(), "");
    }

    #[test]
    fn submit_returns_text_and_clears() {
        let mut bar = BroadcastBar::new();
        bar.set_input("stop all".to_string());
        let result = bar.submit();
        assert_eq!(result, Some("stop all".to_string()));
        assert_eq!(bar.input(), "");
    }

    #[test]
    fn submit_empty_returns_none() {
        let mut bar = BroadcastBar::new();
        bar.set_input("   ".to_string());
        assert_eq!(bar.submit(), None);
    }

    #[test]
    fn submit_emits_event() {
        let mut bar = BroadcastBar::new();
        bar.set_input("update deps".to_string());
        bar.submit();
        let events = bar.drain_events();
        assert_eq!(events.len(), 1);
        assert!(
            matches!(&events[0], BroadcastBarEvent::BroadcastSubmitted(s) if s == "update deps")
        );
    }

    #[test]
    fn full_broadcast_flow() {
        let mut bar = BroadcastBar::new();
        assert!(!bar.is_visible());
        bar.set_visible(true);
        assert!(bar.is_visible());
        bar.push_char('l');
        bar.push_char('s');
        assert_eq!(bar.input(), "ls");
        let result = bar.submit();
        assert_eq!(result, Some("ls".to_string()));
        assert_eq!(bar.input(), "");
        let events = bar.drain_events();
        assert_eq!(events.len(), 1);
        bar.set_input("leftover".to_string());
        bar.set_visible(false);
        assert_eq!(bar.input(), "");
    }

    #[test]
    fn backspace_removes_last_char() {
        let mut bar = BroadcastBar::new();
        bar.set_input("hello".to_string());
        bar.backspace();
        assert_eq!(bar.input(), "hell");
    }

    #[test]
    fn default_is_same_as_new() {
        let default = BroadcastBar::default();
        let new = BroadcastBar::new();
        assert_eq!(default.is_visible(), new.is_visible());
        assert_eq!(default.input(), new.input());
    }
}
