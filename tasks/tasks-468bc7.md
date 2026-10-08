---
id: tasks-468bc7
title: Remove hq's temporary mirror while retaining staged validation
status: doing
priority: 1
size: s
complexity: low
process: direct
owner: docs/claim-guard-design
created: 2026-10-08T15:42:33Z
updated: 2026-10-08T18:08:33Z
started: 2026-10-08T18:07:50Z
depends: []
parent: tasks-56b450
tags: []
source: "tasks-56b450:claim-guard-policy:task-2"
agent: codex
spec: docs/specs/2026-10-08-claim-guard-policy-design.md
plan: docs/plans/2026-10-08-claim-guard-policy.md
step: "Task 2: Remove hq's temporary mirror while retaining staged validation"
---

Execute Task 2: Remove hq's temporary mirror while retaining staged validation only after separate implementation-plan approval. Follow docs/plans/2026-10-08-claim-guard-policy.md and the approved spec. Owner-specific code tasks/worktrees and their gates are required; no live guard or hook activation before the Task 6 approval packet. This proposed step is blocked for plan review.

## Notes

- 2026-10-08T15:42:33Z (docs/claim-guard-design): Implementation plan awaits separate review; do not execute draft steps.
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T17:35:06Z (docs/claim-guard-design): Plan accepted after conditional fixes and scoped verification; execute the recorded dependencies.
- 2026-10-08T18:07:50Z (docs/claim-guard-design): started
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T18:08:33Z (docs/claim-guard-design): Owner hq-eb4f24 implements mirror removal in hq .worktrees/claim-guard; hq-c35a9b owns the later separate wiring commit in that tree. Setup ran immediately; baseline just test is tracked before edits.
