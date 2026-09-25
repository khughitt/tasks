---
id: tasks-92757e
title: Pretty priority column carries a three-level magenta scale
status: todo
priority: 2
size: m
complexity: mid
process: planned
created: 2026-09-25T12:27:49Z
updated: 2026-09-25T12:27:49Z
depends: []
tags: [cli]
source: tasks-142d2f follow-up
agent: claude-code/claude-opus-5-5
---

Paint P1-P3 in three steps of the terminal theme's magenta (palette slot 5), strongest at P1 and fading toward the dimmed foreground, reusing the date scale's Recency/OKLab machinery (docs/specs/2026-09-25-date-recency-color-design.md). Replaces today's bold on P0/P1. Design questions: what P0 and P4 get (P0 full magenta + bold? P4 plain?); the query gains OSC 4;5, and TASKS_PALETTE gains a magenta key, which must not turn existing three-key values into config errors; whether the ramp is three fixed mixes or the same continuous function sampled at three points.
