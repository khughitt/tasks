---
id: tasks-7580d2
title: Record a project's former roots and resolve ids and paths through renames
status: doing
priority: 1
size: m
complexity: high
process: planned
owner: design/tasks-7580d2
created: 2026-10-06T13:23:04Z
updated: 2026-10-06T18:48:17Z
started: 2026-10-06T18:41:11Z
depends: []
parent: tasks-a7460c
tags: []
source: tack-8b7a28
agent: claude-code/claude-fable-5-1
spec: docs/specs/2026-10-06-project-resolution-design.md
---

Why: a prefix rename keeps aliases, but once it completes nothing on the host records the directory the project used to live in (the rename state directory is empty afterwards), and each consumer that maps an id or a path to a project reads [projects] by hand: obs's indexer and join, flows' trial tools, tack's session-episodes. So a rename drops their history (tack docs/specs/2026-10-06-rename-to-hq-design.md §3.5, §3.6; obs-ff4e76, flows-44890e). The user asked on 2026-10-06 that renames be made smoother in general before tack's rename to hq. Done: the registry records each former root when rename, init --prefix --force or rename --adopt changes a root or a prefix, per host; one read-only resolver answers the canonical id for any id form (alias prefixes included) and the project for any path (current root, former roots, and the worktree storage of each); its output is stable JSON a hook or an indexer can call; a host that adopts a rename made elsewhere ends with the same answers; and the existing ai alias gets its former root backfilled or is documented as unknown. This task blocks the cutover (tack-8b7a28) and is the mechanism obs-ff4e76 and flows-44890e are asked to use.

Lane coordination and acceptance: during design, compare the proposed id/path resolver with tasks-9b0a2e's tasks where and locate contracts, distinguishing records root, linked code checkout, and worktree storage. Share registry identity and path classification rather than making consumers read independent registry formats. Agree the boundary before implementation; the linked-checkout feature need not land before former-root support.

Cover canonical and aliased ids, current/former roots and their worktree storage, unregistered or ambiguous paths, and adoption on another host with that host's own storage paths. Record explicit limits where former-root history is unavailable. The rename consumers keep their own historical task/trial identity contract and end-to-end checks; this task supplies the shared resolver rather than taking over their delivery. Preserve the existing written-design and implementation-plan review gates.

## Notes

- 2026-10-06T16:52:01Z (main): organization: first milestone of the P1 project-location lane; coordinate the resolver boundary with tasks-9b0a2e and the existing rename consumers, without making linked-checkout delivery a prerequisite.
- 2026-10-06T18:41:11Z (main): started
  provenance: {"harness_session":"claude-code:00da167b-561d-40c6-aa34-7a035e885cf2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T18:41:20Z (design/tasks-7580d2): resumed
  provenance: {"harness_session":"claude-code:00da167b-561d-40c6-aa34-7a035e885cf2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T18:48:16Z (design/tasks-7580d2): design: separate read-only 'tasks resolve' (registry history, existence-independent) beside 9b0a2e's live 'where', sharing a src/locations.rs index; per-host [locations] table records storage and former roots; rejected inferring storage from the work-link layout
