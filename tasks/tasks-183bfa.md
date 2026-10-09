---
id: tasks-183bfa
title: Polish task move diagnostics
status: todo
priority: 3
size: m
complexity: mid
process: direct
created: 2026-10-09T08:14:24Z
updated: 2026-10-09T08:14:24Z
depends: [tasks-7ba557]
parent: tasks-a7460c
tags: []
agent: codex
spec: docs/specs/2026-10-08-task-move-design.md
---

Deferred whole-branch review M1–M5: include reachable retired registrations in inbound inventory without changing general wide-read policy; omit moved from edit-status completion and metadata; preserve payload/path/title values in tombstone retry hints; render recovery verdict and changed paths in pretty move output; omit impossible dep repair commands for immutable tombstones. Preserve JSON shapes and pointer/dependency correctness. Each fix needs a focused behavioral regression; current findings are nonblocking diagnostics.
