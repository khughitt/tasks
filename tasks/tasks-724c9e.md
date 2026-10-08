---
id: tasks-724c9e
title: Verify live-claim enumeration against Stop policy
status: done
priority: 2
size: s
complexity: mid
process: direct
owner: docs/claim-guard-design
created: 2026-10-06T16:52:00Z
updated: 2026-10-08T18:47:19Z
started: 2026-10-08T18:41:04Z
completed: 2026-10-08T18:47:18Z
depends: []
parent: tasks-2fa8c6
tags: []
source: tasks-27d515
agent: codex
---

Question: Can the existing tasks claims output enumerate every live claim relevant to a controller's Stop policy, including task records that exist only in a worktree, or is a focused command/diagnostic missing?

Where to start: tasks-27d515; src/commands/claims.rs, ClaimInfo/ClaimRow in src/output.rs, ClaimSnapshot and liveness in src/claims.rs; ops hooks/claim-guard and its current claim selection. tasks claims already reads all registered claim stores without opening checkouts; list --status doing reads task records and is not the claim-store inventory. Reuse those commands before proposing another listing API.

Bound: inspect the two readers and compare one registered task, one claimed worktree-only plan-step record, and a dead claim in a disposable main/worktree pair with isolated XDG_CONFIG_HOME and XDG_STATE_HOME. Use explicit scratch identities and stable fixture PIDs; compare id, prefix, owner, session and liveness without publishing private real-session identifiers. Distinguish a controller's own claims from separately owned workers and do not assume parent aggregation already exists. No live host-state edits or product-code changes.

Expected result: record a concrete listings-versus-policy table and the smallest remedy: point the workflow at existing tasks claims with explicit session selection if sufficient, or specify the missing selector/relationship and file the necessary scoped follow-up. Record exact versions, scratch cleanup, and any limits. Update tasks-27d515 and tasks-56b450 with the finding; do not redesign Stop aggregation or grant delegated closure here.

## Notes

- 2026-10-08T18:41:04Z (docs/claim-guard-design): started
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T18:47:18Z (docs/claim-guard-design): Probe: installed tasks 0.2.0 in disposable main/worktree pair with isolated XDG config/state and registry, explicit fixture sessions, stable live Python PID and impossible dead PID. Main doing lists controller+dead records but misses worktree-only plan step; worktree doing lists controller+plan step but not main-only dead record. tasks claims includes all three with exact id/prefix/owner/session and true/true/false liveness. Candidate Stop selects only live controller; SubagentStop selects only live worker. Overrides have no root association. Existing inventory is sufficient; no new listing API. All claims released, empty inventory, scratch worktree unlocked/removed and project/config/state/registry deleted. Table: docs/notes/2026-09-30-delegated-identity-brief.md.
- 2026-10-08T18:47:18Z (docs/claim-guard-design): done
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T18:47:18Z (docs/claim-guard-design): Verified claim inventory with real CLI in disposable main/worktree pair; documented listings/policy table, existing-command remedy, distinct ownership and complete scratch cleanup.
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
