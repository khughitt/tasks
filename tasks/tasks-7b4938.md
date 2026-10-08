---
id: tasks-7b4938
title: "Verify worker turns, release and completion-proven recovery"
status: blocked
priority: 1
size: m
complexity: high
process: direct
created: 2026-10-08T15:42:33Z
updated: 2026-10-08T16:56:29Z
depends: [tasks-f2b0ad]
parent: tasks-56b450
tags: []
source: "tasks-56b450:claim-guard-policy:task-5"
agent: codex
spec: docs/specs/2026-10-08-claim-guard-policy-design.md
plan: docs/plans/2026-10-08-claim-guard-policy.md
step: "Task 5: Verify worker turns, release and completion-proven recovery"
---

Execute Task 5: Verify worker turns, release and completion-proven recovery only after separate implementation-plan approval. Follow docs/plans/2026-10-08-claim-guard-policy.md and the approved spec. Owner-specific code tasks/worktrees and their gates are required; no live guard or hook activation before the Task 6 approval packet. This proposed step is blocked for plan review.

## Notes

- 2026-10-08T15:42:34Z (docs/claim-guard-design): Implementation plan awaits separate review; do not execute draft steps.
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T16:56:29Z (docs/claim-guard-design): Plan round-1 revision requires both worker claims and controller claim retained after hook retry; all actors use a one-line no-mutation retry instruction. Stage B/C resumes only when pre-gate proved worker restoration, otherwise stays in the same process through C. Stage D alone performs unforced refusal and explicit recovery inside actual controller native environment. Step remains blocked for plan re-review.
