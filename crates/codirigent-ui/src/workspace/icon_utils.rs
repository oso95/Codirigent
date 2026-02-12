//! Icon rendering utilities for workspace components.

use crate::icons;
use crate::workspace::gpui::WorkspaceView;
use gpui::{div, px, FontWeight, IntoElement, ParentElement, SharedString, Styled};

impl WorkspaceView {
    /// Render a Lucide icon inside a fixed square for stable alignment.
    ///
    /// Creates a centered icon within a `(size + 2) × (size + 2)` px square.
    /// The extra 2px padding prevents clipping and ensures consistent spacing.
    ///
    /// # Parameters
    /// - `icon`: Icon string (from `crate::icons`)
    /// - `color`: Icon color
    /// - `size`: Icon size in pixels
    ///
    /// # Example
    /// ```ignore
    /// self.centered_lucide_icon(icons::check(), fg_color, 14.0)
    /// ```
    pub(super) fn centered_lucide_icon(
        &self,
        icon: String,
        color: gpui::Hsla,
        size: f32,
    ) -> impl IntoElement {
        self.centered_lucide_icon_with_offset(icon, color, size, 1.0)
    }

    /// Render a Lucide icon in a fixed square with a subtle vertical offset for text-row alignment.
    ///
    /// This variant allows custom vertical offset to align icons with adjacent text.
    /// The default offset (1.0px) typically aligns icons well with most text sizes.
    ///
    /// # Parameters
    /// - `icon`: Icon string (from `crate::icons`)
    /// - `color`: Icon color
    /// - `size`: Icon size in pixels
    /// - `y_offset`: Vertical offset in pixels (positive moves down)
    ///
    /// # Example
    /// ```ignore
    /// // Icon with 2px downward offset for better alignment
    /// self.centered_lucide_icon_with_offset(icons::folder(), fg_color, 16.0, 2.0)
    /// ```
    pub(super) fn centered_lucide_icon_with_offset(
        &self,
        icon: String,
        color: gpui::Hsla,
        size: f32,
        y_offset: f32,
    ) -> impl IntoElement {
        div()
            .w(px(size + 2.0))
            .h(px(size + 2.0))
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .child(
                div()
                    .pt(px(y_offset))
                    .text_size(px(size))
                    .text_color(color)
                    .font_family(icons::LUCIDE_FONT_FAMILY)
                    .child(icon),
            )
    }

    /// Render an icon+text row with consistent vertical alignment.
    ///
    /// Creates a flex row with an icon and label, using default icon offset (2.0px).
    ///
    /// # Parameters
    /// - `icon`: Icon string
    /// - `icon_color`: Icon color
    /// - `icon_size`: Icon size in pixels
    /// - `label`: Text label (can be String, &str, or SharedString)
    /// - `label_color`: Text color
    /// - `label_size`: Text size in pixels
    /// - `label_weight`: Font weight (NORMAL, MEDIUM, BOLD)
    /// - `row_height`: Total row height in pixels
    /// - `gap`: Gap between icon and label in pixels
    ///
    /// # Example
    /// ```ignore
    /// self.aligned_icon_label_row(
    ///     icons::folder(),
    ///     muted_color,
    ///     14.0,
    ///     "Documents",
    ///     fg_color,
    ///     13.0,
    ///     FontWeight::NORMAL,
    ///     20.0,
    ///     6.0,
    /// )
    /// ```
    pub(super) fn aligned_icon_label_row(
        &self,
        icon: String,
        icon_color: gpui::Hsla,
        icon_size: f32,
        label: impl Into<SharedString>,
        label_color: gpui::Hsla,
        label_size: f32,
        label_weight: FontWeight,
        row_height: f32,
        gap: f32,
    ) -> impl IntoElement {
        self.aligned_icon_label_row_with_offset(
            icon,
            icon_color,
            icon_size,
            label,
            label_color,
            label_size,
            label_weight,
            row_height,
            gap,
            2.0,
        )
    }

    /// Render an icon+text row with a custom icon vertical offset.
    ///
    /// This variant allows fine-tuning the icon's vertical position for perfect alignment.
    ///
    /// # Parameters
    /// - `icon`: Icon string
    /// - `icon_color`: Icon color
    /// - `icon_size`: Icon size in pixels
    /// - `label`: Text label
    /// - `label_color`: Text color
    /// - `label_size`: Text size in pixels
    /// - `label_weight`: Font weight
    /// - `row_height`: Total row height in pixels
    /// - `gap`: Gap between icon and label in pixels
    /// - `icon_y_offset`: Icon vertical offset in pixels
    ///
    /// # Example
    /// ```ignore
    /// // Custom offset for specific alignment
    /// self.aligned_icon_label_row_with_offset(
    ///     icons::star(),
    ///     accent_color,
    ///     12.0,
    ///     "Favorite",
    ///     fg_color,
    ///     12.0,
    ///     FontWeight::MEDIUM,
    ///     18.0,
    ///     4.0,
    ///     1.5,  // Custom offset
    /// )
    /// ```
    pub(super) fn aligned_icon_label_row_with_offset(
        &self,
        icon: String,
        icon_color: gpui::Hsla,
        icon_size: f32,
        label: impl Into<SharedString>,
        label_color: gpui::Hsla,
        label_size: f32,
        label_weight: FontWeight,
        row_height: f32,
        gap: f32,
        icon_y_offset: f32,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(gap))
            .child(self.centered_lucide_icon_with_offset(
                icon,
                icon_color,
                icon_size,
                icon_y_offset,
            ))
            .child(
                div().h(px(row_height)).flex().items_center().child(
                    div()
                        .text_size(px(label_size))
                        .font_weight(label_weight)
                        .text_color(label_color)
                        .child(label.into()),
                ),
            )
    }
}
