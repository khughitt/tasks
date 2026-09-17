---
id: tasks-82d845
title: Omit unset fields and empty collections from JSON task records
status: doing
priority: 2
size: s
complexity: mid
process: direct
owner: main
created: 2026-09-17T16:32:56Z
updated: 2026-09-17T16:33:06Z
started: 2026-09-17T16:33:06Z
depends: []
tags: [cli]
agent: codex
---

User approved sparse JSON: omit null fields and empty task collections; retain false and zero and response containers (tasks/warnings). Apply consistently to task records across list, ready, prime, tree, show, next, and parked output. Update the explicit output contract and consumer-facing documentation, verify consumers and regression coverage. Keep non-task envelope semantics and on-disk records unchanged.

## Notes

- 2026-09-17T16:33:06Z (main): Direct process: user settled sparse JSON policy; use serde field omission on task output types, preserve envelope and storage semantics, and verify shared command paths.
