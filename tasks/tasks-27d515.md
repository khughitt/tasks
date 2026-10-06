---
id: tasks-27d515
title: "list --status doing omits claimed plan-step records that the stop-hook claim guard counts, so a session cannot enumerate its own live claims in one command before ending a turn"
status: idea
priority: 2
created: 2026-10-06T11:16:26Z
updated: 2026-10-06T16:52:04Z
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
