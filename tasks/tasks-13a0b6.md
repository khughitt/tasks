---
id: tasks-13a0b6
title: "Quick add: low-friction ingest of thoughts, tasks, and ideas across mind6 and tasks"
status: idea
priority: 2
created: 2026-09-06T21:51:32Z
updated: 2026-09-06T22:41:11Z
depends: [mind6-c1c960]
tags: [quick-add, capture, cross-project]
---

Given rough, unstructured notes (phone notes, Keep, a pasted list), capture them in a useful form with as little friction as possible. Meta exploration spanning mind6 and tasks; scope into pieces per project once the routing story is clear.

Intake paths:
1. Copy + paste (e.g. phone notes -> a claude code session). Targets: mindful v6, tasks. Open question: share one tag system across mind6/tasks (ops owns the shared conventions?).
2. Google Takeout dump. Related earlier draft: keep repo, worktree keep-import, docs/superpowers/plans/2026-08-30-keep-import.md (Takeout JSON -> clustering -> curation packets -> validated import into mindful v3 via POST /thoughts). Revisit against v6.

Possible routing for parsed items:
- project mentions (tasks, ideas, questions) -> tasks
- personal: reminders -> leave in Keep; music -> mindful; others -> learn from data / ask the user

Interactive quick add inside a claude code session:
- "Please create tasks for ..." or a /tasks slash command
- obvious items: the agent files in batches
- less obvious: present to the user and ask

Related: mind6-710dcc (atomic linked capture from the CLI).

## Notes

- 2026-09-06T22:41:11Z (main): Defer the Google Takeout path: earlier plan and the 2026-08-30 Takeout dataset live in the keep repo's keep-import worktree; revisit after paste + interactive quick add
