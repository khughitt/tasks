---
id: tasks-634c4a
title: Continuity in note
status: done
priority: 2
size: s
complexity: mid
process: direct
created: 2026-09-22T14:15:49Z
updated: 2026-09-22T16:25:28Z
completed: 2026-09-22T16:25:28Z
depends: [tasks-95e03a]
parent: tasks-8921f4
tags: []
model: "claude-opus-5[1m]"
agent: "claude-code/claude-opus-5[1m]"
plan: docs/plans/2026-09-22-relay-ancestry-identity.md
step: "Task 7: Continuity in `note`"
---

## Notes

- 2026-09-22T16:25:28Z (design/relay-ancestry): done
  provenance: {"harness_session":"claude-code:3b265943-33e2-4466-947a-581d11a9cbb0","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-22T16:25:28Z (design/relay-ancestry): note keeps its own shape (it guards nothing and refuses nothing) while gaining continuity: resolve_for_guard carries a relay-level failure so the note still lands, and with relay off it raises exactly where identity did before, so an unresolvable native identity is still fatal. The heartbeat now follows Ctx::ownership rather than a session-string comparison, and when a claim exists that could not be established because this session's own identity did not resolve, the note says the heartbeat was skipped and names the holding session — silence there would be indistinguishable from an ordinary foreign note. Adds the harness shim to tests/common: a symlink to /bin/sh named for the harness supplies the comm, which a copy cannot do reliably because a sibling test thread forking across the copy's open descriptor makes the execve fail with ETXTBSY.
  provenance: {"harness_session":"claude-code:3b265943-33e2-4466-947a-581d11a9cbb0","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
