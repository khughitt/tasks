---
id: tasks-7c5a4b
title: Deliver the bounded prime binary and isolated README section
status: doing
priority: 2
size: m
complexity: high
process: direct
owner: design/tasks-e7424b
created: 2026-10-24T16:09:35Z
updated: 2026-10-24T16:50:58Z
started: 2026-10-24T16:25:09Z
depends: [tasks-209f2c]
parent: tasks-65b733
tags: []
source: "tasks-e7424b:plan-task-2"
agent: codex
spec: docs/specs/2026-10-24-bounded-session-summary-design.md
plan: docs/plans/2026-10-24-bounded-session-summary.md
step: "Task 2: Deliver the bounded prime binary and isolated README section"
---

Execute the exact approved plan step inline, with its interfaces, tests, source ownership and review requirements. Record evidence and finish on the task branch before integration.

## Notes

- 2026-10-24T16:25:09Z (design/tasks-e7424b): started
  provenance: {"harness_session":"codex:01a16e34-fd27-7e51-b10c-c1250b33416d","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-24T16:25:11Z (design/tasks-e7424b): Process direct, executing reviewed plan 3a264b9 inline in .worktrees/tasks-e7424b. Deferred full baseline just test passed 1669, 0 failed/ignored before product edits. Start with failing CLI tests; ops option source is now landed.
- 2026-10-24T16:47:28Z (design/tasks-e7424b): Implemented feature at 9cfb0bb with confined target/setup/bin/tasks. Focused CLI 9/9, summary unit 7/7, parser surface 1/1 and just check passed; full deferred baseline was 1669. Explicit-path smoke: summary 11886 bytes, full 135297 bytes, replay retains prefix rows/warnings/halts. Shared PATH binary unchanged. Post-merge reinstall is required and owned by tasks-72093a, which must inspect the current PATH destination, prepare the exact install command and obtain host-action approval. Final fast run and independent branch review precede integration.
- 2026-10-24T16:50:58Z (design/tasks-e7424b): review: impl round 1 — verdict: accept; findings: none; reviewer: codex/gpt-6-astra
