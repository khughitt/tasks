---
id: tasks-31ec63
title: Curation sweep
status: done
priority: 2
complexity: high
process: direct
every: 30d
owner: main
created: 2026-09-10T02:50:12Z
updated: 2026-09-24T13:07:18Z
started: 2026-09-24T13:05:22Z
completed: 2026-09-24T13:07:18Z
last_done: 2026-09-24T13:07:18Z
depends: []
tags: [curate]
spec: docs/specs/2026-09-08-task-curation-design.md
---

Run the curate skill for a bounded task-corpus maintenance pass, then record what changed and close this occurrence.

## Notes

- 2026-09-13T16:47:41Z (main): Complexity high: the curate procedure bounds edits, but each random draw requires fresh evidence across code, history, and linked designs to judge staleness, duplicates, or missing scope; the recurring task cannot assume an easy sample.
- 2026-09-24T13:05:22Z (main): Process direct: the curate skill settles the procedure, edit bounds, and audit trail; record-only work in the main checkout.
- 2026-09-24T13:05:22Z (main): started
  provenance: {"harness_session":"claude-code:c3b66540-0acc-4861-917d-8c10dfaea3bd","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-24T13:07:18Z (main): completed; next due 2026-10-24
  provenance: {"harness_session":"claude-code:c3b66540-0acc-4861-917d-8c10dfaea3bd","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-24T13:07:18Z (main): Curated tasks-f17488 (stale: spec_dirs already covers it), tasks-7a0437 (decision: JSON type field and column presence), tasks-9a9ef3 (refined: current word count, size s, complexity mid)
  provenance: {"harness_session":"claude-code:c3b66540-0acc-4861-917d-8c10dfaea3bd","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
