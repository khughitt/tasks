---
id: tasks-6fcb58
title: Design list organization from the brief
status: todo
priority: 2
size: s
complexity: high
process: planned
created: 2026-10-24T11:22:46Z
updated: 2026-10-24T11:22:46Z
depends: []
parent: tasks-eceb1c
tags: []
source: "docs/notes/2026-10-24-list-organization-brief.md:design"
agent: codex
---

Question: What minimal CLI and presentation contract lets list group by project and nest selected children while preserving its filters, sorting, and JSON contract?

Where to start: Read [ref:brief] for the requests, verified behavior, alternatives, and affected paths. Inspect ListArgs in src/cli.rs, list/search dispatch, src/commands/list.rs, src/output.rs, src/query.rs, src/hierarchy.rs, and the named tests/cli.rs cases.

Bound: Design explicit display options versus the documented alternatives; settle filtered-out parents, arbitrary depth, group/root/sibling ordering, reverse, search parity, output formats, scopes, and parked/periodic/deferred combinations. Preserve both original requests. Do not implement during design. Follow the ops-first CLI vocabulary rule when the reviewed plan reaches parser work.

Expected result: A written design spec reviewed by the user, followed by a written implementation plan reviewed by the user, covering grouping alone, nesting alone, and their combination. Include acceptance cases for exact membership, missing ancestors, ordering, headers, width, colors, warnings, review markers, JSON stability, and refused combinations. Update the brief with decisions and verification routes. High complexity reflects interacting selection and presentation contracts; planned process retains both document review gates.

Ideas it wakes: Before this result's branch merges, run `tasks note tasks-fad561 --no-reviewed "<finding>"` and `tasks note tasks-1b31a4 --no-reviewed "<finding>"` with the reviewed findings.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-24-list-organization-brief.md — evidence and choices for list organization
