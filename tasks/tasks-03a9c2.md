---
id: tasks-03a9c2
title: Alias-aware project resolution
status: done
priority: 2
size: xs
owner: design/prefix-rename
created: 2026-09-08T20:53:27Z
updated: 2026-09-08T22:12:09Z
depends: [tasks-590d4d]
parent: tasks-8c9398
tags: [rename]
plan: docs/plans/2026-09-08-prefix-rename.md
step: "Task 2: Alias-aware project resolution"
---

## Notes

- 2026-09-08T22:12:09Z (design/prefix-rename): open_registered follows an alias to its live project, so a retired prefix names the project in root and --project. Its misconfiguration guard now compares against the canonical prefix, so it still catches a registry pointing a name at the wrong root.
