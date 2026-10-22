---
id: tasks-e90b4f
title: Design dependency satisfaction reporting from the brief
status: todo
priority: 2
size: s
complexity: high
process: planned
created: 2026-10-22T23:04:09Z
updated: 2026-10-22T23:04:09Z
depends: []
parent: tasks-606dd3
tags: []
source: "tasks-1bbe49:dependency-contract"
agent: codex
---

Why: tasks-1bbe49 needs a canonical dependency-met answer independent of ready membership. [ref:brief] establishes the existing rule and explains why resolved only means found. Output shape and consumer batching requirements remain unresolved.

Done: produce a user-reviewed design, then a user-reviewed implementation plan, for the smallest read-only contract that reports dependency satisfaction using tracker resolution. Prefer extending an existing read unless a dedicated query is justified. Preserve resolved semantics, scan-first/local-versus-registered authority, alias handling and moved_to forwarding. Specify empty dependencies, open/done/dropped/shelved targets, recurring done before and after restart, unreachable targets, move cycles and typed errors. Show how dependency-met work can still be excluded by claims, parks, deferrals or goals. No claim acquisition, scheduler or readiness-policy change. If a command/flag is needed, land the ops CLI table rows before adopting them here.

Where to look: [ref:brief]; src/commands/list.rs::resolve_dependency and ready_tasks; src/commands/show.rs::describe; src/output.rs::DepInfo; src/tombstone.rs::follow; src/query.rs::readiness; src/commands/mod.rs::open_deps; tests/cli.rs dependency and recurring cases. Confirm the consumer's single-task or batch use case during design without treating the originating report as a reproduced benchmark.

Ideas it wakes: On completion, update [ref:brief] with the reviewed decision and run `tasks note tasks-1bbe49 --no-reviewed "<finding>"` before this result's branch merges. The idea then returns to scope for implementation assessment.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-22-dependency-state-brief.md — observed dependency rules and remaining contract decisions
