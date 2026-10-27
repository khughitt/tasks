---
id: tasks-2f8377
title: Merged task records still refuse writes as same-stamp stale copies
status: idea
priority: 2
created: 2026-10-27T14:16:27Z
updated: 2026-10-27T14:16:27Z
depends: []
tags: [feedback, friction, "from:mm"]
agent: codex
origin: agent
---

The stale_copy refusal recommends merging the newer main-checkout task record into the worktree. A normal Git merge preserved both note histories and selected the newer updated stamp, but the next note still refuses because that stamp matches the main record while the merged content differs. Provide a supported way to resume writes after reconciling both histories, without editing timestamps by hand or overwriting either history.

## Notes

- 2026-10-27T14:16:27Z (feedback): created
  provenance: {"harness_session":"codex:01a17db4-159e-78a2-965f-3997745b1898","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
