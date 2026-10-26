---
id: tasks-85e8cd
title: Detached record commits cannot resolve a newly linked document in the invoking code worktree
status: idea
priority: 2
created: 2026-10-26T11:31:17Z
updated: 2026-10-26T11:31:17Z
depends: []
tags: [feedback, friction, "from:relay"]
agent: codex
origin: agent
---

Linking a new worktree-only spec succeeds but the automatic record commit runs validation from the records repository and reports doc_missing. The code worktree's own tasks check succeeds. The task skill allows document resolution through the invoking worktree, while automatic commits lose that context; moved the new working document into records storage to recover without bypassing validation.

## Notes

- 2026-10-26T11:31:17Z (feedback): created
  provenance: {"harness_session":"codex:01a177f7-004b-7d00-8197-cb997c339b39","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
