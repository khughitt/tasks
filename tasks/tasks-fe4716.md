---
id: tasks-fe4716
title: Rework the projects output into a scannable table
status: done
priority: 2
size: m
created: 2026-09-06T10:13:28Z
updated: 2026-09-06T10:48:55Z
depends: []
tags: [cli, output]
---

`tasks projects --pretty` prints one unaligned line per project, repeating the status labels on every row and leading with a long absolute path. Rework it into a scannable table.

Agreed shape:

    project   idea  todo  doing  blocked  total  activity
    atoms        0     4      1        0      9  2026-09-01
    autonomy     0     0      0        0      0  -
    beliefs      5    17      0        0    123  2026-09-04

- Default columns: idea, todo, doing, blocked, total, activity. `total` counts every task including done and dropped - it is the project's size and the sort key, so it deliberately exceeds the sum of the visible columns.
- `--closed` inserts done and dropped. `--paths` appends the registered root as the last column; it is hidden by default.
- Header row in Style::Chrome. Each count cell in its Style::Status color, the same colors `list`, `show`, and `tree` already use. Zeros dimmed to Chrome regardless of column, so a red 0 under blocked does not shout.
- Unreachable rows show `-` in every cell plus the existing Error-painted marker.

Dividing rule for where an option lives: sorting is command-level because it reorders the JSON array too; column visibility is pretty-only because JSON always carries every field. JSON is the contract.

`prime` prints a byte-identical `idea N todo N ...` line, hand-written a second time in output.rs. One counts renderer serves both, so the column set and colors cannot drift, and `prime` honors `--closed` the same way.

## Notes

- 2026-09-06T10:48:55Z (projects-table): projects prints an aligned, colored, sortable table: header row, open statuses plus total and activity, --closed/--paths to widen, --sort/--reverse at the command level; prime shares the counts columns
