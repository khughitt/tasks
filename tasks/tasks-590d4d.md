---
id: tasks-590d4d
title: Registry aliases and canonicalization
status: done
priority: 2
size: s
owner: design/prefix-rename
created: 2026-09-08T20:53:27Z
updated: 2026-09-08T22:08:05Z
depends: []
parent: tasks-8c9398
tags: [rename]
plan: docs/plans/2026-09-08-prefix-rename.md
step: "Task 1: Registry aliases and canonicalization"
---

## Notes

- 2026-09-08T22:08:05Z (design/prefix-rename): Registry gains an aliases table with load-time invariants (every target live, no collision with a live prefix), plus canonical_prefix, canonical_id and is_taken. Purely additive: no caller changed.
