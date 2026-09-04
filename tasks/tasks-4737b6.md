---
id: tasks-4737b6
title: Add support for color output
status: doing
priority: 2
owner: feat/color
created: 2026-09-03T16:21:11Z
updated: 2026-09-04T01:31:48Z
depends: []
tags: []
spec: docs/specs/2026-09-03-color-output-design.md
---

Add option for color output support for key tasks list, tasks show. Consider modeling after --pretty so that humans can use the color output without forcing it on agents. Default to using terminal color theme (ansi colors 0-16?); centralize styles/coloring logic to ensure that same styles are used across all views.
