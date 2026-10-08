---
id: tasks-f2b0ad
title: "Establish pilot isolation, inventory and genuine delivery"
status: doing
priority: 1
size: m
complexity: high
process: direct
owner: docs/claim-guard-design
created: 2026-10-08T15:42:33Z
updated: 2026-10-08T18:57:42Z
started: 2026-10-08T18:47:18Z
depends: [tasks-50f850, tasks-1ffde6, tasks-724c9e]
parent: tasks-56b450
tags: []
source: "tasks-56b450:claim-guard-policy:task-4"
agent: codex
spec: docs/specs/2026-10-08-claim-guard-policy-design.md
plan: docs/plans/2026-10-08-claim-guard-policy.md
step: "Task 4: Establish pilot isolation, inventory and genuine delivery"
---

Execute Task 4: Establish pilot isolation, inventory and genuine delivery only after separate implementation-plan approval. Follow docs/plans/2026-10-08-claim-guard-policy.md and the approved spec. Owner-specific code tasks/worktrees and their gates are required; no live guard or hook activation before the Task 6 approval packet. This proposed step is blocked for plan review.

## Notes

- 2026-10-08T15:42:33Z (docs/claim-guard-design): Implementation plan awaits separate review; do not execute draft steps.
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T16:56:29Z (docs/claim-guard-design): Plan round-1 revision adds project-contained .private config/state/auth/evidence paths, a no-claim workspace-write smoke with -a never, no-claim same-worker restoration probe, and a smoke-tested single-process A/B/C fallback with bounded author gates. Recorder failures persist before exit; personal auth copy is exclusive 0600 and deleted on abort/cleanup. Step remains blocked for plan re-review.
- 2026-10-08T17:35:06Z (docs/claim-guard-design): Plan accepted after conditional fixes and scoped verification; execute the recorded dependencies.
- 2026-10-08T18:47:18Z (docs/claim-guard-design): started
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T18:56:36Z (docs/claim-guard-design): Preparation: enumeration prerequisite passed. Private real sibling control directory holds hooks/Codex home/gates/evidence; only isolated tasks config/state are model-workspace writable. Recorder five controls, author-gate match/mismatch, credential failure cleanup and bounded process cleanup pass. Codex 0.161.0; actual session routing gpt-6.1-sol/high; private personal file-auth route and hooks/multi_agent feature checks pass; credential copy removed. Native /hooks inspected: three new definitions, zero active; none trusted and no model request or scratch claim. Exact source/command/file fingerprints and launcher prepared at .superpowers/sdd/claim-guard-policy/pilot/control/hook-trust-review.md. Model sandbox/delivery/transport/lifecycle checks remain unrun. Acceptance docs record actual limits.
- 2026-10-08T18:57:42Z (docs/claim-guard-design): parked (waiting on user, approval): User approves the three exact private pilot hooks in .worktrees/claim-guard-design/.superpowers/sdd/claim-guard-policy/pilot/control/hook-trust-review.md; agent resumes here, uses normal /hooks trust, runs no-claim write/deny/recorder smoke then transport and stage A checks.
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
