# Delegated session identity

Scoping handoff, updated 2026-10-08. Goal: tasks-2fa8c6. Not an approved design.

## Problem

A worker needs stable claim ownership distinct from its controller. The original
split-ID fallback defect is resolved by the reviewed remedy and acceptance below.
Controller-owned closure is settled by tasks-f8c9cc below. The remaining process
identity question concerns callers and registry producers seeing different PID
namespaces. Authorization and process identity remain separate contracts.

## Current behaviour and evidence

- tasks-1ece46 captured one Codex 0.160.1 controller/worker pair: controller native
  IDs agree; worker CODEX_SESSION_ID equals the controller ID, CODEX_THREAD_ID
  equals its own rollout ID, and child metadata names the controller as parent.
  Both logical sessions share a harness process. These are observations, not proof
  of defective identity delivery.
- Before the remedy, `src/claims.rs::identity_from` rejected the unequal pair and
  fell back to a command SID whose PID was gone after exit. `src/provenance.rs::resolve_from`
  independently omitted provenance; `src/relay/resolve.rs::hint_for` rejected the pair.
  A worker's close of a live controller claim reached tasks and failed `claimed`.
- Full capture/control/report tables now live on tasks-1ece46, alongside its
  sanitized attachment at `tasks/files/tasks-1ece46/delegated-identity-evidence.json`.
  The recovered original dead claim has no surviving native-input capture. Relay
  checks and Stop-policy checks were injected controls. The child Stop input used
  a child session_id and does not model the documented subagent hook shape; it
  establishes no child lifecycle coverage. Later genuine root-relay acceptance is
  recorded below; it does not prove worker registration or worker hook delivery.
- The original work-claims spec's Identity section assumed native-ID equality with hook
  session_id, based on top-level sessions. Relay's design already allows inherited
  parent variables. The [official Hooks documentation](https://learn.chatgpt.com/docs/hooks)
  checked during that research described hook session_id as the parent's. SubagentStart/SubagentStop also carry
  agent_id and agent_type; SessionEnd and Interrupt exclude subagents. The exact-release
  findings below establish agent_id/thread equality and the no-child-Stop dispatch;
  the synthetic event still supplies no live delivery evidence.
- Current `claim_guard` still refuses a foreign live close, and the completed
  acceptance captured that refusal in both controller/worker directions. Neither
  shared root association nor shared process identity grants delegated closure.
  tasks-f8c9cc records the controller-closes policy for tasks-f27fe3 and tasks-673b21.
- tasks-b9cbe3 arose during genuine relay acceptance: the root row carried a host
  PID while the isolated command observed the nearest harness as PID 1. Acquisition
  refused; the later passing capture used explicitly approved shared host PID
  visibility. See [the retained acceptance summary](2026-10-06-delegated-identity-acceptance.md).
  Raw private captures were removed at cleanup, so this pass relies on that historical
  verdict and current source, not a fresh namespace reproduction.
- `src/relay/ancestry.rs` reads the caller's proc view; `resolve.rs::resolve` compares
  host/PID/start, then platform/boot/harness and actor evidence. `snapshot::Handle`
  carries no namespace identity. Claim ownership proof and liveness also consume
  process handles; a change only to acquisition would leave an incomplete remedy.

## Constraints

Preserve sources and separate captured behavior from intended contracts. Claim
identity, note provenance, delegated closure and controller completion are separate
contracts. A parent association must not merge ownership or grant authority to close
its claim. Preserve the now-implemented thread identity and foreign-live refusal.
The completed capture established tasks' reaction to unequal inputs, not an upstream
export bug. relay-60ae9e is related feedback, not authority over Codex itself.
Namespace work must preserve the nearest-harness boundary, host/boot/start checks,
concrete actor checks and fail-closed behavior. It does not imply a worker-registration
fix (relay-f56e30), delegated closure or Stop aggregation. Native IDs alone are not
replacement process proof. No change to host permissions is authorized by this brief.

## Alternatives

The following identity alternatives record the original decision, now implemented:

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
controller-closes for controller-owned tasks, as settled below independently of
the identity design.

For namespaces, compare retaining explicit identity and clearer diagnostics with a
reviewed namespace-aware process relationship, possibly requiring producer changes.
Current lean: establish what evidence is available before choosing support; reject
matching a registry row by native session ID alone or weakening process checks.
For closure, the decision below retains controller-closes; the two reports require
no separate implementation.

## Closure decision: tasks-f8c9cc (2026-10-08)

The controller closes a task whose live claim it owns. A worker assigned work under
that claim reports its result and verification to the controller; it does not call
`done` on the controller's task. The controller inspects the result, performs any
remaining review or verification, and closes its own claim.

A worker that owns a separate task retains that task's lifecycle responsibility:
it closes completed work when assigned to do so, or parks it for the controller's
next action. The controller starts the released task before review edits or closure.
Neither controller nor worker may close the other's live claim merely because they
share a root session, a process, or a parent relationship.

Reject explicit delegated closure authority for this task. The captured refusals
protect the live owner's work, and the existing report-and-close or park-and-start
handoffs cover the reported cases. Verbal delegation, identity repair and `done
--force` do not grant authority over a foreign live claim. An authorization feature
would need a separately scoped need and reviewed design; no such need is established
by these reports.

Both tasks-f27fe3 and tasks-673b21 remain dropped as covered by this decision; their
original evidence is retained. No workflow-owner follow-up is needed:
`skills/tasks/SKILL.md`'s Worker handoff section already states the rule, and
`src/commands/mod.rs::claim_guard` enforces it. Existing foreign-claim and delegated
ownership tests cover the implementation. This decision adds no product-code change,
takeover workaround, Stop aggregation or new authority.

## Unanswered questions

- The worker/thread and shared-root contracts are settled by tasks-5e2472 and its
  completed implementation; they are not open design questions for this batch.
- Closure delegation is settled by tasks-f8c9cc above; no authorization feature is
  needed for tasks-f27fe3 or tasks-673b21.
- How should a controller count separately owned worker claims? tasks-56b450 uses the
  verified root/agent_id mapping and SubagentStop for workers, not a child Stop.
  The [approved claim-guard design](../specs/2026-10-08-claim-guard-policy-design.md)
  keeps the executable in ops, reads hq's validated facts, and separates worker
  selection from root-controller aggregation. The
  [implementation plan](../plans/2026-10-08-claim-guard-policy.md) requires separate
  review; actual event delivery remains separate verification.
- What process relationship can the isolated caller actually verify, and can it
  remain valid for acquisition, continued ownership and liveness? tasks-fe6c59
  investigates available namespace evidence and names any producer coordination.

## Proposed decomposition

- tasks-1ece46: done; preserves the investigation report, attachment and limits.
- tasks-c0bad3: completed source research; findings below informed tasks-5e2472.
- tasks-5e2472: completed identity design and reviewed implementation plan.
  tasks-5745bb, tasks-616cd7, tasks-cf8687, tasks-290fa3, tasks-6e2d15 and
  tasks-f537a8 closed together with the remedy. Preserve their sources.
- tasks-0c2c39 closed with its historical missing-input limit explicitly retained;
  the original incident was not newly reproduced.
- tasks-f8c9cc: closure-policy decision recorded above; controller-closes retained.
- tasks-f27fe3 and tasks-673b21: dropped with user approval on 2026-10-08 as covered
  by tasks-f8c9cc. Their reports and original capture remain intact.
- tasks-fe6c59: P2/s/mid/direct bounded namespace/process-proof investigation.
  tasks-b9cbe3 remains an idea and joins this lane; findings must wake it with a
  task note in the same commit and update this brief before re-scoping. No namespace
  implementation or additional design task is filed before that evidence.
- tasks-56b450: existing controller completion-policy decision; reuse it for aggregation.
  tasks-0005a6 is done (`note --stamp`); it cannot repair identity delivery.
  tasks-481fd6 stays shelved until its documented Claude live-check wake condition.


## Historical exact-release findings: tasks-c0bad3

This section preserves the pre-implementation research and its then-current limits;
the reviewed remedy below supersedes its implementation descriptions.

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

## Reviewed remedy and acceptance (2026-10-06)

The [approved design](../specs/2026-10-06-delegated-identity-design.md) and
[implementation plan](../plans/2026-10-06-delegated-identity.md) resolve the
verified split-ID question: select the concrete native thread for claims and note
provenance, retain the shared root only on claims, and require concrete Codex
thread evidence for relay proof-only ownership. Root association does not identify
an immediate parent or authorize closure. Session-only worker isolation and the
historical uncaptured missing-export report remain separate limits.

[Acceptance evidence](2026-10-06-delegated-identity-acceptance.md) records passing
native controller/worker and deterministic tests, plus a genuine isolated relay
root/worker capture with normally trusted hook delivery. The accepted implementation was merged locally and reinstalled after those gates
and final review passed. No worker hook delivery or relay worker registration fix is
claimed; relay-f56e30, tasks-f8c9cc and tasks-56b450 stay outside the remedy.
