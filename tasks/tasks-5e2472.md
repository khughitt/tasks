---
id: tasks-5e2472
title: Design delegated identity from verified Codex semantics
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-10-06T10:04:44Z
updated: 2026-10-06T10:41:55Z
depends: [tasks-c0bad3]
parent: tasks-2fa8c6
tags: []
source: tasks-1ece46
agent: codex/gpt-6.1-sol
---

Why: tasks-1ece46 captured equal controller IDs but parent CODEX_SESSION_ID and child CODEX_THREAD_ID in the worker. This proves disagreement, not a Codex defect. Research tasks-c0bad3 must establish intended native-variable and hook semantics before design chooses the owner or remedy.

Done: after tasks-c0bad3, prepare a reviewed identity design and implementation plan. If distinct parent/session and worker/thread values are intentional, evaluate thread as stable claim identity and session as parent association; otherwise recommend an upstream repair only with evidence that delivery violates Codex's contract. Specify claim identity, native note provenance and any necessary relay representation independently, even if their accepted-input rules need coordinated implementation. Preserve foreign-live-claim refusal and fail-early behavior for genuinely invalid inputs.

Where: docs/notes/2026-09-30-delegated-identity-brief.md; tasks-1ece46 evidence tables and sanitized attachment; src/claims.rs::identity_from, src/provenance.rs::resolve_from and src/relay/resolve.rs::hint_for/resolve. Correct the top-level-only equality premise in the work-claims spec when a reviewed new contract lands.

Bound: identity and parent association only. Delegated closure is tasks-f8c9cc; controller completion and aggregation remain tasks-56b450. Session as a parent link is a candidate input for aggregation, not authority to act as that parent. No new authorization capability or Stop-policy implementation here.

Regression: one real worker has a stable identity distinct from its controller across commands, remains live after command exit, and records provenance according to its separately specified contract. Capture parent relationships without merging ownership. Keep missing, malformed, disagreeing and top-level controls; verify foreign live closure remains refused. A real relay capture is needed only if relay's contract changes.

Reports resolved together: tasks-5745bb, tasks-616cd7, tasks-cf8687, tasks-290fa3, tasks-6e2d15 and tasks-f537a8 share the conflict-to-fallback acceptance check. Close them together when the reviewed remedy produced by this design lands; do not re-scope each independently. tasks-0c2c39 retains its historical missing-input limit and needs an explicit final disposition against that remedy.

## Notes

- 2026-10-06T10:39:15Z (main): review: spec round 1 — verdict: revise; findings: Important 4; reviewer: human
- 2026-10-06T10:41:53Z (main): Review disposition: intended semantics is now prerequisite tasks-c0bad3; delegated closure moved to tasks-f8c9cc; the six conflict/fallback reports share one remedy and closeout; upstream repair is conditional, not the default.
