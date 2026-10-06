---
id: tasks-9b5b0a
title: list --parked does not paint titles with the project color
status: done
priority: 4
size: xs
complexity: low
process: direct
owner: feat/comfy-tables
created: 2026-10-02T08:13:54Z
updated: 2026-10-06T15:27:30Z
started: 2026-10-06T15:27:03Z
completed: 2026-10-06T15:27:30Z
depends: [tasks-2ac5d1]
parent: tasks-02b1ea
tags: []
source: tack-fc26cf
agent: claude-code/claude-opus-5-5
---

Found by a cross-family review experiment (tack-fc26cf) of 75990f0 (tasks-2578c3). parked_table (src/output.rs, render_row with (row.title, None)) passes no color, so 'list --parked' (local and --all-projects) leaves titles unpainted while 'list' and 'list --all-projects' paint them; the README says local list uses the project color with no --parked exception. Done: parked rows paint titles like list does, with a test.

## Notes

- 2026-10-06T13:01:34Z (main): folded into tasks-02b1ea (comfy-table migration): parked titles and every table's ids take the project colour there
- 2026-10-06T13:29:53Z (feat/comfy-tables): closes with tasks-9d8e5d (Task 5: parked rows paint ids and titles with the project colour); close it in that task's commit
- 2026-10-06T14:05:20Z (feat/comfy-tables): closes with tasks-2ac5d1 (plan Task 4, after the round 1 revision)
- 2026-10-06T15:27:03Z (feat/comfy-tables): started
  provenance: {"harness_session":"codex:01a111d2-4c61-7041-a1cf-dc43059ccf89","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-06T15:27:30Z (feat/comfy-tables): done
  provenance: {"harness_session":"codex:01a111d2-4c61-7041-a1cf-dc43059ccf89","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-06T15:27:30Z (feat/comfy-tables): Task 4 painted parked IDs/titles; verified just test-one --test cli parked_rows_paint_ids_and_titles_with_the_project_color passes (local and all-projects cases). Closed with Task 5 per controller assignment.
  provenance: {"harness_session":"codex:01a111d2-4c61-7041-a1cf-dc43059ccf89","harness_session_source":"CODEX_THREAD_ID"}
