---
id: tasks-bdc2c2
title: Moving a newly started task into a fresh worktree fails with stale_copy because its start note remains in the original checkout; the documented handoff requires an extra task-only commit before it can resume.
status: idea
priority: 2
reviewed: true
created: 2026-10-10T17:43:15Z
updated: 2026-10-13T10:12:45Z
depends: []
tags: [feedback, friction, "from:ops"]
agent: codex
---

## Notes

- 2026-10-13T10:12:45Z (main): scope: drop; landed: record commits (tasks-6939f6, 1ef0ba5; record-commit spec §3.4) make start in the main checkout commit the record before git worktree add, so a fresh worktree carries the start note and no task-only commit is owed; ops runs [git] commit = true since ops-0ccfea, and this repo's own start commits (c100df5) show it; projects without [git] commit keep the documented commit-before-worktree step by choice; proposal: close as landed in 1ef0ba5 (tasks-6939f6) and enabled by ops-0ccfea
