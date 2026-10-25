---
id: tasks-91da21
title: "tasks docs: deferred review minors"
status: todo
priority: 3
size: s
complexity: low
process: direct
created: 2026-10-25T14:58:55Z
updated: 2026-10-25T14:58:55Z
depends: []
tags: []
source: tasks-a8e28e
agent: claude-code/claude-opus-5-5
spec: docs/specs/2026-10-22-records-home-design.md
---

Minors the tasks-a8e28e implementation review (round 1, accepted) left for later:

- Titles: title_of takes the first '# ' line even inside a fenced code block, and misses an H1 behind a UTF-8 byte-order mark. resolve::heading_text shares the fence limitation; fix both or neither.
- Pretty output: the tasks: detail line shows id and via but not status, so a done link reads like an open one. Show status (or mark closed links) in pretty only; JSON already has it.
- Test gaps: a linked overlay project (records = overlay), --group, overlapping spec and plan directories (first kind wins: spec, plan, notes), a --ref to a path outside the docs directories (no row), and a link from a moved tombstone (status moved).
