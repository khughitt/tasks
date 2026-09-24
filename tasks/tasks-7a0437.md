---
id: tasks-7a0437
title: A task-type column in pretty rows
status: todo
priority: 3
size: s
complexity: low
process: direct
created: 2026-09-12T19:15:37Z
updated: 2026-09-24T13:07:13Z
depends: []
tags: [quick-add, cli]
source: "mindful:thought:6983d7366cc9441fbb72661bd9cc09fe"
---

Pretty rows do not show that a task is periodic beyond the due marker tasks-b097be added. Add a one-letter type column — `p` for periodic, blank otherwise — placed so future types slot in without reshuffling. Color it when colors are on, but the letter carries the meaning so ASCII output stays honest, the constraint tasks-b097be recorded. Add the same `type` to the JSON row so the column is not pretty-only. Rejected: color-only styling — color is off by default and never applies to JSON, so it would be invisible in most output.

The parallel marker (`||`, 8b6f66e) already occupies a column just before the date, reserved only when some row in the output is marked; `table` in src/output.rs builds both.

## Open questions

- JSON summary rows already carry a `periodic` object on every recurring task (sparse records since e083c9c). Is a derived `type` field still wanted, or does the pretty column read `periodic` and the JSON shape stay unchanged?
- Always-present column (the scoping note) or reserved only when a row needs it, like the parallel marker?

Source: mindful:thought:6983d7366cc9441fbb72661bd9cc09fe

## Notes

- 2026-09-14T11:46:16Z (main): Process direct: the body settles the letter, the blank, the always-present column, coloring, and the JSON field, and records the rejected color-only alternative; only bounded placement choices remain.
- 2026-09-24T13:07:13Z (main): curate: decision; body names the parallel-marker precedent and two open questions; proposal: drop the JSON type field (periodic already carries it) and decide always-present vs reserved-when-needed for the column
