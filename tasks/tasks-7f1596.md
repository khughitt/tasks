---
id: tasks-7f1596
title: "Adopt on this host a project rename made on another host: registry key, alias, and state records"
status: done
priority: 2
size: m
complexity: mid
process: planned
owner: feat/tasks-7f1596-adopt-rename
created: 2026-09-27T13:48:27Z
updated: 2026-09-27T17:45:43Z
started: 2026-09-27T16:13:09Z
completed: 2026-09-27T17:45:43Z
depends: []
tags: []
source: "ai:docs/specs/2026-09-27-rename-to-tack-design.md"
agent: claude-code/claude-opus-5-5
spec: docs/specs/2026-09-27-second-host-rename-adoption-design.md
plan: docs/plans/2026-09-27-second-host-rename-adoption.md
---

Why: a checkout synced between hosts (e.g. Dropbox) can be renamed on one host with tasks rename old new. Every other host then holds a state no command handles: its synced tasks/.config.toml and task files already carry new, but its registry still maps old to the (possibly moved) root. Replaying tasks rename old new refuses this state, and repathing alone (tasks init --prefix new --force) leaves the registry's old key and this host's claims and parks keyed by old behind.

Done: one supported migration, under the registry lock and keeping the registry's invariants, makes new the live key at the checkout's current root, records old as its alias (so old ids keep resolving), and carries this host's state-directory records keyed by old (claims, parks), without touching the synced task files. Relocating a root on the renaming host is already covered by tasks init --prefix <p> --force (verified 2026-09-27: aliases kept, old ids resolve), so it is out of scope.

Check: sandboxes with temporary XDG_CONFIG_HOME and XDG_STATE_HOME both (a config-only sandbox still writes claims, parks and rename inventories into the live state directory): one checkout copy, two config/state pairs; rename (and move plus init --force) under the first; adopt under the second; old ids resolve and a parked task keyed by old survives on the second host.

Requested by the rename in ai (spec docs/specs/2026-09-27-rename-to-tack-design.md §3a in that checkout); the CLI shape is this project's call.

## Notes

- 2026-09-27T16:13:09Z (main): started
  provenance: {"harness_session":"codex:01a0e3a3-7f1c-7353-9e4b-461189bb69f1","harness_session_source":"CODEX_SESSION_ID"}
- 2026-09-27T16:15:48Z (feat/tasks-7f1596-adopt-rename): parked (waiting on user, review): User reviews docs/specs/2026-09-27-second-host-rename-adoption-design.md in .worktrees/tasks-7f1596; after approval, write and review the implementation plan
  provenance: {"harness_session":"codex:01a0e3a3-7f1c-7353-9e4b-461189bb69f1","harness_session_source":"CODEX_SESSION_ID"}
- 2026-09-27T16:35:39Z (feat/tasks-7f1596-adopt-rename): resumed
  provenance: {"harness_session":"codex:01a0e3a3-7f1c-7353-9e4b-461189bb69f1","harness_session_source":"CODEX_SESSION_ID"}
- 2026-09-27T16:37:29Z (feat/tasks-7f1596-adopt-rename): Spec review: support partial init --force, preserve orphaned state with warnings, and allow cleanup after normal commands change the new store; plan follows reviewed revision
- 2026-09-27T16:43:22Z (feat/tasks-7f1596-adopt-rename): parked (waiting on user, review): User reviews docs/plans/2026-09-27-second-host-rename-adoption.md in .worktrees/tasks-7f1596; after approval, agent starts tasks-54df5e and executes the reviewed plan
  provenance: {"harness_session":"codex:01a0e3a3-7f1c-7353-9e4b-461189bb69f1","harness_session_source":"CODEX_SESSION_ID"}
- 2026-09-27T16:56:16Z (feat/tasks-7f1596-adopt-rename): resumed
  provenance: {"harness_session":"codex:01a0e3a3-7f1c-7353-9e4b-461189bb69f1","harness_session_source":"CODEX_SESSION_ID"}
- 2026-09-27T16:56:28Z (feat/tasks-7f1596-adopt-rename): Review found pre-registry target-store writes by local new-prefix commands; adoption must merge or accept consumed carried state before registry save and preserve target claims
- 2026-09-27T17:28:37Z (feat/tasks-7f1596-adopt-rename): Execution finding: consuming the only park leaves an empty subset; persist adopted_from marker in target store across local saves so pre-registry retry knows the write happened without resurrecting state
- 2026-09-27T17:45:43Z (feat/tasks-7f1596-adopt-rename): done
  provenance: {"harness_session":"codex:01a0e3a3-7f1c-7353-9e4b-461189bb69f1","harness_session_source":"CODEX_SESSION_ID"}
- 2026-09-27T17:45:43Z (feat/tasks-7f1596-adopt-rename): Second-host rename adoption landed
  provenance: {"harness_session":"codex:01a0e3a3-7f1c-7353-9e4b-461189bb69f1","harness_session_source":"CODEX_SESSION_ID"}
