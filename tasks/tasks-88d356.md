---
id: tasks-88d356
title: "tree <id> does not resolve an id from another registered project even though dep, note, and show do"
status: idea
priority: 2
created: 2026-09-07T00:07:50Z
updated: 2026-09-07T09:30:50Z
depends: []
tags: [feedback, friction, "from:tasks"]
---

tasks tree <other-prefix>-<hex> from a different project returned task_not_found; the same id worked with tasks dep and tasks show from the same directory. Expected tree to route by prefix like the other id-taking commands.

## Notes

- 2026-09-07T09:30:50Z (feat/read-scope-project): Still open after tasks-7eb169: --project <prefix> gives tree an explicit read scope (tasks tree --project fam <id>), but a bare foreign id in tree is still not routed by its prefix.
