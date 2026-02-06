//! Codirigent Session
//!
//! Session management crate providing PTY abstraction, process tree
//! management, session state tracking, and skill management for Codirigent.
//!
//! # Overview
//!
//! This crate provides the foundational PTY (pseudo-terminal) handling,
//! session management, and skill loading for Codirigent. Each session represents
//! a terminal running an AI coding CLI tool.
//!
//! # Modules
//!
//! - [`pty`] - PTY creation, I/O, and async output reading
//! - [`session`] - Internal session state combining metadata with runtime handles
//! - [`manager`] - Session manager implementing the `SessionManager` trait
//! - [`skill_manager`] - Skill discovery and management from filesystem
//! - [`broadcast_service`] - Broadcast messaging to multiple sessions
//!
//! # Example
//!
//! ```no_run
//! use codirigent_session::{DefaultSessionManager, PtyHandle, PtySize, OutputReader};
//! use codirigent_core::{DefaultEventBus, SessionManager};
//! use std::sync::Arc;
//! use std::path::Path;
//!
//! // Create a session manager with an event bus
//! let event_bus = Arc::new(DefaultEventBus::new(16));
//! let mut manager = DefaultSessionManager::new(event_bus);
//!
//! // Create a new session
//! let id = manager.create_session(
//!     "My Session".to_string(),
//!     std::path::PathBuf::from("/tmp"),
//! ).unwrap();
//!
//! // Send input to the session
//! manager.send_input(id, b"echo hello\n").unwrap();
//!
//! // Close the session when done
//! manager.close_session(id).unwrap();
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod broadcast_service;
pub mod cli_detector;
pub mod clipboard_service;
pub mod git_status;
pub mod manager;
pub mod osc7;
pub mod pty;
pub mod ralph_controller;
pub mod session;
pub mod skill_manager;
pub mod worktree;

pub use broadcast_service::DefaultBroadcastService;
pub use cli_detector::{CliDetector, DefaultCliDetector};
pub use clipboard_service::{ClipboardService, DefaultClipboardService};
pub use manager::DefaultSessionManager;
pub use pty::{spawn_output_reader, OutputReader, PtyHandle, PtySize};
pub use ralph_controller::{DefaultRalphLoopController, LoopStats};
pub use session::SessionState;
pub use skill_manager::DefaultSkillManager;
pub use git_status::GitStatusService;
pub use osc7::extract_osc7_path;
pub use worktree::WorktreeManager;
