---
id: tasks-60cc95
title: Exclude moved-only plans from heading-coverage warnings
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-25T17:19:48Z
updated: 2026-10-26T15:20:12Z
depends: []
parent: tasks-a7460c
tags: [feedback, friction, "from:ops"]
agent: claude-code/claude-opus-5-5
origin: agent
---

Original report: After tasks move carried a planned goal and its step children to another project, the source's moved tombstone kept its plan link, and tasks check in the source warned unlinked_step for each heading of the plan copy left behind (8 warnings), which blocks tools that require a silent check. The only route was deleting the source's plan copy. Deleting it in a worktree did not clear the warning; check still found the file through the main checkout. Expected: move leaves no warning behind (it flags or carries the plan copy, or check skips headings of a plan linked only by a moved tombstone), or the warning names the tombstone and the fix.

Why: At f1d45f2, src/commands/check.rs collects linked_plans from every task status and audits every heading, including plans retained only by moved tombstones. Moved records are immutable history, while the destination owns the live work. Source deletion is unnecessary for that narrow case. The incident's exact move sequence remains unverified: the current move API only accepts unparented, childless records, so do not assume it moves a goal and children in one call. [ref:brief] supplies the resolver context and the separate dispositions for related boundary reports.

Done: Exclude moved-only plan links from unlinked_step heading-coverage auditing, retaining the tombstone's original plan link and document bytes. When any non-moved record still links that same plan, keep the existing heading-coverage contract. Do not let a moved step count as coverage for that surviving local plan. Preserve done/dropped historical behavior, open task doc_missing and step_missing checks, tombstone/counterpart diagnostics, JSON shapes and the existing main-checkout document fallback. Do not delete, move or rewrite source documents or broaden the move API. Update the current check guidance with this rule.

Where: src/commands/check.rs::run (linked_plans, plan_bodies and final unlinked_step loop); src/moving/preflight.rs::prepare (cloned tombstone fields); src/tombstone.rs; tests/cli.rs check and worktree fallback fixtures; tests/move.rs; README.md check documentation. The task-move design and [ref:brief] explain the history and lookup constraints.

Verification: Start with an isolated CLI regression that keeps a plan in the source and moves its last local live link, then check the source and destination. Cover a moved goal-only link, a moved step, a plan also linked by a non-moved local record, and a plan supplied by main-checkout fallback after its deletion only in a worktree. The source emits no tombstone-only unlinked_step warnings, the shared live plan still reports uncovered headings, and the destination still validates its live steps. Preserve existing closed-history and missing-document assertions. Use existing isolated fixture helpers and Git configuration rules, then run just test-one filters and just test-fast before committing. No runtime regression was run during scope.

## References

- brief (input, kind=brief): docs/notes/2026-10-15-branch-only-document-links-brief.md — resolver context and related moved-plan disposition

## Notes

- 2026-10-25T17:19:48Z (feedback): created
  provenance: {"harness_session":"claude-code:d825d1b2-617b-4da6-a70a-92adf0d4852f","harness_session_source":"CLAUDE_CODE_SESSION_ID","origin":"agent"}
