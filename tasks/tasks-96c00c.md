---
id: tasks-96c00c
title: Make CLI JSON practical for agents and dashboards
status: todo
priority: 2
size: m
complexity: mid
process: direct
created: 2026-10-20T02:02:34Z
updated: 2026-10-20T02:02:34Z
depends: []
parent: tasks-1f05a7
tags: []
source: docs/notes/2026-10-19-cli-consumer-reads-brief.md
agent: codex
---

Keep CLI JSON useful to agents and dashboards without making consumers parse records directly. First milestone: review a bounded session-summary contract and establish evidence for listing cost and project identity metadata.

Why: [ref:brief] connects oversized session output, reported cross-project read latency, and missing canonical project names/purposes. Done: the three waiting ideas receive the design or research findings, are reconsidered, and their retained work is completed or explicitly disposed of. Where: src/commands/list.rs, src/commands/projects.rs, src/output.rs, src/repo.rs and the handoff. Coordinate with deferred tasks-282666 without absorbing or bypassing its measurement gate.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-19-cli-consumer-reads-brief.md — cluster evidence and follow-ups
