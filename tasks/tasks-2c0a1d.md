---
id: tasks-2c0a1d
title: "note on a task whose newer copy lives in another worktree writes to the older copy first and warns afterwards; refusing, or naming the worktree to run from, would keep the two copies from diverging"
status: done
priority: 2
created: 2026-09-29T11:36:20Z
updated: 2026-09-30T12:41:55Z
completed: 2026-09-30T12:41:55Z
depends: []
parent: tasks-c4ad8e
tags: [feedback, friction, "from:ns"]
agent: claude-code/claude-fable-5-1
---

Command: tasks note <id> "..." run from the main checkout while the record had been written from a task worktree. Result: the note landed in the main checkout's copy with the warning 'is newer than this copy ... this write may omit changes from that copy'. Expected: no write, or a write to the newer copy.

## Notes

- 2026-09-29T20:50:25Z (main): scope: briefed; refusing a write on a newer sibling reverses the work-claims 'signal, not a gate' rule, so it waits on design tasks-ab8d2d; parented to tasks-c4ad8e; brief: docs/notes/2026-09-29-cross-checkout-records-brief.md
- 2026-09-30T10:28:06Z (ab8d2d-record-home): Decided in tasks-ab8d2d (spec record-home §3): a write from a copy behind a sibling refuses as stale_copy, naming the newer checkout and a tasks -C retry. Implemented by tasks-9949f3.
- 2026-09-30T12:41:55Z (ab8d2d-record-home): done
  provenance: {"harness_session":"codex:01a0f212-0be0-71a0-99f7-5f8efd04e3b6","harness_session_source":"CODEX_SESSION_ID"}
- 2026-09-30T12:41:55Z (ab8d2d-record-home): stale_copy refusal before writes landed in c30bec2..bb13087
  provenance: {"harness_session":"codex:01a0f212-0be0-71a0-99f7-5f8efd04e3b6","harness_session_source":"CODEX_SESSION_ID"}
