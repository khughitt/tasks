# Codex worker identity and root association

Task: tasks-5e2472. Status: approved by Keith on 2026-10-06 after the revised
document was accepted by claude-code/claude-opus-5-5. The implementation plan is
also approved; execution resumed at the user's request. Implementation is on
`feat/delegated-identity`, with real relay acceptance still open before integration.

## Outcome and scope

A Codex worker owns its own claims across commands and stamps its own native
identity on notes. A shared root session groups workers without giving one worker
authority over another worker or its controller. Native claims remain live after
the command that created them exits, using the existing TTL.

This design covers native identity, native provenance, root association on claims,
and the relay consumer's identity checks. It does not implement delegated closure
(tasks-f8c9cc), controller completion or aggregation (tasks-56b450), hook policy,
or worker registration in relay. This task delivers a reviewed design and plan;
the six symptom reports close only after implementation and live acceptance.

## Evidence and current behavior

[The research brief](../notes/2026-09-30-delegated-identity-brief.md) records the
tasks-c0bad3 source audit of Codex 0.160.1, commit
`d27764b82f7118f674371e6d6e76271d9d606edb`, and the tasks-1ece46 capture.
`CODEX_THREAD_ID` identifies the concrete thread; `CODEX_SESSION_ID` identifies
the shared root. Their difference is intentional. In a nested tree the root is
not necessarily the immediate parent. Subagent hook `agent_id` identifies the
worker thread, while `session_id` identifies the root. The
[official Hooks reference](https://learn.chatgpt.com/docs/hooks), rechecked
2026-10-06, also distinguishes subagent events from root events. Source dispatch
and a previous command capture do not establish live hook delivery.

At the tasks baseline for this design:

- `claims::identity_from` skips unequal Codex IDs, yielding a Unix command-session
  claim that dies when the command exits.
- `provenance::resolve_from` independently omits the note stamp for that pair.
- `relay::resolve::hint_for` independently refuses it.
- `claims::proves_ownership` accepts matching process ancestry without a native
  hint. That is insufficient when Codex threads share one process.
- Relay's current Codex adapter has no SubagentStart/SubagentStop mapping. Its
  root registry row cannot be treated as a worker row. Gap: relay-f56e30.

The earlier review notes on tasks-5e2472 concern its scoped task description;
there was no linked, approved design or implementation plan to reuse.

## Decision and alternatives

Adapt tasks to concrete thread identity and retain the root separately. Reuse
the existing identity ladder, claim store, provenance pair, TTL, and typed config
errors. Do not add a new identity service or unify the three resolution policies.

An upstream export repair is rejected because the audited exports satisfy Codex's
intent. Requiring `TASKS_SESSION` for every worker is a workaround, not the remedy:
it requires per-worker setup and cannot supply native provenance. Deliberately
keying a known worker to its root is rejected because it merges ownership; the
indistinguishable session-only input remains a known limit (§1 and §6). A relay
root row is not an acceptable substitute for a missing worker row.

## 1. Native claims

Keep precedence: explicit `TASKS_SESSION` pair, configured relay, Claude native
identity, Codex native identity, then Unix identity when native identity is absent.
This change does not reinterpret explicit overrides or change Claude precedence.

At the Codex level, read both native values without lossy conversion. An empty
string is absent. A present value must be Unicode and contain no control
characters; retain the existing opaque-ID convention rather than impose UUID
syntax or trim/rewrite IDs. Validate both values even when the thread wins.
Malformed Codex input at this level is a typed configuration error naming the
variable, never its value; it cannot fall through to a Unix identity.

Let `R` be a valid session value and `W` a valid thread value:

| Native input | Claim `session` | Tagged identity | `root_session` | PID |
| --- | --- | --- | --- | --- |
| Both, equal | `W` | `codex:W` | `codex:R` | absent |
| Both, different | `W` | `codex:W` | `codex:R` | absent |
| Thread only | `W` | `codex:W` | absent | absent |
| Session only | `R` | `codex:R` | `codex:R` | absent |
| Neither | existing Unix fallback | existing Unix tag | absent | Unix SID |

Session-only input retains existing support. It does not prove that a caller is
a distinct worker: missing thread data cannot be reconstructed from the root.
No claim of fixing genuinely missing native exports is made. The native tier
still trusts its environment; it is not authentication against a process able to
forge environment variables.

Keep the raw native claim key to avoid an unrelated re-keying migration. Park and
escalation identities retain their existing tagged form. A native Codex claim
has no process proof and retains the existing TTL and heartbeat behavior.

## 2. Root association

Add optional `root_session: String` to `Identity`, `Claim`, and the public
`ClaimInfo` projection. It is a qualified `codex:<root>` key, recorded only from
valid `CODEX_SESSION_ID` when either the native Codex level or a relay-matched
Codex actor row supplies the identity. Omit it when
unknown, for other harnesses, and for explicit overrides. An explicit override
does not inherit a native root association merely because it resembles a Codex ID.

This is the only new serialized field: claim-store entries and all JSON views
using `ClaimInfo` expose it. It is optional on read and omitted on write when
absent. A stored value must have a nonempty, control-free `codex:` suffix; invalid
stored values fail the existing claim-store validation. Old claims remain valid.

Fresh acquisition records the current association. A same-owner refresh retains
an existing association when no current association is known. If both old and
new associations are known and disagree, refuse before changing the claim.
An ordinary stale/forced takeover records the new owner's association and does
not carry the displaced owner's root. Proof-only continuation preserves the
existing association, as it preserves the claim's identity. Association-only
changes never establish ownership, heartbeat rights, shared exclusive holds,
or closure permission.

A root disagreement can arise from changed or misconfigured exports for the same
thread. A resumed thread under a different root is a possible input case, not
behavior established by the source audit or live capture. The typed error names
the task and `CODEX_SESSION_ID`, without printing either root value, and directs
the operator to inspect `tasks show <id>` and the environment. Restore incorrect
exports and retry. If reassociation is intentional, the owner runs
`tasks park <id> "release before intentional root reassociation"`, then
`tasks start <id>` with the intended native inputs. The mismatch guard applies
to acquisition/refresh that would overwrite a known association, not to an
otherwise authorized release. The park releases the claim and its holds; the
next start is ordinary acquisition and may refuse if another session acquired
the task or a needed resource meanwhile. `start --force` by the same owner is
not an override for conflicting root metadata.

`root_session` names the tree root, never an immediate parent. A nested worker
`W2` spawned by `W1` under `R` records root `R`, not parent `W1`. This design does
not invent `parent_session`, reconstruct the parent chain, or persist root
association after a claim is parked or released. Durable session-tree history is
outside this remedy; consumers must not infer it from a missing claim field.

## 3. Native note provenance

Resolve provenance independently of claims and relay. Select `CODEX_THREAD_ID`
whenever present, even when it equals the session value. Otherwise retain
`CODEX_SESSION_ID`. The source field names the variable actually selected.

For `R != W`, a lifecycle note or `note --stamp` emits:

```json
{"harness_session":"codex:W","harness_session_source":"CODEX_THREAD_ID"}
```

The existing pair's shape is unchanged. No root field is added to the strict
`HarnessProvenance` record, so existing note readers can parse new stamps. The
meaning is the concrete native actor when available, not an assertion that the
root session and thread are the same entity. Historical stamps remain untouched;
their recorded source remains authoritative about which export they used.

For new top-level stamps with equal native IDs, `harness_session` stays the same
but `harness_session_source` changes from `CODEX_SESSION_ID` to `CODEX_THREAD_ID`.
Both source names are already supported. This is a source-value change despite
the unchanged JSON shape; consumers must not require the former source name.

Retain provenance's existing validation of all inspected inputs. Malformed
values, simultaneous Claude and Codex native identities, and a qualified
`TASKS_SESSION` differing from the selected native actor omit the entire stamp
and warn without values. In particular, explicit `codex:R` conflicts with native
worker `W` even though `R` is its root. A matching override never supplies the
native source. Unqualified overrides remain independent. Missing native inputs
omit provenance without a warning. Provenance errors do not abort an otherwise
valid operation; claim identity errors still can.

## 4. Relay acceptance and continuation

Relay remains opt-in and fail-closed. The registry schema and producer are not
changed here. For a Codex boundary, validate native values before registry matching
and use the thread as the actor hint when present; otherwise use the session.
Unequal valid root/thread values are not a conflict. Other harness hints remain
scoped to the detected harness.

Malformed-input refusals use the same diagnostic rule as native claims: name the
invalid variable and validation failure, never interpolate its value or the
other native ID. Preserve the relay recovery guidance. Acceptance must check
non-Unicode and control-character inputs without leaking either value.

Keep every current process, host, boot, platform, harness, and cardinality check.
The one qualifying registry row's `sessionId` must match the actor hint, and its
`id` remains the acquired claim identity. For a worker `W`, a root row `codex:R`
therefore refuses. Multiple qualifying rows still refuse; this design does not
introduce hint-based tie-breaking before the producer's worker contract exists.
No refusal silently falls to native identity or borrows the controller's row.

If a matching Codex actor row is selected, attach the valid native root association
independently and retain the row's existing process proof. It proves process
liveness, not that a particular worker turn remains active. Shared-process worker
registration and its lifecycle interpretation require a relay-owned design and
real capture; synthetic rows are only controls.

For proof-only continuation at a Codex boundary, require a valid, present
`CODEX_THREAD_ID` matching the held claim through existing `same_session` notation
rules, as well as all existing process checks. A session-only or absent hint cannot
prove a concrete Codex actor. Malformed native values and a different worker's
thread refuse proof. A matching root never substitutes for a thread. Apply this
through the shared ownership path so `start`, `park`, `done`, protected edits,
heartbeat, and exclusive-hold ownership agree. Exact explicit identity matches
retain their current authority and explicit mismatches still disable proof.

This intentionally leaves native worker acquisition supported while relay worker
acquisition remains unavailable against a root-only registry. The remedy must
report that limitation plainly. Do not disable a user's relay setting to make a
test or rollout pass. Existing explicit override recovery remains available.

### Interim worker handoff

Repairing native identity makes a worker's claim live for four hours after its
last heartbeat, even after the worker reports completion. No hook added by this
design releases it: SubagentStop denotes turn completion, and workers do not
receive root Stop or SessionEnd. Controller `done`, `park`, protected edits,
and ownership of the worker's exclusive holds therefore remain foreign and
refused while that claim is live. Root association grants no exception.

Use release before handoff as the normal protocol, rather than relying on TTL
expiry or routine forced takeover. The implementation must ship this rule in
`skills/tasks/SKILL.md` alongside the native identity change:

> Before ending its assignment, a worker releases every task claim it holds.
> If the assigned task is complete and the worker is responsible for closing it,
> use `tasks done`. If controller review or closure remains, use
> `tasks park <id> "Controller starts this task, reviews the result, then closes it"`
> with `--waiting-on agent`, then report the result and task/worktree location.
> Verify the release succeeded before ending. The controller resumes the parked
> task with `tasks start <id>` before review edits or closure. A worker must not
> close a controller-owned task.

A finished or failed worker may have exited without releasing. The controller
first confirms the worker has stopped and inspects its work and claim, then uses
the existing `tasks start <id> --force --reason "finished worker left a live claim"`
to take ownership explicitly, followed by its normal review, park or closure.
Use the task's current authoritative checkout; takeover does not bypass stale-copy
checks. A failure of this recovery remains an actionable error, never a reason
to wait silently for four hours or impersonate the worker with `TASKS_SESSION`.

This is operational handoff using existing commands, not delegated closure
authority. It is consistent with tasks-f8c9cc's proposed controller-closes rule
for controller-owned tasks. A separately worker-owned task must first be released
and acquired by the controller; the proposal does not let a controller close a
live worker claim merely because it spawned that worker.

## 5. Changes and rollout

Expected code surfaces are `src/claims.rs`, `src/provenance.rs`,
`src/relay/resolve.rs`, the relay environment adapter, claim construction and
ownership in `src/commands/mod.rs`, and `src/output.rs`. Keep policies separate;
a small shared native-value validator is appropriate only where it removes
duplicated boundary validation. No registry schema adapter is needed.

When the reviewed remedy lands, amend the work-claims Identity section to remove
its top-level-only equality premise, and amend the relay spec's hint and Codex
proof rules. Update README and the shipped tasks skill for thread-first native
provenance, claim root association, the relay limitation, and the worker-release
handoff above. Keep the evidence brief historical and link its resolved questions
to this decision.

There is no automatic claim migration or inferred parentage. Top-level native
keys remain stable. A worker's old dead Unix claim follows ordinary stale takeover;
a still-live foreign claim requires the existing explicit takeover procedure.
Successful process-proof continuation retains its stored key. **Known compatibility
break:** a relay-acquired Codex claim can no longer be continued by process proof
alone when `CODEX_THREAD_ID` is missing, even if `CODEX_SESSION_ID` is present and
matches. Older Codex sessions or environments with only the session export lose
that fallback. Exact identity matches from successful resolution still work.
For a legitimate owner whose identity cannot resolve, the error directs it to
set `TASKS_SESSION` to the exact existing claim session shown by `tasks show <id>`;
this preserves the explicit recovery path without inventing a thread export.
Set `TASKS_SESSION_PID` only when an actual long-lived process is known; never use
the command PID. This owner recovery is separate from controller takeover of a
foreign worker claim. Neither old Unix claims nor historical provenance gains
guessed root metadata.

Older claim readers ignore the new optional field but can lose it when rewriting
the store; mixed-version writers cannot promise association retention. Use the
updated binary for participating sessions. Existing note readers need no format
upgrade. Deployment and cross-project changes remain separate authorized actions.

## 6. Verification and closeout

The implementation plan must cover these acceptance cases through the test front
door, including negative assertions that no claim or record was mutated on refusal:

1. Equal, unequal, thread-only, session-only, empty, missing, control-character,
   and non-Unicode Codex input. Both present values are validated. Explicit and
   Claude precedence stay intact; invalid selected native input cannot become SID.
2. Claim identity and root persistence across commands; root metadata round trips
   through the store and all `ClaimInfo` views, with absent-field old-record controls,
   same-owner refresh, conflicting-root refusal with value-free recovery guidance,
   release/reacquisition for intentional reassociation, takeover, and proof
   continuation.
3. Provenance selects the thread source, including equal IDs; existing pair-only
   records still round trip. Root-valued explicit overrides conflict with workers;
   mixed-harness and malformed inputs still warn and omit stamps.
4. Relay equal/split hints, root-only-row refusal, missing registry, ambiguity,
   malformed inputs with no native values in errors, and a synthetic matching
   worker row. Shared PID alone, missing thread, sibling thread, and root session
   must not prove ownership. Owner proof
   still works with an unavailable registry and matching thread. Missing-thread
   proof refusal on an existing claim has an explicit-identity recovery control.
5. Foreign-live-claim refusal for closure, park, protected edits, and hold ownership;
   only the worker's own heartbeat refreshes its claim. Test both native identity
   and relay continuation paths, preserving the distinct error policies. Test
   both directions: worker against controller, and controller against a finished
   worker's still-live claim. Completion alone must not clear the latter or grant
   ownership. Worker park followed by controller start and done succeeds without
   force; worker done releases its own completed task. For a worker that exits
   without release, controller start --force records takeover and permits normal
   review/closure, without changing the foreign-live refusal before takeover.

Then run one real controller/worker smoke using the worktree binary by explicit
path, private scratch task/config/state stores, and native exports as delivered.
The worker starts a scratch task, exits that command, reads and refreshes it in a
second command, stamps a note, and attempts to close a live controller-owned
scratch task. Capture equality relationships rather than private IDs. Require a
stable distinct worker claim, `live: true` after command exit, root association,
correct native provenance, and foreign closure refusal. Also require the controller
to fail to close the worker-owned task while its claim remains live. Have the
worker park it before ending, then verify the controller can start and close it
without force. A second scratch task intentionally left claimed by that same
worker supplies the abnormal-exit control: after the worker finishes, the
controller's unforced closure still refuses, then explicit takeover and closure
succeed with the takeover recorded. The worker's report must identify that one
intentional test exception; the smoke owns its cleanup. Existing TTL expiry is a
separate deterministic test, not a long live wait.

Because relay hint/proof semantics change, also capture a real relay-enabled
controller/worker case without repointing host configuration. A real root-only
registry must yield the specified worker refusal, not controller adoption. If a
safe isolated real registry capture cannot be arranged, leave that acceptance
gate open; synthetic controls cannot be reported as live relay support.

No worker Stop, SessionEnd, or Interrupt behavior is inferred from this smoke.
Immediate-parent behavior is tested as an unknown relationship in nested inputs;
root grouping alone never supplies it.

After the remedy lands and these gates pass, close tasks-5745bb, tasks-616cd7,
tasks-cf8687, tasks-290fa3, tasks-6e2d15, and tasks-f537a8 together, preserving each
historical evidence limit. Give tasks-0c2c39 an explicit disposition: the recorded
dead-SID symptom is addressed for the verified unequal-ID case; the historical
claim that exports were missing was never captured and is not proven repaired.
Explicit known limit: a worker receiving only `CODEX_SESSION_ID` is indistinguishable
from a session-only controller and uses the same native claim identity `R`
(tagged `codex:R`) and root `codex:R`. The remedy does not isolate that worker;
the session-only control must demonstrate this retained behavior. Neither that
case nor genuinely missing exports may be reported as fixed under tasks-0c2c39.
Relay worker registration remains relay-f56e30. Closure and completion policy
remain their existing tasks. No report closes merely because this spec is approved.
