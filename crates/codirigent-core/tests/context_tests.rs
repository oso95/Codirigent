//! Context tracking tests.
//!
//! Tests verify context usage tracking, threshold warnings, and pattern detection.

use codirigent_core::context::{ContextConfig, ContextTracker};
use codirigent_core::{ContextThresholdState, SessionId};

/// Test basic context usage update and retrieval.
#[test]
fn test_context_usage_update() {
    let config = ContextConfig::default();
    let mut tracker = ContextTracker::new(config);

    // Update usage for a session
    let event = tracker.update_usage(SessionId(1), 0.5);

    // Should not trigger any events at 50% usage
    assert!(event.is_none());

    // Retrieve usage
    let usage = tracker.get_usage(SessionId(1)).expect("Usage should exist");
    assert!((usage.raw_usage - 0.5).abs() < f32::EPSILON);
    assert_eq!(usage.threshold_state, ContextThresholdState::Normal);
}

/// Test warning threshold detection.
#[test]
fn test_warning_threshold() {
    let config = ContextConfig {
        warning_threshold: 0.7,
        critical_threshold: 0.9,
        ..Default::default()
    };
    let mut tracker = ContextTracker::new(config);

    // Below warning threshold
    let event = tracker.update_usage(SessionId(1), 0.6);
    assert!(event.is_none());

    let usage = tracker.get_usage(SessionId(1)).unwrap();
    assert_eq!(usage.threshold_state, ContextThresholdState::Normal);

    // At warning threshold
    let event = tracker.update_usage(SessionId(1), 0.75);
    assert!(event.is_some());

    let usage = tracker.get_usage(SessionId(1)).unwrap();
    assert_eq!(usage.threshold_state, ContextThresholdState::Warning);
}

/// Test critical threshold detection.
#[test]
fn test_critical_threshold() {
    let config = ContextConfig {
        warning_threshold: 0.7,
        critical_threshold: 0.9,
        ..Default::default()
    };
    let mut tracker = ContextTracker::new(config);

    // At critical threshold
    let event = tracker.update_usage(SessionId(1), 0.95);
    assert!(event.is_some());

    let usage = tracker.get_usage(SessionId(1)).unwrap();
    assert_eq!(usage.threshold_state, ContextThresholdState::Critical);
}

/// Test MCP overhead calculation.
#[test]
fn test_mcp_overhead() {
    let config = ContextConfig {
        warning_threshold: 0.7,
        critical_threshold: 0.9,
        mcp_overhead: 0.2, // 20% overhead
        ..Default::default()
    };
    let mut tracker = ContextTracker::new(config);

    // Raw usage 50% with 20% overhead = 62.5% effective
    tracker.update_usage(SessionId(1), 0.5);

    let usage = tracker.get_usage(SessionId(1)).unwrap();
    assert!((usage.raw_usage - 0.5).abs() < f32::EPSILON);
    assert!((usage.mcp_overhead - 0.2).abs() < f32::EPSILON);

    // Effective = raw / (1 - overhead) = 0.5 / 0.8 = 0.625
    assert!((usage.effective_usage - 0.625).abs() < 0.01);
}

/// Test threshold state transitions.
#[test]
fn test_threshold_state_transitions() {
    let config = ContextConfig {
        warning_threshold: 0.7,
        critical_threshold: 0.9,
        ..Default::default()
    };
    let mut tracker = ContextTracker::new(config);

    // Normal → Warning
    tracker.update_usage(SessionId(1), 0.5);
    assert_eq!(
        tracker.get_usage(SessionId(1)).unwrap().threshold_state,
        ContextThresholdState::Normal
    );

    tracker.update_usage(SessionId(1), 0.75);
    assert_eq!(
        tracker.get_usage(SessionId(1)).unwrap().threshold_state,
        ContextThresholdState::Warning
    );

    // Warning → Critical
    tracker.update_usage(SessionId(1), 0.95);
    assert_eq!(
        tracker.get_usage(SessionId(1)).unwrap().threshold_state,
        ContextThresholdState::Critical
    );

    // Critical → Normal (usage drops)
    tracker.update_usage(SessionId(1), 0.5);
    assert_eq!(
        tracker.get_usage(SessionId(1)).unwrap().threshold_state,
        ContextThresholdState::Normal
    );
}

/// Test pattern detection from CLI output.
#[test]
fn test_context_detection_from_output() {
    let config = ContextConfig::default();
    let mut tracker = ContextTracker::new(config);

    // Test "Context: X%" pattern
    tracker.detect_from_output(SessionId(1), "Context: 75%");

    let usage = tracker.get_usage(SessionId(1));
    assert!(usage.is_some());
    let usage = usage.unwrap();
    assert!((usage.raw_usage - 0.75).abs() < 0.01);
}

/// Test inverted pattern detection (remaining context).
#[test]
fn test_inverted_context_detection() {
    let config = ContextConfig::default();
    let mut tracker = ContextTracker::new(config);

    // "X% context left" should be inverted (100% - X%)
    tracker.detect_from_output(SessionId(1), "80% context remaining");

    let usage = tracker.get_usage(SessionId(1));
    assert!(usage.is_some());
    let usage = usage.unwrap();
    // 80% remaining = 20% used
    assert!((usage.raw_usage - 0.2).abs() < 0.01);
}

/// Test multiple sessions tracked independently.
#[test]
fn test_multiple_sessions() {
    let config = ContextConfig::default();
    let mut tracker = ContextTracker::new(config);

    tracker.update_usage(SessionId(1), 0.5);
    tracker.update_usage(SessionId(2), 0.8);
    tracker.update_usage(SessionId(3), 0.3);

    let usage1 = tracker.get_usage(SessionId(1)).unwrap();
    let usage2 = tracker.get_usage(SessionId(2)).unwrap();
    let usage3 = tracker.get_usage(SessionId(3)).unwrap();

    assert!((usage1.raw_usage - 0.5).abs() < f32::EPSILON);
    assert!((usage2.raw_usage - 0.8).abs() < f32::EPSILON);
    assert!((usage3.raw_usage - 0.3).abs() < f32::EPSILON);
}

/// Test config default values.
#[test]
fn test_context_config_defaults() {
    let config = ContextConfig::default();

    assert!((config.warning_threshold - 0.7).abs() < f32::EPSILON);
    assert!((config.critical_threshold - 0.9).abs() < f32::EPSILON);
    assert!((config.mcp_overhead - 0.0).abs() < f32::EPSILON);
    assert!(config.show_effective_context);
}

/// Test context threshold state enum values.
#[test]
fn test_threshold_state_values() {
    let _normal = ContextThresholdState::Normal;
    let _warning = ContextThresholdState::Warning;
    let _critical = ContextThresholdState::Critical;

    // Verify they're different
    assert_ne!(ContextThresholdState::Normal, ContextThresholdState::Warning);
    assert_ne!(ContextThresholdState::Warning, ContextThresholdState::Critical);
    assert_ne!(ContextThresholdState::Normal, ContextThresholdState::Critical);
}

/// Test ANSI code stripping.
#[test]
fn test_ansi_code_stripping() {
    use codirigent_core::context::strip_ansi_codes;

    let text_with_ansi = "\x1b[31mRed text\x1b[0m Context: 75%";
    let stripped = strip_ansi_codes(text_with_ansi);

    assert_eq!(stripped, "Red text Context: 75%");
}

/// Test usage retrieval for non-existent session.
#[test]
fn test_get_usage_nonexistent_session() {
    let config = ContextConfig::default();
    let tracker = ContextTracker::new(config);

    let usage = tracker.get_usage(SessionId(999));
    assert!(usage.is_none());
}

/// Test edge case: 0% usage.
#[test]
fn test_zero_usage() {
    let config = ContextConfig::default();
    let mut tracker = ContextTracker::new(config);

    tracker.update_usage(SessionId(1), 0.0);

    let usage = tracker.get_usage(SessionId(1)).unwrap();
    assert!((usage.raw_usage - 0.0).abs() < f32::EPSILON);
    assert_eq!(usage.threshold_state, ContextThresholdState::Normal);
}

/// Test edge case: 100% usage.
#[test]
fn test_full_usage() {
    let config = ContextConfig::default();
    let mut tracker = ContextTracker::new(config);

    tracker.update_usage(SessionId(1), 1.0);

    let usage = tracker.get_usage(SessionId(1)).unwrap();
    assert!((usage.raw_usage - 1.0).abs() < f32::EPSILON);
    assert_eq!(usage.threshold_state, ContextThresholdState::Critical);
}
