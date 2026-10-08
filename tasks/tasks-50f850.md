---
id: tasks-50f850
title: Implement the ops guard reader and claim policy
status: done
priority: 1
size: m
complexity: mid
process: direct
owner: docs/claim-guard-design
created: 2026-10-08T15:42:33Z
updated: 2026-10-08T18:07:50Z
started: 2026-10-08T17:38:37Z
completed: 2026-10-08T18:07:50Z
depends: []
parent: tasks-56b450
tags: []
source: "tasks-56b450:claim-guard-policy:task-1"
agent: codex
spec: docs/specs/2026-10-08-claim-guard-policy-design.md
plan: docs/plans/2026-10-08-claim-guard-policy.md
step: "Task 1: Implement the ops guard reader and claim policy"
---

Execute Task 1: Implement the ops guard reader and claim policy only after separate implementation-plan approval. Follow docs/plans/2026-10-08-claim-guard-policy.md and the approved spec. Owner-specific code tasks/worktrees and their gates are required; no live guard or hook activation before the Task 6 approval packet. This proposed step is blocked for plan review.

## Notes

- 2026-10-08T15:42:33Z (docs/claim-guard-design): Implementation plan awaits separate review; do not execute draft steps.
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T17:35:05Z (docs/claim-guard-design): Plan accepted after conditional fixes and scoped verification; execute the recorded dependencies.
- 2026-10-08T17:38:37Z (docs/claim-guard-design): started
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T17:40:37Z (docs/claim-guard-design): Owner code task ops-18352b uses ops .worktrees/claim-guard, branch feat/claim-guard-policy. Ops baseline test-fast running; source guard policy/executable remains on ops main until cutover approval.
- 2026-10-08T18:06:50Z (docs/claim-guard-design): Task 1 candidate committed in ops .worktrees/claim-guard; owner ops-18352b closed with code after RED→GREEN and 87 focused/4 no-skip real-reader/1276 full tests plus check. Live ops main remains unchanged; continuing to hq Tasks 2–3 before pilot and final review.
- 2026-10-08T18:07:50Z (docs/claim-guard-design): done
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T18:07:50Z (docs/claim-guard-design): Task 1 candidate in ops commit 91984f5: direct validated reader, native selectors, claims-first ordering, safe recovery and covered boundaries. Owner task ops-18352b closed with code; 87 focused, 4 real-reader no skips, 1276 full with existing skip and check pass. No live activation.
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
