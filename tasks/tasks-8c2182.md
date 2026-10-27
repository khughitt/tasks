---
id: tasks-8c2182
title: Cross-project dependency resolution cannot target an unmerged worktree task
status: idea
priority: 2
created: 2026-10-27T14:54:33Z
updated: 2026-10-27T14:54:33Z
depends: []
tags: [feedback, gap, "from:obs"]
agent: codex
---

A task created in an active worktree is readable and valid in that checkout, but adding it as a dependency from another registered project fails with unresolvable_id because resolution sees only the registered checkout. The prerequisite is real and must be tracked before its code branch can merge. The affected task remains explicitly blocked with a note naming the authoritative checkout and approval condition; no records were copied or dependency substituted. A supported way to resolve or defer this cross-checkout dependency edge would preserve the graph during planned work.

## Notes

- 2026-10-27T14:54:33Z (feedback): created
  provenance: {"harness_session":"codex:01a17d0b-c7e0-7203-8382-374d995292c9","harness_session_source":"CODEX_THREAD_ID"}
