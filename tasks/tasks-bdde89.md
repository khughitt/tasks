---
id: tasks-bdde89
title: Refuse a stale local checkout
status: done
priority: 2
size: s
owner: design/prefix-rename
created: 2026-09-08T20:53:27Z
updated: 2026-09-08T22:36:03Z
depends: [tasks-66120e]
parent: tasks-8c9398
tags: [rename]
plan: docs/plans/2026-09-08-prefix-rename.md
step: "Task 4: Refuse a stale local checkout"
---

## Notes

- 2026-09-08T22:36:03Z (design/prefix-rename): Every command checks its local project prefix against the registry and refuses when the registry has retired it. Nothing else saw this: open_registered validates the registered destination and Project::locate never reads the registry.
