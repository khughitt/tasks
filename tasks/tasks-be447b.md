---
id: tasks-be447b
title: Task complexity field for model routing
status: idea
priority: 2
created: 2026-09-12T10:18:55Z
updated: 2026-09-12T10:26:44Z
depends: []
tags: []
---

size measures volume, not the judgment a task demands; the two diverge in exactly the cases that matter for routing work to mid-tier models (large mechanical vs. small subtle).

Proposal: a coarse three-level complexity field, defined as the reasoning and judgment required to complete and verify the task given its current specification, plan, and repository context. Rated after preparation, so a high-complexity investigation can yield low-complexity implementation tasks.

- low: approach established, relevant context identified, correctness has a clear check.
- mid: bounded investigation or implementation choices; scope and acceptance criteria are clear.
- high: substantial discovery, subtle reasoning about interacting behavior, or unresolved architectural judgment.

CLI: add/edit --complexity <low|mid|high>; ready/next --max-complexity <level>; a column in list/ready. The tool stays ignorant of models; the level-to-provider mapping lives in each harness's instructions.

Semantics:
- Missing means unassessed, not low or high. --max-complexity excludes unassessed tasks and reports the count in warnings. Without the flag, selection is unchanged.
- The cutoff applies to every candidate path, including the parked work next prefers, so an escalated task never returns to the worker that escalated it.
- Within the eligible set, existing priority ordering is preserved.
- Plan steps are rated explicitly by the planner; a plan is evidence for a lower rating, not a default. No blanket low for --step tasks.
- Set at scoping time by the frontier model; curate fills gaps in the backlog. No need to classify everything before starting: ready work first.

Escalation: a worker parks on observable triggers (a decision the spec or plan leaves unresolved; interacting behavior outside the assessed scope; a bounded attempt with no progress or no way to establish correctness), records the evidence in a note, and raises complexity only when the evidence shows greater reasoning difficulty. Environment or credential failures use --reason environment and do not raise complexity. Consider a dedicated park reason (e.g. capability) so escalations are queryable via list --parked.

Calibration: the model stamp on done is latest-completion attribution only and does not record first attempts or rescues; start with attempt/escalation notes and occasional review, add structured tracking only if notes prove insufficient.

Complexity routes implementation effort; it does not set review requirements. Keep that distinction in harness instructions.

Rejected: reusing size; a routine tag (boolean, not a scale); a five-level scale (not reproducible); defaulting plan steps to low (turns 'has a plan' into 'safe to delegate').

## Notes

- 2026-09-12T10:26:44Z (main): Body revised after a second-opinion review: unassessed semantics, cutoff on next's parked path, explicit per-step ratings, observable escalation triggers, model-stamp caveat.
