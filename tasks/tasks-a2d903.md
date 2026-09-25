---
id: tasks-a2d903
title: Design task attachments from the brief
status: todo
priority: 1
size: m
complexity: high
process: planned
created: 2026-09-25T16:12:10Z
updated: 2026-09-25T16:12:10Z
depends: []
parent: tasks-ce2f58
tags: [design]
agent: "claude-code/claude-opus-5-5[1m]"
---

Write docs/specs/<date>-task-attachments-design.md from docs/notes/2026-09-25-task-attachments-brief.md and review it with the user, then split the implementation into children of tasks-ce2f58 (storage and the attach command, show/check, rename and id-collision recovery, README and skill).

Decisions the spec must settle (brief, Unanswered questions):
- storage: a per-task directory found by convention (tasks/files/<id>/, no schema change, the brief's lean) or a declared front-matter list (breaks older readers: Task is deny_unknown_fields);
- size policy: per-file cap, recompression on attach, or Git LFS; the user sets the acceptable repository growth;
- public repositories and feedback: refuse, warn, or allow;
- input sources: path, stdin, clipboard (wl-paste);
- what show and show --pretty print so an agent can open the file from any checkout or worktree, and the contract a terminal front end can render from later.
