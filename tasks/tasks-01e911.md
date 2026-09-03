---
id: tasks-01e911
title: Warn on plan headings with no task (unlinked_step)
status: done
priority: 2
size: s
owner: open-items
created: 2026-09-03T13:45:11Z
updated: 2026-09-03T21:51:21Z
depends: []
tags: [hierarchy]
spec: docs/specs/2026-09-03-task-hierarchy-design.md
plan: docs/plans/2026-09-03-task-hierarchy.md
step: "Task 5: Warn on plan headings with no task (unlinked_step)"
---

Hierarchy design §4.5: for every plan linked by at least one task, warn unlinked_step for each heading starting with 'Task <digits>:' that no task references as step. Warning, not error. Original design §7 gains the sentence.

## Notes

- 2026-09-03T21:51:21Z (open-items): check warns unlinked_step for Task N: headings in linked plans that no task references
