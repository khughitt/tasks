---
id: tasks-fe6c59
title: Establish the process-proof boundary for relay across PID namespaces
status: doing
priority: 2
size: s
complexity: mid
process: direct
owner: investigate/pidns-proof
created: 2026-10-08T09:57:57Z
updated: 2026-10-14T18:52:15Z
started: 2026-10-14T18:52:07Z
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
