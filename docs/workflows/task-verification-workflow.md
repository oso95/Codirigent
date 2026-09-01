# Task Verification Workflow

Required procedure for any implementation task that is meant to land on a
working branch without regressions.

---

## Goal

Each task must be completed as a small, reviewable unit:

1. implement the task
2. run the full verification matrix
3. do a depth code review of the actual result
4. commit locally
5. move to the next task

Do not batch multiple unfinished tasks into one verification pass or one
commit.

---

## Branch Rules

- Create a dedicated working branch before starting the task series.
- Keep each task in its own local commit.
- Do not push automatically.
- Wait for human review after the full task series is complete.

---

## Per-Task Procedure

### 1. Implement

- Make the code change for exactly one task.
- Keep the scope tight.
- If the task changes UI behavior, include UI/UX quality in the implementation,
  not just raw correctness.
- If the task affects platform-specific behavior, check macOS, Linux, and
  Windows implications before treating it as done.

### 2. Run the Required Verification Matrix

Run these commands after the task implementation is complete:

```bash
cargo clean
cargo build --all-features
cargo test --all --all-targets --all-features
cargo test -p codirigent-ui --lib --features gpui-full
cargo clippy --all --all-targets --all-features -- -D warnings
cargo fmt --all --check
bash scripts/audit-unwraps.sh
```

Notes:

- `cargo clean` is required before the validation pass for each task.
- `gpui-full` should be run where the task touches GPUI-backed UI behavior. In
  practice, use the dedicated `codirigent-ui` command above for UI work.
- `audit-unwraps.sh` is part of the gate. If the repo already has a known
  baseline, confirm the touched code did not add to it.
- If a test failure appears unrelated or flaky, rerun it directly and then
  rerun the broader suite before deciding it is not caused by the task.

### 3. Perform a Depth Code Review

After the verification commands pass, review the resulting diff again with a
fresh code review mindset.

Review for:

- behavioral regressions
- missing edge-case handling
- cross-platform issues
- UI/UX quality issues
- incorrect assumptions about focus, event propagation, timing, or state sync
- dead code, stale constants, and unused paths created by the change
- tests that should have been added but were not

Recommended review commands:

```bash
git diff -- <touched files>
git status --short
rg -n "<symbol-or-flow-you-changed>" <relevant paths>
```

The review pass is not optional. Passing tests is necessary but not sufficient.

### 4. Commit

- Commit only after the implementation, verification pass, and review pass are
  complete.
- Use a commit message that describes the task outcome, not the debugging path.
- Do not amend older commits unless explicitly requested.
- Do not push.

---

## Multi-Task Series

When working through several TODO items:

1. finish task 1
2. run the full matrix
3. review deeply
4. commit task 1
5. repeat the same process for task 2
6. continue until the list is complete

After the last task:

- confirm the final branch state is clean
- summarize all local commits
- note any existing baseline issues that were observed but not introduced
- stop and wait for review

---

## Failure Handling

If any required command fails:

- do not commit
- fix the issue and rerun the required matrix
- if the failure looks flaky, isolate it, rerun it, then rerun the broader
  command that originally failed
- if the failure is outside the task scope and genuinely pre-existing, document
  that explicitly before asking for a decision

---

## Completion Standard

A task is only complete when all of the following are true:

- the code change is implemented
- the required verification commands pass
- the post-verification review is done
- cross-platform concerns were considered
- UI/UX quality was considered where relevant
- the task is committed locally

Anything short of that is still in progress.
