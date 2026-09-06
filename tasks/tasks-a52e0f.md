---
id: tasks-a52e0f
title: Add total and last_activity to the projects JSON row
status: done
priority: 2
size: xs
owner: projects-table
created: 2026-09-06T10:13:39Z
updated: 2026-09-06T10:26:02Z
depends: []
parent: tasks-fe4716
tags: [cli, output]
---

ProjectRow carries prefix, root, reachable, and counts. The table needs two more fields, and both are JSON contract changes, so they land first and alone.

- `total`: every task in the project regardless of status. Redundant with the sum of Counts, but it is the sort key and the displayed column, and computing it once in the row beats every consumer re-summing six fields.
- `last_activity`: the maximum `updated` across all tasks, closed ones included - closing a task is activity. Null for a project with no tasks (autonomy today) and for an unreachable one; pretty renders that as `-`.

Full RFC3339 in JSON, same as every other timestamp; `time::day` trims it to YYYY-MM-DD at the pretty layer, the way `table()` already does for list rows.

Unblocks the activity column and the activity sort key. No rendering changes here - the existing one-line-per-project output keeps working, minus the new fields.

## Notes

- 2026-09-06T10:26:02Z (projects-table): ProjectRow carries total (every task, absent when unreachable) and last_activity (max updated including closed tasks, absent when nothing was scanned or the project is empty)
