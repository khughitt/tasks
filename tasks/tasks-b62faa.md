---
id: tasks-b62faa
title: "A task started in the main checkout and then in a new worktree within the same second leaves two copies with equal stamps; every later write refuses stale_copy even though one copy is a strict descendant of the other, until the branches are made identical by hand"
status: dropped
priority: 2
created: 2026-09-30T14:35:28Z
updated: 2026-09-30T14:39:09Z
depends: []
tags: [feedback, friction, "from:tack"]
agent: claude-code/claude-opus-5-5
---

tasks start <id> in main, commit, git worktree add, tasks start <id> in the worktree (same second). Later tasks note/done in either checkout: stale_copy 'same stamp but different content'. Expected: the resumed copy, whose notes extend the other, to be accepted as newer (or start to bump the stamp).

## Notes

- 2026-09-30T14:39:09Z (main): dropped
  provenance: {"harness_session":"claude-code:4471e860-d17f-47d3-a634-479664d60702","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T14:39:09Z (main): Duplicate of tasks-cff04e, fixed in ec41eb1 (writes stamp strictly after the loaded copy); filed shortly before the fixed binary was installed
  provenance: {"harness_session":"claude-code:4471e860-d17f-47d3-a634-479664d60702","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
