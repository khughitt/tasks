---
id: tasks-ebdc96
title: "After merging the registered checkout's newer copy of a task record into a worktree, the next write still refuses stale_copy: both copies now carry the same updated stamp with different content, so the error's own remedy ('merge its copy into this checkout') leads straight back to the refusal until the branch itself merges"
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-17T17:50:37Z
created: 2026-10-17T10:04:58Z
updated: 2026-10-17T17:50:37Z
depends: []
tags: [feedback, friction, "from:lore"]
agent: claude-code/claude-opus-5-5
---

## Notes

- 2026-10-17T17:50:37Z (main): scope: drop; covered: tasks-29f71f (4095f53) established the lossless recovery (resolve through Git, integrate the resolved record into both equal-stamp siblings, then a CLI retry advances the stamp; a one-sided union is correctly refused); tasks-a20a64 (todo) fixes this exact misleading remedy text, still unchanged at src/stale.rs:151; same report as dropped tasks-2337b3 and tasks-57b66e; proposal: drop as covered by tasks-a20a64 with tasks-29f71f's finding
