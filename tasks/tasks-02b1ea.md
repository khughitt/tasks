---
id: tasks-02b1ea
title: "Pretty tables on comfy-table: every table view, project colours, optional headers, a warnings divider"
status: todo
priority: 2
size: l
complexity: high
process: planned
created: 2026-10-06T13:01:34Z
updated: 2026-10-06T13:01:34Z
depends: []
tags: [pretty]
agent: claude-code/claude-opus-5-5
---

Why: the hand-rolled layout in src/output.rs (fixed paddings, id_width, render_row, wrap_lines, fill_hard_split, TITLE_FLOOR, LongWord) keeps producing warts: a long next: line breaks the parked table, quiet recipes misalign the wait column, and every fix adds a special case. The tasks-8a14be spike (branch spike/comfy-table, c2e0e0e) showed comfy-table 8 lays out the parked table with title and next wrapped side by side in their own columns, within our width and painter colours; the user adopted it.

Scope for the design: move every pretty table view (list, ready, search, list --parked, prime's sections, quiet, projects) onto comfy-table; keep the palette, painter, colour scales and JSON unchanged. Decide per view which columns it keeps; the narrow-terminal policy (a minimum width for the wrapping columns and the fallback below it, e.g. the stacked layout); piped output (no width: no wrapping and no column caps); long words in cells. Fold in: project colours on ids and titles in every table (tasks-9b5b0a, and ids, which the user asked for; the spike also dropped colour); an option to show column headers (a flag now; a config default waits on ops-9122f2); a divider or a small headed box between the output and the warnings.

Where to look: tasks-8a14be notes (costs, gaps), spike commit c2e0e0e, src/output.rs, src/present.rs, src/style.rs, src/main.rs, tasks-380515 and tasks-517584 for the current wrap behaviour, tasks-2578c3 for project colours.
