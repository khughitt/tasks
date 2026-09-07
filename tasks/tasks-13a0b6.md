---
id: tasks-13a0b6
title: "Task source field: opaque origin reference set at add, editable, in JSON"
status: todo
priority: 2
size: s
created: 2026-09-06T21:51:32Z
updated: 2026-09-07T01:02:28Z
depends: []
tags: [quick-add, capture, cross-project]
spec: docs/specs/2026-09-06-task-source-design.md
plan: docs/plans/2026-09-06-task-source.md
---

Section 3 of the quick-add design (ops docs/specs/2026-09-06-quick-add-design.md, goal ops-a46c09). Add an optional single-line, non-empty source string to the task record: frontmatter between tags and spec, JSON field null when absent (authorized contract change), add --source, edit --source / --no-source, the field table in the 2026-08-29 design spec, and the shipped skill. Flag completion is clap-derived; verify only. tasks never interprets the value; the documented convention is a scheme prefix such as mindful:<id>. No filter flag yet.

## Notes

- 2026-09-06T22:41:11Z (main): Defer the Google Takeout path: earlier plan and the 2026-08-30 Takeout dataset live in the keep repo's keep-import worktree; revisit after paste + interactive quick add
