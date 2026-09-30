---
id: tasks-0238e4
title: Keep hostnames and absolute worktree paths out of generated takeover notes
status: doing
priority: 2
size: s
complexity: mid
process: direct
owner: main
created: 2026-09-19T13:13:54Z
updated: 2026-09-30T14:45:49Z
started: 2026-09-30T14:45:49Z
depends: []
parent: tasks-ea2a79
tags: [feedback, idea, "from:tui"]
agent: claude-code/claude-opus-5
---

Why: start copies a detailed takeover warning into the tracked note history; Ctx::describe_claim includes the displaced claim's host, PID, and absolute worktree. The current park implementation already keeps host/worktree in the out-of-git Park record, not its generated note.

Done: persist a takeover summary identifying the displaced session and owner and whether takeover was forced or stale, without host, PID, absolute worktree, or diagnostic text that may embed them. Keep detailed claim diagnostics in command warnings/errors and the local claim store. Cover both forced-live and automatic-stale takeovers. Leave historical notes for the separate redaction design. Keep park's generated note free of injected machine details. Update the work-claims documentation to distinguish persisted summaries from local diagnostics.

Where to look: src/commands/status.rs::start, src/commands/mod.rs::Ctx::describe_claim and the transition claim guard, src/commands/park.rs, docs/specs/2026-09-05-work-claims-design.md, and claim/takeover integration tests in tests/cli.rs. Do not change the diagnostic formatter globally, since its other callers need the full claim details.

Verification: use distinctive host/path/PID values in takeover fixtures; assert they are absent from persisted generated notes, that the displaced session remains identifiable, and that detailed warnings and claim/park records retain their diagnostics. Exercise forced-live, stale, and ordinary park paths through just test-one; run just test-fast before the implementation commit.

Original report: tasks start --force writes 'took over session … host <name>, worktree </abs/path>' into the record, and park notes carry the same fields. In a public repository these expose the sync root and hostname. Consider writing the worktree relative to the project root (.worktrees/<name>) and omitting the host from the generated note text, keeping both in the claim store where they are needed.

## Notes

- 2026-09-30T09:53:59Z (main): scope: scoped; narrowed to the observed takeover leak; park already persists machine details only outside git; s/mid/direct; brief: docs/notes/2026-09-30-note-integrity-brief.md
- 2026-09-30T14:45:49Z (main): started
  provenance: {"harness_session":"claude-code:30d2809d-2d77-4c34-b242-55903be96b97","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
