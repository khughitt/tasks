# Claim-guard policy and capability facts

Task: tasks-56b450. Status: proposed; requires user review before planning.

## Outcome and scope

Keep a controller from leaving unfinished claimed work without a next action, and
give a worker the same reminder for its own claims. Read wake capabilities from
harness support rather than maintaining a second copy inside the guard. Preserve
the controller-closes decision from tasks-f8c9cc: inspecting a related claim never
authorizes closing it.

This design keeps the Python executable at ops `hooks/claim-guard`. Tasks owns the
claim policy and this contract; ops hosts and tests the executable until a later
reviewed transfer. Hq owns capability facts and native hook wiring. The work changes
the existing guard, its tests and documentation, the temporary fact-mirror gates,
and Codex's native `SubagentStop` wiring. It adds no tasks subcommand, dependency,
claim schema, parent ledger, relay adapter or authorization feature.

## Evidence checked

On 2026-10-08, the current source shows:

- ops `hooks/claim-guard` reads all registered claim stores through `tasks claims`,
  embeds `CHILD_WAKES`, `WAKING` and `IN_FLIGHT`, and matches only the stopping
  session. Its first refusal produces a reason; a retry with `stop_hook_active:
  true` is always allowed. It never writes claims.
- Hq's validated `tools/harness-facts list` returns all three facts, including
  explicit `unknown` entries. Its supported command endpoint is
  `~/.agents/bin/harness-facts`; the resolved tool reads its owner's default file.
- Claude settings and both Codex files, hq `codex/hooks.json` and
  `codex/hooks.work.json`, currently run the guard on `Stop` only. Hq `links.toml`
  installs the latter into `~/.codex-work`. Relay's current adapters do not support
  `SubagentStop`, so this change uses native wiring directly.
- `src/commands/claims.rs` opens no task checkout and enumerates each registered
  claim store. `ClaimInfo` already exposes optional `root_session`. No JSON field
  addition is needed. Task tasks-724c9e owns the disposable enumeration probe;
  source inspection here does not replace that acceptance evidence.
- The [delegated-identity brief](../notes/2026-09-30-delegated-identity-brief.md)
  preserves the exact Codex 0.160.1 mapping: worker `agent_id` equals its concrete
  thread ID; hook `session_id` identifies the shared root. Root grouping does not
  identify an immediate parent. The [official Hooks documentation](https://learn.chatgpt.com/docs/hooks)
  checked today documents worker identification on `SubagentStop`, continuation
  requests, and the exclusion of workers from `SessionEnd` and `Interrupt`.
- `src/claims.rs::identity_from` stores the bare native thread ID as `session`,
  and qualifies only `root_session` with `codex:`; `tagged` is a separate identity
  representation. `src/commands/mod.rs::claim_guard` persists `me.session`.
  The native unit test asserts that the raw session matches claims. A disposable
  worktree-binary check today also obtained controller and worker rows from
  `tasks claims`, with bare sessions, the same qualified root, no PID and live
  claims. It used fixed synthetic native exports with relay disabled and isolated
  config/state; it verifies serialization, not actual harness ID delivery. Both
  claims were released and all scratch state removed.

Those sources establish the boundary, not fresh live delivery. The fact entries
also retain their probe-version and observation limits: Codex wake was a bounded
observation on 0.156.1; Claude's `pending` status was retained without an observed
sample. This change does not upgrade either into a current-version probe.

## Ownership decision and rejected alternatives

**Keep the executable in ops and switch its facts reader now.** This is expressly
allowed by the agent-layer split design's section 4.4. It reuses the Python policy,
test suite and native command paths while removing the temporary two-copy contract.
Wrong or unclear blocks continue to name ops as the executable's feedback owner;
policy design remains recorded here in tasks.

Reject moving the script into tasks in this change: that also needs an installed
endpoint, a Python test front door in a Rust repository, and a host-routing change.
None is needed to fix fact sourcing or worker selection. Reject rewriting the guard
as a Rust command: it would add a public interface and recreate tested event and
message handling. A later transfer must move the executable and its tests together,
with its own reviewed installation and rollout contract.

## Event and claim selection

The guard accepts `Stop` for either supported harness, and `SubagentStop` for Codex.
Require the native `hook_event_name` and a nonempty `session_id`; a Codex worker
event additionally requires nonempty `agent_id`. `agent_type` is unused metadata;
its absence or an empty value does not block selection. Do not interpret a missing
event name as `Stop`. Update synthetic test fixtures to the real shapes.
Unsupported or malformed events produce an actionable first refusal.

Selection uses only live rows from the complete `tasks --json claims --all-projects`
answer. Preserve the existing validation of required claim fields; validate an
optional `root_session` as a nonempty string when present. Unreadable or malformed
rows fail the read as a whole, rather than disappearing from selection.

| Event | Relevant live claims | Scope of reminder |
| --- | --- | --- |
| Claude Code `Stop` | Existing exact session selection, including its qualified spelling | This controller's own claims; selection unchanged |
| Codex `Stop` | Native own match: `session == <session_id>`; related match: `root_session == codex:<session_id>`; also retain the existing exact `session == codex:<session_id>` spelling for qualified identities | The root controller's claims and recorded worker claims in its whole tree |
| Codex `SubagentStop` | Native match: `session == <agent_id>`; also exact `session == codex:<agent_id>` for qualified identities | This worker's own claims only |

For native root-controller claims, **own** means the bare `session` equals the
event's `session_id`. **Related worker** means the qualified `root_session` equals
`codex:<session_id>` and `session` differs. A claim matching the caller's exact
qualified spelling is also classified as own, before root association is considered;
relay and explicit overrides can store that spelling. Root metadata on an own
claim does not turn it into a worker claim. These selections are reminders, not
proof of authority: the CLI's existing ownership check still governs every write.

The native classification requires the root controller's concrete thread ID to
equal its hook/root session ID. Tasks-1ece46 observed equal controller native IDs
at Codex 0.160.1, as recorded in the brief's Current behaviour section; that one
capture is not a guarantee for other releases or thread shapes. The live pilot
must compare the root's actual exports, hook ID and emitted claim before activation.
If they disagree, do not deploy this classification: revise the mapping instead
of telling a controller that its own claim belongs to a worker.

Do not match every harness prefix for a Codex event. Do not use the worker event's
shared root ID to select the controller or siblings. No `SubagentStart` ledger is
needed: native claims already carry their concrete owner and optional root grouping.
No event releases a claim. `SubagentStop` means a turn is ready to finish, not that
the assignment or worker is permanently closed.

Root grouping is sufficient for a root controller's tree-wide reminder, but not for
an intermediate worker's descendants. Claims without root metadata cannot be
assigned to a tree from this inventory alone. Explicit identity overrides and relay
claims without that metadata remain subject to exact-session selection; the guard
must not guess ancestry or claim complete coverage of those cases. Claude worker
selection is outside this design because the checked mapping is Codex-specific.

## Decision and capability read

Keep `decide` total and its readers injectable, but deliberately change its decision
order. Today a qualifying Claude child allows a stop **without reading claims**.
This design requires a complete inventory before granting that exception: a tracked
child no longer conceals an unavailable or corrupt claims store. This is a stricter
policy proposed for review, not preserved behavior. Its order is:

1. Parse the JSON object. An actual boolean `stop_hook_active: true` allows the
   retry immediately, before event validation, claim reads or fact reads.
2. Validate the event and select from the complete claim inventory. A read failure
   produces one diagnostic refusal. With no relevant live claim, allow the turn
   without a capability read: no claim policy needs a wake exception.
3. A worker with a relevant live claim is reminded to finish or use its normal
   handoff. It receives no controller-wake exception. A controller considers the
   wake exception below; otherwise it receives a reminder naming each task and
   authoritative worktree.

The production facts reader invokes `~/.agents/bin/harness-facts list` once and
selects the harness entries for `controller-wake`, `controller-wake-kinds`, and
`stop-input-in-flight-statuses` from that validated view. This gives one coherent
file read, reuses the documented command endpoint, and needs neither a custom TOML
parser nor a registry parser. Parse and check its JSON contract at this subprocess
boundary. Missing tool, nonzero exit, malformed output, or timeout is a failed fact
read, with no constant fallback. Each subprocess retains the guard's ten-second
timeout; the claim and fact calls can therefore consume twenty seconds in total.
That adds an inventory read even to qualifying Claude child stops. A failed claim
read blocks after its own timeout without attempting facts; a successful slow
claim read followed by a slow fact read can approach the full twenty seconds.
The retry still allows immediately, so this change remains a one-shot interruption.

In ops `DecideTests`, replace `test_running_child_skips_the_claims_read` with a
check that the reader runs and a failure blocks despite a known child. Change
`test_running_child_beside_a_monitor_allows` to supply a successful inventory rather
than `never_reads`, and add its failed-inventory case. The existing running-child,
background-command and pending-child allow tests continue to allow with successful
claim and fact reads. `test_retry_never_reads` remains, covering both readers and
both supported event shapes. This explicitly replaces the old read-skipping contract.

The private facts reader takes the complete subprocess command as an injected
argument, alongside the existing subprocess-run seam. Production supplies the
installed endpoint plus `list`. Real-reader tests supply the explicit hq tool
path plus `list --file <disposable-file>`. This is a private test seam, not a new
guard flag or production environment override.

Pure injected-reader tests always run. The real-hq integration class locates hq
through `tasks --json resolve hq` and skips with a stated reason only when hq is
not registered, as the current mirror class does. If a registered checkout or
reader is missing, broken or returns invalid facts, the test fails. Acceptance of
this implementation and rollout requires that class to execute with no skips;
an ops checkout without hq cannot supply that acceptance evidence.

A controller's claimed turn may end for tracked work only when all three entries
are `probed`, wake is the boolean `true`, and the supported `background_tasks`
input includes at least one child whose kind and in-flight status are in those
sets. Preserve the existing malformed-list, unrecognized-status, unknown-kind-only
and scheduled-wake diagnostics. A mixed list may qualify through a known child;
the unknown kind itself grants nothing. An empty or unavailable list grants nothing.

`unknown` never grants the exception and is explained as an unprobed capability,
not converted into a negative probe. A false wake fact retains the bounded-wait
instruction. Do not require Codex's unknown status-set fact to be probed when its
false wake fact already rules out the exception. Failed reads produce explicit
repair guidance and no wake claim. Read claims before considering an exception:
a known child cannot hide a failed inventory read.

Keep using the recorded facts across version upgrades, as the current constants
do; this consumer adds no version detector or new probe. Messages must describe
the recorded evidence without claiming verification of the installed release.
Hq owns refreshing that evidence.
The installed endpoint reads hq main's working tree: an uncommitted invalid edit
there would affect the live guard before any commit-time gate. Edit facts in an
isolated hq worktree and integrate the validated change; never edit the live file
in place, as hq's agent guide requires.

Controller messages distinguish their own claims from related worker claims.
For a related worker claim, tell the controller to reconcile the worker's report
and release, or wait on tracked work when required; do not suggest `done` on a
foreign live claim. A live claim is not proof that a child is running. Native Codex
claims have no PID and can remain live for four hours after their last heartbeat
on the TTL path (`src/claims.rs::TTL_HOURS`). A worker may finish after the guard's
one allowed retry without releasing its claim; the controller needs a recovery
action rather than repeated instructions to wait for that TTL.

Messages point to the existing [Worker handoff recovery](../../skills/tasks/SKILL.md#worker-handoff):
only after the controller's own completion/wait result for that specific worker
establishes the assignment has stopped with no pending tool, check or background
process may it inspect the result and claim, then run
`tasks start <id> --force --reason "<worker-specific completion evidence>"` in the
authoritative checkout. Review or close under the acquired claim, and do not resume
the worker on that task without a new ownership handoff. Timeout, silence, root
state and a shared PID are insufficient; uncertain or still-running work requires
reconciliation and waiting. Preserve continue/park/done guidance for the caller's
own claims and the detached-job warning. Recovery stays explicit; no automatic
takeover or worker release is introduced.

## Migration, verification and activation

After spec and plan review, implement with owner-specific tasks and worktrees:

1. In ops, replace the constants with the validated reader, adapt policy and
   event-selection tests, and remove `FactsMirrorTests` and its source-discovery
   helper, `ReasonTests.test_child_wakes_matches_the_probe`, and
   `InFlightTests.test_waking_kinds_match_the_probe`. Adapt other tests referencing
   the constants to injected facts so their behavior coverage remains. Keep the
   executable and owner diagnostic at their current location.
2. In hq, remove the consumer mirror from `.githooks/pre-commit`, the
   `facts-mirror` recipe, its tests and its README instructions. Retain staged
   facts validation through hq's reader. Add the existing guard command to
   Codex `SubagentStop` in **both** `codex/hooks.json` and
   `codex/hooks.work.json`; retain each file's root `Stop` registration. Verify
   their declared homes in `links.toml`, including `~/.codex-work`.
3. Update ops and hq's active ownership/mirror documentation and the tasks brief.
   Historical specs remain evidence; mark their temporary mirror provision as
   superseded with links rather than rewriting their original findings.

The plan must order the cutover so hq's mirror cannot call a deleted test: remove
the temporary hq mirror first while both facts and constants still agree, then
integrate the ops direct reader promptly. Do not change capability values during
that interval. Activate worker wiring only after the new guard supports its event.
Removing the mirror cannot remove hq's own staged fact-validation gate.

Required checks include:

- Existing one-shot retry, claim-read failure, wake/no-wake, detached-work and
  CLI-output tests, with fixtures updated for explicit event names.
- Native worker selection independent of the root; controller selection of live
  root-associated workers across projects; unrelated, stale and sibling controls;
  missing-root and intermediate-worker limits; malformed optional fields. Generate
  primary native selection fixtures by running `start` and `tasks claims` with
  distinct fixed root/thread exports, no explicit/Claude overrides and relay off
  in a disposable config/state/project. Use the emitted rows rather than a
  hand-written qualified-session fixture. Assert both own and related roles;
  keep qualified relay/override controls separate. Synthetic exports establish
  serialization only; the trusted pilot establishes delivered identities.
- Facts from a disposable validated file via the real hq tool: true, false,
  unknown, changed sets, invalid/truncated file, missing executable, malformed
  JSON, nonzero exit and timeout. Verify the changed sets affect decisions without
  changing guard code. A retry must still allow when both readers would fail.
- Worker blocks must never offer closure of the controller's claim; controller
  blocks about worker claims must preserve the controller-closes policy and name
  the completion-proven `start --force --reason` recovery, including its refusal
  to treat timeout, silence or a still-running check as completion.
- Hq's wiring tests, staged-fact validation tests, and removal of every active
  mirror call and constant-pinning assertion; wiring and rollback checks cover
  both Codex hook files. Each owner runs its normal gates through its test front door.

Complete or reuse tasks-724c9e's disposable enumeration evidence before rollout.
First run the smallest headless or nested trusted-harness pilot: one controller
claim and one worker claim in **two separate turns of the same worker thread**.
Before exercising the guard, compare the root's actual session/thread exports,
controller `Stop.session_id`, and `tasks claims` row; require the equality and
stored spellings used by the own/related split above. Then capture genuine
`SubagentStop` events, worker selection, a first refusal and its allowed retry,
and the next worker turn's first stop while its claim remains held. That next
first stop must have a reset flag and be guarded again: the documented description
"already continued" alone does not establish the flag's lifetime. If the flag
stays true across turns, block worker-wiring activation and revise this design;
do not silently claim every worker turn is covered or introduce a persistent ledger.

Finish with normal park/release and an allowed controller stop. Also retain a
worker-specific final completion result for an intentionally unreleased control
claim, demonstrate the controller's unforced refusal, then explicit recovery
under the Worker handoff conditions and release. Retain actual events and verdicts
separately from injected policy tests; record versions, both native hook-home
configurations, cleanup and limits. Native registration does not prove delivery.

Any merge changing the live guard or home wiring remains an external-action gate.
Prepare code, checks, configuration diff and rollback for approval before that
step. Until activation, run by explicit worktree paths in a disposable harness;
never repoint a shared launcher at a worktree. No live desktop capture is needed.
Rollback restores the former executable, the former `Stop`-only registrations in
both hq `codex/hooks.json` and `codex/hooks.work.json`, and both mirror halves as a
coordinated set. Preserve each home's unrelated hooks; never restore only the hq
mirror call to a deleted ops test.

Task completion requires the recorded ownership ruling, the direct validated facts
read and coordinated mirror removal, reviewed worker wiring, passing owner gates,
and a documented live pilot or an explicit unresolved delivery limitation. A
delivery blocker is parked with the concrete next action; it is not reported as
verified. Review of this spec is followed by a separate implementation-plan review.
