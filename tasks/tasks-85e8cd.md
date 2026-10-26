---
id: tasks-85e8cd
title: Detached record commits cannot resolve a newly linked document in the invoking code worktree
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-26T15:19:41Z
created: 2026-10-26T11:31:17Z
updated: 2026-10-26T15:19:41Z
depends: [tasks-6644b7]
parent: tasks-a73dce
tags: [feedback, friction, "from:relay"]
agent: codex
origin: agent
---

Linking a new worktree-only spec succeeds but the automatic record commit runs validation from the records repository and reports doc_missing. The code worktree's own tasks check succeeds. The task skill allows document resolution through the invoking worktree, while automatic commits lose that context; moved the new working document into records storage to recover without bypassing validation.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-15-branch-only-document-links-brief.md — detached automatic commit evidence and the retained-document question

## Notes

- 2026-10-26T11:31:17Z (feedback): created
  provenance: {"harness_session":"codex:01a177f7-004b-7d00-8197-cb997c339b39","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
- 2026-10-26T15:19:41Z (main): scope: briefed; retained original report and reused tasks-a73dce and its branch-document brief; tasks-6644b7 tests detached automatic commits for new records-owned versus retained code documents, with an explicit wait edge; source inspection at f1d45f2 (resolver changes c99900e and 5370117), no new runtime reproduction; origin: filing matched codex:01a177f7-004b-7d00-8197-cb997c339b39 call line 154 and full created result line 157, id and at match 2026-10-26T11:31:17Z; human line 14 only selected an existing task, agent reported the incidental hook failure at lines 135-157, so origin agent stands; no origin_session on occurrence, no older-request stage; brief: docs/notes/2026-10-15-branch-only-document-links-brief.md
  provenance: {"harness_session":"codex:01a178c5-fc4f-79b0-847a-9aa4826e274e","harness_session_source":"CODEX_THREAD_ID"}
