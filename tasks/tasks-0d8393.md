---
id: tasks-0d8393
title: "check warns moved_dep on a tombstone whose own depends names other tombstones; a tombstone refuses every write, so the warning can never be cleared and its swap command is refused with moved"
status: idea
priority: 2
created: 2026-10-17T02:23:40Z
updated: 2026-10-17T02:23:40Z
depends: []
tags: [feedback, friction, "from:ops"]
agent: claude-code/claude-opus-5-5
---

Seen when a chain of mutually dependent tasks is moved together: each source tombstone keeps its frozen depends on the other source ids, and check reports moved_dep on each tombstone, though every counterpart already depends on the live ids. dep on the tombstone refuses (moved) and redirects to a dep on the counterpart that would change nothing. Expected: no moved_dep for a tombstone dependent at all (its depends is history; the counterpart carries the live edges), not only dropping the command as tasks-183bfa M5 proposes. Leaves four permanent warnings in the source project's check on every host.
