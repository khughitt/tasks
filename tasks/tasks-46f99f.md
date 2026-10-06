---
id: tasks-46f99f
title: Keep Unix fallback PIDs out of persisted takeover summaries
status: done
priority: 2
size: xs
complexity: low
process: direct
owner: fix/tasks-46f99f
created: 2026-10-06T10:41:53Z
updated: 2026-10-06T11:10:23Z
started: 2026-10-06T11:06:22Z
completed: 2026-10-06T11:10:23Z
depends: []
parent: tasks-ea2a79
tags: []
source: tasks-0238e4
agent: codex
---

Why: tasks-0238e4 removed separate host/PID/worktree fields from summaries, but Ctx::takeover_summary still persists claim.session. Unix fallback identities encode a PID as sid:<pid>, so the completed privacy guarantee is incomplete.

Done: the shared takeover-summary path omits the numeric Unix fallback identifier from generated task notes while retaining owner and forced/stale reason; ordinary logical session identifiers remain useful. Detailed command warnings and the local claim store retain complete identity/process information. Preserve existing notes and the diagnostic formatter.

Where: src/commands/mod.rs::Ctx::takeover_summary, both claim_guard takeover callers, and the takeover regression fixtures in tests/cli.rs. The note-integrity brief records the distinction between new-note prevention and historical publishing cleanup.

Verification: exercise a distinctive sid:<pid> in forced-live and stale takeover notes; assert its numeric PID is absent from persisted summaries and present in detailed diagnostics. Retain ordinary session-summary coverage. Run just test-one --test cli <filter>, then just test-fast before implementation commit.

## Notes

- 2026-10-06T10:41:53Z (main): concerns: tasks-0238e4 defect — Persisted Unix fallback session identifiers still expose the PID despite the completed takeover-summary privacy requirement.
- 2026-10-06T11:06:22Z (main): started
  provenance: {"harness_session":"codex:01a110e0-1db2-71e0-a82e-64c5c72e2a27","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-06T11:06:31Z (fix/tasks-46f99f): resumed
  provenance: {"harness_session":"codex:01a110e0-1db2-71e0-a82e-64c5c72e2a27","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-06T11:08:09Z (fix/tasks-46f99f): Confirmed both claim_guard takeover branches persist Ctx::takeover_summary, which copies numeric sid identities; redact only numeric Unix session IDs in this formatter, retaining logical names and detailed diagnostics.
- 2026-10-06T11:10:17Z (fix/tasks-46f99f): review: impl round 1 — verdict: accept; findings: none; reviewer: codex
- 2026-10-06T11:10:17Z (fix/tasks-46f99f): Verified: baseline takeover_note tests passed; Unix PID regression failed before fix and passed after; all seven takeover tests, just test-fast and just check passed. Review exclusions match task scope: historical redaction and caller-supplied identifying prose remain separate work.
- 2026-10-06T11:10:23Z (fix/tasks-46f99f): done
  provenance: {"harness_session":"codex:01a110e0-1db2-71e0-a82e-64c5c72e2a27","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-06T11:10:23Z (fix/tasks-46f99f): Redacted numeric Unix fallback identity in shared takeover summaries; preserved logical names, owner/reason and detailed diagnostics. Live/stale regression plus takeover tests, fast suite, hygiene and independent review passed; CLI reinstalled.
  provenance: {"harness_session":"codex:01a110e0-1db2-71e0-a82e-64c5c72e2a27","harness_session_source":"CODEX_SESSION_ID"}
