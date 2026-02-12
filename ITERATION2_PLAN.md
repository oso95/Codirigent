# Iteration 2 Plan - Continue Phase 3 Split gpui.rs

## Starting Point

- gpui.rs: 4,183 lines (target: < 1,500 lines)
- Completed: editor_detection.rs (231 lines), cli_helpers.rs (160 lines)
- Progress: 264 lines extracted (5.9%)
- Remaining: 2,683 lines to extract

## Iteration 2 Goals

Extract 600-800 more lines to reach ~20% completion.

### Task 1: Decompose handle_task_board_event() (Priority: HIGH, Impact: ~250 lines)

**Current state**: 268-line function with large match statement (lines 1887-2155)

**Approach**: Extract each match arm into a separate handler method

1. Extract `handle_task_action_start()`
   - Lines: ~1928-1931
   - Simple: Just calls manager.start_task()

2. Extract `handle_task_action_review()`
   - Lines: ~1932-1958
   - Complex: Moves task to review, clears session assignment
   - Need to handle lock ordering (drop manager before session_manager)

3. Extract `handle_task_action_complete()`
   - Lines: ~1959-1988
   - Similar to review: approve task, clear session assignment

4. Extract `handle_task_action_delete()`
   - Lines: ~1989-2015
   - Similar pattern: delete task, clear session assignment

5. Extract `handle_task_action_assign()`
   - Lines: ~2016-2092
   - Most complex: find session, direct assign, send formatted input
   - Handles deferred Enter logic

6. Keep in main function:
   - TabSelected (simple, 2 lines)
   - AutoAssignModeChanged (simple, 7 lines)
   - AddTaskClicked (simple, 2 lines)
   - TaskSelected (simple, 2 lines)
   - Edit special case (already extracted outside lock)

**Expected outcome**:
- Main function: ~40 lines (down from 268)
- New methods: 5 handlers × ~40 lines = ~200 lines
- Net reduction in gpui.rs: ~200 lines (since methods are in same file initially)

**Verification**: Build, clippy, test after extraction

### Task 2: Extract simple helper methods (Priority: MEDIUM, Impact: ~150 lines)

Look for standalone helper methods that can be moved to utility modules:

1. Review lines 3180-3200 for simple helper methods
2. Extract any formatting, validation, or conversion helpers
3. Move to appropriate utility modules (or create new ones)

**Expected outcome**: 100-150 lines extracted

### Task 3: Extract modal-related helpers (Priority: MEDIUM, Impact: ~200 lines)

**Candidates** (from lines 2400-2700):
- `open_task_creation_modal()`
- `close_task_creation_modal()`
- `open_task_edit_modal()`
- `close_task_edit_modal()`
- `apply_task_creation_modal()`
- `apply_task_edit_modal()`

**Approach**:
1. Create `task_modals.rs` module
2. Move modal-related methods as `impl WorkspaceView` in new file
3. Use `pub(super)` visibility

**Expected outcome**: ~200 lines to new module

### Task 4: Begin output_polling decomposition (Priority: LOW, Impact: ~100 lines)

**Approach**: Start with the easiest sub-sections

1. Extract `process_pending_enters()` (lines 547-574)
   - Clean boundary
   - Only accesses: pending_enters, session_manager
   - ~28 lines

2. Extract `cleanup_expired_enters()` (lines 565-574)
   - Can combine with above or keep separate
   - ~10 lines

3. Extract `refresh_git_status_periodic()` (lines 921-946)
   - Clean boundary
   - ~26 lines

**Expected outcome**: ~65 lines extracted (small start on poll_output)

## Success Criteria

- [ ] gpui.rs < 3,500 lines (16% reduction from current)
- [ ] All builds pass (check, build, clippy)
- [ ] All tests pass (21/21)
- [ ] No behavioral changes
- [ ] Committed working state after each major extraction
- [ ] Updated PHASE3_PROGRESS.md

## Estimated Impact

| Task | Lines Extracted | Cumulative |
|------|----------------|-----------|
| Task 1: handle_task_board_event | ~200 | 464 (10.4%) |
| Task 2: Simple helpers | ~150 | 614 (13.8%) |
| Task 3: Modal helpers | ~200 | 814 (18.3%) |
| Task 4: poll_output start | ~65 | 879 (19.8%) |

**Total**: ~615 lines (13.8% → 19.8% progress)
**Remaining after iteration 2**: ~2,068 lines

## Notes

- Focus on clean, testable extractions
- Maintain working state at all times
- Don't rush complex refactors (poll_output main loop)
- Commit frequently with good messages
- Update progress docs as you go

## Risk Assessment

| Task | Risk | Mitigation |
|------|------|-----------|
| Task 1 | MEDIUM | Test each handler extraction separately |
| Task 2 | LOW | Simple helpers are low risk |
| Task 3 | MEDIUM | Keep as impl WorkspaceView, pub(super) |
| Task 4 | LOW | Starting with simple sections only |
