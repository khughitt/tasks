---
id: tasks-b34076
title: Preserve task evidence when closing work from scratch
status: todo
priority: 2
size: m
complexity: mid
process: direct
created: 2026-10-24T11:32:15Z
updated: 2026-10-24T11:32:15Z
depends: []
parent: tasks-0e432f
tags: []
source: docs/notes/2026-10-24-scratch-evidence-brief.md
agent: codex
---

Why: work prepared in host-local scratch can close with evidence unrecorded, while attaching from that scratch may encounter an unrelated broken git marker. [ref:brief] records both reports and the shared-routing evidence. First milestone: ship the bounded close-time warning and establish the smallest safe remedy for routing a full task id from scratch. Done: the warning task is delivered, the routing investigation records its matrix and recommendation, and its waiting idea is reconsidered with the findings; file any required implementation after that verdict. Where to look: the brief, src/commands/status.rs::close, src/commands/mod.rs::open_id_ctx_unwarned, and src/link.rs::GitTree::of. Keep the existing parked-resume-input effort and attachment-filing research separate.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-24-scratch-evidence-brief.md — evidence, close-time scope and routing questions
