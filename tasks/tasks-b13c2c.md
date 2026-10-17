---
id: tasks-b13c2c
title: "A provenance source for crew attempts, so their notes carry harness provenance"
status: idea
priority: 2
reviewed: true
created: 2026-10-14T01:44:14Z
updated: 2026-10-17T02:37:21Z
depends: [tasks-e7f671]
tags: []
agent: claude-code/claude-opus-5-5
---

Why: crew runs pi workers on tasks items and writes its notes with TASKS_SESSION=crew:<attempt id>, TASKS_OWNER=crew. tasks stamps lifecycle provenance only from Claude Code and Codex sessions, so crew's notes carry none, and obs cannot join an attempt to the task it worked. Named in crew's core design (crew docs/specs/2026-10-13-crew-core-design.md §9; task crew-3fc20c).

Done: a reviewed decision on how a crew attempt (and later other harness runners) is recognised as a provenance source, and the stamp it writes. Not a blocker for crew's first slice, which keeps the join in its own attempt record.

## Notes

- 2026-10-17T02:37:21Z (main): scope: briefed; provenance accepts only native Claude Code/Codex variables (src/provenance.rs validate), so crew notes carry only by: crew; pi 1.1.0 has --session-id but no documented env export; lean pi as a native source if research finds a variable, else declared TASKS_SESSION runner prefixes; research tasks-e7f671 filed and depended on; brief: docs/notes/2026-10-16-runner-provenance-brief.md
