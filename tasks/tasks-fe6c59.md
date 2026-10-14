---
id: tasks-fe6c59
title: Establish the process-proof boundary for relay across PID namespaces
status: done
priority: 2
size: s
complexity: mid
process: direct
owner: investigate/pidns-proof
created: 2026-10-08T09:57:57Z
updated: 2026-10-14T19:34:51Z
started: 2026-10-14T18:52:07Z
completed: 2026-10-14T19:07:01Z
depends: []
parent: tasks-2fa8c6
tags: []
source: tasks-b9cbe3
agent: codex
---

Question: Can the available process evidence safely relate a namespace-local harness PID to relay's host-PID handle for acquisition, later ownership proof and liveness, or should this case use explicit identity with better diagnostics?

Where to start: docs/notes/2026-09-30-delegated-identity-brief.md; docs/notes/2026-10-06-delegated-identity-acceptance.md; tasks-a5a502; src/relay/ancestry.rs, resolve.rs and snapshot.rs; src/claims.rs::proves_ownership/liveness; docs/specs/2026-09-22-relay-ancestry-identity-design.md sections 4 and 6.

Bound: Trace the three proof consumers and the current handle schema. Use one disposable parent/child PID-namespace probe with isolated task config/state and relay state, plus a same-namespace control, only where existing permissions allow it. Record proc visibility and process identity in both views; distinguish synthetic registry fixtures from genuine producer delivery. Include wrong start token/boot, sibling actor and nested nearest-harness controls so a proposed match cannot broaden ownership. No paid model run, live desktop, global config change, permission escalation, product patch or automatic relay bypass. If namespace creation is unavailable, record that environment limit and complete source analysis; do not weaken isolation to obtain a pass. No upstream defect is assumed.

Expected result: attach sanitized evidence, enumerate which handle/proof assumptions fail, and recommend either a bounded diagnostic/documentation remedy or separately reviewed namespace-aware design, identifying any relay-producer coordination. State what requires a later genuine harness capture; existing native thread identity is not a substitute for a missing process relationship. Check for overlapping related-owner work before filing any cross-project follow-up. Update the brief. P2/s/mid/direct: bounded investigation with explicit controls, not implementation approval.

Ideas it wakes: On completion, run tasks note tasks-b9cbe3 with the finding before the evidence's branch merges, update the brief and re-scope the report.

## Notes

- 2026-10-14T18:52:07Z (main): started
  provenance: {"harness_session":"claude-code:8bec6b41-4900-4f8f-80dc-6b5e19c5efa3","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-14T18:52:15Z (investigate/pidns-proof): resumed
  provenance: {"harness_session":"claude-code:8bec6b41-4900-4f8f-80dc-6b5e19c5efa3","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-14T19:04:25Z (investigate/pidns-proof): attached: pidns-proof-evidence.json (22373 bytes): Sanitized probe results: 23 cases (controls, unshare namespaces, genuine codex sandbox) with verdicts; synthetic registry rows
- 2026-10-14T19:04:40Z (investigate/pidns-proof): attached: pidns-probe.sh (12820 bytes): Disposable probe that produced the evidence: private config/state/registry, synthetic relay rows, optional genuine codex sandbox cases
- 2026-10-14T19:06:49Z (investigate/pidns-proof): Probe: unshare user+pid namespaces (private and host /proc) plus genuine codex sandbox 0.162.1 with no model call, synthetic relay rows, private config/state/registry deleted after; 23 cases attached. An earlier run's harness subshell kept comm codex and confounded the control; fixed and superseded. All controls held. Acquisition/proof refuse fail-closed; liveness/prune across PID views is a defect (tasks-860db2, P1 planned); diagnostics tasks-19b244 (P2 direct). No namespace-aware matching filed; no relay-producer change helps today.
- 2026-10-14T19:07:01Z (investigate/pidns-proof): done
  provenance: {"harness_session":"claude-code:8bec6b41-4900-4f8f-80dc-6b5e19c5efa3","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-14T19:07:01Z (investigate/pidns-proof): Namespace process-proof boundary established: no verifiable cross-namespace relationship (Codex helper pid 1, hidden session, reused ns inodes, peer-cred blocked); acquisition/proof fail closed, liveness/prune across views does not (tasks-860db2); diagnostics tasks-19b244; evidence attached; brief and README updated
  provenance: {"harness_session":"claude-code:8bec6b41-4900-4f8f-80dc-6b5e19c5efa3","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-14T19:34:51Z (main): review: impl round 1 — verdict: revise; findings: P1 1, P2 1; reviewer: human
