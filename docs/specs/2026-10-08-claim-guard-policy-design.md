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
- Both native settings currently run the guard on `Stop`. Codex has no guard
  registration on `SubagentStop`. Relay's current adapters do not support that
  worker event, so this change uses native wiring directly.
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
event additionally requires nonempty `agent_id` and `agent_type`. Do not interpret
a missing event name as `Stop`. Update synthetic test fixtures to the real shapes.
Unsupported or malformed events produce an actionable first refusal.

Selection uses only live rows from the complete `tasks --json claims --all-projects`
answer. Preserve the existing validation of required claim fields; validate an
optional `root_session` as a nonempty string when present. Unreadable or malformed
rows fail the read as a whole, rather than disappearing from selection.

| Event | Relevant live claims | Scope of reminder |
| --- | --- | --- |
| Claude Code `Stop` | Existing exact session selection, including its qualified spelling | This controller's own claims; preserve current behavior |
| Codex `Stop` | `session == codex:<session_id>`, or `root_session == codex:<session_id>`; retain the existing unqualified session match | The root controller's claims and recorded worker claims in its whole tree |
| Codex `SubagentStop` | `session == codex:<agent_id>` | This worker's own claims only |

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

Preserve the existing total `decide` function and injected readers for isolated tests.
Its order is:

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

Controller messages distinguish their own claims from related worker claims.
For a related worker claim, tell the controller to reconcile the worker's report
and release, or wait on tracked work when required; do not suggest `done` on a
foreign live claim. A live claim is not proof that a child is running. Preserve the
existing continue/park/done guidance for the caller's own claims and the detached-job
warning. No automatic takeover or worker release is introduced.

## Migration, verification and activation

After spec and plan review, implement with owner-specific tasks and worktrees:

1. In ops, replace the constants with the validated reader, adapt policy and
   event-selection tests, and remove `FactsMirrorTests` and its source-discovery
   helper. Keep the executable and owner diagnostic at their current location.
2. In hq, remove the consumer mirror from `.githooks/pre-commit`, the
   `facts-mirror` recipe, its tests and its README instructions. Retain staged
   facts validation through hq's reader. Add the existing guard command to
   Codex `SubagentStop`; retain the root `Stop` registration.
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
  missing-root and intermediate-worker limits; malformed optional fields.
- Facts from a disposable validated file via the real hq tool: true, false,
  unknown, changed sets, invalid/truncated file, missing executable, malformed
  JSON, nonzero exit and timeout. Verify the changed sets affect decisions without
  changing guard code. A retry must still allow when both readers would fail.
- Worker blocks must never offer closure of the controller's claim; controller
  blocks about worker claims must preserve the controller-closes policy.
- Hq's wiring tests, staged-fact validation tests, and removal of every active
  mirror call; each owner's normal gates through its test front door.

Complete or reuse tasks-724c9e's disposable enumeration evidence before rollout.
First run the smallest headless or nested trusted-harness pilot: one controller
claim and one worker claim, genuine `SubagentStop` selection, one continuation,
normal park/release, and then an allowed controller stop. Retain the actual event
and verdict separately from injected policy tests; record the tested versions and
limits. Native hook registration does not prove live delivery by itself.

Any merge changing the live guard or home wiring remains an external-action gate.
Prepare code, checks, configuration diff and rollback for approval before that
step. Until activation, run by explicit worktree paths in a disposable harness;
never repoint a shared launcher at a worktree. No live desktop capture is needed.
Rollback restores the former executable, registrations and both mirror halves as
a coordinated set; never restore only the hq mirror call to a deleted ops test.

Task completion requires the recorded ownership ruling, the direct validated facts
read and coordinated mirror removal, reviewed worker wiring, passing owner gates,
and a documented live pilot or an explicit unresolved delivery limitation. A
delivery blocker is parked with the concrete next action; it is not reported as
verified. Review of this spec is followed by a separate implementation-plan review.
