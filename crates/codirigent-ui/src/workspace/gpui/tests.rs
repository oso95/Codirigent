//! GPUI View Testing Strategy
//!
//! # Why Limited Tests
//!
//! `WorkspaceView` is a GPUI view component that requires the GPUI runtime
//! for rendering and interaction. Testing GPUI views requires:
//! - GPUI test harness (`gpui::TestAppContext`)
//! - Window creation for rendering tests
//! - Focus simulation for interaction tests
//!
//! # Test Coverage Strategy
//!
//! 1. **Core Business Logic** - Fully tested in `workspace/tests.rs` (29 tests)
//!    - Layout management, session handling, focus navigation
//!    - Bounds calculation, cell info generation
//!    - All non-GPUI logic has 100% test coverage
//!
//! 2. **GPUI Integration** - Deferred to integration tests
//!    - Rendering correctness requires visual inspection or snapshot tests
//!    - Action handlers require GPUI action dispatch simulation
//!
//! # Future: GPUI Test Infrastructure
//!
//! When GPUI test helpers are available, add tests for:
//! - [ ] WorkspaceView renders without panic
//! - [ ] Action handlers (NewSession, CloseSession, etc.) work correctly
//! - [ ] Focus delegation to child components
//! - [ ] Layout changes trigger re-render

#[test]
fn test_core_workspace_is_tested_separately() {
    // Reminder: Core workspace logic has dedicated tests in workspace/tests.rs
    // Run `cargo test workspace::tests` to see all 29 tests pass
    use crate::workspace::Workspace;

    // Quick sanity check that we can create a workspace
    let ws = Workspace::new();
    assert!(ws.sessions().is_empty());
}

#[test]
fn test_normalize_codex_execution_mode_detects_bypass_alias() {
    assert_eq!(
        super::WorkspaceView::normalize_codex_execution_mode("codex --yolo"),
        Some(codirigent_core::CodexExecutionMode::Bypass)
    );
}

#[test]
fn test_normalize_codex_execution_mode_detects_full_auto() {
    assert_eq!(
        super::WorkspaceView::normalize_codex_execution_mode("codex resume abc --full-auto"),
        Some(codirigent_core::CodexExecutionMode::FullAuto)
    );
}

#[test]
fn test_normalize_codex_execution_mode_detects_explicit_never_and_danger() {
    assert_eq!(
        super::WorkspaceView::normalize_codex_execution_mode(
            "codex -a never -s danger-full-access"
        ),
        Some(codirigent_core::CodexExecutionMode::Bypass)
    );
}

#[test]
fn test_keystroke_is_text_input_for_plain_printable_without_key_char() {
    let event = gpui::KeyDownEvent {
        keystroke: gpui::Keystroke {
            modifiers: gpui::Modifiers::default(),
            key: "a".to_string(),
            key_char: None,
        },
        is_held: false,
    };

    assert!(super::WorkspaceView::keystroke_is_text_input(&event));
}

#[test]
fn test_keystroke_is_not_text_input_for_named_terminal_key() {
    let event = gpui::KeyDownEvent {
        keystroke: gpui::Keystroke {
            modifiers: gpui::Modifiers::default(),
            key: "enter".to_string(),
            key_char: None,
        },
        is_held: false,
    };

    assert!(!super::WorkspaceView::keystroke_is_text_input(&event));
}

#[test]
fn test_keystroke_is_text_input_for_task_modal_ascii_and_digits() {
    for key in ["a", "7", "space"] {
        let event = gpui::KeyDownEvent {
            keystroke: gpui::Keystroke {
                modifiers: gpui::Modifiers::default(),
                key: key.to_string(),
                key_char: None,
            },
            is_held: false,
        };

        assert!(
            super::WorkspaceView::keystroke_is_text_input(&event),
            "{key} should continue to the platform text-input handler"
        );
    }
}
