---
id: tasks-b9cbe3
title: "Relay identity cannot match a published host PID from a command running in a private PID namespace, where the nearest harness PID is 1; live acceptance currently requires a shared host PID view."
status: shelved
priority: 2
created: 2026-10-06T20:46:50Z
updated: 2026-10-14T19:06:48Z
depends: []
parent: tasks-2fa8c6
tags: [feedback, gap, "from:tasks"]
agent: codex
---

Why: the delegated-identity live acceptance recorded genuine relay root registration, but acquisition inside a private PID namespace saw the nearest harness as PID 1 while the registry held its host PID. The successful later capture used explicitly approved shared host PID visibility; it did not implement namespace support.

Evidence: docs/notes/2026-10-06-delegated-identity-acceptance.md and tasks-a5a502 retain the verdicts and setup limits. Current src/relay/resolve.rs::resolve compares host, pid and start before platform/boot/harness and actor checks. src/relay/ancestry.rs reads the caller's proc view; snapshot::Handle has no namespace identifier. Claims also use pid/start/boot in ownership proof and liveness, so changing initial matching alone is insufficient. Original private captures were removed at acceptance cleanup; do not present the surviving summary as a fresh reproduction.

Decision still needed: establish whether accessible namespace evidence can safely connect the caller's nearest harness to the producer's handle throughout acquisition, continued ownership and liveness, or whether the supported path should remain explicit identity with clearer diagnostics. Preserve fail-closed behavior; native IDs alone cannot replace process proof. Keep this separate from missing worker registration (relay-f56e30), controller closure and Stop policy.

Original report: Relay identity cannot match a published host PID from a command running in a private PID namespace, where the nearest harness PID is 1; live acceptance currently requires a shared host PID view.

## Notes

- 2026-10-08T09:58:56Z (main): scope: briefed; joined tasks-2fa8c6; retained historical namespace failure and current process-match evidence; tasks-fe6c59 investigates acquisition, continued ownership and liveness before any namespace-support design; brief: docs/notes/2026-09-30-delegated-identity-brief.md
- 2026-10-14T19:06:47Z (investigate/pidns-proof): finding (tasks-fe6c59): inside a private-/proc PID namespace, no process evidence links the caller to relay's host handle. Codex's pid 1 is a per-command codex-linux-sandbox helper, not the session; the session is hidden; NSpid is namespace-local; namespace inodes are reused; the SO_PEERCRED oracle is blocked by Codex's sandbox (EPERM). Acquisition and proof stay fail-closed. Liveness does not: a cross-view claim write silently prunes live host claims (tasks-860db2). Refusal diagnostics: tasks-19b244. Explicit TASKS_SESSION (without a namespace-local PID) stays the supported path; brief updated.
- 2026-10-14T19:06:48Z (investigate/pidns-proof): shelved: Codex (or another harness) lets a sandboxed command reach a host-side process oracle or exposes a verifiable session-process relationship inside its sandbox; then design namespace-aware matching separately
