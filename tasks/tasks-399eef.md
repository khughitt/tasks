---
id: tasks-399eef
title: show reports task_not_found for an unclaimed worktree-only task without locating its checkout
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-25T16:43:15Z
created: 2026-10-24T17:59:59Z
updated: 2026-10-25T16:43:15Z
depends: [tasks-217fbb]
parent: tasks-984769
tags: [feedback, gap, "from:relay"]
agent: codex
---

The user reported that a previously referenced task appeared to have disappeared and asked for its history and prevention. Investigation found the unchanged todo record committed only on an existing linked worktree branch; the registered checkout had never contained it and all-ref path history showed no deletion. The exact task had neither a live claim nor a park, although its parent park named the correct checkout. Plain show returned only task_not_found; show with an explicit checkout succeeded. The current fallback follows only the exact task claim or park, so this is a known visibility limit rather than data loss. Please assess read-only recovery diagnostics for a missing ID using registered git worktrees: name a unique matching checkout and an explicit retry, report ambiguity or inaccessible locations honestly, and preserve write routing and ownership safeguards. Do not require starting or parking untouched work solely to make it discoverable. Coordinate with detached-record migration and the existing record-publication guidance; no automatic copying, merging or runtime activation.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-25-missing-task-recovery-brief.md — current visibility limit and diagnostic decisions

## Notes

- 2026-10-25T16:43:15Z (main): scope: briefed; code at 220bb14 and record-home sections 6.1/8 confirm exact-task claim/park-only recovery, introduced by bd7cb18; retained original body and origin unknown, grouped under tasks-984769, and recorded a wait on design tasks-217fbb for read-only worktree diagnostics without changing write authority; origin: filing event unavailable (no creation occurrence or harness_session stamp in show); no origin_session supplied, so no older-request stage is identifiable; the body attributes history/prevention to the user but this pass cannot verify or separate that ask from agent additions; brief: docs/notes/2026-10-25-missing-task-recovery-brief.md
  provenance: {"harness_session":"codex:01a173ed-fd7b-7fc3-8372-3ad7eca32e70","harness_session_source":"CODEX_THREAD_ID"}
