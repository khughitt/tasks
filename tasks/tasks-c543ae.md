---
id: tasks-c543ae
title: "Enforce the halt tag: refuse new starts in a halted project, with a reasoned override"
status: doing
priority: 1
size: m
complexity: mid
process: planned
owner: main
created: 2026-09-30T10:09:44Z
updated: 2026-09-30T11:22:44Z
started: 2026-09-30T10:58:24Z
depends: []
tags: [cross-project]
source: ops-5beefd
agent: codex
spec: docs/specs/2026-09-30-halt-enforcement-design.md
---

Contract in ops docs/specs/2026-09-29-test-latency-escalation-design.md section 6: a project is halted while its registered checkout holds a task tagged halt whose status is not done or dropped (idea, todo, doing, blocked and shelved all halt; a deferred one too). start refuses every new start (todo, blocked, the next occurrence of a completed recurring task) with error kind halted naming the halt task; allowed: resuming a doing task, the halt task, its descendants and dependencies, and any task whose priority is at least the halt task's. Override: tasks start <id> --force --reason "<why>" writes one note on each task naming the other, the session and the reason; --force without --reason is refused under a halt. prime prints a halt: line first; ready and next list the halt task and what it allows and count what they hid. An unreadable registered checkout fails the start. Tests listed in the spec's section 11 under The halt contract. Spelling of --reason on start is this piece's to confirm against cli.toml.

## Notes

- 2026-09-30T10:58:24Z (main): started
  provenance: {"harness_session":"codex:01a0f1f2-8c7e-71c3-9d0e-3ec49cdcdfef","harness_session_source":"CODEX_SESSION_ID"}
- 2026-09-30T10:58:34Z (main): Using the reviewed ops-5beefd section 6 contract as the design basis; tasks-specific spec and plan will be reviewed before implementation. Existing record-home work is active in another worktree, so integration will check that boundary.
- 2026-09-30T11:14:06Z (feat/halt-enforcement): Tasks-specific halt design drafted from the approved ops contract: registered checkout authority, start guard and override notes, plus prime/ready/next JSON metadata. User review precedes the implementation plan.
- 2026-09-30T11:14:19Z (feat/halt-enforcement): parked (waiting on user, review): User reviews .worktrees/halt-enforcement/docs/specs/2026-09-30-halt-enforcement-design.md; after approval the agent writes and reviews the tasks implementation plan in this worktree.
  provenance: {"harness_session":"codex:01a0f1f2-8c7e-71c3-9d0e-3ec49cdcdfef","harness_session_source":"CODEX_SESSION_ID"}
- 2026-09-30T11:22:44Z (feat/halt-enforcement): Spec self-review clarified that prime's ready rows share the same halt filter as ready and next, so the entry view does not suggest starts that the guard will refuse.
