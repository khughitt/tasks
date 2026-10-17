---
id: tasks-1a3280
title: Design wait-aware review state from the brief
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-10-17T09:19:37Z
updated: 2026-10-17T09:32:22Z
depends: []
parent: tasks-5408fe
tags: []
source: docs/notes/2026-10-17-scope-queue-stranded-ideas-brief.md
agent: claude-code/claude-opus-5-5
---

Question: How should the scope queue tell a reviewed idea that still waits on something recorded from one that nothing will wake, so the stranded ones come back without a waker's note?
Where to start: docs/notes/2026-10-17-scope-queue-stranded-ideas-brief.md (measurements, alternatives, lean 3); docs/specs/2026-10-11-review-state-design.md sections 1, 3.3, 4, 6, 7.2; src/review.rs::needs_review; TaskSummary::of; src/commands/sample.rs::pending_proposal and tasks-22f407; skills/scope/SKILL.md sections 3 and 5; tasks-d4e73d for a done-time wake.
Bound: a design spec for user review covering the rule (what counts as a recorded wait: open edge, pending proposal, user park), its JSON meaning and cost, the scope skill rule, and a backfill for legacy prose waits. Take the user's answers to the brief's four questions (2026-10-17) as inputs; choose the synced wait-on-user marker and the test that tells a newly woken idea from a legacy stranded one. No implementation.
Expected result: a reviewed spec under docs/specs/ attached with --spec, the brief updated with the decisions, and the implementation decomposed under this task's parent.
Ideas it wakes: on completion, run `tasks note tasks-163055 --no-reviewed "<decision>"` before this result's branch merges.

## Notes

- 2026-10-17T09:32:22Z (main): brief revised after the user's review (2026-10-17): ops-1a552c is a missed wake through a recorded edge (ops dc0a36c added it; hq-a558e8 closed 2026-10-09 with no wake), which favours option 3; measurement time and the 09:25Z ops drops recorded; no needs_review reader outside tasks; the user answered all four questions (no park as a wait, proposals count via the shared helper, two tiers, per-project backfill)
