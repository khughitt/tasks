---
id: tasks-31ec63
title: Curation sweep
status: todo
priority: 2
complexity: high
every: 30d
created: 2026-09-10T02:50:12Z
updated: 2026-09-13T16:47:41Z
depends: []
tags: [curate]
spec: docs/specs/2026-09-08-task-curation-design.md
---

Run the curate skill for a bounded task-corpus maintenance pass, then record what changed and close this occurrence.

## Notes

- 2026-09-13T16:47:41Z (main): Complexity high: the curate procedure bounds edits, but each random draw requires fresh evidence across code, history, and linked designs to judge staleness, duplicates, or missing scope; the recurring task cannot assume an easy sample.
