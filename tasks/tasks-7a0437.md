---
id: tasks-7a0437
title: A task-type column in pretty rows
status: todo
priority: 3
size: s
complexity: low
process: direct
created: 2026-09-12T19:15:37Z
updated: 2026-09-24T13:35:22Z
depends: []
tags: [quick-add, cli]
source: "mindful:thought:6983d7366cc9441fbb72661bd9cc09fe"
---

Pretty rows do not show that a task is periodic beyond the due marker tasks-b097be added. Add a one-letter type column — `p` for periodic, blank otherwise — placed so future types slot in without reshuffling. Color it when colors are on, but the letter carries the meaning so ASCII output stays honest, the constraint tasks-b097be recorded. Rejected: color-only styling — color is off by default and never applies to JSON, so it would be invisible in most output.

The column reads the summary row's existing `periodic` object; the JSON shape does not change. Like the parallel marker (`||`, 8b6f66e, `any_parallel` in src/output.rs), it is reserved only when some row in the output needs it, decided once per output so siblings stay aligned. Rejected: an always-present column, blank on nearly every row since few tasks recur; and a derived JSON `type` field, redundant with `periodic`.

Source: mindful:thought:6983d7366cc9441fbb72661bd9cc09fe

## Notes

- 2026-09-14T11:46:16Z (main): Process direct: the body settles the letter, the blank, the always-present column, coloring, and the JSON field, and records the rejected color-only alternative; only bounded placement choices remain.
- 2026-09-24T13:07:13Z (main): curate: decision; body names the parallel-marker precedent and two open questions; proposal: drop the JSON type field (periodic already carries it) and decide always-present vs reserved-when-needed for the column
- 2026-09-24T13:35:22Z (main): Decided: the column reads periodic and the JSON shape stays unchanged (user); reserved only when a row needs it, following the parallel marker (agent, user delegated).
