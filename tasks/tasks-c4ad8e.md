---
id: tasks-c4ad8e
title: Keep a task record coherent across checkouts
status: done
priority: 2
created: 2026-09-29T20:50:01Z
updated: 2026-09-30T14:33:49Z
completed: 2026-09-30T14:33:49Z
depends: []
tags: [worktree]
source: docs/notes/2026-09-29-cross-checkout-records-brief.md
model: claude-opus-5-5
---

Writes land in the copy the work will merge from, reads find worktree-only work, and a fresh worktree passes its own gate. Brief: docs/notes/2026-09-29-cross-checkout-records-brief.md.

## Notes

- 2026-09-30T14:33:49Z (main): done
  provenance: {"harness_session":"claude-code:4471e860-d17f-47d3-a634-479664d60702","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T14:33:49Z (main): Met: writes refuse from a copy that is behind and stamps strictly increase so a same-second handoff cannot fork (9949f3, bb53e5, 2c0a1d, cff04e); reads find worktree-only work (476c6b, fbc32b); a fresh worktree passes its gate by reading git-excluded docs from main (2325a1, ace27b).
  provenance: {"harness_session":"claude-code:4471e860-d17f-47d3-a634-479664d60702","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
