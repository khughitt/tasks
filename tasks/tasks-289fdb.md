---
id: tasks-289fdb
title: "edit --tag replaces the whole tag list, so adding one tag during triage silently drops provenance tags"
status: done
priority: 2
owner: feat/cross-project-writes
created: 2026-09-04T21:25:29Z
updated: 2026-09-05T21:02:53Z
depends: []
tags: [feedback, friction, "from:tasks"]
---

tasks edit <id> --tag cli on a task tagged [feedback, gap, from:<p>] left it tagged [cli]. Expected either append semantics for a repeated flag on edit, or a warning when tags are dropped. Nothing said the list was replaced.

## Notes

- 2026-09-05T21:02:53Z (feat/cross-project-writes): edit --tag appends; --rm-tag removes one, --no-tags clears
