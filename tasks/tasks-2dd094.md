---
id: tasks-2dd094
title: Adopt relay's session-process walk and registry schema 2 in relay identity
status: doing
priority: 1
size: m
complexity: mid
process: planned
owner: feat/relay-schema2
created: 2026-09-23T12:26:23Z
updated: 2026-09-23T20:04:50Z
started: 2026-09-23T17:57:12Z
depends: []
tags: [cli]
agent: "claude-code/claude-opus-5-5[1m]"
spec: docs/specs/2026-09-23-relay-session-process-design.md
plan: docs/plans/2026-09-23-relay-session-process.md
---

Relay spec docs/specs/2026-09-23-session-process-handle.md (relay repo, relay-cb616b; approved after two reviews) changes what Agent.process names: the session's own process, found as the nearest harness process with no terminal requirement. Claude Code is recognized by comm 'claude' or by a version-shaped comm (^[0-9]+(\.[0-9]+)+$) whose executable ends in /claude/versions/<comm>; the nearest Claude process with argv[1] daemon or bg-pty-host yields no identity. The registry moves to schema 2; a schema-1 file is superseded.

tasks must, in its relay identity (src/relay/ancestry.rs, resolve.rs, snapshot.rs, claims.rs proves_ownership):
- recognize the same harness processes and roles in its ancestry walk and nearest-boundary test, reading /proc/<pid>/exe only for a version-shaped comm and cmdline only for the nearest Claude process; unreadable is an error, never a skip
- accept Snapshot schema 2 only; refuse schema 1 with the supersession message (run relay reap or wait for any hook event)
- test against relay's ancestry.json corpus vendored at a pinned revision, as handles.json is
- rewrite the tasks-962300 refusal text and the README relay paragraph, which say a headless session never carries a handle; the README names this release as the minimum for hosts where relay publishes schema 2 with the opt-in on

Verification the relay spec assigns here: fresh acquisition fails closed in both version pairings; the held-claim case (outer session holds a claim; a nested 2.1.280 process under it, with no hint or an inherited outer one) refuses start, park and done; the review chain tasks -> 2.1.280 -> claude with a correct outer row and no hint refuses; live, with the opt-in on against a private relay registry, the outer and a nested session each acquire their own claim.

Ordering: this lands before relay's resolver is released (relay-c19c0a depends on it). An older tasks can continue a held claim by process proof alone and misprove a nested session as its parent, which the schema cannot stop.

## Notes

- 2026-09-23T17:57:12Z (main): started
  provenance: {"harness_session":"claude-code:dd052fc4-8f7c-4dfb-b5cd-c0e19fb82d56","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-23T17:57:12Z (main): Process planned (recorded at filing): new design spec amending the 2026-09-22 relay identity design, then a plan, in worktree .worktrees/relay-schema2
- 2026-09-23T17:59:23Z (feat/relay-schema2): parked (waiting on user, review): User reviews the design spec docs/specs/2026-09-23-relay-session-process-design.md (branch feat/relay-schema2); then write the implementation plan
  provenance: {"harness_session":"claude-code:dd052fc4-8f7c-4dfb-b5cd-c0e19fb82d56","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-23T19:48:45Z (feat/relay-schema2): Spec P2 fixed (null-session corpus chains get an explicit tasks-side table; readable cross-harness case added locally); plan written with five step children
- 2026-09-23T19:48:45Z (feat/relay-schema2): parked (waiting on user, review): User reviews the plan docs/plans/2026-09-23-relay-session-process.md and picks an execution method; then start tasks-8f28ce
  provenance: {"harness_session":"claude-code:dd052fc4-8f7c-4dfb-b5cd-c0e19fb82d56","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-23T20:04:50Z (feat/relay-schema2): resumed
  provenance: {"harness_session":"claude-code:dd052fc4-8f7c-4dfb-b5cd-c0e19fb82d56","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
