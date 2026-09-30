---
id: tasks-44b889
title: "A task that is doing and must wait for calendar time cannot carry a date: edit --defer refuses a doing task and park takes no resume date, so the wait lives only in note prose and prime never surfaces it when due"
status: idea
priority: 2
size: m
complexity: high
process: planned
created: 2026-09-28T08:31:43Z
updated: 2026-09-30T10:05:32Z
depends: []
parent: tasks-46d207
tags: [feedback, gap, "from:ops"]
agent: claude-code/claude-opus-5-5
---

Seen when several in-progress tasks each needed 'wait a week, then read the data': edit --defer on a doing task fails with 'a deferral can only sit on an idea, todo, or blocked task'; park records a next step but no date. The tasks sat in doing for 2-3 weeks past the intended point with nothing in prime marking them due. Expected: park <id> <next> --until <when> (or defer allowed on doing/parked) that prime reports as come due, like the deferred: line does for todo.

## Notes

- 2026-09-30T10:05:32Z (main): scope: briefed; doing deferral is deliberately excluded by the current contract; date storage and due/resume semantics need design; waits on tasks-157d05; brief: docs/notes/2026-09-30-work-selection-brief.md
