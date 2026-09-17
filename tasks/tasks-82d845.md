---
id: tasks-82d845
title: Omit unset fields and empty collections from JSON task records
status: done
priority: 2
size: s
complexity: mid
process: direct
owner: feat/sparse-json
created: 2026-09-17T16:32:56Z
updated: 2026-09-17T16:40:11Z
started: 2026-09-17T16:33:06Z
completed: 2026-09-17T16:40:11Z
depends: []
tags: [cli]
agent: codex
---

User approved sparse JSON: omit null fields and empty task collections; retain false and zero and response containers (tasks/warnings). Apply consistently to task records across list, ready, prime, tree, show, next, and parked output. Update the explicit output contract and consumer-facing documentation, verify consumers and regression coverage. Keep non-task envelope semantics and on-disk records unchanged.

## Notes

- 2026-09-17T16:33:06Z (main): Direct process: user settled sparse JSON policy; use serde field omission on task output types, preserve envelope and storage semantics, and verify shared command paths.
- 2026-09-17T16:33:32Z (feat/sparse-json): took over session sid:1064193 (owner main, host titan, pid 1064193, worktree /mnt/ssd/Dropbox/tasks, since 2026-09-17T16:33:06Z, age 26s, stale: pid 1064193 is gone)
- 2026-09-17T16:34:59Z (feat/sparse-json): Consumer audit found tasks-tui/internal/tasksctl/decode.go requires nullable fields and task arrays. Asked user to authorize companion TUI change; producer work remains scoped, installation waits for compatible consumer.
- 2026-09-17T16:40:11Z (feat/sparse-json): Verified just gate (182 unit and 311 CLI tests), independent review, and TUI integration against sparse binary. User-authorized companion tui-bbcb55 updates strict consumer decoding.
- 2026-09-17T16:40:11Z (feat/sparse-json): Sparse task JSON omits unset fields and empty collections consistently; contract, README, skill and regression assertions updated; required scalars and envelopes preserved.
