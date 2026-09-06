---
id: tasks-1dba40
title: "Render projects as an aligned, colored column table"
status: done
priority: 2
size: m
owner: projects-table
created: 2026-09-06T10:13:57Z
updated: 2026-09-06T10:45:34Z
depends: [tasks-a52e0f]
parent: tasks-fe4716
tags: [cli, output]
---

Replace the hand-rolled line in output.rs with a header row and aligned columns, per the shape in the parent.

Two bugs to kill: only `prefix` is padded today, so `root` and the counts wander; and the status labels repeat on all fifteen rows instead of sitting in one header.

Also extract the counts rendering. `prime` hand-writes the identical `idea N todo N ...` string a second time in the same match. One renderer takes a &Counts plus the column set and returns painted, padded cells, so `projects` and `prime` cannot drift and `prime` picks up `--closed` for free.

Watch the existing invariant: pad first, paint last. ANSI bytes count toward `{:<n}` widths, which is why `table()` formats to final visible width before wrapping in a painter call. Whatever shared helper this grows must keep that ordering or every colored column silently misaligns - a test with `--color always` asserting on byte-exact rows is the cheap guard.

Flags here are pretty-only: `--closed` adds done and dropped, `--paths` appends the root column. JSON keeps every field either way.

## Notes

- 2026-09-06T10:45:34Z (projects-table): projects renders an aligned header-row table with per-status colors, zeros dimmed, --closed and --paths; count_columns is one definition shared with prime, which gains --closed
