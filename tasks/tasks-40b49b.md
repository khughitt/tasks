---
id: tasks-40b49b
title: Cross-project dependencies cannot resolve records in an unmerged task worktree
status: idea
priority: 2
created: 2026-10-26T13:21:06Z
updated: 2026-10-26T13:21:06Z
depends: []
tags: [feedback, gap, "from:obs"]
agent: codex
origin: agent
---

Creating cross-project plan steps with a dependency on a newly filed step returned unresolvable_id: resolution used the registered root while the authoritative record existed in the plan worktree. A way to validate against the authoritative checkout or defer the edge would avoid textual ordering until approved plan records are integrated. The workaround preserves the ordering in the plan and adds the edge after those records land.

## Notes

- 2026-10-26T13:21:06Z (feedback): created
  provenance: {"harness_session":"codex:01a1780f-e78c-7582-93a0-9507ebf89699","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
