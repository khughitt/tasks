---
id: tasks-cf8687
title: Task start reports conflicting Codex session and thread identities in a dispatched worker and falls back to Unix session identity; worker claims then lose harness provenance.
status: idea
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-01T10:18:50Z
updated: 2026-10-02T14:37:27Z
depends: []
parent: tasks-2fa8c6
tags: [feedback, friction, "from:tack"]
agent: codex
---

Why: This newer report also describes conflicting native Codex IDs and Unix fallback, alongside omitted harness provenance. Those symptoms match two distinct documented resolution paths; they do not establish which component delivered the wrong signals.

Next action: Join the existing bounded investigation tasks-1ece46 and goal tasks-2fa8c6. Preserve this report as separate evidence until a capture identifies the native variable relationship, relay mode, process lifetime, and ownership semantics. No new investigation or speculative precedence fix is needed. note --stamp in tasks-0005a6 supplies optional metadata but cannot rescue conflicting native identities.

Where to look: docs/notes/2026-09-30-delegated-identity-brief.md; src/claims.rs::identity_from and liveness_with; src/provenance.rs::resolve_from; src/relay/resolve.rs; existing Codex/relay fixtures. Record claim liveness separately from note provenance.

Original report: Task start reports conflicting Codex session and thread identities in a dispatched worker and falls back to Unix session identity; worker claims then lose harness provenance.

## Notes

- 2026-10-02T14:37:26Z (main): scope: briefed; add the newer conflict/provenance report to existing tasks-2fa8c6 and research tasks-1ece46 without inferring a shared cause; brief: docs/notes/2026-09-30-delegated-identity-brief.md
