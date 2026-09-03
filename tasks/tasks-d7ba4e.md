---
id: tasks-d7ba4e
title: "Add the feedback command: locate reporter and target, create the idea"
status: todo
priority: 1
size: m
created: 2026-09-03T13:45:11Z
updated: 2026-09-03T14:22:57Z
depends: []
tags: [feedback, cli]
spec: docs/specs/2026-09-03-feedback-design.md
plan: docs/plans/2026-09-03-feedback.md
step: "Task 1: Add the feedback command: locate reporter and target, create the idea"
---

Feedback design §3 steps 1, 2, 4 (create branch) and §5: reporter prefix from the current project (no_project outside one); target is the registry entry with prefix tasks, config error otherwise; create an idea in the target with title, body, tags feedback/<category>/from:<prefix>; output { id, action: created, path, warnings } with the uncommitted-file warning. --category required among friction|gap|idea|positive. Tests per §8 for creation, target errors, and from:tasks.
