---
id: tasks-e7424b
title: Design bounded session summaries from the CLI consumer brief
status: doing
priority: 2
size: m
complexity: high
process: planned
owner: main
created: 2026-10-20T02:03:05Z
updated: 2026-10-24T13:59:55Z
started: 2026-10-24T13:59:41Z
depends: []
parent: tasks-96c00c
tags: []
source: tasks-4ca7bd
agent: codex
---

Why: tasks-4ca7bd reports session-start output truncation; [ref:brief] records a 101,001-byte local response whose roadmap is larger than its ready list. The summary must preserve useful work discovery and disclose omissions.

Done: a user-reviewed design and implementation plan specify section/byte or row budgets, essential session fields, recovery of complete data, default versus opt-in behavior and compatibility, with acceptance examples for large roadmaps, lanes, claims, warnings and empty projects. Recommend a bounded summary with an explicit full-data route; assess a default migration rather than silently changing JSON. Check the measurement gate recorded by tasks-282666 before rollout. New options start in ops's CLI table. No implementation in the design task before its document reviews.

Where: src/commands/list.rs::prime, src/output.rs::{PrimeOut,TaskSummary}, src/cli.rs::Command::Prime, tests/cli.rs prime tests, skills/tasks/SKILL.md and README.md.

Ideas it wakes: On completion, update the brief and run `tasks note tasks-4ca7bd --no-reviewed "<approved summary contract and remaining implementation>"` before this result's branch merges.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-19-cli-consumer-reads-brief.md — evidence and question this follow-up must resolve

## Notes

- 2026-10-24T13:59:41Z (main): started
  provenance: {"harness_session":"codex:01a16e34-fd27-7e51-b10c-c1250b33416d","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-24T13:59:55Z (main): Process planned; design and user review first, then implementation plan and user review; no product implementation in this task. Recommend opt-in bounded prime with unchanged default during flow-trial-1; assess default migration explicitly. Trial arm: not enrolled. Workspace .worktrees/tasks-e7424b; planned-work baseline deferred until execution Setup.
