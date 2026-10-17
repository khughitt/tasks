---
id: tasks-aeeef1
title: Implement wait-aware review state
status: todo
priority: 2
size: l
complexity: high
process: planned
created: 2026-10-17T11:25:02Z
updated: 2026-10-17T12:31:12Z
depends: []
parent: tasks-5408fe
tags: []
source: tasks-1a3280
agent: claude-code/claude-opus-5-5
spec: docs/specs/2026-10-17-wait-aware-review-design.md
plan: docs/plans/2026-10-17-wait-aware-review.md
---

Why: reviewed ideas that wait on nothing recorded never return to the scope queue (tasks-163055; brief docs/notes/2026-10-17-scope-queue-stranded-ideas-brief.md).
Done: every step below lands; the plan's Rollout has run on every reading host and registered project.
Where: spec docs/specs/2026-10-17-wait-aware-review-design.md (accepted after spec review round 2); plan docs/plans/2026-10-17-wait-aware-review.md (accepted after plan review round 2, codex/gpt-6.1-sol). Execute subagent-driven, steps in order, read-only reviewers.

## Notes

- 2026-10-17T12:31:12Z (feat/wait-aware-review): review: impl round 1 — verdict: revise; findings: critical 1, minor 4; reviewer: claude-code/claude-opus-5-5
