---
id: tasks-e7424b
title: Design bounded session summaries from the CLI consumer brief
status: doing
priority: 2
size: m
complexity: high
process: planned
owner: design/tasks-e7424b
created: 2026-10-20T02:03:05Z
updated: 2026-10-24T14:47:00Z
started: 2026-10-24T13:59:41Z
depends: []
parent: tasks-96c00c
tags: []
source: tasks-4ca7bd
agent: codex
spec: docs/specs/2026-10-24-bounded-session-summary-design.md
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
- 2026-10-24T14:00:07Z (design/tasks-e7424b): resumed
  provenance: {"harness_session":"codex:01a16e34-fd27-7e51-b10c-c1250b33416d","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-24T14:07:34Z (design/tasks-e7424b): Draft contract: opt-in JSON-only prime --summary, 16384-byte stdout cap, existing row shapes with whole-row eviction, complete warnings/halts and explicit omission/full-data recovery; default and trial consumers unchanged. just test-fast passed 2026-10-24; full planned-work baseline deferred to execution. Canonical measurement: /mnt/ssd3/work/tasks/tasks-e7424b/prime-before.json, sha256 3bda742e27be7913e3475bed7f9d039e09365ec16ad69dd98c40b0f08ab6cd67.
- 2026-10-24T14:08:35Z (design/tasks-e7424b): Verification before frozen spec review: just test-fast passed 1665 tests across 14 test binaries, 0 failed, 4 ignored; tasks check and git diff --check passed; pre-commit hygiene/fmt/clippy/tasks checks passed. Frozen spec revision 5d95ea1fe22f07439f38e5b82dbab1e256770ba8:docs/specs/2026-10-24-bounded-session-summary-design.md sent for read-only builder check. User design review remains required before writing the plan.
- 2026-10-24T14:09:12Z (design/tasks-e7424b): review: spec round 1 — verdict: accept; findings: none; reviewer: codex/gpt-6.1-sol
- 2026-10-24T14:09:14Z (design/tasks-e7424b): attached: spec-review-r1-report.md (463 bytes): Builder acceptance of frozen spec 5d95ea1; no important questions; user review still required
- 2026-10-24T14:09:16Z (design/tasks-e7424b): parked (waiting on user, review): User reviews frozen spec 5d95ea1fe22f07439f38e5b82dbab1e256770ba8 at .worktrees/tasks-e7424b/docs/specs/2026-10-24-bounded-session-summary-design.md. After user approval, agent resumes here and writes the implementation plan for separate user review; no implementation in this design task. Measurement input: /mnt/ssd3/work/tasks/tasks-e7424b/prime-before.json, sha256 3bda742e27be7913e3475bed7f9d039e09365ec16ad69dd98c40b0f08ab6cd67. Builder report attached as spec-review-r1-report.md.
  provenance: {"harness_session":"codex:01a16e34-fd27-7e51-b10c-c1250b33416d","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-24T14:28:17Z (design/tasks-e7424b): resumed
  provenance: {"harness_session":"codex:01a16e34-fd27-7e51-b10c-c1250b33416d","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-24T14:28:20Z (design/tasks-e7424b): review: spec round 2 — verdict: revise; findings: Important 2, Minor 3; reviewer: claude-code/claude-opus-5-5
- 2026-10-24T14:33:23Z (design/tasks-e7424b): attached: spec-review-r2-report.md (5286 bytes): Full round 2 review by claude-code/claude-opus-5-5 against frozen spec 5d95ea1: Important 2, Minor 3
- 2026-10-24T14:33:25Z (design/tasks-e7424b): attached: review-ledger-r2.md (3333 bytes): Five accepted findings resolved in frozen revision 76501bd; reasons, costs and verification; no deferred minors
- 2026-10-24T14:33:26Z (design/tasks-e7424b): Round 2 findings all accepted and addressed in frozen spec 76501bdd333e5c6b49758fdec1b6662b5c57fa6f. Cap now 12288 bytes: estimated <=4096 payload tokens plus 1000 framing tokens leaves 904 against observed 6000-token tool cap; actual consumer pilot remains required. Binary/README/tests separated from gated skill/hooks/session adoption; flow-trial-1 owner is user. Explicit closeout recovery, running-executable full argv, and visible summary_too_large full-file recovery added. Full review and five Why/Cost rulings attached. just test-fast: 1665 passed, 0 failed, 4 ignored; tasks check, diff check and pre-commit checks passed. No implementation or plan; revised spec needs user review.
- 2026-10-24T14:33:28Z (design/tasks-e7424b): parked (waiting on user, review): User reviews revised frozen spec 76501bdd333e5c6b49758fdec1b6662b5c57fa6f at .worktrees/tasks-e7424b/docs/specs/2026-10-24-bounded-session-summary-design.md; compare against 5d95ea1. Agent resumes here after design approval to write the implementation plan for separate user review. Full review and dispositions are attachments spec-review-r2-report.md and review-ledger-r2.md. Canonical measurement: /mnt/ssd3/work/tasks/tasks-e7424b/prime-before.json, sha256 3bda742e27be7913e3475bed7f9d039e09365ec16ad69dd98c40b0f08ab6cd67. No product implementation or consumer rollout performed.
  provenance: {"harness_session":"codex:01a16e34-fd27-7e51-b10c-c1250b33416d","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-24T14:42:56Z (design/tasks-e7424b): resumed
  provenance: {"harness_session":"codex:01a16e34-fd27-7e51-b10c-c1250b33416d","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-24T14:42:59Z (design/tasks-e7424b): review: spec round 3 — verdict: accept; findings: Minor 2; reviewer: claude-code/claude-opus-5-5
- 2026-10-24T14:43:01Z (design/tasks-e7424b): User approved frozen design 76501bdd333e5c6b49758fdec1b6662b5c57fa6f for planning. Round 3 minors to absorb in plan: isolate README summary guidance from every skill-linked section until gated adoption; reject a missing/replaced running-executable recovery path with typed I/O error and cover Linux deleted-executable case. No implementation authorized before plan review.
- 2026-10-24T14:47:00Z (design/tasks-e7424b): Planning uses the existing task-references plan's design-task closeout convention: plan is an output reference while under review; after approval, create a separate implementation goal and one step record per Task heading, then close this design task independently. Proposed sequence: ops CLI row, tasks binary/isolated README, gated skill adoption. No existing automated prime caller was found in the inspected session hook; no new startup hook is planned. Recommend inline execution after plan approval because the steps are sequential and share one command contract.
