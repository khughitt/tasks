---
id: tasks-8e365c
title: "Clearing a stale park on a goal (or any task not about to be worked) needs start then edit --status todo, which leaves started/owner stamps; an explicit unpark command would avoid that"
status: todo
priority: 2
size: s
complexity: mid
process: planned
created: 2026-10-17T10:26:19Z
updated: 2026-10-17T17:50:35Z
depends: []
tags: [feedback, gap, "from:lore"]
agent: claude-code/claude-opus-5-5
---

Why: a park that no longer describes the work (a goal parked waiting on the user after the answer arrived, a park left by a session that moved on) can only be removed from an open task by `tasks start` and then `edit --status todo`. That leaves started/owner stamps and a resume note on a task nobody worked.

Established (scope 2026-10-17): parks live in the host-local claim store (src/claims.rs, `remove_park`). The only paths that remove one are a claim acquisition (`start`, which replaces the entry) and a release into done, dropped or shelved (`clear_park` in src/commands/mod.rs::transition, around line 569). `release` leaves the park alone by design (docs/specs/2026-10-15-claim-release-design.md). Parks on *closed* records are already covered: a repeated `done` clears them (tasks-6b2867, tasks-b9b10e, tasks-6a0050, tasks-081099). This task is only about open tasks.

Done: a command (lean: `tasks unpark <id> "<why>"`, modelled on `release`) removes the park on an open task and records why in a note. Status, owner, started and every other field stay as they were. It refuses when there is no park, and when a live claim holds the task. The spec decides: the name (unpark, or a `release` / `park` option); whether a capability escalation and quiet-host holds go with the park; how it treats a park recorded from another worktree or a removed one; and the note shape. The CLI rows land in ops's cli.toml first and are adopted here (AGENTS.md Rules). The tasks skill documents it beside `park`.

Where: src/commands/park.rs, src/commands/release.rs (the pattern), src/claims.rs, src/commands/mod.rs::transition; docs/specs/2026-09-09-park-design.md and 2026-10-15-claim-release-design.md; tests/cli.rs park cases. Related: tasks-984769 (cross-checkout recovery goal) and docs/notes/2026-10-06-record-continuity-brief.md.

Original title (preserved): Clearing a stale park on a goal (or any task not about to be worked) needs start then edit --status todo, which leaves started/owner stamps; an explicit unpark command would avoid that.

## Notes

- 2026-10-17T17:50:35Z (main): scope: scoped; confirmed no path clears a park on an open task except start (src/claims.rs remove_park via transition's clear_park only on done/dropped/shelved); closed-record parks already covered by tasks-6b2867/b9b10e; planned because it adds a CLI command (ops cli.toml first) with escalation/holds decisions, as release did
