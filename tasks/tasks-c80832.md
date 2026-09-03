---
id: tasks-c80832
title: Add the parent field with validation and cycle check
status: todo
priority: 1
size: m
created: 2026-09-03T13:45:11Z
updated: 2026-09-03T13:45:11Z
depends: []
tags: [hierarchy]
spec: docs/specs/2026-09-03-task-hierarchy-design.md
---

Hierarchy design §2-§4.1: parent in the model, frontmatter (after depends), serializer order; add --parent, edit --parent/--no-parent, editor path; reject foreign prefix (validation), missing parent (unresolvable_id), self/ancestor cycles (cycle). check reports dangling_parent, foreign_parent, parent_cycle as errors. JSON: Task += parent. Tests per §8 for this slice.
