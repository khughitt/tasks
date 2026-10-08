---
id: tasks-f2b0ad
title: "Establish pilot isolation, inventory and genuine delivery"
status: done
priority: 1
size: m
complexity: high
process: direct
owner: docs/claim-guard-design
created: 2026-10-08T15:42:33Z
updated: 2026-10-08T20:57:30Z
started: 2026-10-08T18:47:18Z
completed: 2026-10-08T20:57:29Z
depends: [tasks-50f850, tasks-1ffde6, tasks-724c9e]
parent: tasks-56b450
tags: []
source: "tasks-56b450:claim-guard-policy:task-4"
agent: codex
spec: docs/specs/2026-10-08-claim-guard-policy-design.md
plan: docs/plans/2026-10-08-claim-guard-policy.md
step: "Task 4: Establish pilot isolation, inventory and genuine delivery"
---

Execute Task 4: Establish pilot isolation, inventory and genuine delivery only after separate implementation-plan approval. Follow docs/plans/2026-10-08-claim-guard-policy.md and the approved spec. Owner-specific code tasks/worktrees and their gates are required; no live guard or hook activation before the Task 6 approval packet. This proposed step is blocked for plan review.

## Notes

- 2026-10-08T15:42:33Z (docs/claim-guard-design): Implementation plan awaits separate review; do not execute draft steps.
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T16:56:29Z (docs/claim-guard-design): Plan round-1 revision adds project-contained .private config/state/auth/evidence paths, a no-claim workspace-write smoke with -a never, no-claim same-worker restoration probe, and a smoke-tested single-process A/B/C fallback with bounded author gates. Recorder failures persist before exit; personal auth copy is exclusive 0600 and deleted on abort/cleanup. Step remains blocked for plan re-review.
- 2026-10-08T17:35:06Z (docs/claim-guard-design): Plan accepted after conditional fixes and scoped verification; execute the recorded dependencies.
- 2026-10-08T18:47:18Z (docs/claim-guard-design): started
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T18:56:36Z (docs/claim-guard-design): Preparation: enumeration prerequisite passed. Private real sibling control directory holds hooks/Codex home/gates/evidence; only isolated tasks config/state are model-workspace writable. Recorder five controls, author-gate match/mismatch, credential failure cleanup and bounded process cleanup pass. Codex 0.161.0; actual session routing gpt-6.1-sol/high; private personal file-auth route and hooks/multi_agent feature checks pass; credential copy removed. Native /hooks inspected: three new definitions, zero active; none trusted and no model request or scratch claim. Exact source/command/file fingerprints and launcher prepared at .superpowers/sdd/claim-guard-policy/pilot/control/hook-trust-review.md. Model sandbox/delivery/transport/lifecycle checks remain unrun. Acceptance docs record actual limits.
- 2026-10-08T18:57:42Z (docs/claim-guard-design): parked (waiting on user, approval): User approves the three exact private pilot hooks in .worktrees/claim-guard-design/.superpowers/sdd/claim-guard-policy/pilot/control/hook-trust-review.md; agent resumes here, uses normal /hooks trust, runs no-claim write/deny/recorder smoke then transport and stage A checks.
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T20:35:18Z (docs/claim-guard-design): resumed
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T20:43:29Z (docs/claim-guard-design): Trusted only the exact three private definitions via normal native /hooks after user approval; browser showed all active. Actual-model no-claim smoke passed: isolated config/state writable, control write denied by EROFS with no sentinel, add/note/drop succeeded, empty inventory and genuine first Stop allowed. No-claim restoration passed: same captured worker agent_id across exec resume, distinct turn_ids, actual worker/root exports match captures, zero claims. Select resumed_processes; fallback remains unused. Stage A resumes the same proven root and spawns a fresh held-claim worker.
- 2026-10-08T20:50:55Z (docs/claim-guard-design): Stage A attempt 1 BLOCKED: correct live native controller/worker claims but first stops allowed. Investigation: same captured payload blocks under explicit prepared env; zsh login startup overwrites inherited XDG config/state with personal paths. Private launcher now sets ZDOTDIR to an empty private startup directory, preserving normal HOME/facts endpoint. Shell-env assertion proved RED then GREEN. Native hook definitions and capture.py unchanged. Stopped harness, retained attempt-1 captures and aborted record copies, removed only aborted fixture config/state and initialized fresh unclaimed tasks. Fresh real-model write/deny/empty-inventory/first Stop smoke passes; repeat transport pre-gates before new claims. Feedback hq-c19b34 filed; no live change or foreign takeover.
- 2026-10-08T20:57:29Z (docs/claim-guard-design): Corrected attempt 2 Stage A PASS: root bare session and qualified root agree with actual native exports/Stop; fresh worker ID is distinct and matches both worker claims. Genuine worker and root first stops block, same-turn true retries allow; all three claims remain live with native no-PID rows. Worker reason names only worker tasks; root reason names own/related tasks and completion-proven force recovery. Actual pre-gates and no-claim transport passed; private credential removed.
- 2026-10-08T20:57:29Z (docs/claim-guard-design): done
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T20:57:29Z (docs/claim-guard-design): Established isolated writable/protected state, native trust and genuine delivery; corrected login-shell isolation and passed fresh Stage A identity, first-refusal/retry and retained-claim checks.
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
