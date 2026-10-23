---
id: tasks-7a4486
title: Cross-project dependencies cannot resolve new records held only in another worktree
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-23T10:59:22Z
created: 2026-10-23T00:20:47Z
updated: 2026-10-23T10:59:22Z
depends: []
tags: [feedback, gap, "from:ops"]
agent: codex
---

Creating a task with a cross-project dependency fails with unresolvable_id when the dependency record is committed only in the other project’s worktree. The workaround is to keep the receiving task explicitly blocked until the source records integrate, then add the dependency and unblock. Parent-task ownership does not make those child records discoverable for this lookup.

## Notes

- 2026-10-23T10:59:22Z (main): scope: drop; current src/resolve.rs and src/commands/dep.rs at 60cad3f retain registered authority for foreign dependencies; tasks-d0d520 established record publication and tasks-70f0bc already covers this exact missing-record diagnostic and workflow; proposal: drop as covered by tasks-70f0bc; existing evidence: docs/notes/2026-10-06-record-continuity-brief.md
