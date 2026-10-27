---
id: tasks-07f853
title: ready and prime offer a task from the main checkout's copy while a newer copy in a registered worktree already records it done; only start's stale_copy refusal reveals it
status: idea
priority: 2
created: 2026-10-27T11:52:29Z
updated: 2026-10-27T11:52:29Z
depends: []
tags: [feedback, gap, "from:ops"]
agent: claude-code/claude-opus-5-5
origin: agent
---

Action: picked the top unclaimed P1 from tasks ready in the main checkout; tasks start refused with stale_copy because a worktree branch held a newer record, where the task and two of its siblings were already done days earlier. Expected: ready/prime/next either omit or warn on rows whose record is newer in a worktree of the same project (the data start already consults), so a session does not plan around finished work. The stale_copy guard itself worked and prevented a forked record.

## Notes

- 2026-10-27T11:52:29Z (feedback): created
  provenance: {"harness_session":"claude-code:3fa9b156-0abb-464b-9d25-2c92742495f9","harness_session_source":"CLAUDE_CODE_SESSION_ID","origin":"agent"}
