---
id: tasks-61cc5c
title: "Make the process decision (superpowers, brainstorming, worktree) explicit per task"
status: idea
priority: 2
created: 2026-09-13T10:31:15Z
updated: 2026-09-13T17:09:48Z
depends: []
tags: []
spec: docs/specs/2026-09-13-task-process-design.md
---

prism-28e29c executed straight from the tasks protocol on main: the agent matched the tasks skill, skipped superpowers entirely, and never made a worktree, because the worktree rule in user AGENTS.md is scoped to brainstorming+planning sessions while the superpowers skills claim any conversation and any feature work. Which workflow a task gets is currently implicit in how the agent reads the task. Consider making it explicit: a dedicated task field (e.g. process: direct|planned) chosen at scoping time, or a derivation from existing parameters (size, complexity, priority, spec/plan presence). The derivation route keeps the corpus lean but can misfire on small-but-risky work; the field route is explicit but one more thing to triage. Show the decision in ready/next output so a picking session sees it before starting.

## Notes

- 2026-09-13T17:09:47Z (feat/tasks-61cc5c-process): scope: briefed; draft recommends explicit direct/planned process with no inferred default, and worktree isolation for both code paths; spec: docs/specs/2026-09-13-task-process-design.md; awaiting written design review
- 2026-09-13T17:09:48Z (feat/tasks-61cc5c-process): parked (waiting on user, review): Review .worktrees/tasks-61cc5c-process/docs/specs/2026-09-13-task-process-design.md; after design approval write the implementation plan for separate review
