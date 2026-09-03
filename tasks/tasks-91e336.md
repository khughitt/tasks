---
id: tasks-91e336
title: "Open-descendant counting, closing rules, and ready excludes parents"
status: done
priority: 1
size: m
owner: open-items
created: 2026-09-03T13:45:11Z
updated: 2026-09-03T21:21:02Z
depends: [tasks-c80832]
tags: [hierarchy]
spec: docs/specs/2026-09-03-task-hierarchy-design.md
plan: docs/plans/2026-09-03-task-hierarchy.md
step: "Task 2: Open-descendant counting, closing rules, and ready excludes parents"
---

Hierarchy design §2.2, §4.2, §4.3 (ready only): subtree walk for open descendants; done refuses with an open descendant unless --force; drop refuses with no override; ready = todo, deps closed, no children. TaskSummary += parent, child_count, open_descendant_count. check warns open_child_of_closed_parent. Unit test the force-closed middle node case.

## Notes

- 2026-09-03T21:21:02Z (open-items): descendant walk; done/drop refuse with open descendants (done --force overrides); ready is leaves only; TaskSummary gains parent, child_count, open_descendant_count; check warns open_child_of_closed_parent
