//! Auto-compaction service for task-board sessions.
//!
//! When a Claude Code session finishes implementing a task (Working -> Idle),
//! the context window is typically full of implementation conversation. Before
//! Dirigent runs verification (tests), compacting the context gives the AI
//! maximum room to reason about test output and failures.
//!
//! This module provides [`CompactionConfig`] and [`CompactionService`] which
//! manage the `/compact` command lifecycle for task-board sessions.
//!
//! # Flow
//!
//! 1. Session completes implementation (Working -> Idle)
//! 2. `should_compact()` checks if compaction is warranted
//! 3. `begin_compaction()` marks the session as compacting (re-entrancy guard)
//! 4. `/compact` is sent via PTY stdin
//! 5. Session goes Working (compact processing) then Idle (compact done)
//! 6. `end_compaction()` clears the guard
//! 7. Normal verification flow continues

use crate::types::SessionId;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Configuration for auto-compaction before verification.
///
/// Controls when and how `/compact` is sent to task-board sessions
/// between task implementation and verification.
///
/// # Example
///
/// ```
/// use codirigent_core::compaction::CompactionConfig;
///
/// let config = CompactionConfig::default();
/// assert!(config.enabled);
/// assert!((config.min_context_threshold - 0.3).abs() < f32::EPSILON);
/// assert_eq!(config.timeout_secs, 120);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionConfig {
    /// Whether auto-compaction is enabled.
    pub enabled: bool,

    /// Minimum context usage threshold (0.0-1.0) to trigger compaction.
    /// Sessions below this threshold are skipped since there's nothing
    /// meaningful to compact.
    pub min_context_threshold: f32,

    /// Optional focus instructions for the `/compact` command.
    /// When set, the command becomes `/compact <focus>`.
    pub focus_instructions: Option<String>,

    /// Timeout in seconds for waiting for compaction to complete.
    pub timeout_secs: u64,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_context_threshold: 0.3,
            focus_instructions: None,
            timeout_secs: 120,
        }
    }
}

/// Service managing auto-compaction state for task-board sessions.
///
/// Tracks which sessions are currently being compacted to prevent
/// re-entrancy (e.g., compacting a session that's already compacting).
///
/// # Example
///
/// ```
/// use codirigent_core::compaction::{CompactionConfig, CompactionService};
/// use codirigent_core::SessionId;
///
/// let mut service = CompactionService::new(CompactionConfig::default());
///
/// // Check if compaction should run
/// assert!(service.should_compact(SessionId(1), Some(0.5)));
///
/// // Begin compaction (re-entrancy guard)
/// assert!(service.begin_compaction(SessionId(1)));
/// assert!(!service.begin_compaction(SessionId(1))); // already compacting
///
/// // End compaction
/// service.end_compaction(SessionId(1));
/// assert!(!service.is_compacting(SessionId(1)));
/// ```
pub struct CompactionService {
    config: CompactionConfig,
    compacting_sessions: HashSet<SessionId>,
}

impl CompactionService {
    /// Create a new compaction service with the given configuration.
    pub fn new(config: CompactionConfig) -> Self {
        Self {
            config,
            compacting_sessions: HashSet::new(),
        }
    }

    /// Check whether compaction should be triggered for a session.
    ///
    /// Returns `true` if all conditions are met:
    /// - Compaction is enabled in config
    /// - Context usage is known and above the minimum threshold
    /// - The session is not already being compacted
    pub fn should_compact(&self, session_id: SessionId, context_usage: Option<f32>) -> bool {
        if !self.config.enabled {
            return false;
        }

        if self.compacting_sessions.contains(&session_id) {
            return false;
        }

        match context_usage {
            Some(usage) => usage >= self.config.min_context_threshold,
            None => false, // Conservative: skip if unknown
        }
    }

    /// Mark a session as compacting. Returns `false` if already compacting
    /// (re-entrancy guard).
    pub fn begin_compaction(&mut self, session_id: SessionId) -> bool {
        self.compacting_sessions.insert(session_id)
    }

    /// Clear the compacting flag for a session.
    pub fn end_compaction(&mut self, session_id: SessionId) {
        self.compacting_sessions.remove(&session_id);
    }

    /// Check if a session is currently being compacted.
    pub fn is_compacting(&self, session_id: SessionId) -> bool {
        self.compacting_sessions.contains(&session_id)
    }

    /// Build the `/compact` command string to send via PTY stdin.
    ///
    /// Returns `/compact\n` or `/compact <focus>\n` if focus instructions
    /// are configured.
    pub fn compact_command(&self) -> String {
        match &self.config.focus_instructions {
            Some(focus) => format!("/compact {}\n", focus),
            None => "/compact\n".to_string(),
        }
    }

    /// Get the compaction timeout in seconds.
    pub fn timeout_secs(&self) -> u64 {
        self.config.timeout_secs
    }

    /// Get a reference to the configuration.
    pub fn config(&self) -> &CompactionConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compaction_config_default() {
        let config = CompactionConfig::default();
        assert!(config.enabled);
        assert!((config.min_context_threshold - 0.3).abs() < f32::EPSILON);
        assert!(config.focus_instructions.is_none());
        assert_eq!(config.timeout_secs, 120);
    }

    #[test]
    fn test_compaction_config_serialization() {
        let config = CompactionConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: CompactionConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.enabled, config.enabled);
        assert!((parsed.min_context_threshold - config.min_context_threshold).abs() < f32::EPSILON);
        assert_eq!(parsed.timeout_secs, config.timeout_secs);
    }

    #[test]
    fn test_compaction_config_with_focus() {
        let config = CompactionConfig {
            focus_instructions: Some("Focus on implementation and test requirements".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: CompactionConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed.focus_instructions,
            Some("Focus on implementation and test requirements".to_string())
        );
    }

    #[test]
    fn test_should_compact_enabled_above_threshold() {
        let service = CompactionService::new(CompactionConfig::default());
        assert!(service.should_compact(SessionId(1), Some(0.5)));
    }

    #[test]
    fn test_should_compact_enabled_at_threshold() {
        let service = CompactionService::new(CompactionConfig::default());
        assert!(service.should_compact(SessionId(1), Some(0.3)));
    }

    #[test]
    fn test_should_compact_below_threshold() {
        let service = CompactionService::new(CompactionConfig::default());
        assert!(!service.should_compact(SessionId(1), Some(0.1)));
    }

    #[test]
    fn test_should_compact_disabled() {
        let service = CompactionService::new(CompactionConfig {
            enabled: false,
            ..Default::default()
        });
        assert!(!service.should_compact(SessionId(1), Some(0.9)));
    }

    #[test]
    fn test_should_compact_unknown_context() {
        let service = CompactionService::new(CompactionConfig::default());
        assert!(!service.should_compact(SessionId(1), None));
    }

    #[test]
    fn test_should_compact_already_compacting() {
        let mut service = CompactionService::new(CompactionConfig::default());
        service.begin_compaction(SessionId(1));
        assert!(!service.should_compact(SessionId(1), Some(0.9)));
    }

    #[test]
    fn test_begin_compaction_reentrancy_guard() {
        let mut service = CompactionService::new(CompactionConfig::default());
        assert!(service.begin_compaction(SessionId(1))); // first time: true
        assert!(!service.begin_compaction(SessionId(1))); // second time: false
    }

    #[test]
    fn test_end_compaction() {
        let mut service = CompactionService::new(CompactionConfig::default());
        service.begin_compaction(SessionId(1));
        assert!(service.is_compacting(SessionId(1)));

        service.end_compaction(SessionId(1));
        assert!(!service.is_compacting(SessionId(1)));
    }

    #[test]
    fn test_end_compaction_not_compacting() {
        let mut service = CompactionService::new(CompactionConfig::default());
        // Should not panic
        service.end_compaction(SessionId(1));
        assert!(!service.is_compacting(SessionId(1)));
    }

    #[test]
    fn test_is_compacting() {
        let mut service = CompactionService::new(CompactionConfig::default());
        assert!(!service.is_compacting(SessionId(1)));
        service.begin_compaction(SessionId(1));
        assert!(service.is_compacting(SessionId(1)));
        assert!(!service.is_compacting(SessionId(2))); // different session
    }

    #[test]
    fn test_compact_command_no_focus() {
        let service = CompactionService::new(CompactionConfig::default());
        assert_eq!(service.compact_command(), "/compact\n");
    }

    #[test]
    fn test_compact_command_with_focus() {
        let service = CompactionService::new(CompactionConfig {
            focus_instructions: Some("Focus on implementation and test requirements".to_string()),
            ..Default::default()
        });
        assert_eq!(
            service.compact_command(),
            "/compact Focus on implementation and test requirements\n"
        );
    }

    #[test]
    fn test_timeout_secs() {
        let service = CompactionService::new(CompactionConfig {
            timeout_secs: 60,
            ..Default::default()
        });
        assert_eq!(service.timeout_secs(), 60);
    }

    #[test]
    fn test_multiple_sessions_independent() {
        let mut service = CompactionService::new(CompactionConfig::default());
        service.begin_compaction(SessionId(1));
        service.begin_compaction(SessionId(2));

        assert!(service.is_compacting(SessionId(1)));
        assert!(service.is_compacting(SessionId(2)));

        service.end_compaction(SessionId(1));
        assert!(!service.is_compacting(SessionId(1)));
        assert!(service.is_compacting(SessionId(2)));
    }

    #[test]
    fn test_config_accessor() {
        let config = CompactionConfig {
            enabled: false,
            min_context_threshold: 0.5,
            focus_instructions: Some("test".to_string()),
            timeout_secs: 60,
        };
        let service = CompactionService::new(config);
        assert!(!service.config().enabled);
        assert!((service.config().min_context_threshold - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_compaction_lifecycle() {
        let mut service = CompactionService::new(CompactionConfig::default());
        let sid = SessionId(1);

        // Initially: should compact if above threshold
        assert!(service.should_compact(sid, Some(0.6)));

        // Begin compaction
        assert!(service.begin_compaction(sid));

        // During compaction: should_compact returns false
        assert!(!service.should_compact(sid, Some(0.6)));
        assert!(service.is_compacting(sid));

        // End compaction
        service.end_compaction(sid);

        // After compaction: should_compact works again
        assert!(service.should_compact(sid, Some(0.6)));
        assert!(!service.is_compacting(sid));
    }
}
