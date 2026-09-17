---
id: tasks-c9199a
title: Design relay identity without changing native liveness
status: todo
priority: 2
size: l
complexity: high
process: planned
created: 2026-09-17T00:50:54Z
updated: 2026-09-17T17:39:07Z
depends: [relay-06b1da]
tags: []
source: ops-998bbb
agent: codex
---

Approved source: ops docs/specs/2026-09-16-relay-design.md (reviewed after d7e2c33); execution brief: ops docs/plans/2026-09-16-relay-bootstrap.md (approved after bc53c51 with review corrections).

Next: write the separate tasks identity design for review, then its implementation plan.

Read tasks `src/claims.rs`, `tests/cli.rs`, and
`docs/specs/2026-09-05-work-claims-design.md`. Design opt-in configuration and
identity continuity before implementation; preserve all spec §6 tasks constraints.
Outside recognized harness ancestry, native identity wins without opening the
registry, even with host-wide relay enabled. Unknown ancestry is not a shell.
Inside it, match the nearest harness using same-host PID/start/Linux boot evidence;
do not skip an unmatched inner harness. Missing, ambiguous or unavailable proof is
an acquisition error with explicit-identity recovery. Claude hints must agree with
process proof; shared OpenCode processes do not distinguish independent subagents.

Rust reads schema 1 directly without Node. Copy relay-06b1da's versioned handle fixtures and
label the oversized Linux value as a format test. Include the real Codex case:
live harness ancestor, empty snapshot before its first turn's SessionStart hook.
Test explicit TASKS_SESSION/TASKS_SESSION_PID precedence, nested harnesses, plain
shells with corrupt/missing registry, and held-claim refresh/release after registry
loss. Keep existing Live/Stale, takeover/release retries, mutation locking and TTL.
Darwin epochs must not enter Linux pid_start fields; keep its existing native
unverifiable path. Incompatible schema changes coordinate the producer and reader.

Prerequisites: relay-06b1da. Release goal: relay-1231bf. Bootstrap ops-998bbb closure does not mean this deliverable has shipped.

## Notes

- 2026-09-17T16:57:15Z (main): Consumer requirement from obs-045db1 charter review: expose qualified harness session identity for task/session joins and preserve a timestamped association across start/resume/park/close and claim release. Current parks retain tagged session only in the shared store; start has no durable session note. Account for claude:<id> historical parks versus claude-code:<id> relay keys and raw Claude claim ids. Coordinate native session provenance with the ops task filed from obs-045db1. This is an input to this task’s pending identity design, not approval of a new storage contract.
- 2026-09-17T17:02:07Z (main): Session provenance producer is ops-79f409. Obs input needs the native qualified key (claude-code:<id> or codex:<id>) and a timestamped task association surviving claim/park release; existing park capture superseded tasks-abfd3d but cannot cover an unparked stopped turn on its own.
- 2026-09-17T17:39:07Z (main): Obs plan-review steering: satisfy the durable association requirement by stamping ops-79f409's qualified TASKS_SESSION key into existing timestamped task notes, not a new store. Parks already append notes; ordinary starts currently only stamp started (notes are takeover-only), so add start/resume notes and extend park/close notes. Preserve source timestamps across claim release. Obs now treats sid as unknown and has no Node ancestry bridge. This narrows obs's consumer requirement; the existing tasks identity design still needs its own review.
