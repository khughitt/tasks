---
id: tasks-616cd7
title: Task start reports conflicting Codex session variables and falls back to Unix identity for delegated agents.
status: idea
priority: 2
size: s
complexity: mid
process: direct
created: 2026-09-27T17:11:41Z
updated: 2026-10-02T14:37:27Z
depends: []
parent: tasks-2fa8c6
tags: [feedback, friction, "from:beliefs", "from:tack"]
agent: codex
---

Why: A later feedback recurrence readmitted this previously briefed report. It again describes conflicting worker native IDs and Unix fallback, but supplies no controller/worker capture or relay-mode evidence.

Next action: Reuse tasks-1ece46 under existing goal tasks-2fa8c6; its bounded capture already tests missing/conflicting/agreed native inputs, claim liveness, and note provenance independently. A repeated symptom is not evidence that tasks should choose one conflicting ID or merge the worker with its controller. Keep this an idea until the investigation records its finding.

Where to look: docs/notes/2026-09-30-delegated-identity-brief.md; src/claims.rs::identity_from; src/provenance.rs::resolve_from; src/relay/resolve.rs; existing Codex/relay fixtures. The investigation must publish only variable presence/equality relationships, not private identity values.

Original report: Task start reports conflicting Codex session variables and falls back to Unix identity for delegated agents.

Recurrence: feedback from tack: Subagent task start reports conflicting Codex session variables and falls back to a shared Unix session identity.

## Notes

- 2026-09-30T09:58:28Z (main): scope: briefed; overlaps the conflicting-ID report but lacks capture evidence; share one investigation rather than infer an identical cause; waits on tasks-1ece46; brief: docs/notes/2026-09-30-delegated-identity-brief.md
- 2026-09-30T21:46:30Z (feedback): feedback from tack: Subagent task start reports conflicting Codex session variables and falls back to a shared Unix session identity.
- 2026-10-02T14:37:26Z (main): scope: briefed; recurrence adds no native-ID capture, so reuse tasks-1ece46 and tasks-2fa8c6; claim identity and note provenance remain separate; brief: docs/notes/2026-09-30-delegated-identity-brief.md
