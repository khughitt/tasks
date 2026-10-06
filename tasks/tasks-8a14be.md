---
id: tasks-8a14be
title: "Spike: can comfy-table render the parked table (side-by-side wrapped title and next, painted cells, our width)?"
status: doing
priority: 2
size: s
complexity: mid
process: direct
owner: main
created: 2026-10-06T12:46:30Z
updated: 2026-10-06T12:46:31Z
started: 2026-10-06T12:46:30Z
depends: []
tags: [pretty]
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
