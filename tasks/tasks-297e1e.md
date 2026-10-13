---
id: tasks-297e1e
title: Define supported repair for pending transactions with invalid review state
status: shelved
priority: 2
created: 2026-10-11T17:16:47Z
updated: 2026-10-13T11:10:41Z
depends: []
parent: tasks-a7460c
tags: []
source: "tasks-b8f3c9:pending-transaction-repair"
agent: codex
---

Evidence: the final review-state regressions reproduce pending move/rename snapshots containing reviewed: true on a non-idea. The new fresh/resumed validators correctly refuse before changing records, attachments, configuration, registry or journals. Ordinary corrective edits are also blocked while the transaction is pending, so recovery currently requires consistent manual record/journal/digest repair, which conflicts with the binary-only task writer policy. Investigate whether an existing supported recovery route suffices, and document it if so; otherwise design the smallest explicit CLI repair or rollback route with operator-visible effects and no data loss. Preserve strict validation and transaction digest checks. Start with the review-state spec section 3.2, moving/preflight.rs, moving/mod.rs, moving/journal.rs, rename/inventory.rs, rename/mod.rs, and the fresh/resumed refusal regressions in tests/review.rs. Bound: supported recovery and actionable diagnostics for this invalid-record condition; no general journal editor, silent repair, broader transaction redesign, or changes to the completed review-state behavior. This extends recovery tooling rather than treating fail-early refusal as a defect.

## Notes

- 2026-10-13T11:10:41Z (main): shelved: Wake when a pending move or rename journal is actually refused for reviewed on a non-idea, or a supported writer is found that can put reviewed on a non-idea (it was scoped as unreachable after the 2026-10-12 rollout)
