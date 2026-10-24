---
id: tasks-eceb1c
title: Organize filtered lists by project and parent
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-10-24T11:22:26Z
updated: 2026-10-24T11:22:26Z
depends: []
parent: tasks-1f05a7
tags: []
source: docs/notes/2026-10-24-list-organization-brief.md
agent: codex
---

Why: Readers need filtered task lists organized by project and parent without surprising changes to membership or sorting. [ref:brief] records the two requests, current code, alternatives, and open design choices.

Done: A reviewed design and implementation plan settle both requests and their interactions; the resulting implementation is verified or each idea has an explicit disposition.

Where: src/cli.rs::ListArgs, src/commands/list.rs, src/query.rs, src/output.rs, src/hierarchy.rs, tests/cli.rs, and the ops-owned CLI surface. This goal belongs inside the existing presentation lane.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-24-list-organization-brief.md — evidence and choices for list organization
