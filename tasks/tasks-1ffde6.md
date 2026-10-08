---
id: tasks-1ffde6
title: Prepare worker wiring in both Codex homes
status: done
priority: 1
size: s
complexity: low
process: direct
owner: docs/claim-guard-design
created: 2026-10-08T15:42:33Z
updated: 2026-10-08T18:41:05Z
started: 2026-10-08T18:28:52Z
completed: 2026-10-08T18:41:04Z
depends: [tasks-468bc7]
parent: tasks-56b450
tags: []
source: "tasks-56b450:claim-guard-policy:task-3"
agent: codex
spec: docs/specs/2026-10-08-claim-guard-policy-design.md
plan: docs/plans/2026-10-08-claim-guard-policy.md
step: "Task 3: Prepare worker wiring in both Codex homes"
---

Execute Task 3: Prepare worker wiring in both Codex homes only after separate implementation-plan approval. Follow docs/plans/2026-10-08-claim-guard-policy.md and the approved spec. Owner-specific code tasks/worktrees and their gates are required; no live guard or hook activation before the Task 6 approval packet. This proposed step is blocked for plan review.

## Notes

- 2026-10-08T15:42:33Z (docs/claim-guard-design): Implementation plan awaits separate review; do not execute draft steps.
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T17:35:06Z (docs/claim-guard-design): Plan accepted after conditional fixes and scoped verification; execute the recorded dependencies.
- 2026-10-08T18:28:52Z (docs/claim-guard-design): started
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T18:41:04Z (docs/claim-guard-design): Wiring candidate in hq .worktrees/claim-guard passes 226 agent and 553 hook/tool tests and parsed unrelated-group equality. Fresh flow review accepted; minor (deferred): regression does not forbid a future matcher though actual candidate groups omit it. Native delivery/identity/retry reset remain unverified until pilot.
- 2026-10-08T18:41:04Z (docs/claim-guard-design): done
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T18:41:04Z (docs/claim-guard-design): Task 3 candidate prepared in both homes with no matcher and all unrelated events/routes preserved; owner hq-c35a9b closed with code and fresh flow review. One matcher-assertion minor deferred; full gates pass. No live wiring activation.
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
