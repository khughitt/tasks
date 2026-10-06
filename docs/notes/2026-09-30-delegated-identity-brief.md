# Delegated session identity

Scoping handoff, 2026-09-30. Goal: tasks-2fa8c6. This is not an approved design.
Updated 2026-10-02: recurring and newer worker reports reuse this investigation;
the related opt-in note-stamping capability is scoped independently.
Updated 2026-10-06: five new reports join the same goal and single-pair capture;
delegated closure is investigated separately from identity and provenance.
Investigation completed 2026-10-06 under tasks-1ece46; findings and limits below.
This brief remains a handoff, not an approved ownership design.

## Problem

The original four reports describe Codex workers whose claims become stale as soon as a command
exits, sometimes alongside conflicting session-variable and provenance warnings.
Reliable ownership needs a stable identity that distinguishes the sessions actually
doing the work. Three newer reports repeat conflicting native IDs (tasks-290fa3,
tasks-6e2d15, tasks-f537a8); two report workers unable to close controller-owned
tasks (tasks-f27fe3, tasks-673b21). They share tasks-2fa8c6, but closure is not yet
established as an identity-fallback symptom. The related Claude live-check automation idea is
conditional upkeep.
tasks-0005a6 separately asks for explicit session provenance on ordinary gate notes,
so stage attribution need not infer it from nearby lifecycle markers.

## Current behaviour and evidence

One current controller and one `spawn_agent` worker were captured on 2026-10-06,
using four disposable tasks in one project. Every invocation isolated
`XDG_CONFIG_HOME`, `XDG_STATE_HOME`, and `RELAY_STATE_DIR`; native identity variables
were left unchanged. Evidence is attached to tasks-1ece46 at
`tasks/files/tasks-1ece46/delegated-identity-evidence.json`. IDs are redacted to
relationships; command PIDs are retained as process evidence.

Executables: `tasks 0.2.0` at `$HOME/.cargo/bin/tasks`; `codex-cli 0.160.1`
through `$HOME/.local/bin/codex`, resolving to the standalone release's
`bin/codex` under `$CODEX_HOME/packages/standalone/releases/0.160.1-x86_64-unknown-linux-musl/`.
The capture used the installed CLI; source checks ran at `6edc5c5` in the task
worktree. Both participants' shell commands reached the same installed tasks binary.
The host tasks config has no identity table (relay defaults off); its Codex shell
policy sets only `TASKS_FORMAT` and `TASKS_AGENT`, with neither native ID overridden.
No host relay registry was present. No host setting, launcher, desktop, or product
code changed.

| Check | Controller observation | Worker observation |
| --- | --- | --- |
| Native inputs | Both Codex variables present and equal; both equal controller rollout ID | Both present; session equals controller; thread differs and equals child rollout ID; child metadata identifies controller as parent |
| Other identity inputs | Claude variables and explicit tasks session/PID absent | Same absence |
| Process | Nearest recognized `codex` PID 2084866, start 14894621 | Exactly the same harness PID/start; logical sessions differ |
| Command session | A new Unix session leader for each invocation | A new Unix session leader for each invocation, distinct from the controller's commands |
| Registry | No matching row; host registry absent | Same absence; no real relay success demonstrated |
| Native claim after start exits | Raw native controller ID, no PID/start proof; boot ID recorded; live by four-hour TTL | `sid:<start command PID>`; PID/start/boot recorded; PID already gone; `live:false` |
| Continuation | Later stamped note refreshes the same live identity; park and close succeed | Stamped note lands with omitted provenance; stale claim is pruned rather than refreshed; park records a new command SID |
| Delegated close | Own task remains live until controller cleanup | Controller-owned close exits 1, `claimed`; worker-owned close exits 0 only after automatic stale-claim takeover |

The worker's final proof capture used command PID 3690366. Its stored `pid_start`
and `boot_id` were present, but `/proc/3690366` did not exist by the next invocation.
The tasks JSON claim view does not expose those two proof fields; the isolated store
was inspected separately. The controller claim has no PID or start proof, so sharing
its harness process does not establish worker ownership of that native claim.

The native `ready`/`next` calls returned the two unstarted relay-control tasks and
omitted both doing tasks. That does **not** prove liveness-based exclusion, because
the local records already said `doing`; `tasks claims` independently showed the
controller live and the worker dead. The existing cross-checkout and rival-start
fixtures cover live claim exclusion. There was no second controller or worker.

The exact native delegated refusal, with identity and scratch path redacted, was:

```json
{"error":{"detail":"probe-441b81 is claimed by session <controller-native> (owner keith, host <host>, worktree <scratch>/project, since 2026-10-06T09:59:44Z, age 37s, live)","kind":"claimed"}}
```

This reached tasks; no harness/controller gate prevented execution. The worker's
effective identity was its current command SID, not either exported Codex ID.
`src/commands/mod.rs::ownership` requires matching effective identity or valid
relay-mode process proof; `claim_guard` refuses a foreign live claim. Plain-language
permission from the controller does not supply either. Worker-owned closure warned
that the previous command PID was gone and used the stale-claim takeover path. Its
success is not evidence of stable continuation ownership.

| Injected control | Result and limit |
| --- | --- |
| Scratch relay enabled, real native/process inputs, empty schema-2 registry | Worker start exits 1, `config`: conflicting session/controller and thread/worker hints, with explicit-session recovery instruction. Worker delegated close reports the same conflict plus the held controller identity. Hints are checked before registry loading. Controller start exits 1, `config`: no agent matches PID 2084866. This is not a captured real relay match. |
| Existing Codex fixtures | `just test-one codex`: six tests pass, covering agreeing/thread-only inputs, conflict fallback, claim survival, and relay hint disagreement. Missing-input Unix fallback is covered by the fast suite. These are synthetic inputs, not extra live worker captures. |
| Existing provenance fixtures | `just test-one provenance`: eleven tests pass, including missing/agreeing/conflicting inputs, lifecycle metadata, and opt-in stamped notes. `just test-fast` also passes. |
| Pure controller Stop-policy check | `ops/hooks/claim-guard::decide`, injected Stop events and captured live/dead claim rows: controller blocked on its own live claim, child allowed with no matching live claim. This tests the decision function, not actual Stop delivery or controller wake. |

Current resolution is demonstrated, rather than inferred from the reports:

- `src/claims.rs::identity_from` skips the native Codex level when the pair conflicts;
  Unix fallback records the command session leader. `liveness_with` sees that PID gone
  after exit and immediately marks the claim stale. The agreeing controller uses TTL
  liveness. This is the existing deliberate contract, including the native level from
  `903f04a` (tasks-3190fb).
- `src/provenance.rs::resolve_from` independently rejects the same conflicting pair;
  lifecycle and `note --stamp` metadata are omitted with warnings. Plain notes are
  unstamped by design. Explicit claim overrides do not provide native provenance.
  The independent note-stamping capability in tasks-0005a6 is done.
- `src/relay/resolve.rs::hint_for` rejects the worker pair before loading the registry.
  `resolve` also requires exactly one qualifying process row. Shared process proof
  cannot select two logical sessions by itself; the native child thread signal can
  distinguish this pair, but how to trust/deliver it requires a reviewed contract.
- `ops/hooks/claim-guard::held` filters by live claim and exact/known-prefixed Stop
  session ID. A separately live child identity would still not match a controller's
  Stop ID. Native identity repair alone therefore does not solve controller aggregation.

| Reports | Demonstrated path and remaining historical limit |
| --- | --- |
| tasks-0c2c39 (`ai-2f1271`, now tack-2f1271) | Original controller transcript recovered: tool-result line 1174 records `sid:1112013`, `live:false`; line 1178 removes the scratch capture. No actual native-variable presence capture survived. Missing IDs remain the reporter's inference; current reproduction demonstrates conflicting inherited IDs instead. |
| tasks-5745bb, tasks-616cd7, tasks-cf8687 | Current pair reproduces conflict → Unix fallback → dead claim, plus independent omitted provenance. Their historical pair/config inputs remain uncaptured; the reported shared Unix identity is not established by this pair's distinct command SIDs. |
| tasks-290fa3, tasks-6e2d15, tasks-f537a8 | Same current conflict path; parent session versus child thread is now demonstrated for this pair. This maps the symptoms, without proving every earlier shell used this exact environment. |
| tasks-f27fe3, tasks-673b21 | Current delegated close is a tasks `claimed` refusal against a live controller identity. Sharing an exported parent key or harness process does not mean matching effective identity. Historical refusals were not captured; another layer may have refused those original calls. |

All disposable tasks were closed/dropped by their appropriate owner or controller
cleanup; final isolated `tasks claims` was empty. No detached scratch process was
launched. The original evidence scratch was already deleted by its own controller.

## Constraints

Preserve report sources and distinguish observations from injected controls.
The recovered original dead claim is evidence; its missing-ID explanation is not
verified. This capture establishes the current input mismatch at the worker command
boundary, not which upstream statement exports it, and only for Codex 0.160.1 on
this host. A real relay registry match and actual controller Stop/wake remain gaps.

Keep identity/PID pairs from one resolution level. Neither inherited parent keys nor
a shared process justify merging ownership. Claim identity, native note provenance,
delegated lifecycle authorization, and controller completion policy are separate
contracts. No precedence, ownership, or host configuration change belongs in this
completed investigation. New contracts require reviewed design and implementation
plan work. Opt-in note stamping retains its independent existing semantics.

## Alternatives

1. **Repair worker identity delivery at the harness/adapter boundary (recommended).**
   Deliver one authoritative logical worker identity consistently across commands;
   align supported native variables or remove a stale parent alias at that boundary.
   The child rollout already distinguishes the worker. Codex/native command delivery
   is the demonstrated failure boundary; the exact exporting implementation is not
   identified. The relay harness-adapter owner has the captured recurrence in
   relay-60ae9e and can route an upstream repair. Host config did not override these IDs.
2. **Change tasks' identity contract.** A reviewed design could name a trustworthy
   signal and change claim/provenance/relay hint handling together. Do not silently
   prefer `CODEX_THREAD_ID` while the existing contract treats disagreement as invalid.
3. **Explicit worker claim identity as a bounded workaround.** A stable
   `TASKS_SESSION` with a lifetime-appropriate PID (or no PID for TTL) can name a worker
   sharing a process. It does not repair native provenance or authorize closing a
   controller claim. No such override was used to hide this reproduction.

Choose the boundary repair over loosening tasks' conflict checks. Retain controller
closure of controller-owned tasks under today's contract; separately decide whether
explicit lifecycle delegation is needed. For controller Stop policy, reuse
planned tasks-56b450 rather than assume an identity repair makes child claims count
as controller obligations.

## Unanswered questions

- Which Codex/harness delivery statement leaves the parent session alias in worker
  commands? The capture identifies the boundary and excludes the local native-ID
  config overrides; the harness/adapter owner must trace and repair delivery.
- What reviewed identity contract should tasks accept if upstream alignment cannot
  be provided, and how should relay represent logical sessions sharing a process?
  tasks-5e2472 designs this from the capture; a real relay match is needed before
  claiming a relay-specific fix.
- Is explicit delegated close needed beyond controller cleanup, and what authority
  would grant it? tasks-5e2472 must settle it independently of identity/provenance.
- How should controller completion aggregate separately owned worker obligations?
  tasks-56b450 owns the existing Stop-policy decision; an actual Stop/wake run remains
  outside this bounded capture.

## Proposed decomposition

- tasks-1ece46: completed direct investigation. One pair demonstrated inherited
  parent session plus child thread, short-lived fallback, omitted provenance, tasks
  delegated-close refusal, and stale worker-owned close. Six Codex and eleven
  provenance checks plus the fast suite passed. Sanitized evidence attached; original
  evidence and relay/Stop gaps recorded; all nine reports received finding notes and
  were reconsidered in the same scope pass.
- tasks-5e2472: new priority-2, medium/high/planned design task under tasks-2fa8c6.
  Review identity delivery ownership, accepted native signals, shared-process relay
  representation, and explicit lifecycle delegation before implementation. Regression:
  one real worker's identity matches its rollout, differs from controller, stays live
  and stable across commands, and stamps child provenance; foreign live close remains
  refused unless a separate reviewed authority permits it.
- tasks-0c2c39, tasks-5745bb, tasks-616cd7, tasks-cf8687, tasks-290fa3,
  tasks-6e2d15, tasks-f537a8, tasks-f27fe3, tasks-673b21: briefed ideas remain under
  tasks-2fa8c6, with sources preserved. Wait on tasks-5e2472 for the contract decision;
  this investigation does not turn historical symptom reports into implementation
  specifications or drop requests.
- tasks-56b450: existing planned controller Stop-policy ownership decision; reuse
  it for aggregation and harness-capability facts, rather than create another hook task.
- relay-60ae9e: captured recurrence filed to the harness-adapter owner for delivery
  repair/routing; its maintainer reviews and commits the feedback separately.
- tasks-0005a6: done; independent explicit `note --stamp` capability. It cannot rescue
  conflicting identities, and no consumer adoption or new stamp schema was added here.
- tasks-481fd6: remains shelved. Its existing Claude live-check recipe wakes on an
  actual relay/tasks process-selection change, not this Codex-only capture.
