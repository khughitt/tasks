---
id: tasks-e068a2
title: Keep closed task views readable when a linked plan is missing
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-04T17:51:38Z
updated: 2026-10-09T11:17:27Z
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
