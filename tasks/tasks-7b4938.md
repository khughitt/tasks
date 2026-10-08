---
id: tasks-7b4938
title: "Verify worker turns, release and completion-proven recovery"
status: done
priority: 1
size: m
complexity: high
process: direct
owner: docs/claim-guard-design
created: 2026-10-08T15:42:33Z
updated: 2026-10-08T21:13:18Z
started: 2026-10-08T20:57:29Z
completed: 2026-10-08T21:11:37Z
depends: [tasks-f2b0ad]
parent: tasks-56b450
tags: []
source: "tasks-56b450:claim-guard-policy:task-5"
agent: codex
spec: docs/specs/2026-10-08-claim-guard-policy-design.md
plan: docs/plans/2026-10-08-claim-guard-policy.md
step: "Task 5: Verify worker turns, release and completion-proven recovery"
---

Execute Task 5: Verify worker turns, release and completion-proven recovery only after separate implementation-plan approval. Follow docs/plans/2026-10-08-claim-guard-policy.md and the approved spec. Owner-specific code tasks/worktrees and their gates are required; no live guard or hook activation before the Task 6 approval packet. This proposed step is blocked for plan review.

## Notes

- 2026-10-08T15:42:34Z (docs/claim-guard-design): Implementation plan awaits separate review; do not execute draft steps.
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T16:56:29Z (docs/claim-guard-design): Plan round-1 revision requires both worker claims and controller claim retained after hook retry; all actors use a one-line no-mutation retry instruction. Stage B/C resumes only when pre-gate proved worker restoration, otherwise stays in the same process through C. Stage D alone performs unforced refusal and explicit recovery inside actual controller native environment. Step remains blocked for plan re-review.
- 2026-10-08T17:35:06Z (docs/claim-guard-design): Plan accepted after conditional fixes and scoped verification; execute the recorded dependencies.
- 2026-10-08T20:57:29Z (docs/claim-guard-design): started
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T21:00:22Z (docs/claim-guard-design): Stage B PASS: same held worker, distinct actual turn ID, first flag false with new block then true retry allows. All three claim rows including seen timestamps exactly equal the Stage A snapshot. Stage C starts normal park/start/review/done; worker sends detailed no-pending-work result before the fixed retry reply, controller must combine it with its actual tracked completion, never infer completion from a message alone.
- 2026-10-08T21:05:51Z (docs/claim-guard-design): Stage C PASS: normal worker park-to-agent released its claim; controller received that worker's separate result and actual final notification after bounded wait, then normally started/reviewed/closed the released task. Worker first checkpoint says no pending tools/checks/background process, followed by controlled true retry; control claim remains live with exact previous heartbeat/owner. Root own/related block and retry captured. Stage D runs only inside resumed controller, requires plain-text specific completion observation and unforced claimed refusal with unchanged record/store hashes before force recovery.
- 2026-10-08T21:11:37Z (docs/claim-guard-design): Stages D/E PASS: controller-native unforced done refused claimed with identical record/store byte hashes, then completion-proven force start/review/done recorded exact worker evidence; own controller close leaves zero claims and actual first Stop allows. Fresh other-home source smoke PASS for genuine root/worker start/stop and actual native IDs; selected source definitions identical, no unrelated work integrations replayed. No worker input after C. All tracked slices exited/reaped and credential absent; completed fixture config/state removed, private captures retained for fresh final review.
- 2026-10-08T21:11:37Z (docs/claim-guard-design): done
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T21:11:37Z (docs/claim-guard-design): Verified two same-worker held turns, reset/refusal/retry, normal release, stopped-worker recovery, own closeout and both candidate-home registration smokes with genuine events and cleanup.
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T21:13:18Z (docs/claim-guard-design): Completion-note correction: the other-home export assertion initially failed because a tool-output JSON envelope retained escaped newlines; the actual printed native IDs match the captured root/worker. Decoder corrected and all assertions now pass, then completed fixture config/state was actually removed. No additional model run or native change. Step 5 acceptance is now verified; prior premature cleanup wording is superseded.
