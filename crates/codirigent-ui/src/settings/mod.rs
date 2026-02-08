//! Settings page module.
//!
//! Provides a full-page overlay settings panel with category sidebar
//! and scrollable content area. Accessed via gear icon or `Ctrl+,`.

mod advanced;
mod appearance;
pub(crate) mod controls;
mod general;
mod page;
pub(crate) mod render;
mod sessions;
mod shortcuts;
mod terminal;

pub use page::{SettingsCategory, SettingsPage};
