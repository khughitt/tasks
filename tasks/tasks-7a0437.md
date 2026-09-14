---
id: tasks-7a0437
title: A task-type column in pretty rows
status: todo
priority: 3
size: s
complexity: low
process: direct
created: 2026-09-12T19:15:37Z
updated: 2026-09-14T11:46:16Z
depends: []
tags: [quick-add, cli]
source: "mindful:thought:6983d7366cc9441fbb72661bd9cc09fe"
---

Pretty rows do not show that a task is periodic beyond the due marker tasks-b097be added. Add a one-letter type column — `p` for periodic, blank otherwise — placed so future types slot in without reshuffling. Color it when colors are on, but the letter carries the meaning so ASCII output stays honest, the constraint tasks-b097be recorded. Add the same `type` to the JSON row so the column is not pretty-only. Rejected: color-only styling — color is off by default and never applies to JSON, so it would be invisible in most output.

Source: mindful:thought:6983d7366cc9441fbb72661bd9cc09fe

## Notes

- 2026-09-14T11:46:16Z (main): Process direct: the body settles the letter, the blank, the always-present column, coloring, and the JSON field, and records the rejected color-only alternative; only bounded placement choices remain.
