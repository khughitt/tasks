Ready to merge. The frozen range 5293c88..8cd08140e7db1b9e7b29f97309f34c8db6853023 addresses all eight deferred minors and the spec-status correction for tasks-3a8396. The controller owns task closeout and integration.

Reviewer: **codex/gpt-6.1-sol**, verified by a fresh `timeout 2 relay model --harness codex --pretty` invocation.

**Strengths**

- Missing-reference diagnostics now retain every distinct stored spelling. Canonical map keys still control unresolved counts and dependency relief; the updated regression test verifies both spellings alongside unchanged counts, status maps and examples.
- `scan_lookup` preserves the original scan-first authority and error propagation for both blockers and dependency resolution. Every move hop still canonicalizes through the shared resolver.
- The test-helper extraction preserves claim liveness, PID-view handling and Git isolation. Git subprocesses clear inherited environment variables and assert both repository boundaries before fixture writes.
- Pretty assertions now locate each expected label within its own ranked row. The exact history-footer prefix and fuller-fixture width assertions cover the remaining requested test gaps.
- `long_about` explains both unavailable history metrics without changing `get_about`, which supplies the parser-surface summary.
- The spec records d2940d0 approval, plan 327fe22 with S1, and shipment 074672f consistently with the source task and approval records.

All paths above are under `.worktrees/tasks-3a8396/`.

**Critical findings:** None.

**Important findings:** None.

**Minor findings:** None.

**Verification**

I inspected the full diff, connected resolution/counting/rendering code, affected helper callers, source review reports and saved validation logs. Those logs show:

- Isolated full baseline: **1,743 passed**.
- Final isolated fast suite: **1,739 passed, 4 ignored**.
- Focused checks: blockers **10 passed**, blockers integration **4 passed**, parser surface **1 passed**.
- The deliberate misplaced-label mutation failed the intended row assertion. The restored renderer matches its saved pre-mutation copy byte-for-byte.

Fresh `git diff --check` succeeded. HEAD remains the frozen revision and the worktree remains clean.

**Declined to judge**

- Map-backed lookup and larger-corpus performance: indexing was explicitly optional; this extraction retains existing complexity.
- Actual blocking-history collection: separately owned by tasks-71c74d; this task exposes its absence.
- The three normal-baseline failures caused by inherited host Git hooks: pre-existing and tracked in tasks-2518ad.
- Narrow-terminal title wrapping: an accepted limitation from the original delivery; the renderer is unchanged.

**Ready to merge: Yes.** No corrective round is needed. Nothing was pushed or modified, and this review left no processes running. The branch remains in `.worktrees/tasks-3a8396` for controller closeout and local integration.

Not checked: I did not rerun suites; validation counts above come from the inspected saved logs. The known normal-environment Git-hook failures remain outside this branch.
