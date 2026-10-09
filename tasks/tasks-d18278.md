---
id: tasks-d18278
title: Correct whole-branch task move review findings
status: done
priority: 2
size: m
complexity: high
process: direct
owner: task-migration-design
created: 2026-10-09T07:25:24Z
updated: 2026-10-09T07:49:51Z
started: 2026-10-09T07:36:24Z
completed: 2026-10-09T07:49:51Z
depends: []
parent: tasks-7ba557
tags: []
agent: codex
spec: docs/specs/2026-10-08-task-move-design.md
---

Fix final review I1–I6 in .superpowers/sdd/2026-10-09-task-move/final-review.md: monotonic move stamps; valid historical deferral; pending-parent and resumed-child guards; preserve changed source attachments/stores; coherent journal contents and park/escalation pairs; pointer-aware local halt remedies. Test each RED→GREEN; full fast/check, reinstall, close in code commit. Do not fold deferred Minor M1–M5 into corrective rounds.

## Notes

- 2026-10-09T07:36:24Z (task-migration-design): started
  provenance: {"harness_session":"codex:01a11f97-53f9-7ea3-9d8c-7c1552cd6894","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-09T07:49:51Z (task-migration-design): done
  provenance: {"harness_session":"codex:01a11f97-53f9-7ea3-9d8c-7c1552cd6894","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-09T07:49:51Z (task-migration-design): Corrected I1–I6: monotonic stamps; valid deferred tombstones; pending-parent/resumed-child guards; changed-asset/store retention; coherent journals; pointer-aware local halt remedies. 30 focused regressions and 1280 fast tests pass (1 exhaustive ignored); just check and reinstall passed. Controller owns scoped re-review; Minor M1–M5 remain deferred.
  provenance: {"harness_session":"codex:01a11f97-53f9-7ea3-9d8c-7c1552cd6894","harness_session_source":"CODEX_THREAD_ID"}
