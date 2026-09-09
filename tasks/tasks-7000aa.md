---
id: tasks-7000aa
title: The `rename` command and its phases
status: done
priority: 2
size: l
owner: design/prefix-rename
created: 2026-09-08T20:53:27Z
updated: 2026-09-09T00:08:18Z
depends: [tasks-425ef7]
parent: tasks-8c9398
tags: [rename]
plan: docs/plans/2026-09-08-prefix-rename.md
step: "Task 11: The `rename` command and its phases"
---

## Notes

- 2026-09-09T00:01:54Z (design/prefix-rename): tasks rename runs six recoverable phases under sorted prefix and registry locks, validates before baseline creation, preserves aliases and replay routing, authorizes every mutating resume, and freezes pending projects and reserved names; --explain classifies read-only.
- 2026-09-09T00:08:18Z (design/prefix-rename): Review fixes: rename reports only destination task files written by this invocation, including zero for explain and cleanup; taken names and registry-invariant refusals retain config error kinds after classification. Fourteen focused tests and the full gate pass.
