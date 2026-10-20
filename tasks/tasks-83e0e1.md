---
id: tasks-83e0e1
title: Make binary compatibility identifiable during upgrades
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-10-20T01:57:26Z
updated: 2026-10-20T01:57:26Z
depends: []
parent: tasks-0e432f
tags: []
source: docs/notes/2026-10-19-binary-compatibility-brief.md
agent: codex
---

Make coordinated upgrades verifiable without confusing stale readers with corrupt task records. First milestone: review a minimal binary-identification and record-diagnostic contract from [ref:brief].

Why: tasks-db7049 and tasks-476fd7 expose the same upgrade boundary from the operator and reader sides.

Done: the child design follow-up records a reviewed recommendation, both waiting ideas receive its findings and an evidence-backed disposition, and any implementation is scoped separately under the required reviews. No host upgrade or live activation is authorized by this goal.

Where: [ref:brief] contains the version history, timestamp-parser mechanism, alternatives and unknown original incident inputs.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-19-binary-compatibility-brief.md — version history, diagnostic constraints and decisions
