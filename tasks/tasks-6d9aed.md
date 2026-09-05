---
id: tasks-6d9aed
title: "list: show a date column and add --sort"
status: todo
priority: 2
size: s
created: 2026-09-05T10:52:21Z
updated: 2026-09-05T10:52:21Z
depends: []
tags: [cli]
---

Two extensions to tasks list. (1) Print a date on each row; the default is the date of last activity (updated). (2) A --sort flag choosing the order (priority is today's fixed order: priority, updated desc, id). JSON already carries updated on every summary; the date column is a --pretty change, and any new JSON field (created) is additive. Open design points at creation: compact date format for pretty output (mon+yy like sep26 vs ISO day 2026-09-05), whether the column appears on every pretty row (ready, prime, tree share the renderer) or list only, and whether --sort created also switches the printed date to created.
