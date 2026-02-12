//! Task board panel rendering for workspace.
//!
//! This module handles rendering of the right-side task board panel,
//! including task cards, task creation modal, and task management UI.

use crate::components::text_input::{text_input, TextInputStyle};
use crate::icons;
use crate::task_board::{TaskAction, TaskItem, TaskItemAction, TaskPriority as UIPriority, TaskStatus as UIStatus, AutoAssignMode};
use crate::theme::CodirigentTheme;
use crate::workspace::gpui::WorkspaceView;
use codirigent_core::{TaskPriority as CorePriority, TaskStatus as CoreStatus};
use gpui::{
    div, px, ClickEvent, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled,
};

    /// Convert core Task to UI TaskItem with status mapping.
    fn core_task_to_ui_item(&self, task: &codirigent_core::Task) -> crate::task_board::TaskItem {
        use crate::task_board::{TaskItem, TaskPriority as UIPriority, TaskStatus as UIStatus};
        use codirigent_core::{TaskPriority as CorePriority, TaskStatus as CoreStatus};

        // Map priority
        let ui_priority = match task.priority {
            CorePriority::Critical | CorePriority::High => UIPriority::High,
            CorePriority::Medium => UIPriority::Medium,
            CorePriority::Low => UIPriority::Low,
        };

        // Map status
        let ui_status = match task.status {
            CoreStatus::Queued => UIStatus::Queued,
            CoreStatus::Assigned | CoreStatus::Working => UIStatus::InProgress,
            CoreStatus::Verifying | CoreStatus::Review => UIStatus::PendingReview,
            CoreStatus::Done => UIStatus::Completed,
            CoreStatus::Blocked => UIStatus::Queued, // Treat blocked as queued in UI
        };

        // Format estimated time
        let estimated_time = task.estimated_minutes.map(|mins| {
            if mins < 60 {
                format!("{}m", mins)
            } else {
                format!("{}h {}m", mins / 60, mins % 60)
            }
        });

        // Format created_at as relative time
        let created_at = {
            let now = chrono::Utc::now();
            let duration = now.signed_duration_since(task.created_at);
            if duration.num_minutes() < 60 {
                Some(format!("{}m ago", duration.num_minutes()))
            } else if duration.num_hours() < 24 {
                Some(format!("{}h ago", duration.num_hours()))
            } else {
                Some(format!("{}d ago", duration.num_days()))
            }
        };

        TaskItem::new(task.id.0.clone(), task.title.clone())
            .with_priority(ui_priority)
            .with_status(ui_status)
            .with_estimated_time(estimated_time.unwrap_or_else(|| "?".to_string()))
            .with_created_at(created_at.unwrap_or_else(|| "now".to_string()))
    }
    /// Build a priority selection button for the task creation modal.
    ///
    /// Creates a rounded button with a colored dot indicator and label.
    /// The button highlights when the given priority matches the modal's current priority.
    ///
    /// # Parameters
    /// - `id`: Element ID for the button
    /// - `priority`: The priority this button represents
    /// - `label`: Display label (e.g., "High", "Medium", "Low")
    /// - `color`: Base color for this priority level
    /// - `current_priority`: The currently selected priority from the modal
    /// - `fg`: Foreground color for active state
    /// - `muted`: Muted color for inactive state
    /// - `border_color`: Default border color
    /// - `input_bg`: Default background color
    /// - `cx`: GPUI context
    fn build_priority_button(
        &self,
        id: impl Into<SharedString>,
        priority: codirigent_core::TaskPriority,
        label: impl Into<SharedString>,
        color: gpui::Hsla,
        current_priority: codirigent_core::TaskPriority,
        fg: gpui::Hsla,
        muted: gpui::Hsla,
        border_color: gpui::Hsla,
        input_bg: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_selected = current_priority == priority;
        div()
            .id(id.into())
            .px_3()
            .py(px(4.0))
            .rounded_md()
            .border_1()
            .border_color(if is_selected { color } else { border_color })
            .bg(if is_selected {
                color.opacity(0.15)
            } else {
                input_bg
            })
            .cursor_pointer()
            .hover(|style| style.bg(color.opacity(0.1)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                if let Some(modal) = &mut this.task_creation_modal {
                    modal.priority = priority;
                }
                cx.notify();
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(color))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(if is_selected { fg } else { muted })
                            .child(label.into()),
                    ),
            )
    }
    /// Render the task creation modal.
    pub(super) fn render_task_creation_modal(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let modal = self.task_creation_modal.clone()?;
        let is_editing = modal.editing_task_id.is_some();

        let theme = self.workspace().theme();
        let panel_bg: gpui::Hsla = theme.panel_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let muted: gpui::Hsla = theme.muted.into();
        let primary: gpui::Hsla = theme.primary.into();
        let input_bg: gpui::Hsla = theme.terminal_background.into();
        let error_color: gpui::Hsla = gpui::Hsla::red();
        let input_style = TextInputStyle {
            height: 36.0,
            padding_x: 12.0,
            bg: input_bg,
            border: border_color,
            focus_border: primary,
            error_border: error_color,
            text: fg,
        };

        // Add cursor only to focused field
        let title_focused = modal.focused_field == 0;
        let desc_focused = modal.focused_field == 1;
        let plan_focused = modal.focused_field == 2;

        let title_value = if title_focused {
            if modal.title.is_empty() {
                "|".to_string()
            } else {
                format!("{}|", modal.title)
            }
        } else {
            if modal.title.is_empty() {
                "Enter task title...".to_string()
            } else {
                modal.title.clone()
            }
        };

        let description_value = if desc_focused {
            if modal.description.is_empty() {
                "|".to_string()
            } else {
                format!("{}|", modal.description)
            }
        } else {
            if modal.description.is_empty() {
                "Enter description...".to_string()
            } else {
                modal.description.clone()
            }
        };

        let plan_file_value = if plan_focused {
            if modal.plan_file.is_empty() {
                "|".to_string()
            } else {
                format!("{}|", modal.plan_file)
            }
        } else {
            if modal.plan_file.is_empty() {
                "e.g. plans/phase-1-stage-2.md".to_string()
            } else {
                modal.plan_file.clone()
            }
        };

        let project_dir_display = modal
            .project_dir
            .as_ref()
            .map(|p| format!("Project: {}", p.display()))
            .unwrap_or_else(|| "Project: (none)".to_string());

        Some(
            div()
                .id("task-creation-overlay")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::Hsla::black().opacity(0.5))
                .on_mouse_down(MouseButton::Left, cx.listener(|this, _: &MouseDownEvent, _window, cx| {
                    this.close_task_creation_modal();
                    cx.notify();
                    cx.stop_propagation();
                }))
                .child(
                    div()
                        .id("task-creation-modal")
                        .w(px(500.0))
                        .bg(panel_bg)
                        .border_1()
                        .border_color(border_color)
                        .rounded_lg()
                        .flex()
                        .flex_col()
                        // Prevent click from closing modal or reaching sessions behind
                        .on_mouse_down(MouseButton::Left, cx.listener(|_this, _: &MouseDownEvent, _window, cx| {
                            cx.stop_propagation();
                        }))
                        // Header
                        .child(
                            div()
                                .h(px(48.0))
                                .px_4()
                                .border_b_1()
                                .border_color(border_color)
                                .flex()
                                .items_center()
                                .child(
                                    self.aligned_icon_label_row(
                                        if is_editing { icons::pencil() } else { icons::clipboard_plus() },
                                        fg,
                                        16.0,
                                        if is_editing { "Edit Task" } else { "Create New Task" },
                                        fg,
                                        16.0,
                                        FontWeight::SEMIBOLD,
                                        20.0,
                                        8.0,
                                    ),
                                ),
                        )
                        // Content
                        .child(
                            div()
                                .p_4()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(muted)
                                                .child(format!("Title:{}",
                                                    if modal.focused_field == 0 { " (active)" } else { "" }
                                                )),
                                        )
                                        .child(text_input(
                                            "task-title-input",
                                            title_value,
                                            title_focused,
                                            modal.error.is_some(),
                                            &input_style,
                                        ).on_mouse_down(MouseButton::Left, cx.listener(|this, _event, _window, cx| {
                                            if let Some(modal) = &mut this.task_creation_modal {
                                                modal.focused_field = 0;
                                            }
                                            cx.notify();
                                        }))),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(muted)
                                                .child(format!("Description:{}",
                                                    if modal.focused_field == 1 { " (active)" } else { "" }
                                                )),
                                        )
                                        .child(
                                            div()
                                                .h(px(120.0))
                                                .w_full()
                                                .p_3()
                                                .bg(input_bg)
                                                .border_1()
                                                .border_color(if desc_focused { primary } else { border_color })
                                                .rounded_md()
                                                .text_sm()
                                                .text_color(if desc_focused || !modal.description.is_empty() { fg } else { muted })
                                                .cursor_pointer()
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _event, _window, cx| {
                                                    if let Some(modal) = &mut this.task_creation_modal {
                                                        modal.focused_field = 1;
                                                    }
                                                    cx.notify();
                                                }))
                                                .child(description_value),
                                        ),
                                )
                                // Priority selector
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(muted)
                                                .child("Priority:"),
                                        )
                                        .child(
                                            div().flex().items_center().gap_2()
                                                .child(self.build_priority_button(
                                                    "priority-high",
                                                    codirigent_core::TaskPriority::High,
                                                    "High",
                                                    gpui::Hsla::from(gpui::Rgba { r: 1.0, g: 0.42, b: 0.42, a: 1.0 }),
                                                    modal.priority,
                                                    fg,
                                                    muted,
                                                    border_color,
                                                    input_bg,
                                                    cx,
                                                ))
                                                .child(self.build_priority_button(
                                                    "priority-medium",
                                                    codirigent_core::TaskPriority::Medium,
                                                    "Medium",
                                                    gpui::Hsla::from(gpui::Rgba { r: 0.96, g: 0.62, b: 0.04, a: 1.0 }),
                                                    modal.priority,
                                                    fg,
                                                    muted,
                                                    border_color,
                                                    input_bg,
                                                    cx,
                                                ))
                                                .child(self.build_priority_button(
                                                    "priority-low",
                                                    codirigent_core::TaskPriority::Low,
                                                    "Low",
                                                    gpui::Hsla::from(gpui::Rgba { r: 0.36, g: 0.55, b: 0.94, a: 1.0 }),
                                                    modal.priority,
                                                    fg,
                                                    muted,
                                                    border_color,
                                                    input_bg,
                                                    cx,
                                                )),
                                        ),
                                )
                                // Project dir label (read-only)
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(muted)
                                        .child(project_dir_display),
                                )
                                // Plan file input
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(muted)
                                                .child(format!("Plan File (relative path):{}",
                                                    if modal.focused_field == 2 { " (active)" } else { "" }
                                                )),
                                        )
                                        .child(text_input(
                                            "task-plan-file-input",
                                            plan_file_value,
                                            plan_focused,
                                            false,
                                            &input_style,
                                        ).on_mouse_down(MouseButton::Left, cx.listener(|this, _event, _window, cx| {
                                            if let Some(modal) = &mut this.task_creation_modal {
                                                modal.focused_field = 2;
                                            }
                                            cx.notify();
                                        }))),
                                )
                                .when_some(modal.error.clone(), |this, error| {
                                    this.child(
                                        div()
                                            .text_sm()
                                            .text_color(error_color)
                                            .child(error),
                                    )
                                })
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted)
                                        .child("Press Tab to switch fields, Enter to create, Esc to cancel"),
                                ),
                        )
                        // Footer
                        .child(
                            div()
                                .h(px(60.0))
                                .px_4()
                                .border_t_1()
                                .border_color(border_color)
                                .flex()
                                .items_center()
                                .justify_end()
                                .gap_2()
                                .child(
                                    div()
                                        .id("task-creation-cancel")
                                        .px_4()
                                        .py_2()
                                        .border_1()
                                        .border_color(border_color)
                                        .rounded_md()
                                        .text_sm()
                                        .text_color(fg)
                                        .cursor_pointer()
                                        .hover(|style| style.bg(border_color.opacity(0.1)))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                            this.close_task_creation_modal();
                                            cx.notify();
                                        }))
                                        .child(self.aligned_icon_label_row(
                                            icons::x(),
                                            fg,
                                            12.0,
                                            "Cancel",
                                            fg,
                                            14.0,
                                            FontWeight::MEDIUM,
                                            16.0,
                                            4.0,
                                        )),
                                )
                                .child(
                                    div()
                                        .id("task-creation-create")
                                        .px_4()
                                        .py_2()
                                        .bg(primary)
                                        .rounded_md()
                                        .text_sm()
                                        .text_color(gpui::Hsla::white())
                                        .cursor_pointer()
                                        .hover(|style| style.bg(primary.opacity(0.8)))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                            this.apply_task_creation_modal(cx);
                                        }))
                                        .child(self.aligned_icon_label_row(
                                            icons::plus(),
                                            gpui::Hsla::white(),
                                            12.0,
                                            if is_editing { "Save Changes" } else { "Create Task" },
                                            gpui::Hsla::white(),
                                            14.0,
                                            FontWeight::MEDIUM,
                                            16.0,
                                            4.0,
                                        )),
                                ),
                        ),
                ),
        )
    }

    /// Render a dropdown menu item with icon.
    fn render_menu_item(
        &self,
        label: &str,
        session_id: SessionId,
        action: SessionMenuAction,
        _theme: &CodirigentTheme,
        hover_bg: gpui::Hsla,
        fg: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label = label.to_string();
        let icon = match &action {
            SessionMenuAction::Rename => icons::pencil(),
            SessionMenuAction::AssignToGroup(_) => icons::users(),
            SessionMenuAction::NewGroup => icons::plus(),
            SessionMenuAction::RemoveGroup => icons::user_minus(),
            SessionMenuAction::EndSession => icons::x_circle(),
        };
        let id_suffix = match &action {
            SessionMenuAction::AssignToGroup(g) => format!("assign-{}", g),
            other => format!("{:?}", other),
        };
        div()
            .id(SharedString::from(format!(
                "menu-{}-{}",
                id_suffix, session_id.0
            )))
            .h(px(30.0))
            .px_3()
            .flex()
            .items_center()
            .cursor_pointer()
            .hover(move |style| style.bg(hover_bg.opacity(0.1)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                this.handle_session_menu_action(session_id, action.clone(), cx);
            }))
            .child(self.aligned_icon_label_row(
                icon,
                fg,
                12.0,
                label,
                fg,
                11.0,
                FontWeight::MEDIUM,
                14.0,
                8.0,
            ))
    }

    // =========================================================================
    // New panel render methods (icon rail, drawer, broadcast bar, right task board)
    // =========================================================================

    /// Render the narrow icon rail (56px).
    /// Render a single task card for the right task board.
    fn render_task_card(
        &self,
        item: &crate::task_board::TaskItem,
        theme: &CodirigentTheme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        use crate::task_board::{TaskAction, TaskItemAction};

        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let muted: gpui::Hsla = theme.muted.into();
        let active_bg: gpui::Hsla = theme.active.into();
        let priority_color = item.priority.color().to_hsla();
        let status_color = item.status.badge_color().to_hsla();

        let task_id = item.id.clone();
        let task_id_for_click = task_id.clone();

        let mut card = div()
            .id(SharedString::from(format!("task-card-{}", task_id)))
            .w_full()
            .p_2()
            .bg(active_bg.opacity(0.3))
            .border_1()
            .border_color(border_color.opacity(0.5))
            .rounded_md()
            .cursor_pointer()
            .hover(|style| style.bg(active_bg.opacity(0.6)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _window, _cx| {
                this.task_board.select_task(task_id_for_click.clone());
            }))
            // Row 1: priority dot + title
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .w(px(6.0))
                            .h(px(6.0))
                            .rounded_full()
                            .bg(priority_color)
                            .flex_shrink_0(),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(fg)
                            .overflow_hidden()
                            .child(item.title.clone()),
                    ),
            )
            // Row 2: status badge + assigned session
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .mt(px(4.0))
                    .child(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .rounded(px(3.0))
                            .bg(status_color.opacity(0.15))
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(status_color)
                                    .child(item.status.label()),
                            ),
                    )
                    .when_some(item.assigned_to.clone(), |this, session| {
                        this.child(
                            div()
                                .text_size(px(10.0))
                                .text_color(muted.opacity(0.7))
                                .child(format!("→ {}", session)),
                        )
                    }),
            );

        // Row 3: action buttons from available_actions()
        let actions = item.available_actions();
        if !actions.is_empty() {
            let mut action_row = div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(6.0))
                .mt(px(6.0));
            for action in actions {
                let (label, board_action, btn_bg) = match action {
                    TaskItemAction::Assign => (
                        "Assign",
                        TaskAction::Assign,
                        gpui::hsla(0.48, 0.6, 0.55, 0.2),
                    ), // teal
                    TaskItemAction::Edit => ("Edit", TaskAction::Edit, active_bg),
                    TaskItemAction::Delete => (
                        "Delete",
                        TaskAction::Delete,
                        gpui::hsla(0.0, 0.7, 0.56, 0.15),
                    ), // red
                    TaskItemAction::MarkForReview => (
                        "Review",
                        TaskAction::Review,
                        gpui::hsla(0.11, 0.9, 0.6, 0.2),
                    ), // orange
                    TaskItemAction::Approve => (
                        "Approve",
                        TaskAction::Complete,
                        gpui::hsla(0.44, 0.7, 0.45, 0.2),
                    ), // green
                    TaskItemAction::Reject => (
                        "Reject",
                        TaskAction::Delete,
                        gpui::hsla(0.0, 0.7, 0.56, 0.15),
                    ), // red
                    TaskItemAction::Reopen => ("Start", TaskAction::Start, active_bg),
                };
                let action_task_id = task_id.clone();
                action_row = action_row.child(
                    div()
                        .id(SharedString::from(format!(
                            "task-action-{}-{}",
                            task_id, label
                        )))
                        .px(px(8.0))
                        .py(px(3.0))
                        .min_w(px(48.0))
                        .flex()
                        .justify_center()
                        .rounded(px(4.0))
                        .bg(btn_bg)
                        .cursor_pointer()
                        .hover(|style| style.bg(active_bg.opacity(0.8)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _window, _cx| {
                            this.task_board
                                .trigger_task_action(action_task_id.clone(), board_action);
                        }))
                        .child(div().text_size(px(11.0)).text_color(fg).child(label)),
                );
            }
            card = card.child(action_row);
        }

        card
    }
    /// Render the task board as a right sidebar panel (288px).
    pub(super) fn render_right_task_board(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.workspace().theme();
        let panel_bg: gpui::Hsla = theme.icon_rail_background.into();
        let header_bg: gpui::Hsla = theme.drawer_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let muted: gpui::Hsla = theme.muted.into();
        let primary: gpui::Hsla = theme.primary.into();
        let active_bg: gpui::Hsla = theme.active.into();
        let panel_label_size = 11.0;
        let panel_label_row_height = 14.0;
        let panel_icon_y_offset = 1.0;

        // Fetch real task data from TaskManager
        let (
            running_items,
            queued_items,
            review_items,
            done_items,
            auto_assign_mode,
            pending_assignments,
        ) = if let Ok(manager) = self.task_manager.lock() {
            let all_tasks = manager.list_tasks();

            let running: Vec<_> = all_tasks
                .iter()
                .filter(|t| {
                    matches!(
                        t.status,
                        codirigent_core::TaskStatus::Assigned
                            | codirigent_core::TaskStatus::Working
                    )
                })
                .map(|t| self.core_task_to_ui_item(t))
                .collect();
            let queued: Vec<_> = all_tasks
                .iter()
                .filter(|t| {
                    matches!(
                        t.status,
                        codirigent_core::TaskStatus::Queued | codirigent_core::TaskStatus::Blocked
                    )
                })
                .map(|t| self.core_task_to_ui_item(t))
                .collect();
            let review: Vec<_> = all_tasks
                .iter()
                .filter(|t| {
                    matches!(
                        t.status,
                        codirigent_core::TaskStatus::Verifying
                            | codirigent_core::TaskStatus::Review
                    )
                })
                .map(|t| self.core_task_to_ui_item(t))
                .collect();
            let done: Vec<_> = all_tasks
                .iter()
                .filter(|t| t.status == codirigent_core::TaskStatus::Done)
                .map(|t| self.core_task_to_ui_item(t))
                .collect();
            let config = manager.assignment().config();
            let mode = crate::task_board::AutoAssignMode::from_config(
                config.auto_assign,
                config.confirm_before_assign,
            );

            // Collect pending assignments for the confirmation banner
            let pending: Vec<_> = manager
                .assignment()
                .pending_assignments()
                .iter()
                .map(|p| {
                    let task_title = all_tasks
                        .iter()
                        .find(|t| t.id == p.task_id)
                        .map(|t| t.title.clone())
                        .unwrap_or_else(|| p.task_id.to_string());
                    (p.task_id.to_string(), p.session_id.0, task_title)
                })
                .collect();

            let queue_count = queued.len();
            let in_progress_count = running.len();
            let review_count = review.len();
            let done_count = done.len();
            drop(manager);
            self.task_board.set_task_counts(
                queue_count,
                in_progress_count,
                review_count,
                done_count,
            );

            (running, queued, review, done, mode, pending)
        } else {
            (
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                crate::task_board::AutoAssignMode::Off,
                Vec::new(),
            )
        };

        let running_count = running_items.len();
        let queued_count = queued_items.len();
        let review_count = review_items.len();
        let done_count = done_items.len();

        // Auto-assign badge colors based on three-state mode
        let amber: gpui::Hsla = gpui::hsla(0.11, 0.95, 0.55, 1.0); // Amber for Confirm
        let (auto_dot_color, auto_text_opacity, auto_bg_opacity, auto_border_opacity, auto_label) =
            match auto_assign_mode {
                crate::task_board::AutoAssignMode::Off => {
                    (muted.opacity(0.4), 0.4, 0.05, 0.1, "Off")
                }
                crate::task_board::AutoAssignMode::Confirm => (amber, 0.8, 0.1, 0.2, "Confirm"),
                crate::task_board::AutoAssignMode::Auto => (primary, 0.8, 0.1, 0.2, "Auto"),
            };
        let auto_badge_accent = match auto_assign_mode {
            crate::task_board::AutoAssignMode::Off => muted,
            crate::task_board::AutoAssignMode::Confirm => amber,
            crate::task_board::AutoAssignMode::Auto => primary,
        };

        // Render task cards for each section
        let theme_ref = self.workspace().theme().clone();
        let mut running_section = div().flex().flex_col().gap(px(4.0));
        for item in &running_items {
            running_section = running_section.child(self.render_task_card(item, &theme_ref, cx));
        }

        let mut queued_section = div().flex().flex_col().gap(px(4.0));
        for item in &queued_items {
            queued_section = queued_section.child(self.render_task_card(item, &theme_ref, cx));
        }

        let mut review_section = div().flex().flex_col().gap(px(4.0));
        for item in &review_items {
            review_section = review_section.child(self.render_task_card(item, &theme_ref, cx));
        }

        let mut done_section = div().flex().flex_col().gap(px(4.0));
        for item in &done_items {
            done_section = done_section.child(self.render_task_card(item, &theme_ref, cx));
        }

        div()
            .id("right-task-board")
            .w(px(crate::layout::RIGHT_PANEL_WIDTH))
            .h_full()
            .bg(panel_bg)
            .border_l_1()
            .border_color(border_color)
            .flex()
            .flex_col()
            // Header
            .child(
                div()
                    .h(px(48.0))
                    .w_full()
                    .bg(header_bg)
                    .border_b_1()
                    .border_color(border_color)
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(self.centered_lucide_icon_with_offset(
                                icons::list_todo(),
                                muted,
                                panel_label_size,
                                panel_icon_y_offset,
                            ))
                            .child(
                                div()
                                    .h(px(panel_label_row_height))
                                    .flex()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_size(px(panel_label_size))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(muted)
                                            .child("TASKS"),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .id("auto-assign-toggle")
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .px_2()
                            .py(px(2.0))
                            .rounded_md()
                            .bg(auto_badge_accent.opacity(auto_bg_opacity))
                            .border_1()
                            .border_color(auto_badge_accent.opacity(auto_border_opacity))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _: &ClickEvent, _window, _cx| {
                                // Cycle: Off -> Confirm -> Auto -> Off
                                if let Ok(mut manager) = this.task_manager.lock() {
                                    let config = manager.assignment().config().clone();
                                    let current = crate::task_board::AutoAssignMode::from_config(
                                        config.auto_assign,
                                        config.confirm_before_assign,
                                    );
                                    let next = current.next();
                                    let (auto, confirm) = next.to_config();
                                    manager.assignment_mut().set_auto_assign(auto);
                                    manager.assignment_mut().set_confirm_before_assign(confirm);
                                }
                            }))
                            .child(
                                div()
                                    .w(px(6.0))
                                    .h(px(6.0))
                                    .rounded_full()
                                    .bg(auto_dot_color),
                            )
                            .child(
                                div()
                                    .h(px(panel_label_row_height))
                                    .flex()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_size(px(panel_label_size))
                                            .text_color(
                                                auto_badge_accent.opacity(auto_text_opacity),
                                            )
                                            .child(auto_label),
                                    ),
                            ),
                    ),
            )
            // Pending assignment confirmation banners
            .children(
                pending_assignments
                    .into_iter()
                    .map(|(task_id, session_num, task_title)| {
                        let confirm_task_id = task_id.clone();
                        let reject_task_id = task_id.clone();
                        let amber_bg: gpui::Hsla = gpui::hsla(0.11, 0.95, 0.55, 0.08);
                        let amber_border: gpui::Hsla = gpui::hsla(0.11, 0.95, 0.55, 0.25);
                        let amber_text: gpui::Hsla = gpui::hsla(0.11, 0.95, 0.55, 0.9);
                        let green_bg: gpui::Hsla = gpui::hsla(0.40, 0.7, 0.45, 0.20);
                        let green_fg: gpui::Hsla = gpui::hsla(0.40, 0.8, 0.60, 1.0);

                        div()
                            .id(SharedString::from(format!("pending-confirm-{}", task_id)))
                            .mx_2()
                            .mt_2()
                            .p_2()
                            .rounded_md()
                            .bg(amber_bg)
                            .border_1()
                            .border_color(amber_border)
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            // Row 1: pause icon + task title + "Proposed for Session N"
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .child(
                                        div().text_size(px(11.0)).text_color(amber_text).child("⏸"),
                                    )
                                    .child(
                                        div().flex_1().overflow_hidden().child(
                                            div()
                                                .text_size(px(12.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(amber_text)
                                                .child(task_title),
                                        ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .text_color(muted.opacity(0.7))
                                            .child(format!("→ Session {}", session_num)),
                                    ),
                            )
                            // Row 2: Send + Skip buttons
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "confirm-send-{}",
                                                confirm_task_id
                                            )))
                                            .px(px(10.0))
                                            .py(px(3.0))
                                            .rounded(px(4.0))
                                            .bg(green_bg)
                                            .cursor_pointer()
                                            .hover(|style| {
                                                style.bg(gpui::hsla(0.40, 0.7, 0.45, 0.35))
                                            })
                                            .on_click(cx.listener(
                                                move |this, _: &ClickEvent, _window, _cx| {
                                                    this.task_board.confirm_pending_assignment(
                                                        confirm_task_id.clone(),
                                                    );
                                                },
                                            ))
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(green_fg)
                                                    .child("Send"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "confirm-skip-{}",
                                                reject_task_id
                                            )))
                                            .px(px(10.0))
                                            .py(px(3.0))
                                            .rounded(px(4.0))
                                            .bg(active_bg.opacity(0.4))
                                            .cursor_pointer()
                                            .hover(|style| {
                                                style.bg(gpui::hsla(0.0, 0.0, 0.5, 0.15))
                                            })
                                            .on_click(cx.listener(
                                                move |this, _: &ClickEvent, _window, _cx| {
                                                    this.task_board.reject_pending_assignment(
                                                        reject_task_id.clone(),
                                                    );
                                                },
                                            ))
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .text_color(muted.opacity(0.7))
                                                    .child("Skip"),
                                            ),
                                    ),
                            )
                    }),
            )
            // Scrollable content - Running + Queue sections
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_6()
                    // Running section
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .items_center()
                                    .mb_2()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_1()
                                            .child(self.centered_lucide_icon_with_offset(
                                                icons::play(),
                                                muted,
                                                panel_label_size,
                                                panel_icon_y_offset,
                                            ))
                                            .child(
                                                div()
                                                    .h(px(panel_label_row_height))
                                                    .flex()
                                                    .items_center()
                                                    .child(
                                                        div()
                                                            .text_size(px(panel_label_size))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(muted)
                                                            .child("RUNNING"),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div().px(px(6.0)).rounded_full().bg(active_bg).child(
                                            div()
                                                .text_xs()
                                                .text_color(muted)
                                                .child(format!("{}", running_count)),
                                        ),
                                    ),
                            )
                            .when(running_count == 0, |this| {
                                this.child(
                                    div()
                                        .text_xs()
                                        .text_color(muted.opacity(0.5))
                                        .child("No running tasks"),
                                )
                            })
                            .when(running_count > 0, |this| this.child(running_section)),
                    )
                    // Queue section
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .items_center()
                                    .mb_2()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_1()
                                            .child(self.centered_lucide_icon_with_offset(
                                                icons::clock(),
                                                muted,
                                                panel_label_size,
                                                panel_icon_y_offset,
                                            ))
                                            .child(
                                                div()
                                                    .h(px(panel_label_row_height))
                                                    .flex()
                                                    .items_center()
                                                    .child(
                                                        div()
                                                            .text_size(px(panel_label_size))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(muted)
                                                            .child("QUEUE"),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div().px(px(6.0)).rounded_full().bg(active_bg).child(
                                            div()
                                                .text_xs()
                                                .text_color(muted)
                                                .child(format!("{}", queued_count)),
                                        ),
                                    ),
                            )
                            .when(queued_count == 0, |this| {
                                this.child(
                                    div()
                                        .text_xs()
                                        .text_color(muted.opacity(0.5))
                                        .child("No queued tasks"),
                                )
                            })
                            .when(queued_count > 0, |this| this.child(queued_section)),
                    )
                    // Review section
                    .when(review_count > 0, |this| {
                        this.child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .flex()
                                        .justify_between()
                                        .items_center()
                                        .mb_2()
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_1()
                                                .child(self.centered_lucide_icon_with_offset(
                                                    icons::eye(),
                                                    muted,
                                                    panel_label_size,
                                                    panel_icon_y_offset,
                                                ))
                                                .child(
                                                    div()
                                                        .h(px(panel_label_row_height))
                                                        .flex()
                                                        .items_center()
                                                        .child(
                                                            div()
                                                                .text_size(px(panel_label_size))
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(muted)
                                                                .child("REVIEW"),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            div().px(px(6.0)).rounded_full().bg(active_bg).child(
                                                div()
                                                    .text_xs()
                                                    .text_color(muted)
                                                    .child(format!("{}", review_count)),
                                            ),
                                        ),
                                )
                                .child(review_section),
                        )
                    })
                    // Done section
                    .when(done_count > 0, |this| {
                        this.child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .flex()
                                        .justify_between()
                                        .items_center()
                                        .mb_2()
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_1()
                                                .child(self.centered_lucide_icon_with_offset(
                                                    icons::check_circle(),
                                                    muted,
                                                    panel_label_size,
                                                    panel_icon_y_offset,
                                                ))
                                                .child(
                                                    div()
                                                        .h(px(panel_label_row_height))
                                                        .flex()
                                                        .items_center()
                                                        .child(
                                                            div()
                                                                .text_size(px(panel_label_size))
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(muted)
                                                                .child("DONE"),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            div().px(px(6.0)).rounded_full().bg(active_bg).child(
                                                div()
                                                    .text_xs()
                                                    .text_color(muted)
                                                    .child(format!("{}", done_count)),
                                            ),
                                        ),
                                )
                                .child(done_section),
                        )
                    }),
            )
            // Footer: Add Task button
            .child(
                div()
                    .p_3()
                    .border_t_1()
                    .border_color(border_color)
                    .bg(header_bg)
                    .child(
                        div()
                            .id("add-task-btn")
                            .w_full()
                            .py(px(6.0))
                            .bg(active_bg)
                            .rounded_md()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                this.open_task_creation_modal();
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(self.centered_lucide_icon_with_offset(
                                        icons::plus(),
                                        fg,
                                        panel_label_size,
                                        panel_icon_y_offset,
                                    ))
                                    .child(
                                        div()
                                            .h(px(panel_label_row_height))
                                            .flex()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_size(px(panel_label_size))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(fg)
                                                    .child("Add Task"),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}
