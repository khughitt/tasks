---
id: tasks-dd78a5
title: "The tasks skill's worktree setup sentence (just setup, otherwise only the root guide's explicit command) is narrower than the global instructions' setup order, which also allows a README/CONTRIBUTING command that writes only inside the worktree and a lockfile's frozen worktree-local install, reports a bare manifest before the first commit, and requires hydration before the first commit; align the skill with that order"
status: idea
priority: 2
created: 2026-10-17T12:57:57Z
updated: 2026-10-17T12:57:57Z
depends: []
tags: [feedback, gap, "from:lore"]
agent: claude-code/claude-opus-5-5
---

Process and workspace section, 'Then run just setup when defined; otherwise run only the root guide's explicit setup command'. Two instructions now disagree on a checkout with only a package-lock.json, so an agent cannot tell whether to run npm ci; unhydrated worktrees make hooks stop at an npx install prompt or fetch unpinned tools.
