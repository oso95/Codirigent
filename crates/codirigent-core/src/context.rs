//! Context tracking for AI CLI sessions.
//!
//! This module provides context window tracking for AI CLI sessions,
//! including usage estimation, threshold detection, and pattern-based
//! detection from CLI output.
//!
//! ## Overview
//!
//! Context tracking helps monitor how much of the AI's context window
//! is being used. When context approaches certain thresholds (e.g., 70%
//! warning, 90% critical), events are emitted to alert the user.
//!
//! ## MCP Overhead
//!
//! When using Model Context Protocol (MCP) servers, some context is
//! reserved for tool definitions. The [`ContextConfig::mcp_overhead`]
//! setting adjusts the effective context calculation to account for this.
//!
//! ## Example
//!
//! ```
//! use codirigent_core::context::{ContextConfig, ContextTracker};
//! use codirigent_core::SessionId;
//!
//! let config = ContextConfig::default();
//! let mut tracker = ContextTracker::new(config);
//!
//! // Update context usage for a session
//! let event = tracker.update_usage(SessionId(1), 0.5);
//! assert!(event.is_none()); // No threshold crossed
//!
//! // Detect context from CLI output
//! tracker.detect_from_output(SessionId(2), "Context: 75%");
//! let usage = tracker.get_usage(SessionId(2));
//! assert!(usage.is_some());
//! ```

use crate::events::CodirigentEvent;
use crate::types::{ContextThresholdState, SessionId};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Get the compiled ANSI stripping regex (compiled once, reused).
///
/// Covers CSI sequences (`\x1b[...X`), OSC sequences (`\x1b]...BEL`),
/// character set sequences (`\x1b(X`), and simple escape codes (`\x1bX`).
fn ansi_strip_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\x1b\[[0-9;?]*[a-zA-Z~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[()#][A-Z0-9]|\x1b[a-zA-Z]")
            .expect("ANSI strip regex must compile")
    })
}

/// Strip ANSI escape sequences from terminal output, returning plain text.
pub fn strip_ansi_codes(text: &str) -> String {
    ansi_strip_regex().replace_all(text, "").to_string()
}

/// Context tracking configuration.
///
/// Defines thresholds for context usage warnings and MCP overhead
/// adjustments.
///
/// # Example
///
/// ```
/// use codirigent_core::context::ContextConfig;
///
/// let config = ContextConfig::default();
/// assert!((config.warning_threshold - 0.7).abs() < f32::EPSILON);
/// assert!((config.critical_threshold - 0.9).abs() < f32::EPSILON);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    /// Warning threshold (0.0-1.0). Default: 0.7 (70%).
    ///
    /// When effective context usage reaches this level, a warning
    /// event is emitted.
    pub warning_threshold: f32,

    /// Critical threshold (0.0-1.0). Default: 0.9 (90%).
    ///
    /// When effective context usage reaches this level, a critical
    /// event is emitted.
    pub critical_threshold: f32,

    /// MCP overhead as percentage (0.0-1.0). Default: 0.0.
    ///
    /// Represents the portion of context reserved for MCP tool
    /// definitions. Effective context is calculated as:
    /// `raw_usage / (1.0 - mcp_overhead)`.
    pub mcp_overhead: f32,

    /// Whether to show effective context (after MCP overhead).
    ///
    /// If true, UI should display the effective usage. If false,
    /// display the raw usage.
    pub show_effective_context: bool,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            warning_threshold: 0.7,
            critical_threshold: 0.9,
            mcp_overhead: 0.0,
            show_effective_context: true,
        }
    }
}

/// Context usage data for a session.
///
/// Contains both raw and effective usage values, along with the
/// current threshold state.
///
/// # Example
///
/// ```
/// use codirigent_core::context::ContextUsage;
/// use codirigent_core::{ContextThresholdState, SessionId};
///
/// let usage = ContextUsage {
///     session_id: SessionId(1),
///     raw_usage: 0.5,
///     mcp_overhead: 0.0,
///     effective_usage: 0.5,
///     threshold_state: ContextThresholdState::Normal,
///     updated_at: chrono::Utc::now(),
/// };
/// assert!((usage.raw_usage - 0.5).abs() < f32::EPSILON);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextUsage {
    /// Session ID.
    pub session_id: SessionId,

    /// Raw context usage (0.0-1.0).
    pub raw_usage: f32,

    /// MCP overhead applied (0.0-1.0).
    pub mcp_overhead: f32,

    /// Effective context usage after MCP overhead.
    pub effective_usage: f32,

    /// Current threshold state.
    pub threshold_state: ContextThresholdState,

    /// When the usage was last updated.
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Pattern for detecting context usage in CLI output.
///
/// Patterns use regular expressions to extract context percentage
/// values from CLI output text.
///
/// # Example
///
/// ```
/// use codirigent_core::context::ContextPattern;
///
/// let pattern = ContextPattern {
///     pattern: r"Context:\s*(\d+(?:\.\d+)?)\s*%".to_string(),
///     capture_group: 1,
///     cli_type: Some("claude".to_string()),
///     inverted: false,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPattern {
    /// Regex pattern to match.
    pub pattern: String,

    /// Capture group index for the percentage value (1-based).
    pub capture_group: usize,

    /// CLI type this pattern applies to (None = all CLIs).
    pub cli_type: Option<String>,

    /// If true, the matched percentage means "remaining" and should be
    /// inverted (usage = 1.0 - matched). E.g. "100% context left" → 0% used.
    #[serde(default)]
    pub inverted: bool,
}

/// Compiled pattern with cached regex.
struct CompiledPattern {
    /// Original pattern definition.
    pattern: ContextPattern,
    /// Compiled regex.
    regex: Regex,
}

/// Context tracker manages context usage for all sessions.
///
/// Tracks context window usage across multiple sessions, detects
/// usage from CLI output, and emits events when thresholds are crossed.
///
/// # Example
///
/// ```
/// use codirigent_core::context::{ContextConfig, ContextTracker};
/// use codirigent_core::{ContextThresholdState, SessionId};
///
/// let mut tracker = ContextTracker::new(ContextConfig::default());
///
/// // Update usage
/// tracker.update_usage(SessionId(1), 0.5);
///
/// // Check state
/// let usage = tracker.get_usage(SessionId(1)).unwrap();
/// assert_eq!(usage.threshold_state, ContextThresholdState::Normal);
///
/// // Detect from output
/// tracker.detect_from_output(SessionId(2), "Context: 80%");
/// let usage = tracker.get_usage(SessionId(2)).unwrap();
/// assert_eq!(usage.threshold_state, ContextThresholdState::Warning);
/// ```
pub struct ContextTracker {
    /// Configuration.
    config: ContextConfig,

    /// Context usage by session.
    usage: HashMap<SessionId, ContextUsage>,

    /// Detection patterns (uncompiled).
    patterns: Vec<ContextPattern>,

    /// Compiled patterns (cached on first use).
    compiled_patterns: Vec<CompiledPattern>,
}

impl ContextTracker {
    /// Create a new context tracker with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The context tracking configuration
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::context::{ContextConfig, ContextTracker};
    ///
    /// let tracker = ContextTracker::new(ContextConfig::default());
    /// ```
    pub fn new(config: ContextConfig) -> Self {
        let patterns = Self::default_patterns();
        let compiled_patterns = Self::compile_patterns(&patterns);

        Self {
            config,
            usage: HashMap::new(),
            patterns,
            compiled_patterns,
        }
    }

    /// Default patterns for common CLIs.
    fn default_patterns() -> Vec<ContextPattern> {
        vec![
            // "X% context left" / "X% context remaining" (Codex, Claude Code)
            // The percentage is how much remains, so invert to get usage.
            ContextPattern {
                pattern: r"(?i)(\d+(?:\.\d+)?)\s*%\s*context\s*(?:left|remaining)".to_string(),
                capture_group: 1,
                cli_type: None,
                inverted: true,
            },
            // "X% context" without "left"/"remaining" — treat as remaining too
            ContextPattern {
                pattern: r"(?i)(\d+(?:\.\d+)?)\s*%\s*context".to_string(),
                capture_group: 1,
                cli_type: None,
                inverted: true,
            },
            // "Context: 65%" or "Context 65%" (label before percentage = usage)
            ContextPattern {
                pattern: r"(?i)Context:?\s*(\d+(?:\.\d+)?)\s*%".to_string(),
                capture_group: 1,
                cli_type: None,
                inverted: false,
            },
            // Generic: "context" anywhere before "X%" (e.g. "context window at 42%")
            ContextPattern {
                pattern: r"(?i)context.*?(\d+(?:\.\d+)?)\s*%".to_string(),
                capture_group: 1,
                cli_type: None,
                inverted: false,
            },
            // Tokens ratio: "X/Y tokens" or "tokens: X/Y"
            ContextPattern {
                pattern: r"(\d+)\s*/\s*(\d+)\s*tokens".to_string(),
                capture_group: 0, // Special: use group 0 to signal ratio calculation
                cli_type: None,
                inverted: false,
            },
        ]
    }

    /// Compile patterns into cached regexes.
    fn compile_patterns(patterns: &[ContextPattern]) -> Vec<CompiledPattern> {
        patterns
            .iter()
            .filter_map(|p| {
                Regex::new(&p.pattern).ok().map(|regex| CompiledPattern {
                    pattern: p.clone(),
                    regex,
                })
            })
            .collect()
    }

    /// Calculate effective usage considering MCP overhead.
    ///
    /// If MCP overhead is 20% and raw usage is 50%, effective usage is:
    /// 50% / (100% - 20%) = 50% / 80% = 62.5%
    fn calculate_effective_usage(&self, raw_usage: f32) -> f32 {
        let available = 1.0 - self.config.mcp_overhead;
        if available > 0.0 {
            (raw_usage / available).min(1.0)
        } else {
            1.0
        }
    }

    /// Determine threshold state from effective usage.
    fn determine_threshold_state(&self, effective_usage: f32) -> ContextThresholdState {
        if effective_usage >= self.config.critical_threshold {
            ContextThresholdState::Critical
        } else if effective_usage >= self.config.warning_threshold {
            ContextThresholdState::Warning
        } else {
            ContextThresholdState::Normal
        }
    }

    /// Update context usage for a session.
    ///
    /// Returns an event if the threshold state changed (worsened).
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session to update
    /// * `raw_usage` - The raw context usage (0.0-1.0)
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::context::{ContextConfig, ContextTracker};
    /// use codirigent_core::SessionId;
    ///
    /// let mut tracker = ContextTracker::new(ContextConfig::default());
    ///
    /// // Normal usage, no event
    /// let event = tracker.update_usage(SessionId(1), 0.5);
    /// assert!(event.is_none());
    ///
    /// // Cross warning threshold
    /// let event = tracker.update_usage(SessionId(1), 0.75);
    /// assert!(event.is_some());
    /// ```
    pub fn update_usage(
        &mut self,
        session_id: SessionId,
        raw_usage: f32,
    ) -> Option<CodirigentEvent> {
        let effective_usage = self.calculate_effective_usage(raw_usage);
        let new_state = self.determine_threshold_state(effective_usage);

        let old_state = self
            .usage
            .get(&session_id)
            .map(|u| u.threshold_state)
            .unwrap_or(ContextThresholdState::Normal);

        let usage = ContextUsage {
            session_id,
            raw_usage,
            mcp_overhead: self.config.mcp_overhead,
            effective_usage,
            threshold_state: new_state,
            updated_at: chrono::Utc::now(),
        };

        self.usage.insert(session_id, usage);

        // Emit threshold event if state worsened
        if new_state != old_state {
            match new_state {
                ContextThresholdState::Warning => Some(CodirigentEvent::ContextThresholdReached {
                    session_id,
                    threshold: self.config.warning_threshold,
                    state: new_state,
                }),
                ContextThresholdState::Critical => Some(CodirigentEvent::ContextThresholdReached {
                    session_id,
                    threshold: self.config.critical_threshold,
                    state: new_state,
                }),
                ContextThresholdState::Normal => None,
            }
        } else {
            None
        }
    }

    /// Get context usage for a session.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session to query
    ///
    /// # Returns
    ///
    /// The context usage data, if available.
    pub fn get_usage(&self, session_id: SessionId) -> Option<&ContextUsage> {
        self.usage.get(&session_id)
    }

    /// Try to detect context usage from output text.
    ///
    /// Scans the output for patterns that indicate context usage
    /// and updates the session's tracked usage if found.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session that produced the output
    /// * `output` - The output text to scan
    ///
    /// # Returns
    ///
    /// An event if a threshold was crossed.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::context::{ContextConfig, ContextTracker};
    /// use codirigent_core::SessionId;
    ///
    /// let mut tracker = ContextTracker::new(ContextConfig::default());
    ///
    /// // Detect from Claude Code format
    /// tracker.detect_from_output(SessionId(1), "Context: 65%");
    /// let usage = tracker.get_usage(SessionId(1)).unwrap();
    /// assert!((usage.raw_usage - 0.65).abs() < 0.01);
    /// ```
    pub fn detect_from_output(
        &mut self,
        session_id: SessionId,
        output: &str,
    ) -> Option<CodirigentEvent> {
        // Strip ANSI escape sequences so regex patterns can match plain text
        let cleaned = strip_ansi_codes(output);
        for compiled in &self.compiled_patterns {
            if let Some(captures) = compiled.regex.captures(&cleaned) {
                // Special case for ratio patterns (e.g., "1000/2000 tokens")
                if compiled.pattern.capture_group == 0 && captures.len() >= 3 {
                    if let (Some(used), Some(total)) = (captures.get(1), captures.get(2)) {
                        if let (Ok(used_val), Ok(total_val)) =
                            (used.as_str().parse::<f32>(), total.as_str().parse::<f32>())
                        {
                            if total_val > 0.0 {
                                let ratio = used_val / total_val;
                                return self.update_usage(session_id, ratio);
                            }
                        }
                    }
                    continue;
                }

                // Standard percentage pattern
                if let Some(matched) = captures.get(compiled.pattern.capture_group) {
                    if let Ok(percentage) = matched.as_str().parse::<f32>() {
                        // Convert percentage (0-100) to ratio (0.0-1.0) if needed
                        let mut ratio = if percentage > 1.0 {
                            percentage / 100.0
                        } else {
                            percentage
                        };
                        // Invert if the pattern measures "remaining" not "used"
                        if compiled.pattern.inverted {
                            ratio = (1.0 - ratio).max(0.0);
                        }
                        return self.update_usage(session_id, ratio);
                    }
                }
            }
        }
        None
    }

    /// Add a custom detection pattern.
    ///
    /// # Arguments
    ///
    /// * `pattern` - The pattern to add
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::context::{ContextConfig, ContextTracker, ContextPattern};
    ///
    /// let mut tracker = ContextTracker::new(ContextConfig::default());
    /// tracker.add_pattern(ContextPattern {
    ///     pattern: r"usage:\s*(\d+)%".to_string(),
    ///     capture_group: 1,
    ///     cli_type: None,
    ///     inverted: false,
    /// });
    /// ```
    pub fn add_pattern(&mut self, pattern: ContextPattern) {
        // Compile and add to cached patterns
        if let Ok(regex) = Regex::new(&pattern.pattern) {
            self.compiled_patterns.push(CompiledPattern {
                pattern: pattern.clone(),
                regex,
            });
        }
        self.patterns.push(pattern);
    }

    /// Get all registered patterns.
    pub fn patterns(&self) -> &[ContextPattern] {
        &self.patterns
    }

    /// Clear all patterns (including defaults).
    pub fn clear_patterns(&mut self) {
        self.patterns.clear();
        self.compiled_patterns.clear();
    }

    /// Remove a session's tracking data.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session to remove
    pub fn remove_session(&mut self, session_id: SessionId) {
        self.usage.remove(&session_id);
    }

    /// Get all tracked sessions.
    pub fn sessions(&self) -> impl Iterator<Item = SessionId> + '_ {
        self.usage.keys().copied()
    }

    /// Get the current configuration.
    pub fn config(&self) -> &ContextConfig {
        &self.config
    }

    /// Update the configuration.
    ///
    /// Note: This does not retroactively change existing usage data's
    /// threshold states. Call `update_usage` to recalculate.
    pub fn set_config(&mut self, config: ContextConfig) {
        self.config = config;
    }
}

/// Trait for context tracking operations.
///
/// This trait provides a simplified interface for context tracking
/// that can be used as a service abstraction.
pub trait ContextTrackingService: Send + Sync {
    /// Update context usage for a session.
    ///
    /// Returns an event if a threshold was crossed.
    fn update(&mut self, session_id: SessionId, usage: f32) -> Option<CodirigentEvent>;

    /// Get raw context usage for a session.
    fn get(&self, session_id: SessionId) -> Option<f32>;

    /// Get effective context usage (after MCP overhead).
    fn get_effective(&self, session_id: SessionId) -> Option<f32>;

    /// Get the threshold state for a session.
    fn threshold_state(&self, session_id: SessionId) -> Option<ContextThresholdState>;
}

impl ContextTrackingService for ContextTracker {
    fn update(&mut self, session_id: SessionId, usage: f32) -> Option<CodirigentEvent> {
        self.update_usage(session_id, usage)
    }

    fn get(&self, session_id: SessionId) -> Option<f32> {
        self.get_usage(session_id).map(|u| u.raw_usage)
    }

    fn get_effective(&self, session_id: SessionId) -> Option<f32> {
        self.get_usage(session_id).map(|u| u.effective_usage)
    }

    fn threshold_state(&self, session_id: SessionId) -> Option<ContextThresholdState> {
        self.get_usage(session_id).map(|u| u.threshold_state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ContextConfig tests
    #[test]
    fn test_context_config_default() {
        let config = ContextConfig::default();
        assert!((config.warning_threshold - 0.7).abs() < f32::EPSILON);
        assert!((config.critical_threshold - 0.9).abs() < f32::EPSILON);
        assert!((config.mcp_overhead - 0.0).abs() < f32::EPSILON);
        assert!(config.show_effective_context);
    }

    #[test]
    fn test_context_config_serialization() {
        let config = ContextConfig {
            warning_threshold: 0.8,
            critical_threshold: 0.95,
            mcp_overhead: 0.1,
            show_effective_context: false,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ContextConfig = serde_json::from_str(&json).unwrap();
        assert!((parsed.warning_threshold - 0.8).abs() < f32::EPSILON);
        assert!((parsed.critical_threshold - 0.95).abs() < f32::EPSILON);
        assert!((parsed.mcp_overhead - 0.1).abs() < f32::EPSILON);
        assert!(!parsed.show_effective_context);
    }

    #[test]
    fn test_context_config_clone() {
        let config = ContextConfig::default();
        let cloned = config.clone();
        assert!((config.warning_threshold - cloned.warning_threshold).abs() < f32::EPSILON);
    }

    #[test]
    fn test_context_config_debug() {
        let config = ContextConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("ContextConfig"));
        assert!(debug_str.contains("warning_threshold"));
    }

    // ContextThresholdState tests
    #[test]
    fn test_threshold_state_default() {
        assert_eq!(
            ContextThresholdState::default(),
            ContextThresholdState::Normal
        );
    }

    #[test]
    fn test_threshold_state_equality() {
        assert_eq!(ContextThresholdState::Normal, ContextThresholdState::Normal);
        assert_ne!(
            ContextThresholdState::Normal,
            ContextThresholdState::Warning
        );
        assert_ne!(
            ContextThresholdState::Warning,
            ContextThresholdState::Critical
        );
    }

    #[test]
    fn test_threshold_state_serialization() {
        let states = [
            ContextThresholdState::Normal,
            ContextThresholdState::Warning,
            ContextThresholdState::Critical,
        ];
        for state in states {
            let json = serde_json::to_string(&state).unwrap();
            let parsed: ContextThresholdState = serde_json::from_str(&json).unwrap();
            assert_eq!(state, parsed);
        }
    }

    #[test]
    fn test_threshold_state_clone_copy() {
        let state = ContextThresholdState::Warning;
        let cloned = state;
        assert_eq!(state, cloned);
    }

    // ContextUsage tests
    #[test]
    fn test_context_usage_creation() {
        let usage = ContextUsage {
            session_id: SessionId(1),
            raw_usage: 0.5,
            mcp_overhead: 0.1,
            effective_usage: 0.556,
            threshold_state: ContextThresholdState::Normal,
            updated_at: chrono::Utc::now(),
        };
        assert_eq!(usage.session_id, SessionId(1));
        assert!((usage.raw_usage - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_context_usage_serialization() {
        let usage = ContextUsage {
            session_id: SessionId(42),
            raw_usage: 0.75,
            mcp_overhead: 0.0,
            effective_usage: 0.75,
            threshold_state: ContextThresholdState::Warning,
            updated_at: chrono::Utc::now(),
        };
        let json = serde_json::to_string(&usage).unwrap();
        let parsed: ContextUsage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.session_id, SessionId(42));
        assert!((parsed.raw_usage - 0.75).abs() < f32::EPSILON);
        assert_eq!(parsed.threshold_state, ContextThresholdState::Warning);
    }

    #[test]
    fn test_context_usage_clone() {
        let usage = ContextUsage {
            session_id: SessionId(1),
            raw_usage: 0.5,
            mcp_overhead: 0.0,
            effective_usage: 0.5,
            threshold_state: ContextThresholdState::Normal,
            updated_at: chrono::Utc::now(),
        };
        let cloned = usage.clone();
        assert_eq!(usage.session_id, cloned.session_id);
        assert!((usage.raw_usage - cloned.raw_usage).abs() < f32::EPSILON);
    }

    // ContextPattern tests
    #[test]
    fn test_context_pattern_creation() {
        let pattern = ContextPattern {
            pattern: r"Context:\s*(\d+)%".to_string(),
            capture_group: 1,
            cli_type: Some("claude".to_string()),
            inverted: false,
        };
        assert_eq!(pattern.capture_group, 1);
        assert_eq!(pattern.cli_type, Some("claude".to_string()));
        assert!(!pattern.inverted);
    }

    #[test]
    fn test_context_pattern_serialization() {
        let pattern = ContextPattern {
            pattern: r"test".to_string(),
            capture_group: 1,
            cli_type: None,
            inverted: true,
        };
        let json = serde_json::to_string(&pattern).unwrap();
        let parsed: ContextPattern = serde_json::from_str(&json).unwrap();
        assert_eq!(pattern.pattern, parsed.pattern);
        assert_eq!(pattern.capture_group, parsed.capture_group);
        assert_eq!(pattern.cli_type, parsed.cli_type);
        assert!(parsed.inverted);
    }

    #[test]
    fn test_context_pattern_clone() {
        let pattern = ContextPattern {
            pattern: r"test".to_string(),
            capture_group: 1,
            cli_type: Some("test".to_string()),
            inverted: false,
        };
        let cloned = pattern.clone();
        assert_eq!(pattern.pattern, cloned.pattern);
    }

    // ContextTracker tests
    #[test]
    fn test_tracker_new() {
        let tracker = ContextTracker::new(ContextConfig::default());
        assert!(!tracker.patterns().is_empty()); // Has default patterns
    }

    #[test]
    fn test_calculate_effective_usage_no_overhead() {
        let tracker = ContextTracker::new(ContextConfig::default());
        let effective = tracker.calculate_effective_usage(0.5);
        assert!((effective - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_calculate_effective_usage_with_mcp() {
        let config = ContextConfig {
            mcp_overhead: 0.2, // 20% MCP overhead
            ..Default::default()
        };
        let tracker = ContextTracker::new(config);

        // 50% raw with 20% MCP = 50/80 = 62.5%
        let effective = tracker.calculate_effective_usage(0.5);
        assert!((effective - 0.625).abs() < 0.01);
    }

    #[test]
    fn test_calculate_effective_usage_capped_at_one() {
        let config = ContextConfig {
            mcp_overhead: 0.5,
            ..Default::default()
        };
        let tracker = ContextTracker::new(config);

        // High raw usage with high overhead should cap at 1.0
        let effective = tracker.calculate_effective_usage(0.9);
        assert!((effective - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_calculate_effective_usage_full_overhead() {
        let config = ContextConfig {
            mcp_overhead: 1.0, // 100% overhead (edge case)
            ..Default::default()
        };
        let tracker = ContextTracker::new(config);

        let effective = tracker.calculate_effective_usage(0.5);
        assert!((effective - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_determine_threshold_state_normal() {
        let tracker = ContextTracker::new(ContextConfig::default());
        assert_eq!(
            tracker.determine_threshold_state(0.5),
            ContextThresholdState::Normal
        );
        assert_eq!(
            tracker.determine_threshold_state(0.69),
            ContextThresholdState::Normal
        );
    }

    #[test]
    fn test_determine_threshold_state_warning() {
        let tracker = ContextTracker::new(ContextConfig::default());
        assert_eq!(
            tracker.determine_threshold_state(0.7),
            ContextThresholdState::Warning
        );
        assert_eq!(
            tracker.determine_threshold_state(0.75),
            ContextThresholdState::Warning
        );
        assert_eq!(
            tracker.determine_threshold_state(0.89),
            ContextThresholdState::Warning
        );
    }

    #[test]
    fn test_determine_threshold_state_critical() {
        let tracker = ContextTracker::new(ContextConfig::default());
        assert_eq!(
            tracker.determine_threshold_state(0.9),
            ContextThresholdState::Critical
        );
        assert_eq!(
            tracker.determine_threshold_state(0.95),
            ContextThresholdState::Critical
        );
        assert_eq!(
            tracker.determine_threshold_state(1.0),
            ContextThresholdState::Critical
        );
    }

    #[test]
    fn test_update_usage_normal() {
        let mut tracker = ContextTracker::new(ContextConfig::default());
        let event = tracker.update_usage(SessionId(1), 0.5);
        assert!(event.is_none()); // No threshold crossed

        let usage = tracker.get_usage(SessionId(1)).unwrap();
        assert!((usage.raw_usage - 0.5).abs() < f32::EPSILON);
        assert_eq!(usage.threshold_state, ContextThresholdState::Normal);
    }

    #[test]
    fn test_update_usage_triggers_warning() {
        let mut tracker = ContextTracker::new(ContextConfig::default());

        // Start normal
        tracker.update_usage(SessionId(1), 0.5);

        // Cross warning threshold
        let event = tracker.update_usage(SessionId(1), 0.75);
        assert!(event.is_some());

        if let Some(CodirigentEvent::ContextThresholdReached {
            session_id,
            threshold,
            state,
        }) = event
        {
            assert_eq!(session_id, SessionId(1));
            assert!((threshold - 0.7).abs() < f32::EPSILON);
            assert_eq!(state, ContextThresholdState::Warning);
        } else {
            panic!("Expected ContextThresholdReached event");
        }
    }

    #[test]
    fn test_update_usage_triggers_critical() {
        let mut tracker = ContextTracker::new(ContextConfig::default());

        // Jump directly to critical
        let event = tracker.update_usage(SessionId(1), 0.95);
        assert!(event.is_some());

        let usage = tracker.get_usage(SessionId(1)).unwrap();
        assert_eq!(usage.threshold_state, ContextThresholdState::Critical);
    }

    #[test]
    fn test_update_usage_no_event_on_same_state() {
        let mut tracker = ContextTracker::new(ContextConfig::default());

        // First update to warning
        let event = tracker.update_usage(SessionId(1), 0.75);
        assert!(event.is_some());

        // Second update still in warning - no event
        let event = tracker.update_usage(SessionId(1), 0.80);
        assert!(event.is_none());

        let usage = tracker.get_usage(SessionId(1)).unwrap();
        assert_eq!(usage.threshold_state, ContextThresholdState::Warning);
    }

    #[test]
    fn test_update_usage_no_event_on_improvement() {
        let mut tracker = ContextTracker::new(ContextConfig::default());

        // Start at warning
        tracker.update_usage(SessionId(1), 0.75);

        // Go back to normal - no event (improvement)
        let event = tracker.update_usage(SessionId(1), 0.5);
        assert!(event.is_none());

        let usage = tracker.get_usage(SessionId(1)).unwrap();
        assert_eq!(usage.threshold_state, ContextThresholdState::Normal);
    }

    #[test]
    fn test_detect_from_output_claude_format() {
        let mut tracker = ContextTracker::new(ContextConfig::default());

        let output = "Some text\nContext: 65%\nMore text";
        tracker.detect_from_output(SessionId(1), output);

        let usage = tracker.get_usage(SessionId(1)).unwrap();
        assert!((usage.raw_usage - 0.65).abs() < 0.01);
    }

    #[test]
    fn test_detect_from_output_claude_format_decimal() {
        let mut tracker = ContextTracker::new(ContextConfig::default());

        let output = "Context: 65.5%";
        tracker.detect_from_output(SessionId(1), output);

        let usage = tracker.get_usage(SessionId(1)).unwrap();
        assert!((usage.raw_usage - 0.655).abs() < 0.01);
    }

    #[test]
    fn test_detect_from_output_case_insensitive() {
        let mut tracker = ContextTracker::new(ContextConfig::default());

        // Lowercase
        tracker.detect_from_output(SessionId(1), "context: 50%");
        let usage = tracker.get_usage(SessionId(1)).unwrap();
        assert!((usage.raw_usage - 0.50).abs() < 0.01);

        // Mixed case
        tracker.detect_from_output(SessionId(2), "CONTEXT: 60%");
        let usage = tracker.get_usage(SessionId(2)).unwrap();
        assert!((usage.raw_usage - 0.60).abs() < 0.01);
    }

    #[test]
    fn test_detect_from_output_generic() {
        let mut tracker = ContextTracker::new(ContextConfig::default());

        let output = "context window at 42% capacity";
        tracker.detect_from_output(SessionId(1), output);

        let usage = tracker.get_usage(SessionId(1)).unwrap();
        assert!((usage.raw_usage - 0.42).abs() < 0.01);
    }

    #[test]
    fn test_detect_from_output_tokens_ratio() {
        let mut tracker = ContextTracker::new(ContextConfig::default());

        let output = "1000/2000 tokens used";
        tracker.detect_from_output(SessionId(1), output);

        let usage = tracker.get_usage(SessionId(1)).unwrap();
        assert!((usage.raw_usage - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_detect_from_output_no_match() {
        let mut tracker = ContextTracker::new(ContextConfig::default());

        let output = "No context info here";
        let event = tracker.detect_from_output(SessionId(1), output);

        assert!(event.is_none());
        assert!(tracker.get_usage(SessionId(1)).is_none());
    }

    #[test]
    fn test_custom_pattern() {
        let mut tracker = ContextTracker::new(ContextConfig::default());

        tracker.add_pattern(ContextPattern {
            pattern: r"usage:\s*(\d+)%".to_string(),
            capture_group: 1,
            cli_type: None,
            inverted: false,
        });

        let output = "usage: 55%";
        tracker.detect_from_output(SessionId(1), output);

        let usage = tracker.get_usage(SessionId(1)).unwrap();
        assert!((usage.raw_usage - 0.55).abs() < 0.01);
    }

    #[test]
    fn test_clear_patterns() {
        let mut tracker = ContextTracker::new(ContextConfig::default());
        assert!(!tracker.patterns().is_empty());

        tracker.clear_patterns();
        assert!(tracker.patterns().is_empty());

        // Default patterns no longer work
        let output = "Context: 65%";
        tracker.detect_from_output(SessionId(1), output);
        assert!(tracker.get_usage(SessionId(1)).is_none());
    }

    #[test]
    fn test_remove_session() {
        let mut tracker = ContextTracker::new(ContextConfig::default());

        tracker.update_usage(SessionId(1), 0.5);
        assert!(tracker.get_usage(SessionId(1)).is_some());

        tracker.remove_session(SessionId(1));
        assert!(tracker.get_usage(SessionId(1)).is_none());
    }

    #[test]
    fn test_remove_nonexistent_session() {
        let mut tracker = ContextTracker::new(ContextConfig::default());
        // Should not panic
        tracker.remove_session(SessionId(999));
    }

    #[test]
    fn test_sessions_iterator() {
        let mut tracker = ContextTracker::new(ContextConfig::default());

        tracker.update_usage(SessionId(1), 0.5);
        tracker.update_usage(SessionId(2), 0.6);
        tracker.update_usage(SessionId(3), 0.7);

        let sessions: Vec<SessionId> = tracker.sessions().collect();
        assert_eq!(sessions.len(), 3);
        assert!(sessions.contains(&SessionId(1)));
        assert!(sessions.contains(&SessionId(2)));
        assert!(sessions.contains(&SessionId(3)));
    }

    #[test]
    fn test_config_getter() {
        let config = ContextConfig {
            warning_threshold: 0.6,
            ..Default::default()
        };
        let tracker = ContextTracker::new(config);

        assert!((tracker.config().warning_threshold - 0.6).abs() < f32::EPSILON);
    }

    #[test]
    fn test_set_config() {
        let mut tracker = ContextTracker::new(ContextConfig::default());

        let new_config = ContextConfig {
            warning_threshold: 0.5,
            ..Default::default()
        };
        tracker.set_config(new_config);

        assert!((tracker.config().warning_threshold - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_invalid_pattern_ignored() {
        let mut tracker = ContextTracker::new(ContextConfig::default());
        let initial_count = tracker.compiled_patterns.len();

        // Invalid regex pattern
        tracker.add_pattern(ContextPattern {
            pattern: r"[invalid".to_string(), // Unclosed bracket
            capture_group: 1,
            cli_type: None,
            inverted: false,
        });

        // Pattern added to patterns list but not compiled
        assert_eq!(tracker.patterns().len(), initial_count + 1);
        // Compiled patterns unchanged (invalid regex ignored)
        assert_eq!(tracker.compiled_patterns.len(), initial_count);
    }

    // ContextTrackingService trait tests
    #[test]
    fn test_service_update() {
        let mut tracker = ContextTracker::new(ContextConfig::default());

        let event = tracker.update(SessionId(1), 0.75);
        assert!(event.is_some());
    }

    #[test]
    fn test_service_get() {
        let mut tracker = ContextTracker::new(ContextConfig::default());
        tracker.update(SessionId(1), 0.5);

        let usage = tracker.get(SessionId(1));
        assert!(usage.is_some());
        assert!((usage.unwrap() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_service_get_effective() {
        let config = ContextConfig {
            mcp_overhead: 0.2,
            ..Default::default()
        };
        let mut tracker = ContextTracker::new(config);
        tracker.update(SessionId(1), 0.5);

        let effective = tracker.get_effective(SessionId(1));
        assert!(effective.is_some());
        assert!((effective.unwrap() - 0.625).abs() < 0.01);
    }

    #[test]
    fn test_service_threshold_state() {
        let mut tracker = ContextTracker::new(ContextConfig::default());
        tracker.update(SessionId(1), 0.75);

        let state = tracker.threshold_state(SessionId(1));
        assert_eq!(state, Some(ContextThresholdState::Warning));
    }

    #[test]
    fn test_service_get_nonexistent() {
        let tracker = ContextTracker::new(ContextConfig::default());
        assert!(tracker.get(SessionId(999)).is_none());
        assert!(tracker.get_effective(SessionId(999)).is_none());
        assert!(tracker.threshold_state(SessionId(999)).is_none());
    }

    // Multiple sessions test
    #[test]
    fn test_multiple_sessions() {
        let mut tracker = ContextTracker::new(ContextConfig::default());

        tracker.update_usage(SessionId(1), 0.5);
        tracker.update_usage(SessionId(2), 0.75);
        tracker.update_usage(SessionId(3), 0.95);

        assert_eq!(
            tracker.get_usage(SessionId(1)).unwrap().threshold_state,
            ContextThresholdState::Normal
        );
        assert_eq!(
            tracker.get_usage(SessionId(2)).unwrap().threshold_state,
            ContextThresholdState::Warning
        );
        assert_eq!(
            tracker.get_usage(SessionId(3)).unwrap().threshold_state,
            ContextThresholdState::Critical
        );
    }

    // Event consistency tests
    #[test]
    fn test_warning_to_critical_event() {
        let mut tracker = ContextTracker::new(ContextConfig::default());

        // Normal to warning
        let event = tracker.update_usage(SessionId(1), 0.75);
        assert!(matches!(
            event,
            Some(CodirigentEvent::ContextThresholdReached {
                state: ContextThresholdState::Warning,
                ..
            })
        ));

        // Warning to critical
        let event = tracker.update_usage(SessionId(1), 0.95);
        assert!(matches!(
            event,
            Some(CodirigentEvent::ContextThresholdReached {
                state: ContextThresholdState::Critical,
                ..
            })
        ));
    }

    #[test]
    fn test_detect_triggers_event() {
        let mut tracker = ContextTracker::new(ContextConfig::default());

        // Start normal
        tracker.update_usage(SessionId(1), 0.5);

        // Detect output that crosses threshold
        let event = tracker.detect_from_output(SessionId(1), "Context: 85%");
        assert!(event.is_some());
    }

    // ANSI stripping tests

    #[test]
    fn test_strip_ansi_codes_plain_text() {
        assert_eq!(strip_ansi_codes("hello world"), "hello world");
    }

    #[test]
    fn test_strip_ansi_codes_sgr_colors() {
        assert_eq!(
            strip_ansi_codes("\x1b[38;5;33mContext: 65%\x1b[0m"),
            "Context: 65%"
        );
    }

    #[test]
    fn test_strip_ansi_codes_24bit_colors() {
        assert_eq!(
            strip_ansi_codes("\x1b[38;2;100;200;50mContext: 72%\x1b[0m"),
            "Context: 72%"
        );
    }

    #[test]
    fn test_strip_ansi_codes_cursor_movement() {
        assert_eq!(
            strip_ansi_codes("\x1b[2J\x1b[HContext: 80%"),
            "Context: 80%"
        );
    }

    #[test]
    fn test_strip_ansi_codes_osc_sequences() {
        assert_eq!(
            strip_ansi_codes("\x1b]0;My Title\x07Context: 50%"),
            "Context: 50%"
        );
    }

    #[test]
    fn test_detect_from_output_with_ansi_codes() {
        let mut tracker = ContextTracker::new(ContextConfig::default());

        // Simulate what Claude Code PTY output looks like
        let output = "\x1b[38;5;33mContext: 65%\x1b[0m  \x1b[38;5;239m│\x1b[0m  2.1M tokens";
        tracker.detect_from_output(SessionId(1), output);

        let usage = tracker.get_usage(SessionId(1)).unwrap();
        assert!((usage.raw_usage - 0.65).abs() < 0.01);
    }

    #[test]
    fn test_detect_from_output_with_complex_ansi() {
        let mut tracker = ContextTracker::new(ContextConfig::default());

        // Heavy ANSI: cursor positioning + 24-bit color + reset
        let output = "\x1b[2J\x1b[H\x1b[38;2;100;100;100mContext: 92%\x1b[0m\n\x1b[38;2;200;50;50m";
        tracker.detect_from_output(SessionId(1), output);

        let usage = tracker.get_usage(SessionId(1)).unwrap();
        assert!((usage.raw_usage - 0.92).abs() < 0.01);
    }

    #[test]
    fn test_detect_tokens_ratio_with_ansi() {
        let mut tracker = ContextTracker::new(ContextConfig::default());

        let output = "\x1b[33m150000\x1b[0m / \x1b[33m200000\x1b[0m tokens";
        tracker.detect_from_output(SessionId(1), output);

        let usage = tracker.get_usage(SessionId(1)).unwrap();
        assert!((usage.raw_usage - 0.75).abs() < 0.01);
    }
}
