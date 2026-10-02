---
id: tasks-9b5b0a
title: list --parked does not paint titles with the project color
status: todo
priority: 4
size: xs
complexity: low
process: direct
created: 2026-10-02T08:13:54Z
updated: 2026-10-02T08:13:54Z
depends: []
tags: []
source: tack-fc26cf
agent: claude-code/claude-opus-5-5
---

Found by a cross-family review experiment (tack-fc26cf) of 75990f0 (tasks-2578c3). parked_table (src/output.rs, render_row with (row.title, None)) passes no color, so 'list --parked' (local and --all-projects) leaves titles unpainted while 'list' and 'list --all-projects' paint them; the README says local list uses the project color with no --parked exception. Done: parked rows paint titles like list does, with a test.
