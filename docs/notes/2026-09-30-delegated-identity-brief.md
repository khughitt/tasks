# Delegated session identity

Scoping handoff, updated 2026-10-06. Goal: tasks-2fa8c6. Not an approved design.

## Problem

A worker needs stable claim ownership distinct from its controller. The current
Codex worker environment triggers tasks' conflict-to-Unix-fallback path, leaving
claims dead after command exit and native note provenance absent. Whether that
input difference violates Codex's contract remains unresolved. Closing a
controller-owned task is a separate authorization decision.

## Current behaviour and evidence

- tasks-1ece46 captured one Codex 0.160.1 controller/worker pair: controller native
  IDs agree; worker CODEX_SESSION_ID equals the controller ID, CODEX_THREAD_ID
  equals its own rollout ID, and child metadata names the controller as parent.
  Both logical sessions share a harness process. These are observations, not proof
  of defective identity delivery.
- `src/claims.rs::identity_from` rejects the unequal pair and falls back to a
  command SID whose PID is gone after exit. `src/provenance.rs::resolve_from`
  independently omits provenance; `src/relay/resolve.rs::hint_for` rejects the pair.
  A worker's close of a live controller claim reached tasks and failed `claimed`.
- Full capture/control/report tables now live on tasks-1ece46, alongside its
  sanitized attachment at `tasks/files/tasks-1ece46/delegated-identity-evidence.json`.
  The recovered original dead claim has no surviving native-input capture. Relay
  checks and Stop-policy checks were injected controls. The child Stop input used
  a child session_id and does not model the documented subagent hook shape; it
  establishes no child lifecycle coverage. Real relay and hook delivery remain unverified.
- The work-claims spec's Identity section assumes native-ID equality with hook
  session_id, based on top-level sessions. Relay's design already allows inherited
  parent variables. Current [official Hooks documentation](https://learn.chatgpt.com/docs/hooks)
  answers hook session_id: it is the parent's. SubagentStart/SubagentStop also carry
  agent_id and agent_type; SessionEnd and Interrupt exclude subagents. Research must
  establish agent_id/thread equality and verify the review's no-child-Stop finding
  against exact-release dispatch, rather than assume the synthetic event is real.

## Constraints

Preserve sources and separate captured behavior from intended contracts. Claim
identity, note provenance, delegated closure and controller completion are separate
contracts. A parent association must not merge ownership or grant authority to close
its claim. Existing precedence and refusal rules remain until reviewed changes land.
The completed capture established tasks' reaction to unequal inputs, not an upstream
export bug. relay-60ae9e is related feedback, not authority over Codex itself.

## Alternatives

1. **Adapt tasks to intentional parent/worker semantics.** If Codex defines the
   observed difference deliberately, evaluate thread as worker identity and session
   as parent association. Specify provenance and relay acceptance separately.
2. **Repair upstream delivery.** Prefer this only if source or documentation proves
   the captured inputs violate Codex's intended contract; identify the actual owner.
3. **Explicit worker claim override.** A stable TASKS_SESSION can be a bounded
   workaround. It does not settle native provenance or delegated authorization.

Current lean: documented parent hook identity and captured parent session values
favor option 1; verify native semantics and agent_id/thread mapping before design.
Upstream repair still requires evidence of a contract violation. Keep
controller-closes for controller-owned tasks; evaluate that policy in its own small
follow-up, not inside the identity design.

## Unanswered questions

- Does SubagentStart/SubagentStop agent_id equal CODEX_THREAD_ID, and do the native
  variables intentionally distinguish parent conversation from worker? tasks-c0bad3
  traces those assignments and actual event dispatch at the captured release; parent
  hook session_id is already documented, not an unanswered question.
- Which signal identifies the worker and which associates its parent, and how should
  each identity/provenance/relay contract use them? tasks-5e2472 follows that evidence.
- Is explicit closure delegation needed? tasks-f8c9cc recommends controller-closes
  and records an independent disposition for tasks-f27fe3 and tasks-673b21.
- How should a controller count separately owned worker claims? tasks-56b450 uses the
  verified parent/agent_id mapping and SubagentStop for workers, not a child Stop.
  Actual event delivery remains separate verification.

## Proposed decomposition

- tasks-1ece46: done; preserves the investigation report, attachment and limits.
- tasks-c0bad3: P2/s/mid/direct source research; prerequisite for tasks-5e2472.
- tasks-5e2472: P2/m/high/planned identity design and reviewed implementation plan.
  tasks-5745bb, tasks-616cd7, tasks-cf8687, tasks-290fa3, tasks-6e2d15 and
  tasks-f537a8 share one conflict/fallback acceptance check and close together when
  its reviewed remedy lands. Preserve their sources; do not scope them individually.
- tasks-0c2c39: preserve its historical missing-input limit and give it an explicit
  disposition against the remedy rather than treating that inference as reproduced.
- tasks-f8c9cc: P2/s/mid/direct closure-policy decision for the two closure reports.
- tasks-56b450: existing controller completion-policy decision; reuse it for aggregation.
  tasks-0005a6 is done (`note --stamp`); it cannot repair identity delivery.
  tasks-481fd6 stays shelved until its documented Claude live-check wake condition.
