---
id: tasks-608d02
title: Design bounded task-detail reads from the CLI consumer brief
status: todo
priority: 2
size: s
complexity: high
process: planned
created: 2026-10-25T16:02:29Z
updated: 2026-10-25T16:02:29Z
depends: []
parent: tasks-96c00c
tags: []
source: tasks-c750e1
agent: codex
origin: agent
---

Why: tasks-c750e1 reports that task-detail reads overflow a tool response. At e792f05, src/commands/show.rs returns the full Task through ShowFields in src/output.rs; Command::Show has no bounded-history option. A local read of tasks-65b733 was 35902 bytes, including 33401 bytes of notes (54 total, 47 retained-item entries). [ref:brief] records this evidence and the existing opt-in prime summary pattern.

Done: deliver a user-reviewed design and implementation plan for a discoverable bounded task-detail read, or a justified existing-command alternative. Settle the byte budget, which body/notes/relationships/inventory/ownership fields must remain complete, selection or paging of history, explicit omission disclosure, full-data recovery and typed failure when essential content cannot fit. Preserve ordinary show JSON and append-only stored notes. Include acceptance cases for one oversized note, a large body/inventory, warnings, claims, origin/creation provenance and retained-item history; do not silently classify old or retained notes as disposable. Assess the existing flow-trial-1 measurement gate before proposing any guidance adoption. No implementation or guide rollout in this design task; new CLI options start with ops's source table.

Where: src/commands/show.rs::{run,describe}, src/output.rs::{ShowFields,ShowOut}, src/cli.rs::Command::Show, src/model.rs::Task, tests/cli.rs, and the bounded-session-summary spec/README section linked by [ref:brief].

Ideas it wakes: On completion, update [ref:brief] and run `tasks note tasks-c750e1 --no-reviewed "<approved contract or existing-route recommendation and remaining work>"` before this result's branch merges.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-19-cli-consumer-reads-brief.md — measured output sizes, unresolved history contract and existing summary precedent

## Notes

- 2026-10-25T16:02:29Z (add): created
  provenance: {"harness_session":"codex:01a173c9-06d1-7b71-8e1d-5e90cc4839ab","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
