---
id: tasks-46f99f
title: Keep Unix fallback PIDs out of persisted takeover summaries
status: doing
priority: 2
size: xs
complexity: low
process: direct
owner: main
created: 2026-10-06T10:41:53Z
updated: 2026-10-06T11:06:22Z
started: 2026-10-06T11:06:22Z
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
