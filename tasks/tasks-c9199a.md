---
id: tasks-c9199a
title: Design relay identity without changing native liveness
status: doing
priority: 2
size: l
complexity: high
process: planned
owner: main
created: 2026-09-17T00:50:54Z
updated: 2026-09-17T21:51:35Z
started: 2026-09-17T21:04:30Z
depends: [relay-06b1da]
tags: []
source: ops-998bbb
agent: codex
plan: docs/plans/2026-09-17-lifecycle-provenance.md
---

Two independently deliverable tracks: (1) approved lifecycle-note provenance, implemented by tasks-d51eda and tasks-b07adc using ops docs/specs/2026-09-17-session-provenance-design.md and this task's lifecycle-provenance implementation plan; (2) the separate opt-in relay-ancestry identity design in tasks-8921f4. The ops-79f409 attended evidence is complete. Fixed fields are harness_session and harness_session_source; note provenance must not change native claim identity or liveness. Obs depends only on tasks-b07adc, not the ancestry track. This parent stays open until both tracks are delivered. Lifecycle implementation plan is approved with three review adjustments; tasks-d51eda delivers the reader first, and tasks-b07adc waits for reader rollout to titan and europa (Europa).

## Notes

- 2026-09-17T16:57:15Z (main): Consumer requirement from obs-045db1 charter review: expose qualified harness session identity for task/session joins and preserve a timestamped association across start/resume/park/close and claim release. Current parks retain tagged session only in the shared store; start has no durable session note. Account for claude:<id> historical parks versus claude-code:<id> relay keys and raw Claude claim ids. Coordinate native session provenance with the ops task filed from obs-045db1. This is an input to this task’s pending identity design, not approval of a new storage contract.
- 2026-09-17T17:02:07Z (main): Session provenance producer is ops-79f409. Obs input needs the native qualified key (claude-code:<id> or codex:<id>) and a timestamped task association surviving claim/park release; existing park capture superseded tasks-abfd3d but cannot cover an unparked stopped turn on its own.
- 2026-09-17T17:39:07Z (main): Obs plan-review steering: satisfy the durable association requirement by stamping ops-79f409's qualified TASKS_SESSION key into existing timestamped task notes, not a new store. Parks already append notes; ordinary starts currently only stamp started (notes are takeover-only), so add start/resume notes and extend park/close notes. Preserve source timestamps across claim release. Obs now treats sid as unknown and has no Node ancestry bridge. This narrows obs's consumer requirement; the existing tasks identity design still needs its own review.
- 2026-09-17T19:33:59Z (main): Supersedes the 17:39 TASKS_SESSION-export steering: the approved ops-79f409 design fixes harness_session and harness_session_source in lifecycle notes, separate from claim identity. Implement that contract without redesigning fields or changing liveness; broader opt-in relay ancestry remains separately designed. Obs joins Codex notes even while claims remain sid:<pid>. No implementation performed in this synchronization.
- 2026-09-17T20:57:12Z (main): ops-79f409 is complete: independently reviewed ops docs/reports/2026-09-17-session-provenance-probe.md verifies native distinctness and command/harness equality for Claude and two attended Codex sessions. Both Codex variables agree with familiar; neither TASKS override is set. Implement the approved harness_session/harness_session_source lifecycle-note contract without changing claims or liveness; evidence acquisition is no longer pending. Note persistence and claim regression checks remain here, separately from broader relay-ancestry design.
- 2026-09-17T21:04:30Z (main): Starting only the approved lifecycle-note provenance slice. Reuse ops docs/specs/2026-09-17-session-provenance-design.md; write a tasks implementation plan for user review before code changes. Broader opt-in relay ancestry remains separate. Trace includes transition callers (start and flag/editor status edits), park, close without a message, and retry paths. Workspace: .worktrees/lifecycle-provenance.
- 2026-09-17T21:09:24Z (feat/lifecycle-provenance): parked (waiting on user, review): User review required for the lifecycle-note implementation plan at .worktrees/lifecycle-provenance/docs/plans/2026-09-17-lifecycle-provenance.md. Resume tasks-d51eda after approval, then tasks-b07adc; tasks-8921f4 is separate ancestry work. No implementation is running.
- 2026-09-17T21:42:13Z (feat/lifecycle-provenance): parked (waiting on agent, dependency): Approved lifecycle plan is executing as tasks-d51eda. After reader delivery, tasks-b07adc waits for user-confirmed reader rollout on every Dropbox host; tasks-8921f4 remains separate ancestry work.
- 2026-09-17T21:51:35Z (feat/lifecycle-provenance): parked (waiting on user, environment): Task 1 reader is delivered on titan. User must confirm Europa reader install/read evidence before tasks-b07adc writer work. tasks-8921f4 ancestry remains separate. Nothing is executing.
