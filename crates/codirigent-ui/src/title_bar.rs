//! Title bar component.
//!
//! Provides a custom title bar with window controls, logo,
//! project path, and settings button.

use crate::sidebar::Color;
use std::path::{Path, PathBuf};

/// Title bar events.
#[derive(Debug, Clone, PartialEq)]
pub enum TitleBarEvent {
    /// Close button clicked.
    CloseClicked,
    /// Minimize button clicked.
    MinimizeClicked,
    /// Maximize/restore button clicked.
    MaximizeClicked,
    /// Settings button clicked.
    SettingsClicked,
    /// Project path was clicked (to open file browser).
    ProjectPathClicked,
}

/// Window control button type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowControl {
    /// Close window.
    Close,
    /// Minimize window.
    Minimize,
    /// Maximize/restore window.
    Maximize,
}

impl WindowControl {
    /// Get all controls in platform order.
    ///
    /// On macOS: Close, Minimize, Maximize (left side).
    /// On Windows/Linux: Minimize, Maximize, Close (right side).
    #[cfg(target_os = "macos")]
    pub fn all() -> &'static [WindowControl] {
        &[
            WindowControl::Close,
            WindowControl::Minimize,
            WindowControl::Maximize,
        ]
    }

    /// Get all controls in platform order.
    ///
    /// On Windows/Linux: Minimize, Maximize, Close (right side).
    #[cfg(not(target_os = "macos"))]
    pub fn all() -> &'static [WindowControl] {
        &[
            WindowControl::Minimize,
            WindowControl::Maximize,
            WindowControl::Close,
        ]
    }

    /// Get the color for this control (macOS traffic light style).
    pub fn color(&self) -> Color {
        match self {
            Self::Close => Color::from_hex("#ff5f57"),    // Red
            Self::Minimize => Color::from_hex("#febc2e"), // Yellow
            Self::Maximize => Color::from_hex("#28c840"), // Green
        }
    }

    /// Get the hover color for this control.
    pub fn hover_color(&self) -> Color {
        match self {
            Self::Close => Color::from_hex("#ff3b30"),
            Self::Minimize => Color::from_hex("#ffa500"),
            Self::Maximize => Color::from_hex("#00cc00"),
        }
    }
}

/// Window control button state.
#[derive(Debug, Clone, Copy)]
pub struct WindowControlButton {
    /// The control type.
    pub control: WindowControl,
    /// Whether hovered.
    pub is_hovered: bool,
    /// Whether focused (window active).
    pub is_focused: bool,
}

impl WindowControlButton {
    /// Create a new window control button.
    pub fn new(control: WindowControl) -> Self {
        Self {
            control,
            is_hovered: false,
            is_focused: true,
        }
    }

    /// Get the current color based on state.
    pub fn current_color(&self) -> Color {
        if !self.is_focused {
            Color::from_hex("#666666") // Inactive gray
        } else if self.is_hovered {
            self.control.hover_color()
        } else {
            self.control.color()
        }
    }
}

/// Title bar component state.
#[derive(Debug)]
pub struct TitleBar {
    /// Project path to display.
    project_path: Option<PathBuf>,
    /// Window control buttons.
    controls: Vec<WindowControlButton>,
    /// Settings button hovered.
    settings_hovered: bool,
    /// Whether the window is focused.
    is_focused: bool,
    /// Whether window is maximized.
    is_maximized: bool,
    /// Title bar height.
    height: f32,
    /// Pending events.
    pending_events: Vec<TitleBarEvent>,
}

impl Default for TitleBar {
    fn default() -> Self {
        Self::new()
    }
}

impl TitleBar {
    /// Default title bar height.
    pub const DEFAULT_HEIGHT: f32 = 32.0;
    /// Logo text.
    pub const LOGO_TEXT: &'static str = "CODIRIGENT";

    /// Create a new title bar.
    pub fn new() -> Self {
        Self {
            project_path: None,
            controls: WindowControl::all()
                .iter()
                .map(|&c| WindowControlButton::new(c))
                .collect(),
            settings_hovered: false,
            is_focused: true,
            is_maximized: false,
            height: Self::DEFAULT_HEIGHT,
            pending_events: Vec::new(),
        }
    }

    /// Set the project path.
    pub fn set_project_path(&mut self, path: impl Into<PathBuf>) {
        self.project_path = Some(path.into());
    }

    /// Clear the project path.
    pub fn clear_project_path(&mut self) {
        self.project_path = None;
    }

    /// Get the project path.
    pub fn project_path(&self) -> Option<&Path> {
        self.project_path.as_deref()
    }

    /// Get the display path (shortened for display).
    pub fn display_path(&self) -> Option<String> {
        self.project_path.as_ref().map(|p| {
            // Show last two components for brevity
            let components: Vec<_> = p.components().collect();
            if components.len() <= 2 {
                p.to_string_lossy().to_string()
            } else {
                let last_two: PathBuf = components
                    .iter()
                    .skip(components.len() - 2)
                    .collect();
                format!(".../{}", last_two.to_string_lossy())
            }
        })
    }

    /// Set window focused state.
    pub fn set_focused(&mut self, focused: bool) {
        self.is_focused = focused;
        for control in &mut self.controls {
            control.is_focused = focused;
        }
    }

    /// Is the window focused?
    pub fn is_focused(&self) -> bool {
        self.is_focused
    }

    /// Set window maximized state.
    pub fn set_maximized(&mut self, maximized: bool) {
        self.is_maximized = maximized;
    }

    /// Is the window maximized?
    pub fn is_maximized(&self) -> bool {
        self.is_maximized
    }

    /// Get window control buttons.
    pub fn controls(&self) -> &[WindowControlButton] {
        &self.controls
    }

    /// Get mutable control buttons (for hover state).
    pub fn controls_mut(&mut self) -> &mut [WindowControlButton] {
        &mut self.controls
    }

    /// Set hover state on a control.
    pub fn set_control_hovered(&mut self, control: WindowControl, hovered: bool) {
        for btn in &mut self.controls {
            if btn.control == control {
                btn.is_hovered = hovered;
            }
        }
    }

    /// Click a window control.
    pub fn click_control(&mut self, control: WindowControl) {
        let event = match control {
            WindowControl::Close => TitleBarEvent::CloseClicked,
            WindowControl::Minimize => TitleBarEvent::MinimizeClicked,
            WindowControl::Maximize => TitleBarEvent::MaximizeClicked,
        };
        self.pending_events.push(event);
    }

    /// Is settings button hovered?
    pub fn is_settings_hovered(&self) -> bool {
        self.settings_hovered
    }

    /// Set settings button hover state.
    pub fn set_settings_hovered(&mut self, hovered: bool) {
        self.settings_hovered = hovered;
    }

    /// Click the settings button.
    pub fn click_settings(&mut self) {
        self.pending_events.push(TitleBarEvent::SettingsClicked);
    }

    /// Click the project path.
    pub fn click_project_path(&mut self) {
        self.pending_events.push(TitleBarEvent::ProjectPathClicked);
    }

    /// Get the title bar height.
    pub fn height(&self) -> f32 {
        self.height
    }

    /// Set the title bar height.
    pub fn set_height(&mut self, height: f32) {
        self.height = height.max(24.0);
    }

    /// Take pending events.
    pub fn take_events(&mut self) -> Vec<TitleBarEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

/// Rendering hints for the title bar.
#[derive(Debug, Clone)]
pub struct TitleBarRenderHints {
    /// Window control buttons.
    pub controls: Vec<WindowControlButton>,
    /// Logo text.
    pub logo: &'static str,
    /// Display path (if any).
    pub project_path: Option<String>,
    /// Whether settings is hovered.
    pub settings_hovered: bool,
    /// Whether window is focused.
    pub is_focused: bool,
    /// Whether window is maximized.
    pub is_maximized: bool,
    /// Bar height.
    pub height: f32,
    /// Background color.
    pub background: Color,
    /// Text color.
    pub text_color: Color,
    /// Muted text color.
    pub muted_color: Color,
}

impl TitleBar {
    /// Generate rendering hints.
    pub fn render_hints(&self) -> TitleBarRenderHints {
        TitleBarRenderHints {
            controls: self.controls.clone(),
            logo: Self::LOGO_TEXT,
            project_path: self.display_path(),
            settings_hovered: self.settings_hovered,
            is_focused: self.is_focused,
            is_maximized: self.is_maximized,
            height: self.height,
            background: Color::from_hex("#141418"),
            text_color: Color::from_hex("#e0e0e0"),
            muted_color: Color::from_hex("#888888"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_title_bar_new() {
        let bar = TitleBar::new();
        assert!(bar.project_path().is_none());
        assert!(bar.is_focused());
        assert!(!bar.is_maximized());
        assert_eq!(bar.height(), TitleBar::DEFAULT_HEIGHT);
    }

    #[test]
    fn test_title_bar_default() {
        let bar = TitleBar::default();
        assert!(bar.project_path().is_none());
    }

    #[test]
    fn test_set_project_path() {
        let mut bar = TitleBar::new();
        bar.set_project_path("/home/user/project");
        assert_eq!(bar.project_path(), Some(Path::new("/home/user/project")));
    }

    #[test]
    fn test_clear_project_path() {
        let mut bar = TitleBar::new();
        bar.set_project_path("/tmp");
        bar.clear_project_path();
        assert!(bar.project_path().is_none());
    }

    #[test]
    fn test_display_path_short() {
        let mut bar = TitleBar::new();
        bar.set_project_path("/project");
        let display = bar.display_path();
        assert!(display.is_some());
    }

    #[test]
    fn test_display_path_long() {
        let mut bar = TitleBar::new();
        bar.set_project_path("/home/user/projects/my-project");
        let display = bar.display_path().unwrap();
        assert!(display.contains("..."));
        assert!(display.contains("my-project"));
    }

    #[test]
    fn test_set_focused() {
        let mut bar = TitleBar::new();
        bar.set_focused(false);
        assert!(!bar.is_focused());

        // Controls should also be unfocused
        for control in bar.controls() {
            assert!(!control.is_focused);
        }
    }

    #[test]
    fn test_set_maximized() {
        let mut bar = TitleBar::new();
        bar.set_maximized(true);
        assert!(bar.is_maximized());
    }

    #[test]
    fn test_window_controls() {
        let bar = TitleBar::new();
        let controls = bar.controls();
        assert_eq!(controls.len(), 3);
    }

    #[test]
    fn test_control_hover() {
        let mut bar = TitleBar::new();
        bar.set_control_hovered(WindowControl::Close, true);

        let close = bar.controls().iter().find(|c| c.control == WindowControl::Close);
        assert!(close.is_some());
        assert!(close.unwrap().is_hovered);
    }

    #[test]
    fn test_click_close() {
        let mut bar = TitleBar::new();
        bar.click_control(WindowControl::Close);

        let events = bar.take_events();
        assert!(matches!(&events[0], TitleBarEvent::CloseClicked));
    }

    #[test]
    fn test_click_minimize() {
        let mut bar = TitleBar::new();
        bar.click_control(WindowControl::Minimize);

        let events = bar.take_events();
        assert!(matches!(&events[0], TitleBarEvent::MinimizeClicked));
    }

    #[test]
    fn test_click_maximize() {
        let mut bar = TitleBar::new();
        bar.click_control(WindowControl::Maximize);

        let events = bar.take_events();
        assert!(matches!(&events[0], TitleBarEvent::MaximizeClicked));
    }

    #[test]
    fn test_click_settings() {
        let mut bar = TitleBar::new();
        bar.click_settings();

        let events = bar.take_events();
        assert!(matches!(&events[0], TitleBarEvent::SettingsClicked));
    }

    #[test]
    fn test_click_project_path() {
        let mut bar = TitleBar::new();
        bar.set_project_path("/tmp");
        bar.click_project_path();

        let events = bar.take_events();
        assert!(matches!(&events[0], TitleBarEvent::ProjectPathClicked));
    }

    #[test]
    fn test_click_project_path_no_path() {
        let mut bar = TitleBar::new();
        bar.click_project_path();

        let events = bar.take_events();
        assert!(matches!(&events[0], TitleBarEvent::ProjectPathClicked));
    }

    #[test]
    fn test_settings_hover() {
        let mut bar = TitleBar::new();
        assert!(!bar.is_settings_hovered());

        bar.set_settings_hovered(true);
        assert!(bar.is_settings_hovered());
    }

    #[test]
    fn test_set_height() {
        let mut bar = TitleBar::new();
        bar.set_height(40.0);
        assert_eq!(bar.height(), 40.0);

        // Minimum enforced
        bar.set_height(10.0);
        assert!(bar.height() >= 24.0);
    }

    #[test]
    fn test_window_control_colors() {
        assert!(WindowControl::Close.color().r > 0.9); // Red
        assert!(WindowControl::Minimize.color().g > 0.7); // Yellow (high green)
        assert!(WindowControl::Maximize.color().g > 0.7); // Green
    }

    #[test]
    fn test_control_current_color_focused() {
        let control = WindowControlButton::new(WindowControl::Close);
        let color = control.current_color();
        assert!(color.r > 0.9); // Red when focused
    }

    #[test]
    fn test_control_current_color_unfocused() {
        let mut control = WindowControlButton::new(WindowControl::Close);
        control.is_focused = false;
        let color = control.current_color();
        // Should be gray when unfocused
        assert!(color.r < 0.5 && color.g < 0.5 && color.b < 0.5);
    }

    #[test]
    fn test_render_hints() {
        let mut bar = TitleBar::new();
        bar.set_project_path("/home/user/project");

        let hints = bar.render_hints();
        assert_eq!(hints.logo, TitleBar::LOGO_TEXT);
        assert!(hints.project_path.is_some());
        assert!(hints.is_focused);
    }

    #[test]
    fn test_logo_text() {
        assert_eq!(TitleBar::LOGO_TEXT, "CODIRIGENT");
    }
}
