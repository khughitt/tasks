---
id: tasks-2f9022
title: "resolve maps paths under a linked checkout's .worktrees storage, as it does for registered roots"
status: doing
priority: 2
size: s
complexity: mid
process: direct
owner: fix/linked-storage
created: 2026-10-07T16:47:08Z
updated: 2026-10-07T17:15:33Z
started: 2026-10-07T17:13:38Z
depends: []
parent: tasks-a7460c
tags: [cross-project]
agent: claude-code/claude-opus-5-5
---

tasks resolve answers a path under a registered root's .worktrees storage (via storage) even after the worktree is removed, but for a linked project it indexes only the checkout itself (via checkout) and live worktrees (via git). Measured 2026-10-07 in a scratch registry: <checkout>/.worktrees/gone and <storage>/gone resolve as unregistered for a linked project, while the same layout in an in-tree project resolves via storage. obs places sessions through tasks resolve (obs-ff4e76), so sessions recorded in a since-removed worktree of a linked checkout (nexcode's normal case) would lose their project. Done: the linked checkout's .worktrees link target is a storage location of that prefix (recorded like a root's storage, so it survives the link's removal), with a test of both the logical and physical path of a removed worktree.

## Notes

- 2026-10-07T16:47:17Z (main): concerns: tasks-7580d2 extension — resolve indexes a linked checkout but not its .worktrees storage, so removed linked worktrees resolve as unregistered
- 2026-10-07T17:13:38Z (main): started
  provenance: {"harness_session":"claude-code:cd13df6e-7206-46e3-bd3f-30be99c37559","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-07T17:13:39Z (fix/linked-storage): resumed
  provenance: {"harness_session":"claude-code:cd13df6e-7206-46e3-bd3f-30be99c37559","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-07T17:15:33Z (fix/linked-storage): bounded choice: the linked checkout's storage is read from its live .worktrees link only, not persisted in [locations] (the registry's storage record describes the root; a record for the checkout would be a schema change). A removed worktree resolves while the link stands, which is nexcode's normal case; a removed link loses it, with no warning.
