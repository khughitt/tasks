---
id: tasks-73f253
title: The tasks skill's Process and workspace setup-hydration sentence lags the global rule on confining a declared setup command and on read-only worktrees
status: idea
priority: 2
created: 2026-10-23T10:20:40Z
updated: 2026-10-23T10:20:40Z
depends: []
tags: [feedback, gap, "from:lore"]
agent: claude-code/claude-opus-5-5
---

The global Git rule's worktree-hydration sentences now say: a setup command named by the root guide, README or CONTRIBUTING may have its build or install outputs pointed inside the worktree through its own documented settings or flags without user approval (and must, when it would replace a tool on the host's PATH); a shared download cache is not an environment; the missing-setup-route report informs and asks no approval; and a worktree checked out only to be read needs no hydration or baseline until something runs or is committed in it. The skill's Process and workspace paragraph still carries the earlier wording, so an agent reading only the skill can still treat confinement as an approval stop or hydrate a read-only snapshot. Expected: the skill's sentence follows the same order and exemptions.
