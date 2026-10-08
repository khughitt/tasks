---
id: tasks-27d515
title: "list --status doing omits claimed plan-step records that the stop-hook claim guard counts, so a session cannot enumerate its own live claims in one command before ending a turn"
status: dropped
priority: 2
created: 2026-10-06T11:16:26Z
updated: 2026-10-08T18:47:19Z
depends: []
parent: tasks-2fa8c6
tags: [feedback, friction, "from:tack"]
agent: claude-code/claude-fable-5-1
---

Reported symptom: list --status doing omits claimed plan-step records visible to the Stop guard, leaving the controller unsure which live claims it must settle.

Existing mechanism: tasks claims reads registered claim stores directly, including claims whose records exist only in a worktree. It has all-project scope and exposes claim identity/liveness; the report does not yet establish whether that output or its session selection is insufficient.

Next action: tasks-724c9e compares existing claim listings with the actual Stop-policy readers in isolated fixtures. Keep record status, claim liveness, worker identity, and controller aggregation distinct. Prefer existing-command guidance if sufficient; preserve this report as an idea until the investigation provides an evidence-backed disposition.

## Notes

- 2026-10-06T16:52:03Z (main): scope: bounded investigation tasks-724c9e checks existing tasks claims before proposing another enumeration surface; related Stop-policy delivery remains tasks-56b450.
- 2026-10-08T18:47:18Z (docs/claim-guard-design): Disposition from tasks-724c9e: use existing tasks claims with explicit live/session selection; list --status doing is a checkout record listing and intentionally misses worktree-only records. Disposable native CLI probe includes worktree-only and dead rows; no new listing API or ownership aggregation is required for this symptom. Evidence table in docs/notes/2026-09-30-delegated-identity-brief.md; genuine native root/worker grouping and hooks remain tasks-56b450.
- 2026-10-08T18:47:18Z (docs/claim-guard-design): dropped
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T18:47:18Z (docs/claim-guard-design): Covered by tasks-724c9e: existing tasks claims is the complete claim inventory; select live sessions explicitly. No product-code remedy is needed.
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
