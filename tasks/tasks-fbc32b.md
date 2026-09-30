---
id: tasks-fbc32b
title: "show and list miss a task whose record exists only on a worktree branch, though the park store knows its checkout"
status: idea
priority: 2
created: 2026-09-23T17:07:01Z
updated: 2026-09-30T10:28:06Z
depends: []
parent: tasks-c4ad8e
tags: [feedback, friction, "from:material"]
agent: claude-code/claude-opus-5-5
---

A task added and parked inside a git worktree (its file committed only on the worktree's branch) is invisible from the main checkout: 'tasks show <id>' returns task_not_found, and plain 'tasks list' omits it. Yet 'list --parked' and 'quiet' find it from the shared park store and warn 'parked in <worktree>; resume it from that checkout'. Expected: show routes to (or reads from) the checkout the park or claim store names, and list includes such tasks (marked with their checkout), so a person standing in the main checkout does not lose track of parked worktree work.

## Notes

- 2026-09-29T20:50:25Z (main): scope: briefed; show routing can reuse park §5.3 resolution but plain list inclusion changes the JSON contract, so it waits on design tasks-ab8d2d; parented to tasks-c4ad8e; brief: docs/notes/2026-09-29-cross-checkout-records-brief.md
- 2026-09-30T10:28:06Z (ab8d2d-record-home): Decided in tasks-ab8d2d (spec record-home §6): show falls back to the claim's or park's checkout with a warning; plain list stays local (declined, JSON contract). Implemented by tasks-9949f3.
