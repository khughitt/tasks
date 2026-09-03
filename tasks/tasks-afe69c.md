---
id: tasks-afe69c
title: "Add tree command, show parent/children, list --parent"
status: todo
priority: 1
size: m
created: 2026-09-03T13:45:11Z
updated: 2026-09-03T14:22:57Z
depends: [tasks-91e336]
tags: [hierarchy]
spec: docs/specs/2026-09-03-task-hierarchy-design.md
plan: docs/plans/2026-09-03-task-hierarchy.md
step: "Task 3: Add tree command, show parent/children, list --parent"
---

Hierarchy design §4.4 and §6: tasks tree [<id>] [--all] with TreeNode = TaskSummary + children, pruned to nodes that are open or have an open descendant unless --all, ready order for roots and siblings; show += parent {id,title,status}|null and children [{id,title,status}]; list --parent ID. Pretty tree as an indented list.
