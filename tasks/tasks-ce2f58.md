---
id: tasks-ce2f58
title: "Task attachments: screenshots and files that travel with the record"
status: done
priority: 1
process: planned
owner: main
created: 2026-09-25T16:12:10Z
updated: 2026-09-28T14:13:07Z
started: 2026-09-28T14:13:07Z
completed: 2026-09-28T14:13:07Z
depends: []
tags: [design]
source: docs/notes/2026-09-25-task-attachments-brief.md
model: claude-opus-5-5
agent: "claude-code/claude-opus-5-5[1m]"
spec: docs/specs/2026-09-28-task-attachments-design.md
plan: docs/plans/2026-09-28-task-attachments.md
---

Goal for tasks-e7a870. A task can link a spec and a plan but not an image; screenshots end up transcribed in prose while the file stays in a downloads folder. Brief: docs/notes/2026-09-25-task-attachments-brief.md (problem, evidence, three alternatives with a lean toward a per-task directory found by convention, open questions on size policy, public repositories, and input sources). Done when a task can carry a screenshot that show prints, check validates, and rename carries along.

## Notes

- 2026-09-28T12:52:05Z (design/task-attachments): final-review fix wave: blank captions rejected, --name and caption checked before any read, empty clipboard image refused, file:0 resume and FIFO deadline tests, old-inventory and R9–R11 doc pointers
- 2026-09-28T12:57:35Z (design/task-attachments): parked (waiting on user, decision): user: add the attach/detach rows to ops/cli.toml and re-vendor, then choose merge/PR/keep for design/task-attachments; then agent: merge to main, cargo install --path . from the main checkout, drop idea tasks-e7a870 as superseded, close tasks-ce2f58, remove the worktree after tt-report
  provenance: {"harness_session":"claude-code:aa2a7d96-c0fb-4863-8009-2bfa035e9a14","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-28T14:13:07Z (main): started
  provenance: {"harness_session":"claude-code:aa2a7d96-c0fb-4863-8009-2bfa035e9a14","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-28T14:13:07Z (main): done
  provenance: {"harness_session":"claude-code:aa2a7d96-c0fb-4863-8009-2bfa035e9a14","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-28T14:13:07Z (main): met: tasks attach/detach store files under tasks/files/<id>/ with ledger notes; show/next list them, check audits six kinds, rename moves them (R9-R11); docs and skill updated; merged at d8ad9e6. Follow-up tasks-4105af syncs the cli.toml rows to ops
  provenance: {"harness_session":"claude-code:aa2a7d96-c0fb-4863-8009-2bfa035e9a14","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
