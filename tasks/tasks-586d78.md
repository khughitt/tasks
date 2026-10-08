---
id: tasks-586d78
title: "Review, approve and activate the coordinated cutover"
status: doing
priority: 1
size: m
complexity: mid
process: direct
owner: docs/claim-guard-design
created: 2026-10-08T15:42:34Z
updated: 2026-10-08T21:16:15Z
started: 2026-10-08T21:11:37Z
depends: [tasks-50f850, tasks-468bc7, tasks-1ffde6, tasks-7b4938]
parent: tasks-56b450
tags: []
source: "tasks-56b450:claim-guard-policy:task-6"
agent: codex
spec: docs/specs/2026-10-08-claim-guard-policy-design.md
plan: docs/plans/2026-10-08-claim-guard-policy.md
step: "Task 6: Review, approve and activate the coordinated cutover"
---

Execute Task 6: Review, approve and activate the coordinated cutover only after separate implementation-plan approval. Follow docs/plans/2026-10-08-claim-guard-policy.md and the approved spec. Owner-specific code tasks/worktrees and their gates are required; no live guard or hook activation before the Task 6 approval packet. This proposed step is blocked for plan review.

## Notes

- 2026-10-08T15:42:34Z (docs/claim-guard-design): Implementation plan awaits separate review; do not execute draft steps.
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T17:35:06Z (docs/claim-guard-design): Plan accepted after conditional fixes and scoped verification; execute the recorded dependencies.
- 2026-10-08T21:11:37Z (docs/claim-guard-design): started
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T21:16:15Z (docs/claim-guard-design): Final owner verification exits 0: ops RealFactsTests 4 pass/no skips, fast 1276 with one existing skip, hygiene; hq full gate 226 agents +553 hook/tools and live facts validation; tasks fast 1159 with one ignored and hygiene. Corrected native pilot A–E and both selected native-home sources pass. Prepare fresh whole-branch review package; live merges and home trust remain unapproved.
