---
id: tasks-0238e4
title: "Takeover and park notes embed the absolute worktree path and hostname, which leak machine layout when tasks/ is public"
status: idea
priority: 2
created: 2026-09-19T13:13:54Z
updated: 2026-09-19T13:13:54Z
depends: []
tags: [feedback, idea, "from:tui"]
agent: claude-code/claude-opus-5
---

tasks start --force writes 'took over session … host <name>, worktree </abs/path>' into the record, and park notes carry the same fields. In a public repository these expose the sync root and hostname. Consider writing the worktree relative to the project root (.worktrees/<name>) and omitting the host from the generated note text, keeping both in the claim store where they are needed.
