---
id: tasks-b8f3c9
title: Implement explicit idea review state and pretty review markers
status: todo
priority: 2
size: m
complexity: mid
process: planned
created: 2026-10-11T15:08:58Z
updated: 2026-10-11T15:08:58Z
depends: []
parent: tasks-46d207
tags: []
source: tasks-a9b8d3
agent: codex
plan: docs/plans/2026-10-11-review-state.md
---

Execute the reviewed docs/plans/2026-10-11-review-state.md with subagent-driven development. Build the reviewed flag, atomic note/edit writes, needs_review JSON/filter, pretty markers, skill/docs updates and frozen-snapshot backfill tool. Rollout is gated on a writer pause and upgrading every reading host; no live reviewed writes before that. Done: reviewed implementation, gates passing, then explicitly approved rollout.
