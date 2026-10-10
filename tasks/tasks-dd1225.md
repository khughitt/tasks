---
id: tasks-dd1225
title: Point record-commit recovery warnings at the affected project roots
status: todo
priority: 3
size: s
complexity: mid
process: direct
created: 2026-10-10T17:51:21Z
updated: 2026-10-10T17:51:21Z
depends: []
tags: [cli]
source: "tasks-6939f6:final-review:M1"
agent: codex
---

Deferred final-review Minor M1 from tasks-6939f6: automatic commit grouping correctly runs at the Git working-tree top level, but its warning tells a nested or linked project to run tasks commit there, which can select no project or the wrong one. Keep one commit per working tree; render explicit recovery commands against each affected project root while preserving the actual worktree route. Cover a nested project and a shared-repository write.
