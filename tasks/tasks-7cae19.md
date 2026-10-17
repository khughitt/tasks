---
id: tasks-7cae19
title: "The project resolver (tasks where / linked lookup) fails in a repository with no commits yet when GIT_TRACE is set: git ls-tree HEAD's trace lines on stderr seem to defeat the no-HEAD classification, so the lookup errors instead of answering"
status: idea
priority: 2
created: 2026-10-17T11:24:25Z
updated: 2026-10-17T11:24:25Z
depends: []
tags: [feedback, friction, "from:ops"]
agent: claude-code/claude-opus-5-5
---

Repro: git init a scratch repo, then run a client that asks the resolver about it (here a pre-commit hook) under GIT_TRACE=1: error 'io: git ls-tree HEAD in <repo> failed: <trace line> trace: built-in: git ls-tree -z HEAD -- tasks/.config.toml'. Without GIT_TRACE, or once HEAD exists, it answers. Expected: an unborn HEAD is recognised regardless of git trace output (e.g. check rev-parse --verify HEAD, or run git with GIT_TRACE* unset).
