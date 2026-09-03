---
id: tasks-fb75b5
title: Add roadmap and closeout to prime
status: todo
priority: 1
size: s
created: 2026-09-03T13:45:11Z
updated: 2026-09-03T13:45:11Z
depends: [tasks-afe69c]
tags: [hierarchy]
spec: docs/specs/2026-09-03-task-hierarchy-design.md
---

Hierarchy design §4.3 and §6: prime += roadmap [TreeNode] (the pruned open forest, same as tree) and closeout [TaskSummary] (open tasks with >= 1 child and no open descendant, ready order). Pretty: closeout: and roadmap: sections above ready:; roadmap prints subtrees of roots with children and one count line for childless roots.
