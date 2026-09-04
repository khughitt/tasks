---
id: tasks-4737b6
title: Add support for color output
status: done
priority: 2
owner: feat/color
created: 2026-09-03T16:21:11Z
updated: 2026-09-04T03:05:04Z
depends: []
tags: []
spec: docs/specs/2026-09-03-color-output-design.md
plan: docs/plans/2026-09-03-color-output.md
---

Add option for color output support for key tasks list, tasks show. Consider modeling after --pretty so that humans can use the color output without forcing it on agents. Default to using terminal color theme (ansi colors 0-16?); centralize styles/coloring logic to ensure that same styles are used across all views.

## Notes

- 2026-09-04T02:24:23Z (feat/color): Implementation plan splits policy/stream plumbing, table-family rendering, and show/docs closeout into three dependent commits.
- 2026-09-04T03:05:04Z (feat/color): implemented opt-in, stream-aware color across pretty output
