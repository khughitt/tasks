---
id: tasks-02b1ea
title: "Pretty tables on comfy-table: every table view, project colours, optional headers, a warnings divider"
status: doing
priority: 2
size: l
complexity: high
process: planned
owner: feat/comfy-tables
created: 2026-10-06T13:01:34Z
updated: 2026-10-06T13:13:49Z
started: 2026-10-06T13:01:47Z
depends: []
tags: [pretty]
agent: claude-code/claude-opus-5-5
spec: docs/specs/2026-10-06-comfy-tables-design.md
---

Why: the hand-rolled layout in src/output.rs (fixed paddings, id_width, render_row, wrap_lines, fill_hard_split, TITLE_FLOOR, LongWord) keeps producing warts: a long next: line breaks the parked table, quiet recipes misalign the wait column, and every fix adds a special case. The tasks-8a14be spike (branch spike/comfy-table, c2e0e0e) showed comfy-table 8 lays out the parked table with title and next wrapped side by side in their own columns, within our width and painter colours; the user adopted it.

Scope for the design: move every pretty table view (list, ready, search, list --parked, prime's sections, quiet, projects) onto comfy-table; keep the palette, painter, colour scales and JSON unchanged. Decide per view which columns it keeps; the narrow-terminal policy (a minimum width for the wrapping columns and the fallback below it, e.g. the stacked layout); piped output (no width: no wrapping and no column caps); long words in cells. Fold in: project colours on ids and titles in every table (tasks-9b5b0a, and ids, which the user asked for; the spike also dropped colour); an option to show column headers (a flag now; a config default waits on ops-9122f2); a divider or a small headed box between the output and the warnings.

Where to look: tasks-8a14be notes (costs, gaps), spike commit c2e0e0e, src/output.rs, src/present.rs, src/style.rs, src/main.rs, tasks-380515 and tasks-517584 for the current wrap behaviour, tasks-2578c3 for project colours.

## Notes

- 2026-10-06T13:01:47Z (main): started
  provenance: {"harness_session":"claude-code:5cee487d-b8b1-430e-8b20-7e4be6194daf","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T13:01:48Z (feat/comfy-tables): resumed
  provenance: {"harness_session":"claude-code:5cee487d-b8b1-430e-8b20-7e4be6194daf","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T13:03:57Z (feat/comfy-tables): spec drafted: docs/specs/2026-10-06-comfy-tables-design.md (one table module over comfy-table; per-view columns; narrow fallback; --headers; warnings rule; project colour on ids and titles)
- 2026-10-06T13:04:17Z (feat/comfy-tables): parked (waiting on user, review): User reviews docs/specs/2026-10-06-comfy-tables-design.md (in .worktrees/comfy-tables); on approval the agent writes the implementation plan (ops cli.toml --headers lands first)
  provenance: {"harness_session":"claude-code:5cee487d-b8b1-430e-8b20-7e4be6194daf","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T13:10:45Z (feat/comfy-tables): review: spec round 1 — verdict: revise; findings: P2 3, P3 1; reviewer: codex/model-unreported
- 2026-10-06T13:10:45Z (feat/comfy-tables): Spec review findings: share fixed-column widths across prime sections, not only id_width; populate project_color for ready/tree/prime/lanes as well as list/search; resolve claims colours without requiring checkout access; qualify width assertions for unwrapped fallback, fixed-only tables and Detail::Whole. Spec unchanged; review remains parked on the user.
- 2026-10-06T13:13:48Z (feat/comfy-tables): spec round 1 reviewer was codex/gpt-6.1-sol (named by the user; the round's note says model-unreported)
- 2026-10-06T13:13:48Z (feat/comfy-tables): spec revised for round 1, all four findings verified in code and accepted: shared widths for every fixed column across same-schema prime sections (id only across schemas); one ReadCtx colour helper applied by list, search, ready, tree, prime, lanes and parked rows (ParkedRow gains the only new field); claims exempt from colour; width tests bound lines where wrapping is feasible and assert intact text where overflow is permitted
