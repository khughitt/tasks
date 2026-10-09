---
id: tasks-6a0050
title: Release a recurring task's park once its recorded worktree is gone
status: done
priority: 2
size: s
complexity: mid
process: direct
owner: fix/recurring-release
created: 2026-10-09T13:50:09Z
updated: 2026-10-09T13:59:51Z
started: 2026-10-09T13:57:22Z
completed: 2026-10-09T13:59:50Z
depends: []
parent: tasks-984769
tags: [cli]
source: tasks-b9b10e
model: claude-opus-5-5
agent: claude-code/claude-opus-5-5
---

Why: tasks-b9b10e's probe (attached there) parked a recurring task from a worktree under host A, closed it under host B, then removed the worktree. On A, `tasks done <id>` from the main checkout refuses with validation "already done here but has work in <wt>; finish that occurrence in its owning checkout", before and after removal, and `park` refuses invalid_transition. Once the worktree is gone that checkout no longer exists, so no command clears the park; the only path, `start` then `done`, records a spurious occurrence (new completed, "completed; next due" note, model cleared). Inside the worktree, after merging main, the same retry releases with the "already completed" warning and changes only updated.

Done: in src/commands/mod.rs::transition's recurring retry guard, a claim or park whose recorded worktree no longer exists on this host (claims and parks are host-local) no longer blocks; the retry releases it with the existing warning, records no occurrence, and changes only updated. The guard still refuses while the recorded worktree exists, and its message also says to integrate the closed record there first. Cover both in tests/cli.rs with two XDG_STATE_HOME stores, asserting record bytes and store state. JSON shapes unchanged.

## Notes

- 2026-10-09T13:57:22Z (main): started
  provenance: {"harness_session":"claude-code:b9b50abb-49d0-48fa-8632-4d295da3f9b2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-09T13:57:24Z (fix/recurring-release): resumed
  provenance: {"harness_session":"claude-code:b9b50abb-49d0-48fa-8632-4d295da3f9b2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-09T13:59:50Z (fix/recurring-release): Absence is tested with Path::try_exists on the recorded worktree; a worktree on storage that is unmounted at retry time reads as gone and is released. Only the claim/park entry goes, never the worktree's files, so the cost is a lost park note
- 2026-10-09T13:59:50Z (fix/recurring-release): done
  provenance: {"harness_session":"claude-code:b9b50abb-49d0-48fa-8632-4d295da3f9b2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-09T13:59:50Z (fix/recurring-release): A recurring done retry releases a claim or park whose recorded worktree no longer exists on this host, changing only updated; while it exists the refusal stands and now says how to release a covered park. Periodic spec amended
  provenance: {"harness_session":"claude-code:b9b50abb-49d0-48fa-8632-4d295da3f9b2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
