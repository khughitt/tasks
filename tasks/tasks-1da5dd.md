---
id: tasks-1da5dd
title: Fit warning dividers to the width for non-table outputs
status: done
priority: 2
size: xs
complexity: low
process: direct
owner: feat/comfy-tables
created: 2026-10-06T15:55:42Z
updated: 2026-10-06T15:58:54Z
started: 2026-10-06T15:56:13Z
completed: 2026-10-06T15:58:54Z
depends: []
parent: tasks-02b1ea
tags: [pretty]
agent: codex
---

Final whole-branch review Minor finding: main resolves width only for tables/show/next, so pretty warnings from non-table outputs use a 100-column divider despite valid COLUMNS=80. Resolve pretty warning width too, preserve invalid-width warning/error policy and avoid silent fallback, add a focused non-table warning-width regression. Final corrective work before integration, not a post-close concern.

## Notes

- 2026-10-06T15:56:13Z (feat/comfy-tables): started
  provenance: {"harness_session":"codex:01a111ed-c042-7550-b6d2-36e2b2115278","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-06T15:58:44Z (feat/comfy-tables): Resolve pretty width once before dispatch; reject invalid width before writers, defer read errors so warnings remain first. Controller accepted the broader invalid-COLUMNS validation for pretty non-table commands, including clean outputs; graph layout and JSON unchanged.
- 2026-10-06T15:58:54Z (feat/comfy-tables): done
  provenance: {"harness_session":"codex:01a111ed-c042-7550-b6d2-36e2b2115278","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-06T15:58:54Z (feat/comfy-tables): Pretty warning dividers use resolved COLUMNS/terminal width across outputs; invalid pretty widths reject writers before mutation and preserve read warning-before-error ordering. Regression and focused width/warning tests, just check, and just test-fast passed.
  provenance: {"harness_session":"codex:01a111ed-c042-7550-b6d2-36e2b2115278","harness_session_source":"CODEX_THREAD_ID"}
