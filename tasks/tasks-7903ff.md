---
id: tasks-7903ff
title: Design blocker context in prime from the blocker brief
status: todo
priority: 2
size: s
complexity: high
process: planned
created: 2026-10-26T14:59:55Z
updated: 2026-10-26T14:59:55Z
depends: []
parent: tasks-46d207
tags: [cli]
source: tasks-14ba63
agent: codex
origin: agent
---

Why: the first blocker delivery shipped a dedicated view; its follow-up asks whether a short prime section would help discovery. [ref:brief] records that prime now has an opt-in 12,288-byte summary, while startup adoption remains trial-gated. Placement, scope and budget are unresolved design choices, not permission to expand the default JSON.

Done: write a reviewable design comparing guidance to the dedicated view, compact blocker rows, and retained dedicated-only behavior. Choose the full-prime/summary/pretty surfaces deliberately; define dependent scope, partial-coverage and lower-bound labels, report-versus-picker wording, row cap, omitted/recovery metadata, byte-budget eviction and failure behavior. Project one mixed local/cross-project fixture and one oversized mandatory-metadata fixture through the full verdict, retaining the summary's 12,288-byte limit and valid complete JSON. Specify scan reuse and a bounded runtime check for implementation; reuse the pure blocker analysis rather than spawning tasks blockers or adding another project walker. Keep ready/next order, eligibility and startup-trial gates intact. Bring the design to the user; implementation requires a separately reviewed plan and scoped steps. No product-code or agent-startup change is part of this design task.

Where to start: [ref:brief], src/commands/list.rs::prime, src/commands/blockers.rs, src/blockers.rs, src/prime_summary.rs::bound, src/output.rs::PrimeOut, src/format.rs and tests/cli.rs. Read docs/specs/2026-10-24-bounded-session-summary-design.md against the shipped code, not its stale draft status. Coordinate with tasks-65b733 and tasks-dc696d; no dependency on their startup rollout is needed merely to design this extension.

Ideas it wakes: on completion record the result and recommendation on this task and in [ref:brief], then run tasks note tasks-14ba63 --no-reviewed "<design result and remaining decision>" before this result's branch merges. Choosing dedicated-view guidance instead of new rows is a valid result and must be explicit.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-25-blocker-ranking-brief.md — current report contracts and follow-on design questions

## Notes

- 2026-10-26T14:59:55Z (add): created
  provenance: {"harness_session":"codex:01a178b3-d016-7562-a8e1-983f62cc0118","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
