---
id: tasks-8a14be
title: "Spike: can comfy-table render the parked table (side-by-side wrapped title and next, painted cells, our width)?"
status: done
priority: 2
size: s
complexity: mid
process: direct
owner: spike/comfy-table
created: 2026-10-06T12:46:30Z
updated: 2026-10-06T13:00:59Z
started: 2026-10-06T12:46:30Z
completed: 2026-10-06T13:00:57Z
depends: []
tags: [pretty]
model: claude-opus-5-5
agent: claude-code/claude-opus-5-5
---

Question: Can comfy-table replace the hand-rolled layout for pretty tables, starting with parked_table?
Where to start: src/output.rs parked_table, render_row, wrap_lines, Wrap, id_width; src/style.rs Painter; src/main.rs wrap width (COLUMNS over the tty); comfy-table 8 with custom_styling, preset NOTHING, ContentArrangement::Dynamic, set_width, ColumnConstraint.
Bound: a throwaway branch that ports parked_table only (row: id, status, wait reason, parked date, title, next; title and next wrap side by side in their own columns), driven by our Wrap width and our Painter colours. No other view, no merge.
Expected result: a note on this task and the user's verdict on three checks: (1) title and next wrap within their columns at 120/163/236 columns, nothing cut; (2) ANSI-painted cells keep alignment under --color=always; (3) the width is ours (COLUMNS, then the tty), and no line exceeds it. Also record the dependency cost (crates added, release binary size delta, build time) and anything comfy-table cannot express that the current renderer does (Wrap::NONE one-line rows for pipes, tree indent, long-word handling). Recommend adopt (and file a planned migration of the table views, folding in tasks-9b5b0a and id colours) or not.
Ideas it wakes: tasks-9b5b0a (parked titles painted with the project colour).

## Notes

- 2026-10-06T12:46:30Z (main): started
  provenance: {"harness_session":"claude-code:5cee487d-b8b1-430e-8b20-7e4be6194daf","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T12:46:31Z (spike/comfy-table): resumed
  provenance: {"harness_session":"claude-code:5cee487d-b8b1-430e-8b20-7e4be6194daf","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T12:51:19Z (spike/comfy-table): spike findings (branch spike/comfy-table, not for merge): (1) title and next wrap side by side in their own columns at 120/163/236, nothing cut; long words (paths) are hard-split inside a cell. (2) under --color=always every line stays within the width and the title column starts at the same offset on every row; comfy-table re-paints each wrapped word separately (correct, more bytes). (3) width is ours via set_width + force_no_tty; Wrap::NONE maps to ContentArrangement::Disabled, but column caps must then be left unset (the spike set them, so piped titles still wrapped at 50). Gaps the migration must design: a minimum next width and a fallback below it (at 120 the fixed columns take 67 and next gets ~27); which columns each view keeps. Cost: +20 crates (crossterm, console, ansi-str/nom; Windows shims unbuilt), release binary 5.66 MB to 5.88 MB (+3.8%), clean release build 30 s to 37 s. Churn: porting this one view failed 6 tests, all asserting the dropped process column, the 'waits on' wording or the next: label, none a comfy-table limitation. Recommendation: adopt; file a planned migration of the table views folding in tasks-9b5b0a and id colours.
- 2026-10-06T12:51:19Z (spike/comfy-table): parked (waiting on user, review): User judges the spike output (the three checks and the 120-column fallback gap); on adopt, agent files the planned table-view migration (spec first) with tasks-9b5b0a and id colours folded in, and drops the spike branch
  provenance: {"harness_session":"claude-code:5cee487d-b8b1-430e-8b20-7e4be6194daf","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T13:00:57Z (spike/comfy-table): resumed
  provenance: {"harness_session":"claude-code:5cee487d-b8b1-430e-8b20-7e4be6194daf","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T13:00:57Z (spike/comfy-table): verdict: adopt (user, 2026-10-06). Asked alongside: colours in the new tables (the spike lost project/recency colour), an option to show column headers (default via a tasks config file once ops settles cross-project config conventions, ops-9122f2), and a divider or a headed box between the table and the warnings.
- 2026-10-06T13:00:57Z (spike/comfy-table): done
  provenance: {"harness_session":"claude-code:5cee487d-b8b1-430e-8b20-7e4be6194daf","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T13:00:57Z (spike/comfy-table): spike answered: comfy-table renders the parked table side by side within our width and colours; adopt, migration filed
  provenance: {"harness_session":"claude-code:5cee487d-b8b1-430e-8b20-7e4be6194daf","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
