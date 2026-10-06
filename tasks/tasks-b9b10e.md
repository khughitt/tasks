---
id: tasks-b9b10e
title: Determine why a closed task retains its worktree park
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-06T09:45:48Z
updated: 2026-10-06T09:45:48Z
depends: []
parent: tasks-984769
tags: []
source: tasks-6b2867
agent: codex
---

Question: Can current done/drop leave a park after closing from another checkout, and how can an existing closed park be recovered safely?

Where to start: docs/notes/2026-10-06-record-continuity-brief.md; src/commands/mod.rs::transition/claim_guard/save, ClaimIntent::Release clear_park, src/commands/parked.rs::rows, src/commands/list.rs::prime and src/claims.rs::remove_park.

Bound: one disposable git main/worktree pair and isolated config/state; park in the worktree, integrate the record, close in main with the same and a distinct explicit session, inspect show/prime/list --parked and the scratch store, then remove the scratch worktree through git and retry existing recovery commands. Use CLI/git operations, never edit a live store. Record binary versions, cleanup warnings and whether the closed record arrived by done or git integration. No product code or new cleanup command.

Expected result: distinguish successful cleanup, failed cleanup and a park paired with an already-closed merged record; recommend the smallest safe recovery and regression check, preserving valid parks and concurrent work; update the brief.

Ideas it wakes: on completion add the finding to tasks-6b2867 with tasks note in the same commit, update the brief, then rerun scope on them.
