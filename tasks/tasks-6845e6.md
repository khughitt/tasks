---
id: tasks-6845e6
title: tasks projects reports each project's name and purpose from its identity.toml
status: idea
priority: 2
created: 2026-10-17T21:04:56Z
updated: 2026-10-20T02:02:17Z
depends: []
tags: []
source: "tui:docs/notes/2026-09-15-project-identity-brief.md"
agent: claude-code/claude-opus-5-5
---

Why: tasks-tui's Projects view shows each checkout's directory basename as the project name; the canonical name and one-line purpose now live in each project root's `identity.toml` (ops's per-project identity schema, ops-03f3e3; ops also mirrors them in `identity-mirror.toml`). The TUI may only consume the `tasks` JSON contract, so it waits on this (tui-0fa4d0). Decided by the user 2026-10-17: `tasks projects` exposes `name` and `purpose`, read from the root's `identity.toml`, optional until every project adopts one (16 of 34 registered roots hold one on 2026-10-17).

Shape: two optional fields on `ProjectRow` (src/output.rs), filled in src/commands/projects.rs beside `color`, omitted when the root has no `identity.toml` or the project is unreachable, like `color` and `checkout`.

Left for scoping here: a malformed or unreadable `identity.toml` (error vs a `warnings` entry); whether `aliases` travel too (the TUI's lean: filter-only, if at all); whether the pretty table shows the name.

Scope handoff: [ref:brief] records the current code, unresolved contract or measurement question, and the follow-up that must answer it before implementation is scoped.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-19-cli-consumer-reads-brief.md — current evidence, constraints and next questions
