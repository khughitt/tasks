---
id: tasks-0d975b
title: "tasks skill: the same-commit rule and task ids in a linked checkout"
status: todo
priority: 2
size: xs
complexity: low
process: direct
created: 2026-10-07T16:48:39Z
updated: 2026-10-07T16:48:39Z
depends: []
parent: tasks-a7460c
tags: [cross-project]
source: tack-956458
agent: claude-code/claude-opus-5-5
---

Piece 7 (tasks-skill half) of ops docs/specs/2026-10-01-linked-checkouts-design.md §4.4, re-homed from tack-956458. The skill's linked-project paragraph (from tasks-9b0a2e) says where records live; add what changes in practice: 'tasks done in the same commit as the code' reads as the matching commit in the overlay repository; commit the record before git worktree add means the overlay commit; task ids never appear in the code repository (commits, branches, pull requests, docs cite the repository's own issue key, e.g. a Linear key via --source).
