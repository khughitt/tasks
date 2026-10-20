---
id: tasks-c9d4ba
title: "Renumbering a plan's step headings needs one edit per step child, and each edit's auto-commit fails the pre-commit tasks check until the last one lands"
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-20T09:10:46Z
created: 2026-10-18T22:14:12Z
updated: 2026-10-20T09:10:46Z
depends: [tasks-110ba6]
parent: tasks-e959ec
tags: [feedback, friction, "from:crew"]
agent: claude-code/claude-opus-5-5
---

tasks edit <id> --step '<new heading>' under [git] commit = true, after renaming several '### Task N:' headings at once: every write but the last reports commit_failed (step_missing for the not-yet-renamed siblings) and leaves its record uncommitted; expected a batch step rename, or a way to defer the per-write commit until the set is consistent

## References

- brief (input, required, kind=brief): docs/notes/2026-10-20-plan-step-renumbering-brief.md — evidence and alternatives for consistent plan-step renumbering

## Notes

- 2026-10-20T09:10:46Z (main): scope: briefed; inspected one-id edit validation, global step_missing errors and post-save commit warnings at 7a7c85c; live reproduction remains unverified; tasks-110ba6 will test existing consistent-commit workflows before new syntax; brief: docs/notes/2026-10-20-plan-step-renumbering-brief.md
