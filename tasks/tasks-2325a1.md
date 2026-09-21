---
id: tasks-2325a1
title: "check resolves plan/spec doc existence against the current worktree, so a fresh worktree errors doc_missing for docs the main checkout keeps uncommitted (git-excluded), and the pre-commit gate then blocks all work there"
status: idea
priority: 2
created: 2026-09-20T10:08:20Z
updated: 2026-09-20T10:08:20Z
depends: []
tags: [feedback, friction, "from:mind6"]
agent: opencode/glm-5.3
---

command: `tasks check` as run by a pre-commit hook inside a newly linked git worktree. error kind: doc_missing on every task whose plan/spec file is excluded via .git/info/exclude and therefore never reaches the worktree. expected: doc existence resolved against the registered project root — or downgraded to a warning when the doc exists in the main checkout — so worktree sessions are not blocked by files git cannot carry into them
