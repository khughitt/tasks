---
id: tasks-a2d903
title: Design task attachments from the brief
status: doing
priority: 1
size: m
complexity: high
process: planned
owner: design/task-attachments
created: 2026-09-25T16:12:10Z
updated: 2026-09-28T11:10:24Z
started: 2026-09-28T10:36:18Z
depends: []
parent: tasks-ce2f58
tags: [design]
agent: "claude-code/claude-opus-5-5[1m]"
spec: docs/specs/2026-09-28-task-attachments-design.md
---

Write docs/specs/<date>-task-attachments-design.md from docs/notes/2026-09-25-task-attachments-brief.md and review it with the user, then split the implementation into children of tasks-ce2f58 (storage and the attach command, show/check, rename and id-collision recovery, README and skill).

Decisions the spec must settle (brief, Unanswered questions):
- storage: a per-task directory found by convention (tasks/files/<id>/, no schema change, the brief's lean) or a declared front-matter list (breaks older readers: Task is deny_unknown_fields);
- size policy: per-file cap, recompression on attach, or Git LFS; the user sets the acceptable repository growth;
- public repositories and feedback: refuse, warn, or allow;
- input sources: path, stdin, clipboard (wl-paste);
- what show and show --pretty print so an agent can open the file from any checkout or worktree, and the contract a terminal front end can render from later.

## Notes

- 2026-09-28T10:36:18Z (main): started
  provenance: {"harness_session":"claude-code:aa2a7d96-c0fb-4863-8009-2bfa035e9a14","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-28T10:39:21Z (main): design decisions (user, 2026-09-28): size policy = per-file cap, default 2 MiB, per-project override in tasks/.config.toml [attachments]; feedback takes no attachments and attach does no public-repo check. Proposed and unopposed: storage tasks/files/<id>/ by convention, inputs path/stdin/--clipboard (wl-paste), attached: note per file; worktree .worktrees/task-attachments
- 2026-09-28T10:43:09Z (design/task-attachments): parked (waiting on user, review): user: review .worktrees/task-attachments/docs/specs/2026-09-28-task-attachments-design.md; then agent: apply corrections, run writing-plans, and file the four children under tasks-ce2f58
  provenance: {"harness_session":"claude-code:aa2a7d96-c0fb-4863-8009-2bfa035e9a14","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-28T11:10:24Z (design/task-attachments): resumed
  provenance: {"harness_session":"claude-code:aa2a7d96-c0fb-4863-8009-2bfa035e9a14","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-28T11:10:24Z (design/task-attachments): spec revised after review: ownership ledger (attached:/detached: notes by name) with check drift errors; branch-aware collision split; storage symlink refusal (attachment_unsafe); rename inventory baseline with R5-R7; ledger-first detach and file-first attach with idempotent retry
- 2026-09-28T11:10:24Z (design/task-attachments): parked (waiting on user, review): user: review the revised .worktrees/task-attachments/docs/specs/2026-09-28-task-attachments-design.md; then agent: apply corrections, run writing-plans, and file the four children under tasks-ce2f58
  provenance: {"harness_session":"claude-code:aa2a7d96-c0fb-4863-8009-2bfa035e9a14","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
