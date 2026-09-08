---
id: tasks-6725e3
title: `unregister` and `init` honour aliases
status: done
priority: 2
size: s
owner: design/prefix-rename
created: 2026-09-08T20:53:27Z
updated: 2026-09-08T22:52:09Z
depends: [tasks-52ffa3]
parent: tasks-8c9398
tags: [rename]
plan: docs/plans/2026-09-08-prefix-rename.md
step: "Task 6: `unregister` and `init` honour aliases"
---

## Notes

- 2026-09-08T22:48:46Z (design/prefix-rename): took over session sid:579930 (owner design/prefix-rename, host titan, pid 579930, worktree /mnt/ssd/Dropbox/tasks/.worktrees/rename, since 2026-09-08T22:47:29Z, age 77s, stale: pid 579930 is gone)
- 2026-09-08T22:52:09Z (design/prefix-rename): unregister removes a project with every alias targeting it and names them in its output; unregistering an alias is refused with a pointer to the live name; init refuses a prefix taken as either a live prefix or an alias.
