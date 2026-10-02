---
id: tasks-ead38f
title: Record the all-of tag-filter exception in the shared CLI vocabulary
status: done
priority: 2
size: s
complexity: mid
process: direct
owner: docs/tag-exception
created: 2026-09-30T18:32:25Z
updated: 2026-10-02T19:03:21Z
started: 2026-10-02T19:00:18Z
completed: 2026-10-02T19:03:21Z
depends: []
parent: tasks-cea445
tags: [cli]
model: claude-opus-5-5
agent: codex
---

list and ready keep --tag all-of (tasks-964c95 left it); the shared vocabulary says repeatable filters widen. Decide and change or record an exception on the cli.toml rows.

Why: The reviewed and implemented task-filter design explicitly retains all-of tag filtering to preserve callers. src/filter.rs::TaskFilter::matches and src/cli.rs::FilterArgs agree, but the list and ready tag-filter rows in tools/cli.toml carry no exception.

Done: Retain runtime and JSON behavior. Record why repeated --tag means every requested tag on the list and ready rows in the authoritative ops CLI inventory, then publish the matching vendored inventory through its existing workflow. Inspect that workflow and destination changes before publishing; do not edit only the tasks vendor or overwrite unrelated work. Confirm list --parked inherits the list exception. The reviewed filter design settles the choice; no new design or plan is needed for this bounded documentation change.

Verification: The source inventory validator/conformance checks accept the exception and published copies match the authority. Existing tags_are_all_of_and_scalars_are_exact coverage and a live list with one versus two tags still demonstrate intersection; run through the relevant project's test front door. Repeats of priority, size, complexity, and process continue to widen.

Where to look: tools/cli.toml (header, list and ready rows), src/filter.rs, src/cli.rs, docs/specs/2026-09-30-task-filters-design.md (Flags and Boundaries), tasks-964c95 and its rollout notes. The authoritative ops workflow was not inspected in this pass; inspect it before executing the inventory change.

## Notes

- 2026-10-02T15:14:13Z (main): scope: scoped; retain reviewed all-of tag filtering and record the CLI inventory exception through the authoritative publication workflow; priority 2, size s, complexity mid, process direct; brief: docs/notes/2026-10-02-tag-vocabulary-brief.md
- 2026-10-02T19:00:18Z (main): started
  provenance: {"harness_session":"claude-code:4315d401-9c79-47bd-a5c6-86ff6afb8586","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-02T19:02:30Z (docs/tag-exception): resumed
  provenance: {"harness_session":"claude-code:4315d401-9c79-47bd-a5c6-86ff6afb8586","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-02T19:03:21Z (docs/tag-exception): done
  provenance: {"harness_session":"claude-code:4315d401-9c79-47bd-a5c6-86ff6afb8586","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-02T19:03:21Z (docs/tag-exception): list/ready --tag rows carry an all-of exception in the shared CLI inventory (ops 3808a07, adopted via vendored adopt); list --parked inherits the list row; filter design boundary updated; behavior unchanged (one tag A,B; two tags B; -p repeats widen)
  provenance: {"harness_session":"claude-code:4315d401-9c79-47bd-a5c6-86ff6afb8586","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
