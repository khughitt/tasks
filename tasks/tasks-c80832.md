---
id: tasks-c80832
title: Add the parent field with validation and cycle check
status: done
priority: 1
size: m
owner: open-items
created: 2026-09-03T13:45:11Z
updated: 2026-09-03T21:12:07Z
depends: []
tags: [hierarchy]
spec: docs/specs/2026-09-03-task-hierarchy-design.md
plan: docs/plans/2026-09-03-task-hierarchy.md
step: "Task 1: Add the parent field with validation and cycle check"
---

Hierarchy design §2-§4.1: parent in the model, frontmatter (after depends), serializer order; add --parent, edit --parent/--no-parent, editor path; reject foreign prefix (validation), missing parent (unresolvable_id), self/ancestor cycles (cycle). check reports dangling_parent, foreign_parent, parent_cycle as errors. JSON: Task += parent. Tests per §8 for this slice.

## Notes

- 2026-09-03T21:12:07Z (open-items): parent field with prefix/existence/cycle validation on add, edit, and editor path; check reports dangling_parent, foreign_parent, parent_cycle
