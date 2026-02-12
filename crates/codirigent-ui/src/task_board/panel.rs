//! Task board panel component.

use crate::sidebar::Color;

/// Three-state auto-assign mode.
///
/// Off: no auto-assignment
/// Confirm: auto-assign proposes tasks but waits for user to click "Send"
/// Auto: immediate assignment (original behavior)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AutoAssignMode {
    /// Auto-assign is disabled.
    #[default]
    Off,
    /// Auto-assign proposes tasks but waits for confirmation.
    Confirm,
    /// Auto-assign sends tasks immediately.
    Auto,
}

impl AutoAssignMode {
    /// Cycle to the next mode: Off -> Confirm -> Auto -> Off.
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Confirm,
            Self::Confirm => Self::Auto,
            Self::Auto => Self::Off,
        }
    }

    /// Display label for the mode.
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Confirm => "Confirm",
            Self::Auto => "Auto",
        }
    }

    /// Derive the mode from the (auto_assign, confirm_before_assign) pair.
    pub fn from_config(auto_assign: bool, confirm_before_assign: bool) -> Self {
        match (auto_assign, confirm_before_assign) {
            (true, true) => Self::Confirm,
            (true, false) => Self::Auto,
            _ => Self::Off,
        }
    }

    /// Convert mode to the (auto_assign, confirm_before_assign) pair.
    pub fn to_config(self) -> (bool, bool) {
        match self {
            Self::Off => (false, false),
            Self::Confirm => (true, true),
            Self::Auto => (true, false),
        }
    }
}

/// Task board panel events.
#[derive(Debug, Clone, PartialEq)]
pub enum TaskBoardEvent {
    /// Tab was selected.
    TabSelected(TaskBoardTab),
    /// Auto-assign mode changed (three-state cycle).
    AutoAssignModeChanged(AutoAssignMode),
    /// Add task button clicked.
    AddTaskClicked,
    /// Task was selected.
    TaskSelected(String),
    /// Task action was triggered.
    TaskAction {
        /// Task ID.
        task_id: String,
        /// Action type.
        action: TaskAction,
    },
    /// User confirmed a pending assignment (clicked "Send").
    ConfirmAssignment {
        /// Task ID to confirm.
        task_id: String,
    },
    /// User rejected a pending assignment (clicked "Skip").
    RejectAssignment {
        /// Task ID to reject.
        task_id: String,
    },
}

/// Task board tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TaskBoardTab {
    /// Task queue (pending tasks).
    #[default]
    Queue,
    /// Tasks in progress.
    InProgress,
    /// Tasks pending review.
    Review,
    /// Completed tasks.
    Done,
}

impl TaskBoardTab {
    /// Get all tabs in order.
    pub const ALL: &'static [TaskBoardTab] = &[
        TaskBoardTab::Queue,
        TaskBoardTab::InProgress,
        TaskBoardTab::Review,
        TaskBoardTab::Done,
    ];

    /// Get the display label for this tab.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Queue => "Queue",
            Self::InProgress => "In Progress",
            Self::Review => "Review",
            Self::Done => "Done",
        }
    }

    /// Get the icon label for this tab.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Queue => "[Q]",
            Self::InProgress => "[IP]",
            Self::Review => "[RV]",
            Self::Done => "[OK]",
        }
    }
}

/// Task actions available in the task board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskAction {
    /// Assign task to a session.
    Assign,
    /// Start working on task.
    Start,
    /// Mark task for review.
    Review,
    /// Mark task as complete.
    Complete,
    /// Delete task.
    Delete,
    /// Edit task.
    Edit,
}

/// Tab button state for rendering.
#[derive(Debug, Clone)]
pub struct TabButton {
    /// The tab this button represents.
    pub tab: TaskBoardTab,
    /// Display label.
    pub label: &'static str,
    /// Tab icon.
    pub icon: &'static str,
    /// Whether this tab is active.
    pub is_active: bool,
    /// Whether this tab is hovered.
    pub is_hovered: bool,
    /// Number of items in this tab.
    pub count: usize,
}

impl TabButton {
    /// Create a new tab button.
    pub fn new(tab: TaskBoardTab, is_active: bool, count: usize) -> Self {
        Self {
            tab,
            label: tab.label(),
            icon: tab.icon(),
            is_active,
            is_hovered: false,
            count,
        }
    }
}

/// Auto-assign toggle state.
#[derive(Debug, Clone, Copy)]
pub struct AutoAssignToggle {
    /// Current auto-assign mode.
    pub mode: AutoAssignMode,
    /// Whether the toggle is hovered.
    pub is_hovered: bool,
}

impl Default for AutoAssignToggle {
    fn default() -> Self {
        Self {
            mode: AutoAssignMode::Off,
            is_hovered: false,
        }
    }
}

impl AutoAssignToggle {
    /// Create a new toggle with the given mode.
    pub fn new(mode: AutoAssignMode) -> Self {
        Self {
            mode,
            is_hovered: false,
        }
    }

    /// Whether auto-assign is active (Confirm or Auto mode).
    pub fn is_active(&self) -> bool {
        !matches!(self.mode, AutoAssignMode::Off)
    }

    /// Get the dot color based on mode.
    pub fn dot_color(&self) -> Color {
        match self.mode {
            AutoAssignMode::Off => Color::from_hex("#6b7280"), // Gray
            AutoAssignMode::Confirm => Color::from_hex("#f59e0b"), // Amber
            AutoAssignMode::Auto => Color::from_hex("#39d353"), // Green
        }
    }

    /// Get the background color based on state (legacy compat).
    pub fn background_color(&self) -> Color {
        if self.is_active() {
            Color::from_hex("#39d353") // Green when enabled
        } else {
            Color::from_hex("#1a1a1f") // Border color when disabled
        }
    }
}

/// Task board panel state.
#[derive(Debug)]
pub struct TaskBoardPanel {
    /// Currently active tab.
    active_tab: TaskBoardTab,
    /// Tab buttons.
    tabs: Vec<TabButton>,
    /// Auto-assign toggle state.
    auto_assign: AutoAssignToggle,
    /// Whether the panel is expanded.
    is_expanded: bool,
    /// Panel height when expanded.
    expanded_height: f32,
    /// Panel height when collapsed.
    collapsed_height: f32,
    /// Pending events.
    pending_events: Vec<TaskBoardEvent>,
    /// Task counts per tab.
    task_counts: [usize; 4],
}

impl Default for TaskBoardPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskBoardPanel {
    /// Default expanded panel height.
    pub const DEFAULT_EXPANDED_HEIGHT: f32 = 200.0;
    /// Default collapsed (header-only) height.
    pub const DEFAULT_COLLAPSED_HEIGHT: f32 = 44.0;
    /// Header height.
    pub const HEADER_HEIGHT: f32 = 44.0;

    /// Create a new task board panel.
    pub fn new() -> Self {
        let active_tab = TaskBoardTab::default();
        let task_counts = [0, 0, 0, 0];
        Self {
            tabs: Self::create_tabs(active_tab, &task_counts),
            active_tab,
            auto_assign: AutoAssignToggle::default(),
            is_expanded: false,
            expanded_height: Self::DEFAULT_EXPANDED_HEIGHT,
            collapsed_height: Self::DEFAULT_COLLAPSED_HEIGHT,
            pending_events: Vec::new(),
            task_counts,
        }
    }

    /// Create tab buttons from active tab and counts.
    fn create_tabs(active: TaskBoardTab, counts: &[usize; 4]) -> Vec<TabButton> {
        TaskBoardTab::ALL
            .iter()
            .enumerate()
            .map(|(i, &tab)| TabButton::new(tab, tab == active, counts[i]))
            .collect()
    }

    /// Get the active tab.
    pub fn active_tab(&self) -> TaskBoardTab {
        self.active_tab
    }

    /// Set the active tab.
    pub fn set_active_tab(&mut self, tab: TaskBoardTab) {
        self.active_tab = tab;
        self.tabs = Self::create_tabs(tab, &self.task_counts);
    }

    /// Click a tab.
    pub fn click_tab(&mut self, tab: TaskBoardTab) {
        self.set_active_tab(tab);
        self.pending_events.push(TaskBoardEvent::TabSelected(tab));
    }

    /// Get the tab buttons.
    pub fn tabs(&self) -> &[TabButton] {
        &self.tabs
    }

    /// Get mutable tab buttons (for hover state).
    pub fn tabs_mut(&mut self) -> &mut [TabButton] {
        &mut self.tabs
    }

    /// Get the auto-assign toggle state.
    pub fn auto_assign(&self) -> &AutoAssignToggle {
        &self.auto_assign
    }

    /// Get mutable auto-assign toggle (for hover state).
    pub fn auto_assign_mut(&mut self) -> &mut AutoAssignToggle {
        &mut self.auto_assign
    }

    /// Cycle auto-assign mode: Off -> Confirm -> Auto -> Off.
    pub fn cycle_auto_assign_mode(&mut self) {
        self.auto_assign.mode = self.auto_assign.mode.next();
        self.pending_events
            .push(TaskBoardEvent::AutoAssignModeChanged(self.auto_assign.mode));
    }

    /// Set auto-assign mode directly.
    pub fn set_auto_assign_mode(&mut self, mode: AutoAssignMode) {
        if self.auto_assign.mode != mode {
            self.auto_assign.mode = mode;
            self.pending_events
                .push(TaskBoardEvent::AutoAssignModeChanged(mode));
        }
    }

    /// Is auto-assign active (Confirm or Auto)?
    pub fn is_auto_assign_active(&self) -> bool {
        self.auto_assign.is_active()
    }

    /// Get current auto-assign mode.
    pub fn auto_assign_mode(&self) -> AutoAssignMode {
        self.auto_assign.mode
    }

    /// Emit a ConfirmAssignment event.
    pub fn confirm_pending_assignment(&mut self, task_id: impl Into<String>) {
        self.pending_events.push(TaskBoardEvent::ConfirmAssignment {
            task_id: task_id.into(),
        });
    }

    /// Emit a RejectAssignment event.
    pub fn reject_pending_assignment(&mut self, task_id: impl Into<String>) {
        self.pending_events.push(TaskBoardEvent::RejectAssignment {
            task_id: task_id.into(),
        });
    }

    /// Click the add task button.
    pub fn click_add_task(&mut self) {
        self.pending_events.push(TaskBoardEvent::AddTaskClicked);
    }

    /// Is the panel expanded?
    pub fn is_expanded(&self) -> bool {
        self.is_expanded
    }

    /// Toggle panel expanded state.
    pub fn toggle_expanded(&mut self) {
        self.is_expanded = !self.is_expanded;
    }

    /// Set expanded state.
    pub fn set_expanded(&mut self, expanded: bool) {
        self.is_expanded = expanded;
    }

    /// Get the current panel height.
    pub fn height(&self) -> f32 {
        if self.is_expanded {
            self.expanded_height
        } else {
            self.collapsed_height
        }
    }

    /// Set the expanded height.
    pub fn set_expanded_height(&mut self, height: f32) {
        self.expanded_height = height.max(Self::HEADER_HEIGHT + 50.0);
    }

    /// Update task counts.
    pub fn set_task_counts(
        &mut self,
        queue: usize,
        in_progress: usize,
        review: usize,
        done: usize,
    ) {
        self.task_counts = [queue, in_progress, review, done];
        self.tabs = Self::create_tabs(self.active_tab, &self.task_counts);
    }

    /// Get task count for a tab.
    pub fn task_count(&self, tab: TaskBoardTab) -> usize {
        match tab {
            TaskBoardTab::Queue => self.task_counts[0],
            TaskBoardTab::InProgress => self.task_counts[1],
            TaskBoardTab::Review => self.task_counts[2],
            TaskBoardTab::Done => self.task_counts[3],
        }
    }

    /// Take pending events.
    pub fn take_events(&mut self) -> Vec<TaskBoardEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// Select a task.
    pub fn select_task(&mut self, task_id: impl Into<String>) {
        self.pending_events
            .push(TaskBoardEvent::TaskSelected(task_id.into()));
    }

    /// Trigger a task action.
    pub fn trigger_task_action(&mut self, task_id: impl Into<String>, action: TaskAction) {
        self.pending_events.push(TaskBoardEvent::TaskAction {
            task_id: task_id.into(),
            action,
        });
    }
}

/// Rendering hints for the task board panel.
#[derive(Debug, Clone)]
pub struct TaskBoardRenderHints {
    /// Tab buttons.
    pub tabs: Vec<TabButton>,
    /// Active tab.
    pub active_tab: TaskBoardTab,
    /// Auto-assign toggle.
    pub auto_assign: AutoAssignToggle,
    /// Whether expanded.
    pub is_expanded: bool,
    /// Panel height.
    pub height: f32,
    /// Header height.
    pub header_height: f32,
}

impl TaskBoardPanel {
    /// Generate rendering hints.
    pub fn render_hints(&self) -> TaskBoardRenderHints {
        TaskBoardRenderHints {
            tabs: self.tabs.clone(),
            active_tab: self.active_tab,
            auto_assign: self.auto_assign,
            is_expanded: self.is_expanded,
            height: self.height(),
            header_height: Self::HEADER_HEIGHT,
        }
    }
}
