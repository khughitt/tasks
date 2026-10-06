---
id: tasks-5e2472
title: Design delegated identity delivery and ownership from the captured boundary
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-10-06T10:04:44Z
updated: 2026-10-06T10:08:21Z
depends: []
parent: tasks-2fa8c6
tags: []
source: tasks-1ece46
agent: codex/gpt-6.1-sol
---

Why: tasks-1ece46 reproduced one Codex 0.160.1 controller/worker pair. CODEX_SESSION_ID remains the controller ID while CODEX_THREAD_ID matches the worker rollout; both logical sessions share one Codex process. Native claims fall to short-lived command sessions and provenance is omitted. Choosing a conflicting ID or allowing delegated close changes an ownership contract and requires reviewed design and plan work.

Done: Review a design, then an implementation plan, assigning responsibility for authoritative worker identity delivery (prefer the harness/adapter boundary), native claim/provenance agreement, relay representation of logical sessions sharing a process, and explicit delegated lifecycle ownership. State the existing controller-closes policy and decide whether any delegation capability is needed; repair of native identity alone must not authorize a foreign live claim. Reuse tasks-56b450 for the controller Stop policy rather than include a hook rewrite here. Preserve fail-early conflict behavior until a new contract is approved.

Where: docs/notes/2026-09-30-delegated-identity-brief.md; tasks-1ece46 sanitized attachment; src/claims.rs::identity_from and proves_ownership; src/provenance.rs::resolve_from; src/relay/resolve.rs::hint_for and resolve; src/commands/mod.rs::ownership and claim_guard. Harness/adapter ownership of input delivery is the recommendation, not a demonstrated location of the exporting statement.

Regression: Repeat one real controller/worker capture across separate commands: worker identity matches its child rollout and differs from controller, claim remains live with stable ownership, native provenance names the worker, rival/foreign live closure is refused unless separately authorized by the reviewed contract, and controller completion policy is verified independently. Keep existing missing/conflicting/agreeing-input fixtures. A real relay success/ambiguity capture remains unavailable on the investigation host and is required only for a relay contract change.
