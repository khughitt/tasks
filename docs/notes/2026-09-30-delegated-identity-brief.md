# Delegated session identity

Scoping handoff, updated 2026-10-06. Goal: tasks-2fa8c6. Not an approved design.

## Problem

A worker needs stable claim ownership distinct from its controller. The current
Codex worker environment triggers tasks' conflict-to-Unix-fallback path, leaving
claims dead after command exit and native note provenance absent. Exact-release source research now establishes that the
root-session/worker-thread difference is intentional. Closing a
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
  agent_id and agent_type; SessionEnd and Interrupt exclude subagents. The exact-release
  findings below establish agent_id/thread equality and the no-child-Stop dispatch;
  the synthetic event still supplies no live delivery evidence.

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

Recommendation: option 1. Codex 0.160.1 explicitly assigns a shared root-session
identity and a concrete worker-thread identity; SubagentStart/SubagentStop agent_id
equals the latter. Reject upstream export repair for this captured difference: no
contract violation was found. This is research input, not approval of a new schema. Keep
controller-closes for controller-owned tasks; evaluate that policy in its own small
follow-up, not inside the identity design.

## Unanswered questions

- Which signal identifies the worker and which associates its parent, and how should
  each identity/provenance/relay contract use them? tasks-5e2472 follows that evidence.
- Is explicit closure delegation needed? tasks-f8c9cc recommends controller-closes
  and records an independent disposition for tasks-f27fe3 and tasks-673b21.
- How should a controller count separately owned worker claims? tasks-56b450 uses the
  verified root/agent_id mapping and SubagentStop for workers, not a child Stop.
  Actual event delivery remains separate verification.

## Proposed decomposition

- tasks-1ece46: done; preserves the investigation report, attachment and limits.
- tasks-c0bad3: P1/s/mid/direct source research; findings below unblock tasks-5e2472.
- tasks-5e2472: P1/m/high/planned identity design and reviewed implementation plan.
  tasks-5745bb, tasks-616cd7, tasks-cf8687, tasks-290fa3, tasks-6e2d15 and
  tasks-f537a8 share one conflict/fallback acceptance check and close together when
  its reviewed remedy lands. Preserve their sources; do not scope them individually.
- tasks-0c2c39: preserve its historical missing-input limit and give it an explicit
  disposition against the remedy rather than treating that inference as reproduced.
- tasks-f8c9cc: P1/s/mid/direct closure-policy decision for the two closure reports.
- tasks-56b450: existing controller completion-policy decision; reuse it for aggregation.
  tasks-0005a6 is done (`note --stamp`); it cannot repair identity delivery.
  tasks-481fd6 stays shelved until its documented Claude live-check wake condition.


## Exact-release findings: tasks-c0bad3

Checked 2026-10-06 against OpenAI's `rust-v0.160.1` annotated tag
`c3e23d4c4385619ecec78408766e46b7fa7dd9ad`, which resolves to commit
`d27764b82f7118f674371e6d6e76271d9d606edb` ([tag object][release]).
Downloaded source files were checked against that commit's Git tree blob IDs.
This traces source dispatch and serialization; it does not capture live hooks.

### Identifier mapping

| Signal | Root/controller | Thread-spawned worker | Evidence |
| --- | --- | --- | --- |
| `CODEX_SESSION_ID` | Root session identity | Same root session identity | [Session accessors][session-access], [session selection][session-select], [environment injection][env] |
| `CODEX_THREAD_ID` | Concrete root thread ID | Concrete worker thread ID | [Command environment][exec], [user shell][user-shell], [thread injection][thread-env] |
| Hook `session_id` | Root session identity | Same root session identity | [Start request][start], [stop request][stop], [wire start][wire-start], [wire stop][wire-stop] |
| Subagent hook `agent_id` | Not a worker lifecycle event | Worker thread ID, equal to its `CODEX_THREAD_ID` | [Subagent context][context] |
| Immediate parent | Root has no spawning parent | `ThreadSpawn.parent_thread_id`; distinct from the shared root ID when workers spawn workers | [Stop dispatch][stop] |

`Session::session_id()` uses the controller identity shared across the tree;
`Session::thread_id()` identifies the concrete thread. Command construction writes
both values deliberately after shell environment policy; the parent-session value
in the earlier capture need not be an accidentally inherited stale export.
`CODEX_SESSION_ID` and hook `session_id` identify the **root**, not necessarily the
immediate spawning parent. The captured pair had only one worker level, so those
parents coincided. Root grouping alone cannot reconstruct a nested parent chain.

A SubagentStart/SubagentStop hook can therefore address a worker claim keyed by
`codex:<agent_id>` **if a reviewed claim contract adopts thread identity**. Today's
unequal-pair fallback claim is Unix-keyed and cannot be addressed that way. Neither
root grouping nor agent_id grants permission to close another owner's claim.

### Lifecycle dispatch

These are the release's user-configured lifecycle targets, conditional on enabled,
trusted matching hooks and reaching the relevant execution path.

| Point | Root/controller | Thread-spawned worker | Evidence |
| --- | --- | --- | --- |
| Pending startup/fork at turn execution | `SessionStart` | `SubagentStart` for startup/fork; other worker start sources skip | [Start dispatch][start] |
| Turn ready to finish | `Stop` | `SubagentStop`, with worker `agent_id` | [Turn call site][turn-stop], [stop dispatch][stop], [wire stop][wire-stop] |
| Session shutdown | `SessionEnd` | Excluded | [Session-end guard][end], [shutdown call site][shutdown] |
| Active-turn interruption | `Interrupt` | Excluded | [Interrupt guard][interrupt] |

Internal/system `SubAgent` sources are excluded from these start/stop hooks;
thread-spawned workers are the scope here. SubagentStop is a **turn** completion
hook and may request continuation; it is not proof that a worker or its claims
have been permanently closed. The synthetic child Stop in tasks-1ece46 remains
only a pure policy control, with no claim about real delivery.

### Documentation comparison and next contracts

The [official Hooks page][hooks], read 2026-10-06, agrees on parent session_id,
worker agent_id, and the SessionEnd/Interrupt exclusions. Exact-release source
makes that parent term precise as the shared root session and confirms no child
Stop target. The [App Server page][app-server] independently describes the live
session-tree root and warns clients not to derive session identity from thread ID.
The [environment-variable reference][env-docs] lists neither native ID export;
it does not establish an equal-ID requirement or a stable export API. No contrary
version contract was found in the checked pages; those pages are mutable and are
not evidence of other releases' behavior.

- **tasks-5e2472:** evaluate concrete thread identity for claims and root association
  separately, then specify native provenance and relay acceptance independently.
  Preserve missing/malformed-input controls and foreign-live-claim refusal. The
  equality premise in the work-claims spec's Identity section describes today's
  implementation but is disproved for workers; amend it when reviewed changes land.
- **tasks-56b450:** design worker handling around SubagentStart/SubagentStop plus
  agent_id, controller completion around Stop, and root grouping without assuming
  an immediate-parent link. Do not rely on worker SessionEnd/Interrupt. Relay's
  inherited-environment/process-handle constraints still apply: a shared process
  is not a unique worker identity. Verify real delivery and any relay change later.
- **tasks-f8c9cc:** retain the separate controller-owned closure decision. Source
  semantics settle neither delegation authority nor controller claim aggregation.

No native-ID override, live harness sweep, product-code change, host-config change,
or new schema was performed or approved by this research.

[release]: https://api.github.com/repos/openai/codex/git/tags/c3e23d4c4385619ecec78408766e46b7fa7dd9ad
[session-access]: https://github.com/openai/codex/blob/d27764b82f7118f674371e6d6e76271d9d606edb/codex-rs/core/src/session/session.rs#L644-L653
[session-select]: https://github.com/openai/codex/blob/d27764b82f7118f674371e6d6e76271d9d606edb/codex-rs/core/src/session/session.rs#L895-L917
[env]: https://github.com/openai/codex/blob/d27764b82f7118f674371e6d6e76271d9d606edb/codex-rs/core/src/exec_env.rs#L22-L43
[exec]: https://github.com/openai/codex/blob/d27764b82f7118f674371e6d6e76271d9d606edb/codex-rs/core/src/unified_exec/process_manager.rs#L1438-L1453
[user-shell]: https://github.com/openai/codex/blob/d27764b82f7118f674371e6d6e76271d9d606edb/codex-rs/core/src/tasks/user_shell.rs#L169-L170
[thread-env]: https://github.com/openai/codex/blob/d27764b82f7118f674371e6d6e76271d9d606edb/codex-rs/protocol/src/shell_environment.rs#L150-L153
[start]: https://github.com/openai/codex/blob/d27764b82f7118f674371e6d6e76271d9d606edb/codex-rs/core/src/hook_runtime.rs#L127-L167
[stop]: https://github.com/openai/codex/blob/d27764b82f7118f674371e6d6e76271d9d606edb/codex-rs/core/src/hook_runtime.rs#L392-L468
[context]: https://github.com/openai/codex/blob/d27764b82f7118f674371e6d6e76271d9d606edb/codex-rs/core/src/hook_runtime.rs#L1042-L1061
[wire-start]: https://github.com/openai/codex/blob/d27764b82f7118f674371e6d6e76271d9d606edb/codex-rs/hooks/src/events/session_start.rs#L153-L169
[wire-stop]: https://github.com/openai/codex/blob/d27764b82f7118f674371e6d6e76271d9d606edb/codex-rs/hooks/src/events/stop.rs#L178-L200
[turn-stop]: https://github.com/openai/codex/blob/d27764b82f7118f674371e6d6e76271d9d606edb/codex-rs/core/src/session/turn.rs#L653-L661
[end]: https://github.com/openai/codex/blob/d27764b82f7118f674371e6d6e76271d9d606edb/codex-rs/core/src/hook_runtime.rs#L470-L499
[shutdown]: https://github.com/openai/codex/blob/d27764b82f7118f674371e6d6e76271d9d606edb/codex-rs/core/src/session/handlers.rs#L323-L324
[interrupt]: https://github.com/openai/codex/blob/d27764b82f7118f674371e6d6e76271d9d606edb/codex-rs/core/src/hook_runtime.rs#L502-L536
[hooks]: https://learn.chatgpt.com/docs/hooks
[app-server]: https://learn.chatgpt.com/docs/app-server
[env-docs]: https://learn.chatgpt.com/docs/config-file/environment-variables
