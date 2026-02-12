//! Workspace view for Codirigent.
//!
//! This module provides the main workspace view that contains the grid
//! of session panes and manages layout switching.
//!
//! # Architecture
//!
//! The workspace module is split into:
//! - [`core`] - Core workspace logic (layout, sessions, focus management)
//! - [`gpui`] - GPUI rendering implementation (requires `gpui-full` feature)
//!
//! # Example
//!
//! ```
//! use codirigent_ui::workspace::Workspace;
//! use codirigent_ui::layout::LayoutProfile;
//! use codirigent_core::{Session, SessionId};
//! use std::path::PathBuf;
//!
//! let mut workspace = Workspace::new();
//! workspace.set_layout(LayoutProfile::Grid2x2);
//!
//! let session = Session::new(SessionId(1), "Session 1".to_string(), PathBuf::from("/tmp"));
//! workspace.add_session(session);
//! ```

mod core;

#[cfg(feature = "gpui-full")]
pub mod gpui;

#[cfg(feature = "gpui-full")]
mod render;

#[cfg(feature = "gpui-full")]
mod icon_utils;

#[cfg(feature = "gpui-full")]
mod settings_panels;

#[cfg(feature = "gpui-full")]
mod ui_primitives;

#[cfg(test)]
mod tests;

// Re-export core types
pub use core::{CellInfo, Workspace};

// Re-export GPUI view type when feature is enabled
#[cfg(feature = "gpui-full")]
pub use gpui::WorkspaceView;
