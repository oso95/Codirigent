//! Drawer panel rendering for WorkspaceView.
//!
//! Contains rendering for the expandable drawer panels:
//! - Sessions list with group headers
//! - Worktrees/git status panel
//! - File explorer with tree navigation
//! - File tree context menu
//! - Session row and group header components

use super::gpui::WorkspaceView;
use super::types::{
    git_colors, DRAWER_HEADER_HEIGHT, HEADER_HEIGHT, SESSION_DRAWER_ROW_HEIGHT, SESSION_ROW_HEIGHT,
};
use crate::icons;
use crate::theme::CodirigentTheme;
use codirigent_core::{Session, SessionId};
use gpui::{
    div, prelude::FluentBuilder, px, ClickEvent, Context, Focusable, FontWeight,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, SharedString,
    StatefulInteractiveElement, Styled,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum SessionDrawerGroupKey {
    Explicit(String),
    Project {
        display_name: String,
        identity: String,
    },
}

impl SessionDrawerGroupKey {
    fn display_name(&self) -> &str {
        match self {
            Self::Explicit(name) => name,
            Self::Project { display_name, .. } => display_name,
        }
    }

    fn cache_key(&self) -> String {
        match self {
            Self::Explicit(name) => format!("explicit:{name}"),
            Self::Project { identity, .. } => format!("project:{identity}"),
        }
    }
}

type SessionDrawerGroups<'a> = (
    Vec<&'a Session>,
    BTreeMap<SessionDrawerGroupKey, Vec<&'a Session>>,
);

fn session_drawer_groups<'a>(sessions: &'a [Session]) -> SessionDrawerGroups<'a> {
    let mut ungrouped: Vec<&Session> = Vec::new();
    let mut groups: BTreeMap<SessionDrawerGroupKey, Vec<&Session>> = BTreeMap::new();

    for session in sessions {
        let key = session
            .group
            .as_ref()
            .filter(|group| !group.is_empty())
            .cloned()
            .map(SessionDrawerGroupKey::Explicit)
            .or_else(|| {
                let identity = session
                    .git_info
                    .as_ref()
                    .map(|git_info| git_info.repo_root.to_string_lossy().into_owned())
                    .or_else(|| Some(session.working_directory.to_string_lossy().into_owned()))?;
                let display_name =
                    super::gpui::session_project_name(session).unwrap_or_else(|| identity.clone());
                Some(SessionDrawerGroupKey::Project {
                    display_name,
                    identity,
                })
            });

        match key {
            Some(group_key) => groups.entry(group_key).or_default().push(session),
            None => ungrouped.push(session),
        }
    }

    (ungrouped, groups)
}

impl WorkspaceView {
    pub(super) fn render_drawer(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.workspace().theme();
        let drawer_bg: gpui::Hsla = theme.drawer_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let header_bg: gpui::Hsla = theme.header_background.into();
        let muted: gpui::Hsla = theme.muted.into();
        let width = self.drawer.width();
        let panel = self.drawer.active_panel();

        let panel_title = match panel {
            Some(crate::icon_rail::DrawerPanel::Sessions) => "SESSIONS",
            Some(crate::icon_rail::DrawerPanel::Files) => "EXPLORER",
            Some(crate::icon_rail::DrawerPanel::Worktrees) => "WORKTREES",
            None => "",
        };

        // Build content based on active panel
        let content = match panel {
            Some(crate::icon_rail::DrawerPanel::Sessions) => {
                self.render_drawer_sessions_content(cx).into_any_element()
            }
            Some(crate::icon_rail::DrawerPanel::Worktrees) => {
                self.render_drawer_worktrees_content(cx).into_any_element()
            }
            Some(crate::icon_rail::DrawerPanel::Files) => {
                self.render_drawer_files_content(cx).into_any_element()
            }
            None => {
                let session_label = match self.selection.selected_session_id {
                    Some(id) => format!("Session {}", id.0),
                    None => "No session selected".to_string(),
                };
                div()
                    .flex_1()
                    .overflow_hidden()
                    .p_3()
                    .child(div().text_xs().text_color(muted).child(session_label))
                    .into_any_element()
            }
        };

        div()
            .id("drawer-panel")
            .w(px(width))
            .h_full()
            .bg(drawer_bg)
            .border_r_1()
            .border_color(border_color)
            .flex()
            .flex_col()
            // Header
            .child(
                div()
                    .h(px(DRAWER_HEADER_HEIGHT))
                    .w_full()
                    .bg(header_bg)
                    .border_b_1()
                    .border_color(border_color)
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(muted)
                            .child(panel_title),
                    )
                    .child(
                        div()
                            .id("drawer-close")
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.icon_rail.close_drawer();
                                    this.process_icon_rail_events();
                                    cx.notify();
                                }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .font_family(icons::LUCIDE_FONT_FAMILY)
                                    .child(icons::x()),
                            ),
                    ),
            )
            // Content
            .child(content)
    }

    /// Render the sessions list content for the drawer panel.
    fn render_drawer_sessions_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.workspace().theme().clone();
        let muted: gpui::Hsla = theme.muted.into();
        let border_color: gpui::Hsla = theme.border.into();
        let header_bg: gpui::Hsla = theme.header_background.into();

        let sessions: Vec<Session> = self.workspace().sessions().to_vec();
        let focused_id = self.workspace().focused_session_id();
        let visible_session_ids: std::collections::HashSet<SessionId> =
            self.workspace().visible_session_ids().into_iter().collect();
        let session_count = sessions.len();

        // Separate ungrouped and grouped sessions
        let (ungrouped, groups) = session_drawer_groups(&sessions);

        let mut content = div()
            .id("sessions-scroll")
            .flex_1()
            .overflow_y_scroll()
            .flex()
            .flex_col();

        // Render ungrouped sessions first
        for session in &ungrouped {
            content = content.child(self.render_session_row(
                session,
                None,
                focused_id,
                visible_session_ids.contains(&session.id),
                &theme,
                cx,
            ));
        }

        // Render grouped sessions with headers
        let expanded_map = self.cache.drawer_group_expanded.clone();
        for (group_key, group_sessions) in &groups {
            let color = group_sessions.first().and_then(|s| s.color.clone());
            let cache_key = group_key.cache_key();
            let expanded = expanded_map.get(&cache_key).copied().unwrap_or(true);

            content = content.child(self.render_session_group_header(
                group_key,
                color.as_deref(),
                group_sessions.len(),
                expanded,
                &theme,
                cx,
            ));

            if expanded {
                for session in group_sessions {
                    content = content.child(self.render_session_row(
                        session,
                        Some(group_key.display_name()),
                        focused_id,
                        visible_session_ids.contains(&session.id),
                        &theme,
                        cx,
                    ));
                }
            }
        }

        div()
            .flex_1()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(content)
            // Footer
            .child(
                div()
                    .p_3()
                    .border_t_1()
                    .border_color(border_color)
                    .bg(header_bg)
                    .child(div().text_xs().text_color(muted).child(format!(
                        "{} session{}",
                        session_count,
                        if session_count == 1 { "" } else { "s" }
                    ))),
            )
    }

    /// Render the worktrees/git panel content for the drawer.
    fn render_drawer_worktrees_content(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.workspace().theme().clone();
        let muted: gpui::Hsla = theme.muted.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let border_color: gpui::Hsla = theme.border.into();
        let header_bg: gpui::Hsla = theme.header_background.into();
        let green = git_colors::STAGED;
        let orange = git_colors::MODIFIED;
        let red = git_colors::DELETED;
        let blue = git_colors::RENAMED;

        // Show focused session git info, or first session if none focused
        let focused_id = self.workspace().focused_session_id();
        let sessions: Vec<Session> = self.workspace().sessions().to_vec();
        let session = focused_id
            .and_then(|id| sessions.iter().find(|s| s.id == id))
            .or_else(|| sessions.first());

        let dir_name = match session {
            Some(s) => s
                .working_directory
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string(),
            None => "No session".to_string(),
        };

        let mut content = div()
            .flex_1()
            .overflow_hidden()
            .flex()
            .flex_col()
            .p_2()
            .gap_2();

        if session.is_none() {
            return div()
                .flex_1()
                .flex()
                .flex_col()
                .overflow_hidden()
                // Sub-header showing current location
                .child(
                    div()
                        .h(px(HEADER_HEIGHT))
                        .w_full()
                        .bg(header_bg)
                        .border_b_1()
                        .border_color(border_color)
                        .flex()
                        .items_center()
                        .px_3()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(muted)
                                .child("No session selected"),
                        ),
                )
                .child(
                    div().flex_1().p_2().child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child("Select a session to view git worktrees"),
                    ),
                );
        }

        let session = session.expect("BUG: session should be Some after is_none() guard");

        let gi = match session.git_info.as_ref() {
            Some(gi) => gi,
            None => {
                return div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    // Sub-header showing current location
                    .child(
                        div()
                            .h(px(HEADER_HEIGHT))
                            .w_full()
                            .bg(header_bg)
                            .border_b_1()
                            .border_color(border_color)
                            .flex()
                            .items_center()
                            .px_3()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(fg)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(dir_name),
                            ),
                    )
                    .child(
                        div().flex_1().p_2().child(
                            div()
                                .text_xs()
                                .text_color(muted.opacity(0.5))
                                .child("Not a git repository"),
                        ),
                    );
            }
        };

        // Branch + HEAD
        let branch_color = super::types::BRANCH_NAME_COLOR;
        content = content.child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(branch_color)
                        .font_family(icons::LUCIDE_FONT_FAMILY)
                        .child(icons::git_branch()),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(fg)
                        .child(gi.branch.clone()),
                )
                .when_some(gi.head_sha.as_ref(), |el, sha| {
                    el.child(
                        div()
                            .text_xs()
                            .text_color(muted.opacity(0.5))
                            .child(sha.clone()),
                    )
                }),
        );

        // Staged changes section
        if !gi.staged_files.is_empty() {
            content = content.child(
                div()
                    .mt_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(green)
                    .child(format!("Staged ({})", gi.staged_files.len())),
            );
            for file in &gi.staged_files {
                let (label, color) = Self::change_kind_display(&file.change, green, red, blue);
                content = content.child(
                    div()
                        .pl_2()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(color)
                                .child(label),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(fg.opacity(0.8))
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(file.path.clone()),
                        ),
                );
            }
        }

        // Unstaged changes section
        if !gi.unstaged_files.is_empty() {
            content = content.child(
                div()
                    .mt_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(orange)
                    .child(format!("Changes ({})", gi.unstaged_files.len())),
            );
            for file in &gi.unstaged_files {
                let (label, color) = Self::change_kind_display(&file.change, green, red, blue);
                content = content.child(
                    div()
                        .pl_2()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(color)
                                .child(label),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(fg.opacity(0.8))
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(file.path.clone()),
                        ),
                );
            }
        }

        // Clean state
        if gi.staged_files.is_empty() && gi.unstaged_files.is_empty() {
            content = content.child(
                div()
                    .mt_1()
                    .text_xs()
                    .text_color(green)
                    .child("Working tree clean"),
            );
        }

        // Worktrees section
        let worktrees = self.project.worktree_panel.worktrees();
        if !worktrees.is_empty() {
            // Section divider + header
            content = content.child(
                div()
                    .mt_3()
                    .pt_2()
                    .border_t_1()
                    .border_color(border_color.opacity(0.3))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(muted)
                            .child("Worktrees"),
                    ),
            );

            // Collect session lookup data
            let all_sessions: Vec<Session> = self.workspace().sessions().to_vec();

            for wt in worktrees {
                // Icon: star for main, diamond for linked
                let icon = if wt.is_main { "\u{2605}" } else { "\u{25C6}" };
                let icon_color = if wt.is_main { green } else { blue };

                let sha_text = wt
                    .head_sha
                    .as_ref()
                    .map(|s| {
                        if s.len() > 3 {
                            s[..3].to_string()
                        } else {
                            s.clone()
                        }
                    })
                    .unwrap_or_default();

                // Session binding label
                let binding_label = match wt.bound_session {
                    Some(sid) => {
                        let session_name = all_sessions
                            .iter()
                            .find(|s| s.id == sid)
                            .map(|s| s.name.clone())
                            .unwrap_or_else(|| format!("Session {}", sid.0));
                        format!("\u{2192} {}", session_name)
                    }
                    None => "(unbound)".to_string(),
                };

                content = content
                    .child(
                        div()
                            .pl_1()
                            .flex()
                            .items_center()
                            .gap_1()
                            // Icon
                            .child(div().text_xs().text_color(icon_color).child(icon))
                            // Branch name
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(fg)
                                    .child(wt.branch.clone()),
                            )
                            // Short SHA
                            .when(!sha_text.is_empty(), |el| {
                                el.child(
                                    div()
                                        .text_xs()
                                        .text_color(muted.opacity(0.5))
                                        .child(sha_text),
                                )
                            }),
                    )
                    .child(
                        div()
                            .pl_4()
                            .text_xs()
                            .text_color(muted.opacity(0.6))
                            .child(binding_label),
                    );
            }
        }

        // Branches section — show branches not already in a worktree
        let available_branches = self.project.worktree_panel.available_branches();
        let worktree_branches: Vec<&str> = self
            .project
            .worktree_panel
            .worktrees()
            .iter()
            .map(|wt| wt.branch.as_str())
            .collect();
        let extra_branches: Vec<&String> = available_branches
            .iter()
            .filter(|b| !worktree_branches.contains(&b.as_str()))
            .collect();

        if !extra_branches.is_empty() {
            content = content.child(
                div()
                    .mt_3()
                    .pt_2()
                    .border_t_1()
                    .border_color(border_color.opacity(0.3))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(muted)
                            .child("Branches"),
                    ),
            );

            for branch in &extra_branches {
                content = content.child(
                    div()
                        .pl_3()
                        .text_xs()
                        .text_color(muted.opacity(0.6))
                        .child((*branch).clone()),
                );
            }
        }

        // Footer — summary counts
        let wt_count = self.project.worktree_panel.worktrees().len();
        let branch_count = available_branches.len();
        if wt_count > 0 || branch_count > 0 {
            content = content.child(
                div()
                    .mt_3()
                    .pt_2()
                    .border_t_1()
                    .border_color(border_color.opacity(0.3))
                    .text_xs()
                    .text_color(muted.opacity(0.4))
                    .child(format!(
                        "{} worktree{} \u{00B7} {} branch{}",
                        wt_count,
                        if wt_count == 1 { "" } else { "s" },
                        branch_count,
                        if branch_count == 1 { "" } else { "es" },
                    )),
            );
        }

        // Return with sub-header matching file tree design
        div()
            .flex_1()
            .flex()
            .flex_col()
            .overflow_hidden()
            // Sub-header showing current location
            .child(
                div()
                    .h(px(HEADER_HEIGHT))
                    .w_full()
                    .bg(header_bg)
                    .border_b_1()
                    .border_color(border_color)
                    .flex()
                    .items_center()
                    .px_3()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(fg)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(dir_name),
                    ),
            )
            // Scrollable content area
            .child(content)
    }

    /// Map a git change kind to a display label and color.
    fn change_kind_display(
        kind: &codirigent_core::GitChangeKind,
        green: gpui::Hsla,
        red: gpui::Hsla,
        blue: gpui::Hsla,
    ) -> (&'static str, gpui::Hsla) {
        match kind {
            codirigent_core::GitChangeKind::Modified => ("M", blue),
            codirigent_core::GitChangeKind::Added => ("A", green),
            codirigent_core::GitChangeKind::Deleted => ("D", red),
            codirigent_core::GitChangeKind::Renamed => ("R", blue),
        }
    }

    /// Render the file tree content for the drawer panel.
    fn render_drawer_files_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.workspace().theme().clone();
        let muted: gpui::Hsla = theme.muted.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let border_color: gpui::Hsla = theme.border.into();
        let header_bg: gpui::Hsla = theme.header_background.into();
        let active_bg: gpui::Hsla = theme.active.into();

        // Project root name for sub-header
        let root_name = self
            .project
            .project_root
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Project")
            .to_string();

        let show_hidden = self.project.file_tree.show_hidden();
        let item_count = self.project.file_tree.visible_count();

        // Collect items into owned vec for the closure
        let items: Vec<crate::sidebar::FileTreeRenderItem> =
            self.project.file_tree.visible_items().to_vec();

        // Build scrollable tree rows
        let mut tree_content = div()
            .id("file-tree-scroll")
            .flex_1()
            .overflow_y_scroll()
            .flex()
            .flex_col();

        for (idx, item) in items.iter().enumerate() {
            tree_content = tree_content.child(self.render_file_tree_row(idx, item, &theme, cx));
        }

        // Eye icon: show/hide hidden files
        let eye_icon = if show_hidden {
            icons::eye()
        } else {
            icons::eye_off()
        };

        div()
            .flex_1()
            .flex()
            .flex_col()
            .overflow_hidden()
            // Sub-header toolbar
            .child(
                div()
                    .h(px(HEADER_HEIGHT))
                    .w_full()
                    .bg(header_bg)
                    .border_b_1()
                    .border_color(border_color)
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(fg)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(root_name),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            // Eye toggle
                            .child(
                                div()
                                    .id("file-tree-toggle-hidden")
                                    .cursor_pointer()
                                    .px(px(4.0))
                                    .py(px(2.0))
                                    .rounded_sm()
                                    .hover(|style| style.bg(active_bg))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            let new_val = !this.project.file_tree.show_hidden();
                                            this.project.file_tree.set_show_hidden(new_val);
                                            if let Some(tree) =
                                                this.project.file_tree_model.as_mut()
                                            {
                                                tree.set_show_hidden(new_val);
                                                if let Err(e) = tree.refresh() {
                                                    tracing::warn!(
                                                        "Failed to refresh file tree: {}",
                                                        e
                                                    );
                                                }
                                            }
                                            this.refresh_file_tree_panel();
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(muted)
                                            .font_family(icons::LUCIDE_FONT_FAMILY)
                                            .child(eye_icon),
                                    ),
                            )
                            // Refresh button
                            .child(
                                div()
                                    .id("file-tree-refresh")
                                    .cursor_pointer()
                                    .px(px(4.0))
                                    .py(px(2.0))
                                    .rounded_sm()
                                    .hover(|style| style.bg(active_bg))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            if let Some(tree) =
                                                this.project.file_tree_model.as_mut()
                                            {
                                                if let Err(e) = tree.refresh() {
                                                    tracing::warn!(
                                                        "Failed to refresh file tree: {}",
                                                        e
                                                    );
                                                }
                                            }
                                            this.refresh_file_tree_panel();
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(muted)
                                            .font_family(icons::LUCIDE_FONT_FAMILY)
                                            .child(icons::refresh()),
                                    ),
                            ),
                    ),
            )
            // Scrollable tree list
            .child(tree_content)
            // Footer
            .child(
                div()
                    .p_3()
                    .border_t_1()
                    .border_color(border_color)
                    .bg(header_bg)
                    .child(div().text_xs().text_color(muted).child(format!(
                        "{} item{}",
                        item_count,
                        if item_count == 1 { "" } else { "s" }
                    ))),
            )
    }

    /// Render a single file tree row.
    fn render_file_tree_row(
        &mut self,
        idx: usize,
        item: &crate::sidebar::FileTreeRenderItem,
        theme: &CodirigentTheme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let muted: gpui::Hsla = theme.muted.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let active_bg: gpui::Hsla = theme.active.into();

        let depth = item.depth as f32;
        let is_dir = item.is_dir;
        let expanded = item.expanded;
        let is_selected = item.is_selected;
        let path = item.path.clone();
        let icon_color: gpui::Hsla = item.icon.color().into();
        let icon_str = item.icon.lucide_icon();
        let name = item.name.clone();

        let name_color = if is_selected { fg } else { muted };
        let row_bg = if is_selected {
            active_bg
        } else {
            gpui::Hsla::transparent_black()
        };

        // Chevron for directories, spacer for files
        let chevron = if is_dir {
            let chevron_str = if expanded {
                icons::chevron_down()
            } else {
                icons::chevron_right()
            };
            div()
                .w(px(14.0))
                .h(px(14.0))
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .child(
                    div()
                        .text_color(muted)
                        .font_family(icons::LUCIDE_FONT_FAMILY)
                        .text_size(px(10.0))
                        .child(chevron_str),
                )
        } else {
            div().w(px(14.0)).h(px(14.0)).flex_shrink_0()
        };

        // Multiple closures need owned path copies since:
        // 1. Rust closures consume captured variables (move)
        // 2. GPUI event handlers require 'static lifetime
        // 3. Each handler needs independent ownership
        // Alternative: Use Arc<PathBuf> everywhere (overkill for this use case)
        let path_for_click = path.clone();
        let path_for_dbl = path.clone();
        let path_for_ctx = path.clone();

        div()
            .id(SharedString::from(format!("file-tree-row-{}", idx)))
            .h(px(crate::sidebar::FileTreePanel::ITEM_HEIGHT))
            .w_full()
            .pl(px(depth * crate::sidebar::FileTreePanel::INDENT_SIZE + 4.0))
            .pr(px(8.0))
            .flex()
            .items_center()
            .gap(px(4.0))
            .bg(row_bg)
            .cursor_pointer()
            .hover(|style| style.bg(active_bg))
            .on_click(cx.listener(move |this, event: &ClickEvent, _window, cx| {
                if event.click_count() >= 2 && !is_dir {
                    // Double-click on file -> activate (insert path)
                    let ev = crate::sidebar::FileTreeEvent::FileActivated(path_for_dbl.clone());
                    this.handle_file_tree_event(ev, cx);
                } else if is_dir {
                    // Click on directory -> toggle
                    let ev =
                        crate::sidebar::FileTreeEvent::DirectoryToggled(path_for_click.clone());
                    this.handle_file_tree_event(ev, cx);
                } else {
                    // Single click on file -> select
                    let ev = crate::sidebar::FileTreeEvent::FileSelected(path_for_click.clone());
                    this.handle_file_tree_event(ev, cx);
                }
                cx.notify();
            }))
            // Right-click -> context menu
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _window, cx| {
                    this.open_file_tree_context_menu(path_for_ctx.clone(), event.position, cx);
                }),
            )
            // Chevron
            .child(chevron)
            // Icon
            .child(self.centered_lucide_icon(icon_str, icon_color, 12.0))
            // Name
            .child(
                div()
                    .text_xs()
                    .text_color(name_color)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(name),
            )
    }

    /// Render the file tree context menu (right-click menu).
    pub(super) fn render_file_tree_context_menu(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let menu = self.selection.file_tree_context_menu.clone()?;

        let theme = self.workspace().theme().clone();
        let panel_bg: gpui::Hsla = theme.panel_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let hover_bg: gpui::Hsla = theme.active.into();

        let path_for_insert = menu.path.clone();
        let path_for_copy = menu.path.clone();
        let path_for_task = menu.path.clone();

        // Click-away backdrop (transparent)
        let backdrop = div()
            .id("file-ctx-menu-backdrop")
            .absolute()
            .inset_0()
            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                this.close_file_tree_context_menu(cx);
            }));

        // Menu items
        let insert_item = div()
            .id("ctx-insert-path")
            .h(px(SESSION_ROW_HEIGHT))
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .cursor_pointer()
            .hover(move |style| style.bg(hover_bg))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    let path = path_for_insert.clone();
                    this.insert_path_to_terminal(&path);
                    this.close_file_tree_context_menu(cx);
                    cx.stop_propagation();
                }),
            )
            .child(div().text_xs().text_color(fg).child("Insert path"));

        let copy_item = div()
            .id("ctx-copy-path")
            .h(px(SESSION_ROW_HEIGHT))
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .cursor_pointer()
            .hover(move |style| style.bg(hover_bg))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    let path = path_for_copy.clone();
                    this.copy_path_to_clipboard(&path);
                    this.close_file_tree_context_menu(cx);
                    cx.stop_propagation();
                }),
            )
            .child(div().text_xs().text_color(fg).child("Copy path"));

        let create_task_item = div()
            .id("ctx-create-task")
            .h(px(SESSION_ROW_HEIGHT))
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .cursor_pointer()
            .hover(move |style| style.bg(hover_bg))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    let path = path_for_task.clone();
                    this.open_task_creation_modal_for_file(&path);
                    this.close_file_tree_context_menu(cx);
                    cx.stop_propagation();
                }),
            )
            .child(div().text_xs().text_color(fg).child("Create task"));

        let dropdown = div()
            .w(px(140.0))
            .bg(panel_bg)
            .border_1()
            .border_color(border_color)
            .rounded_md()
            .overflow_hidden()
            .shadow_lg()
            .flex()
            .flex_col()
            .py_1()
            // Prevent clicks on the menu from propagating to elements behind it
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_this, _: &MouseDownEvent, _window, cx| {
                    cx.stop_propagation();
                }),
            )
            .child(insert_item)
            .child(copy_item)
            .child(create_task_item);

        // Position the menu at the click location
        let menu_container = div()
            .absolute()
            .top(menu.position.y)
            .left(menu.position.x)
            .child(dropdown);

        Some(
            div()
                .id("file-tree-context-menu-overlay")
                .absolute()
                .inset_0()
                .child(backdrop)
                .child(menu_container),
        )
    }

    /// Render a single session row in the drawer session list.
    fn render_session_row(
        &mut self,
        session: &Session,
        group_name: Option<&str>,
        focused_id: Option<SessionId>,
        is_visible: bool,
        theme: &CodirigentTheme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let muted: gpui::Hsla = theme.muted.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let status_color: gpui::Hsla = theme.status_color(session.status).into();
        let is_focused = focused_id == Some(session.id);
        let is_hidden = !is_visible;
        let row_bg = if is_focused {
            theme.active.into()
        } else {
            gpui::Hsla::transparent_black()
        };
        let hover_bg: gpui::Hsla = if is_focused {
            row_bg
        } else {
            theme.active.into()
        };
        let orange: gpui::Hsla = theme.orange.into();
        let primary: gpui::Hsla = theme.primary.into();

        let session_id = session.id;
        let session_name = session.name.clone();
        let project_name = super::gpui::session_project_name(session);
        let project_subtitle = project_name
            .clone()
            .or_else(|| Some(session.working_directory.to_string_lossy().into_owned()))
            .filter(|project| group_name != Some(project.as_str()));
        let cli_name = self.session_cli_display_name(session_id);
        let context_pct = session.context_usage;
        let (shell_label, shell_warning) =
            self.session_shell_display(session_id, session.shell.as_deref());
        let show_shell_label = session.shell.is_some() || shell_warning.is_some();
        let branch_badge = session.git_info.as_ref().map(|git_info| {
            let mut branch = git_info.branch.clone();
            if branch.chars().count() > 16 {
                branch = branch.chars().take(13).collect::<String>() + "...";
            }
            branch
        });
        div()
            .id(SharedString::from(format!("session-row-{}", session_id.0)))
            .h(px(SESSION_DRAWER_ROW_HEIGHT))
            .w_full()
            .px_3()
            .py(px(6.0))
            .flex()
            .items_start()
            .gap_2()
            .bg(row_bg)
            .border_l_2()
            .border_color(if is_focused { primary } else { row_bg })
            .cursor_pointer()
            .hover(move |style| style.bg(hover_bg))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.select_session_with_cx(session_id, cx);
                    window.focus(&this.focus_handle(cx));
                    cx.notify();
                }),
            )
            // Status dot
            .child(
                div()
                    .w(px(8.0))
                    .h(px(8.0))
                    .rounded_full()
                    .bg(status_color)
                    .flex_shrink_0(),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(fg)
                                    .text_ellipsis()
                                    .child(session_name),
                            )
                            .when(is_hidden, |el| {
                                el.child(Self::render_session_metadata_badge(
                                    "Hidden",
                                    muted.opacity(0.85),
                                    muted.opacity(0.12),
                                ))
                            })
                            .when_some(context_pct, |el, pct| {
                                let context_color: gpui::Hsla =
                                    crate::terminal_header::ContextLevel::from_percentage(pct)
                                        .color()
                                        .into();
                                el.child(
                                    div()
                                        .text_xs()
                                        .text_color(context_color)
                                        .flex_shrink_0()
                                        .child(format!("{}%", (pct * 100.0) as u32)),
                                )
                            })
                            .when(show_shell_label, |el| {
                                el.child(
                                    div()
                                        .text_xs()
                                        .text_color(if shell_warning.is_some() {
                                            orange
                                        } else {
                                            muted.opacity(0.8)
                                        })
                                        .flex_shrink_0()
                                        .child(shell_label.clone()),
                                )
                            })
                            .when_some(cli_name, |el, cli_name| {
                                el.child(Self::render_session_metadata_badge(
                                    &cli_name,
                                    primary,
                                    primary.opacity(0.12),
                                ))
                            }),
                    )
                    .when(project_subtitle.is_some() || branch_badge.is_some(), |el| {
                        el.child(
                            div()
                                .w_full()
                                .flex()
                                .items_center()
                                .gap_2()
                                .when_some(project_subtitle.clone(), |row, project_subtitle| {
                                    row.child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .text_xs()
                                            .text_color(if is_focused {
                                                muted.opacity(0.95)
                                            } else {
                                                muted.opacity(0.82)
                                            })
                                            .text_ellipsis()
                                            .child(project_subtitle),
                                    )
                                })
                                .when_some(branch_badge.clone(), |row, branch| {
                                    row.child(
                                        div()
                                            .max_w(px(96.0))
                                            .overflow_hidden()
                                            .text_xs()
                                            .text_color(muted.opacity(0.8))
                                            .text_ellipsis()
                                            .flex_shrink_0()
                                            .child(branch),
                                    )
                                }),
                        )
                    }),
            )
            // Menu button
            .child(
                div()
                    .id(SharedString::from(format!("session-menu-{}", session_id.0)))
                    .w(px(24.0))
                    .h(px(24.0))
                    .flex_shrink_0()
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|style| style.bg(super::types::CANCEL_BUTTON_HOVER))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                            this.open_session_menu(
                                session_id,
                                Some(event.position.y.into()),
                                None,
                                cx,
                            );
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .font_family(icons::LUCIDE_FONT_FAMILY)
                            .child(icons::more_horizontal()),
                    ),
            )
    }

    pub(super) fn session_drawer_row_offset(&self, session_id: SessionId) -> Option<f32> {
        let sessions: Vec<Session> = self.workspace().sessions().to_vec();
        let (ungrouped, groups) = session_drawer_groups(&sessions);

        let mut offset = 0.0;
        for session in ungrouped {
            if session.id == session_id {
                return Some(offset);
            }
            offset += SESSION_DRAWER_ROW_HEIGHT;
        }

        for (group_key, group_sessions) in groups {
            offset += SESSION_ROW_HEIGHT;
            let cache_key = group_key.cache_key();
            let expanded = self
                .cache
                .drawer_group_expanded
                .get(&cache_key)
                .copied()
                .unwrap_or(true);
            if !expanded {
                continue;
            }

            for session in group_sessions {
                if session.id == session_id {
                    return Some(offset);
                }
                offset += SESSION_DRAWER_ROW_HEIGHT;
            }
        }

        None
    }

    fn render_session_metadata_badge(
        text: &str,
        text_color: gpui::Hsla,
        background: gpui::Hsla,
    ) -> gpui::Div {
        div()
            .flex_shrink_0()
            .max_w(px(120.0))
            .px(px(4.0))
            .py_px()
            .rounded_sm()
            .bg(background)
            .child(
                div()
                    .overflow_hidden()
                    .text_xs()
                    .text_ellipsis()
                    .text_color(text_color)
                    .child(text.to_owned()),
            )
    }

    /// Render a session group header in the drawer session list.
    fn render_session_group_header(
        &mut self,
        group_key: &SessionDrawerGroupKey,
        color: Option<&str>,
        count: usize,
        expanded: bool,
        theme: &CodirigentTheme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let muted: gpui::Hsla = theme.muted.into();

        // Parse group color or use a default
        let bar_color: gpui::Hsla = color
            .and_then(crate::theme::hex_to_hsla)
            .map(|h| h.into())
            .unwrap_or(muted);

        let chevron = if expanded {
            icons::chevron_down()
        } else {
            icons::chevron_right()
        };

        let group_name = group_key.display_name();
        let group_label = format!("{} ({})", group_name, count);
        let toggle_key = group_key.cache_key();

        div()
            .id(SharedString::from(format!("group-header-{}", toggle_key)))
            .h(px(SESSION_ROW_HEIGHT))
            .w_full()
            .px_3()
            .flex()
            .items_center()
            .gap(px(6.0))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    let current = this
                        .cache
                        .drawer_group_expanded
                        .get(&toggle_key)
                        .copied()
                        .unwrap_or(true);
                    this.cache
                        .drawer_group_expanded
                        .insert(toggle_key.clone(), !current);
                    cx.notify();
                }),
            )
            // Color bar
            .child(
                div()
                    .w(px(3.0))
                    .h(px(16.0))
                    .rounded_sm()
                    .bg(bar_color)
                    .flex_shrink_0(),
            )
            .child(self.aligned_icon_label_row(
                chevron,
                muted,
                12.0,
                group_label,
                muted,
                11.0,
                FontWeight::BOLD,
                14.0,
                6.0,
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codirigent_core::types::session::generate_session_uuid;
    use codirigent_core::{GitRepoInfo, SessionId, SessionStatus};
    use std::path::PathBuf;

    fn test_session(id: u64, working_directory: &str) -> Session {
        Session {
            id: SessionId(id),
            session_uuid: generate_session_uuid(),
            name: format!("Session {id}"),
            status: SessionStatus::Idle,
            working_directory: PathBuf::from(working_directory),
            shell: None,
            current_task: None,
            context_usage: None,
            created_at: chrono::Utc::now(),
            group: None,
            color: None,
            git_info: None,
            claude_session_id: None,
            codex_session_id: None,
            codex_execution_mode: None,
            codex_started_at: None,
            gemini_session_id: None,
        }
    }

    #[test]
    fn session_drawer_groups_falls_back_to_project_name() {
        let sessions = vec![
            test_session(1, "/workspace/dirigent"),
            test_session(2, "/workspace/dirigent"),
        ];

        let (ungrouped, groups) = session_drawer_groups(&sessions);

        assert!(ungrouped.is_empty());
        assert_eq!(groups.len(), 1);
        assert_eq!(
            groups
                .get(&SessionDrawerGroupKey::Project {
                    display_name: "dirigent".to_string(),
                    identity: "/workspace/dirigent".to_string(),
                })
                .map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn session_drawer_groups_keep_explicit_and_project_groups_distinct() {
        let mut explicit = test_session(1, "/workspace/dirigent");
        explicit.group = Some("dirigent".to_string());

        let mut derived = test_session(2, "/workspace/dirigent/subdir");
        derived.git_info = Some(GitRepoInfo {
            repo_root: PathBuf::from("/workspace/dirigent"),
            branch: "main".to_string(),
            dirty_count: 0,
            has_staged: false,
            head_sha: None,
            unstaged_files: Vec::new(),
            staged_files: Vec::new(),
        });

        let sessions = vec![explicit, derived];
        let (_ungrouped, groups) = session_drawer_groups(&sessions);

        assert_eq!(groups.len(), 2);
        assert_eq!(
            groups
                .get(&SessionDrawerGroupKey::Explicit("dirigent".to_string()))
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            groups
                .get(&SessionDrawerGroupKey::Project {
                    display_name: "dirigent".to_string(),
                    identity: "/workspace/dirigent".to_string(),
                })
                .map(Vec::len),
            Some(1)
        );
    }

    #[test]
    fn session_drawer_groups_keep_duplicate_project_names_distinct_by_path() {
        let sessions = vec![
            test_session(1, "/workspace/apps/dirigent"),
            test_session(2, "/workspace/tools/dirigent"),
        ];

        let (_ungrouped, groups) = session_drawer_groups(&sessions);

        assert_eq!(groups.len(), 2);
        assert_eq!(
            groups
                .get(&SessionDrawerGroupKey::Project {
                    display_name: "dirigent".to_string(),
                    identity: "/workspace/apps/dirigent".to_string(),
                })
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            groups
                .get(&SessionDrawerGroupKey::Project {
                    display_name: "dirigent".to_string(),
                    identity: "/workspace/tools/dirigent".to_string(),
                })
                .map(Vec::len),
            Some(1)
        );
    }
}
