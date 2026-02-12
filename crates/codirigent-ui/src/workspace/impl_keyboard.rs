//! Keyboard input handlers for WorkspaceView.
//!
//! This module contains all methods related to:
//! - Custom layout picker keyboard handling
//! - Layout profile persistence to settings

use super::gpui::WorkspaceView;
use crate::toolbar::CustomLayoutMode;
use codirigent_core::config_service::ConfigService;
use codirigent_core::SplitDirection;
use gpui::{Context, KeyDownEvent};
use tracing::warn;

impl WorkspaceView {
    pub(super) fn save_layout_profiles_to_settings(&self) {
        if let Some(ref config_service) = self.config_service {
            // Load current user settings
            let mut user_settings = config_service.load_user_settings().unwrap_or_default();

            // Update saved_layouts with current profiles
            user_settings.saved_layouts = self.top_bar.export_user_profiles();

            // Save back to disk
            if let Err(e) = config_service.save_user_settings(&user_settings) {
                warn!("Failed to save layout profiles: {}", e);
            }
        }
    }

    pub(super) fn handle_custom_layout_key_down(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.custom_picker.is_open {
            return false;
        }

        let key = event.keystroke.key.to_lowercase();

        // Universal keys (both modes)
        match key.as_str() {
            "escape" => {
                self.custom_picker.close();
                cx.notify();
                return true;
            }
            "enter" => {
                match self.custom_picker.mode {
                    CustomLayoutMode::Grid => {
                        if let Some((rows, cols)) = self.custom_picker.validate() {
                            self.custom_picker.close();
                            let profile = crate::layout::LayoutProfile::Custom { rows, cols };
                            self.workspace.set_layout(profile);
                            // Save as a named profile in the profile manager
                            let id = format!("custom-{}x{}", rows, cols);
                            let name = format!("{}x{}", rows, cols);
                            let saved = crate::layout_profile::SavedLayoutProfile::new(
                                id.clone(),
                                name,
                                codirigent_core::LayoutMode::Grid { rows, cols },
                            );
                            self.top_bar.profile_manager.add_profile(saved);
                            self.top_bar.set_active_profile_id(&id);
                            // Persist to disk
                            self.save_layout_profiles_to_settings();
                        }
                    }
                    CustomLayoutMode::Split => {
                        if let Some(tree) = self.custom_picker.validate_split() {
                            self.custom_picker.close();
                            let pane_count = tree.leaf_count();
                            let id = format!("custom-split-{}", pane_count);
                            let name = format!("Split ({})", pane_count);
                            let saved = crate::layout_profile::SavedLayoutProfile::new(
                                id.clone(),
                                name,
                                codirigent_core::LayoutMode::SplitTree { root: tree.clone() },
                            );
                            self.workspace.set_split_tree(tree);
                            self.top_bar.profile_manager.add_profile(saved);
                            self.top_bar.set_active_profile_id(&id);
                            // Persist to disk
                            self.save_layout_profiles_to_settings();
                        }
                    }
                }
                cx.notify();
                return true;
            }
            _ => {}
        }

        // Mode-specific keys
        match self.custom_picker.mode {
            CustomLayoutMode::Grid => {
                match key.as_str() {
                    "tab" => {
                        let current = self.custom_picker.focused_input().unwrap_or(0);
                        let next = if current == 0 { 1 } else { 0 };
                        self.custom_picker.set_focus(next);
                        cx.notify();
                        return true;
                    }
                    "backspace" => {
                        self.custom_picker.handle_backspace();
                        cx.notify();
                        return true;
                    }
                    _ => {}
                }

                if event.keystroke.modifiers.control
                    || event.keystroke.modifiers.alt
                    || event.keystroke.modifiers.platform
                {
                    return true;
                }

                if let Some(ref key_char) = event.keystroke.key_char {
                    if let Some(ch) = key_char.chars().next() {
                        if ch.is_ascii_digit() {
                            self.custom_picker.handle_char_input(ch);
                            cx.notify();
                        }
                    }
                }
            }
            CustomLayoutMode::Split => {
                match key.as_str() {
                    "h" => {
                        self.custom_picker
                            .split_selected(SplitDirection::Horizontal);
                        cx.notify();
                        return true;
                    }
                    "v" => {
                        self.custom_picker.split_selected(SplitDirection::Vertical);
                        cx.notify();
                        return true;
                    }
                    "backspace" | "delete" => {
                        self.custom_picker.remove_selected();
                        cx.notify();
                        return true;
                    }
                    "tab" => {
                        // Cycle selected slot
                        let slots = self.custom_picker.split_tree.slots_in_order();
                        if !slots.is_empty() {
                            let current_idx = self
                                .custom_picker
                                .selected_slot
                                .and_then(|s| slots.iter().position(|&o| o == s));
                            let next = match current_idx {
                                Some(i) => (i + 1) % slots.len(),
                                None => 0,
                            };
                            self.custom_picker.selected_slot = Some(slots[next]);
                        }
                        cx.notify();
                        return true;
                    }
                    _ => {}
                }
            }
        }

        true
    }
}
