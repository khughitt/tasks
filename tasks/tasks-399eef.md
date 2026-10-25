---
id: tasks-399eef
title: show reports task_not_found for an unclaimed worktree-only task without locating its checkout
status: idea
priority: 2
created: 2026-10-24T17:59:59Z
updated: 2026-10-25T16:43:13Z
depends: [tasks-217fbb]
parent: tasks-984769
tags: [feedback, gap, "from:relay"]
agent: codex
---

The user reported that a previously referenced task appeared to have disappeared and asked for its history and prevention. Investigation found the unchanged todo record committed only on an existing linked worktree branch; the registered checkout had never contained it and all-ref path history showed no deletion. The exact task had neither a live claim nor a park, although its parent park named the correct checkout. Plain show returned only task_not_found; show with an explicit checkout succeeded. The current fallback follows only the exact task claim or park, so this is a known visibility limit rather than data loss. Please assess read-only recovery diagnostics for a missing ID using registered git worktrees: name a unique matching checkout and an explicit retry, report ambiguity or inaccessible locations honestly, and preserve write routing and ownership safeguards. Do not require starting or parking untouched work solely to make it discoverable. Coordinate with detached-record migration and the existing record-publication guidance; no automatic copying, merging or runtime activation.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-25-missing-task-recovery-brief.md — current visibility limit and diagnostic decisions
