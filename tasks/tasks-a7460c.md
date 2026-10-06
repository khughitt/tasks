---
id: tasks-a7460c
title: Project location and migration
status: todo
priority: 1
size: m
complexity: high
process: direct
lane: true
created: 2026-10-06T16:49:53Z
updated: 2026-10-06T16:49:57Z
depends: []
tags: []
source: "user:2026-10-06 tasks lane organization"
agent: codex
---

Keep project identity and location continuous across renames, linked checkouts, moves, and retirement. First milestone: tasks-7580d2 delivers a reviewed id/path resolver and former-root contract for the rename consumers; align linked-checkout discovery with that interface before their implementations diverge.

Scope: tasks-7580d2, tasks-9b0a2e, tasks-2d58d1, tasks-7ba557, and tasks-b4b9d4. Preserve the distinction between the records root, code checkout, current/former roots, and worktree storage. Reuse registry identity and lookup code, retain explicit ambiguity and failure behavior, and coordinate interface changes with the relevant ops-owned CLI vocabulary.

Sequence: project-resolution work is the first priority because it blocks the rename cutover. Linked checkouts and feedback routing follow their existing handoffs; task moves remain an idea pending evidence and design. Retirement review follow-ups may proceed independently. Done when retained children have completed or received explicit dispositions and their agreed checks pass. This lane does not approve any child design, implementation plan, or host cutover.

## Notes

- 2026-10-06T16:49:57Z (main): organization: created from the user's approved 2026-10-06 lane proposal; coordination uses existing tasks and preserves nested goals and child review requirements.
