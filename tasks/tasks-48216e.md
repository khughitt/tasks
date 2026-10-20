---
id: tasks-48216e
title: Establish the project identity JSON contract for name and purpose
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-20T02:03:08Z
updated: 2026-10-20T02:03:08Z
depends: []
parent: tasks-96c00c
tags: []
source: tasks-6845e6
agent: codex
---

Question: What existing identity schema and root-selection rules should supply optional `name` and `purpose` in `tasks projects`, and what concrete read/error contract meets its consumer's need?
Where to start: [ref:brief], tasks-6845e6's preserved source `tui:docs/notes/2026-09-15-project-identity-brief.md`, schema task ops-03f3e3, consumer tui-0fa4d0, src/commands/projects.rs::run, src/output.rs::ProjectRow and src/scope.rs's linked-project model.
Bound: Read the current owner schema and consumer decision; identify field types and validation, unknown-field policy, and registered-record root versus linked-code checkout authority. Check fixtures for valid, absent, malformed and unreadable metadata and unreachable roots. Preserve the recorded 2026-10-17 user decision to expose optional name/purpose; recommend typed failures for invalid existing metadata and leave aliases and pretty-table changes out unless the evidence requires them. No parser, new dependency or consumer implementation.
Expected result: Record the verified schema/version or commit, authoritative path, exact omission/error rules and acceptance fixtures on this task and in the brief; recommend whether that settles a direct implementation scope. Distinguish unavailable evidence from lookup failure. Coordinate with tasks-282666's existing gate without expanding into its consumer migration.
Ideas it wakes: On completion, run `tasks note tasks-6845e6 --no-reviewed "<verified identity contract and recommended implementation scope>"` before this result's branch merges.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-19-cli-consumer-reads-brief.md — evidence and question this follow-up must resolve
