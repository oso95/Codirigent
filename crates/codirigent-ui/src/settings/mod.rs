//! Settings page module.
//!
//! Provides a full-page overlay settings panel with category sidebar
//! and scrollable content area. Accessed via gear icon or `Ctrl+,`.

mod page;
pub(crate) mod controls;
pub(crate) mod render;
mod general;
mod appearance;
mod terminal;
mod shortcuts;
mod sessions;
mod advanced;

pub use page::{SettingsCategory, SettingsPage};
