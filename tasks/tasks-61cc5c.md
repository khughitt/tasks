---
id: tasks-61cc5c
title: "Make the process decision (superpowers, brainstorming, worktree) explicit per task"
status: idea
priority: 2
created: 2026-09-13T10:31:15Z
updated: 2026-09-13T10:31:15Z
depends: []
tags: []
---

prism-28e29c executed straight from the tasks protocol on main: the agent matched the tasks skill, skipped superpowers entirely, and never made a worktree, because the worktree rule in user AGENTS.md is scoped to brainstorming+planning sessions while the superpowers skills claim any conversation and any feature work. Which workflow a task gets is currently implicit in how the agent reads the task. Consider making it explicit: a dedicated task field (e.g. process: direct|planned) chosen at scoping time, or a derivation from existing parameters (size, complexity, priority, spec/plan presence). The derivation route keeps the corpus lean but can misfire on small-but-risky work; the field route is explicit but one more thing to triage. Show the decision in ready/next output so a picking session sees it before starting.
