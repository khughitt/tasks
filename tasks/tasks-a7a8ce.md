---
id: tasks-a7a8ce
title: "A read of a full task id fails with an io error when the cwd sits under an empty .git directory, instead of routing by prefix as it does from a plain non-git directory"
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-25T16:43:20Z
created: 2026-10-25T11:26:06Z
updated: 2026-10-25T16:43:20Z
depends: [tasks-e147b9]
parent: tasks-b34076
tags: [feedback, friction, "from:hq"]
agent: claude-code
---

Running 'tasks show <prefix>-<hex> --json' from a non-repository directory works, but from a directory whose ancestor holds an empty .git directory it exits 1 with kind io (git rev-parse failed, not a git repository). Expected: the id's prefix routes to its registered project regardless of cwd, as it does when no .git ancestor exists; the empty directory is not a repository. The error does name the empty directory and suggests removing it, which helped diagnosis.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-24-scratch-evidence-brief.md — shared routing evidence and existing investigation

## Notes

- 2026-10-25T16:43:20Z (main): scope: briefed; code at 220bb14 confirms open_id_ctx_unwarned falls back only on NoProject while GitTree::of returns io for an empty ancestor marker (diagnostic introduced by 328790d); no live reproduction in this pass. Reused tasks-e147b9, added this idea to its completion wake list and recorded the dependency under existing goal tasks-b34076; original body and origin unknown retained; origin: filing event unavailable (no creation occurrence or harness_session stamp in show); no origin_session supplied, so no older-request stage is identifiable; brief: docs/notes/2026-10-24-scratch-evidence-brief.md
  provenance: {"harness_session":"codex:01a173ed-fd7b-7fc3-8372-3ad7eca32e70","harness_session_source":"CODEX_THREAD_ID"}
