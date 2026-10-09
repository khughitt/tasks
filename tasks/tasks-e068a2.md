---
id: tasks-e068a2
title: Keep closed task views readable when a linked plan is missing
status: done
priority: 2
size: s
complexity: mid
process: direct
owner: fix/historical-plan-reads
created: 2026-10-04T17:51:38Z
updated: 2026-10-09T12:33:56Z
started: 2026-10-09T12:23:55Z
completed: 2026-10-09T12:32:59Z
depends: []
parent: tasks-0e432f
tags: [feedback, friction, "from:tack"]
agent: claude-code/claude-opus-5-5
---

Why: closed records retain historical plan/step links. Current src/commands/show.rs::describe calls Resolver::step_exists for them, which reads a missing plan and fails the entire view. A disposable current-binary probe on 2026-10-06 reproduced check exiting 0 and show exiting 1 with kind io after removing the closed task's plan. The separate check complaint tasks-bf1604 is already covered by 8861cd5.

Done: when a closed task's linked plan is absent in both its checkout and the supported main-checkout document location, show returns the complete task and historical links, step_found false when a step was linked, and an explicit warning that its plan is unavailable. Existing available-plan checks and open-task missing-plan failures retain their behavior. Do not catch unrelated permission/read failures or change JSON field shapes. Apply the status-aware rule in the shared describe path used by show and next, without weakening Resolver::step_exists globally.

Where to start: src/commands/show.rs::describe, src/resolve.rs::Located and Resolver::locate/step_exists, src/commands/list.rs's describe caller; docs/specs/2026-09-30-record-home-design.md document fallback and tests/cli.rs::check_holds_only_open_records_to_their_doc_links.

Verification: an integration regression links a plan and step, closes the task, deletes the plan, and asserts check and show succeed with the preserved record, false step_found and warning; repeat for dropped history, and verify open work still fails while a plan available in main is read normally. Use just test-one --test cli <filter>, then just test-fast.

Original report: tasks show --json fails whole (io error) for a done task whose plan: file was later removed, so every reader that shows done records (trial tools, reports) breaks; tasks check, which only guards open tasks, passes beforehand

## Notes

- 2026-10-06T09:45:47Z (main): scope: scoped; reproduced missing historical plan failure; scoped shared describe-path fix with closed/open and main-checkout checks; P2/s/mid/direct
- 2026-10-09T11:17:27Z (main): Task-move review D12: include moved tombstones in closed missing-plan coverage. Preserve moved_to/moved_from and historical plan/step links; show should warn when the plan is unavailable while keeping the pointer visible. Reuse the closed-record fix, without weakening live destination checks.
- 2026-10-09T12:23:55Z (main): started
  provenance: {"harness_session":"codex:01a12066-6cd3-7793-bec2-ff08cc0f6519","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-09T12:24:01Z (main): Process direct: implement the scoped historical-plan reader fix and regressions in .worktrees/tasks-e068a2; preserve open-task and unrelated I/O failures.
- 2026-10-09T12:24:15Z (fix/historical-plan-reads): resumed
  provenance: {"harness_session":"codex:01a12066-6cd3-7793-bec2-ff08cc0f6519","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-09T12:25:20Z (fix/historical-plan-reads): Scope confirmed: shared show::describe serves show and next; Resolver::step_exists also validates edits and must stay strict. Cover done/dropped/moved history, plan-only records, main-checkout fallback, open failures, and unrelated read errors; update README contract.
- 2026-10-09T12:28:24Z (fix/historical-plan-reads): Verification: just test-one show_closed and moved-history regression failed before the fix; just test-one historical_plan now passes all four tests. just test-fast passed 1,285 tests (one intentionally ignored); just check passed. Read-only implementation review pending.
- 2026-10-09T12:29:13Z (fix/historical-plan-reads): review: impl round 1 — verdict: revise; findings: Important 1; reviewer: codex
- 2026-10-09T12:30:25Z (fix/historical-plan-reads): Corrective round 1: main-checkout permission/directory regression failed before fix and passes after checking try_exists at both supported locations; all five historical_plan regressions pass. Resolver remains unchanged; fresh scoped re-review dispatched.
- 2026-10-09T12:31:40Z (fix/historical-plan-reads): review: impl round 2 — verdict: accept; findings: none; reviewer: codex
- 2026-10-09T12:31:40Z (fix/historical-plan-reads): Final verification: all five focused historical_plan regressions pass; just test-fast passed 1,286 tests with one intentionally ignored; just check passed. Fresh scoped review accepted the error-handling correction with no remaining or deferred findings.
- 2026-10-09T12:32:59Z (fix/historical-plan-reads): done
  provenance: {"harness_session":"codex:01a12066-6cd3-7793-bec2-ff08cc0f6519","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-09T12:32:59Z (fix/historical-plan-reads): Closed task readers preserve unavailable historical plans and steps with an explicit warning; done, dropped, and moved records remain readable. Main-checkout fallback and open/I/O failures retained; five regressions, 1,286 fast tests, checks, fresh review, and installed CLI smoke passed.
  provenance: {"harness_session":"codex:01a12066-6cd3-7793-bec2-ff08cc0f6519","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-09T12:33:56Z (fix/historical-plan-reads): Installed CLI smoke: initial disposable fixture stopped at mkdir because init already creates docs/plans; corrected fixture passed unchanged historical record, unavailable-plan warning, false step_found, and open-task I/O failure. No product-code changes followed review.
