# Claim handling for interactive walkthroughs

Task: ops-e4d678. Status: draft for review; no implementation or activation authorized.

## Decision

Park once for a bounded review or observation phase, keep the task parked while
the user answers its items, append evidence notes while parked, and resume when
agent execution must continue. A reply
containing another answer or question does not itself resume execution. Keep the
existing claim release and Stop policy. Do not introduce a conversation lease,
question detector, silent lifecycle option, or new command.

This is the author's recommendation, not a prior user decision. It directly
addresses the measured ten-item review. It also covers operator-paced observation
where the agent only explains prepared instructions and reads results. It does
not remove claim transitions when each item requires an artifact change or a resource
reservation. Those transitions represent actual work and remain necessary.

## Evidence and boundary

Checked on 2026-10-22 against ops guard revision
`b12b11396289100f2fc85204a89fb92b9fb64ab9` (unchanged at the design branch base
`605bacb`) and tasks source `49fd5c1824dd0a6f89427f3cf704bae49804b1ee`:

- `hooks/claim-guard::decide` allows a Stop with no relevant live claims, before
  reading wake facts. A held claim with a question has no exemption. Retry is
  one-shot, and the guard never changes ownership.
- `src/commands/park.rs` writes a stamped park note and replaces the claim with a
  park. `src/commands/mod.rs::transition` appends a lifecycle note for every
  transition to doing, including a repeated start. `src/claims.rs::Park` has no
  heartbeat or expiry; it is a handoff, not a reservation. Exclusive holds belong
  to claims and are released by parking.
- `src/commands/status.rs::note` can append without acquiring or requiring a
  claim. It still validates the record/store and stale-copy rules; it refreshes
  an existing owned claim, but does not create one for a parked task. The
  evidence-note exception below uses this existing behavior, not a new CLI mode.
- Hq's `docs/notes/2026-10-08-claim-guard-rollout-judgment.md`, under hq-a558e8,
  records ten of 24 blocks from one session presenting ten review items. Each
  item resumed, asked, was blocked, and parked. Ops-f99904 is the same incident.
  Ops-e76969 is an additional report without recovered event counts. Neither
  doubles the measured ten, and raw transcripts were not reread for this design.
- The review identified obs-175f84 by matching the judgment's description to its
  record. That match is by content, not a transcript/event join. The record was
  checked directly: 2026-10-01 11:04:15–13:27:47Z contains 11 `resumed`, ten
  `parked` and two substantive decision notes. The first decision is at 11:25:05,
  before the item-three question; the second is at 13:17:55 after item eight.
  The item-three park at 11:25:21 precedes the next resume by 1h44m07s.
  Obs commit `f9beaba3ea47f69936b261f2c46136c8b08cf11b` writes all ten verdicts
  to `docs/reports/2026-09-29-outcome-measures-validation.md` at the end.
  An earlier 2026-09-29 park already hands off the ten-item review.
- The [source brief](../notes/2026-09-30-adaptive-hook-messages-brief.md) and
  tasks-56b450's accepted `docs/specs/2026-10-08-claim-guard-policy-design.md`
  assign claim policy to tasks, the executable to ops, and validated wake facts
  to hq. Lore owns global instructions. This proposal changes guidance, not
  that split or the guard's executable policy.

The task record remains the durable handoff. Individual answers and decisions
receive append-only notes without a park/resume pair. This preserves evidence
across idle time, compaction and session loss, and retains required review notes.
It does not erase historical notes or treat evidence as authority to execute.

## Alternatives

| Approach | Benefit | Cost and decision |
| --- | --- | --- |
| Start and park for every item | Existing, explicit ownership at each execution step | Keep for work between questions. Proactive parking removes avoidable Stop retries but retains two lifecycle notes per complete cycle. |
| One released review phase | Existing commands; evidence notes without claims; one park/start pair when entering from claimed work | Recommended for review and observation. Reuse an existing suitable park; acquire before execution, artifact changes or gate notes. |
| Explicit conversation state retaining a claim | Could preserve ownership across questions | Rejected here: needs expiry independent of process liveness, authenticated renewal, restart and late-answer semantics, exclusive-resource handling, worker rules, reader compatibility and guard changes. A trailing question cannot supply those. |

A synchronous native question tool may keep an ordinary turn open, but availability,
answer types and cancellation differ by harness. It is not the cross-harness
contract. Park before the first walkthrough question however it is asked,
including a native question tool that waits within a turn. Neither a blocked
question tool nor a queued child completion may retain a claim or exclusive hold
for this phase. Tool cancellation and timeout grant no answer or approval.

## Workflow contract

1. **Prepare while claimed.** Identify the bounded set of review items or an
   existing runbook, the revision under review, what the person must supply, and
   the precise condition for resuming agent work. Save the necessary artifacts
   and evidence. Resolve all work and live checks owned by this session first;
   a read-only review does not excuse leaving other active assignments behind.
2. **Release before asking the first question.** This includes native question
   tools, not just messages that end a turn. Reuse an existing park only when its
   artifact, phase, worktree and next action still match; otherwise prepare the
   new handoff while claimed. Use the existing command,
   for example `tasks park <id> "User reviews items 1–10 of <frozen artifact>,
   one per turn; agent answers questions and reads observations, then starts
   this task in <worktree> to apply agreed decisions; evidence notes track
   answered and remaining items"
   `--waiting-on user --reason review`. The actual argument is one line and
   uses a real artifact, revision and worktree. Use another existing reason
   when appropriate, or omit it; no new reason vocabulary is proposed. Confirm
   the park succeeded. Read `tasks claims --all-projects` and confirm no claim
   held by this session remains, including parent and step claims. Settle each
   own claim through its legitimate park or completion; do not close foreign
   worker claims. Related workers and pending work retain their existing wait
   and recovery rules. A failed inventory read is not proof of release.
3. **Record and check in one call per answer.** During a known parked phase,
   use `tasks note <id> "<evidence>" && tasks show <id>` in one shell/tool call
   from the recorded worktree, then inspect its result before continuing. If
   the reply also completes a review, append the required plain `review:` note
   in the same call before `show`. Do not add separate pre-note and post-note
   model round trips. The append is evidence about the named frozen revision,
   not authorization to execute. Honor validation and stale-copy refusals;
   if note fails, `show` is not run by this command and recovery needs a read.
   Confirm the resulting task is still open, its park names the expected phase
   and worktree, no other claimant has taken it, and the frozen artifact still
   applies. Read-only artifact verification can be included in the same call.
   With a question-only reply, run `tasks show` without an invented note. On a
   new session or after context loss, first read the record to recover the
   phase; that exceptional recovery is not the ordinary per-item path.
   Do not run `tasks start` merely because the user replied. The sole write
   exception is append-only evidence: answers, decisions, observations and
   required `review:` round notes. Use ordinary validation;
   do not pass flags that change review status or other task fields. Its normal
   record auto-commit is allowed; unrelated staging/commits are not. Do not
   edit task fields, code, artifacts, gate state or other shared state, launch
   work, or perform an operation needing exclusive access. Observation does not include desktop
   control or screenshots; their existing host-use approval still applies.
4. **Cross back into work explicitly.** Before applying an answer, creating a
   changed artifact, writing a `gate:` note, or performing any mutation other
   than the evidence-note exception, run ordinary `tasks start` in the recorded worktree and follow
   its ownership, halt, needs and stale-copy checks. Run `trial-arm` as usual.
   Revalidate the decision against current state, record required review notes
   before acting on findings, and continue the authorized work. If more human
   review is necessary after that work, park for the next bounded phase.
   An answer that makes the next action agent-owned, including the final answer
   or permission to fix an item, triggers acquisition and that work in the same
   turn. Do not end with "I will continue", wait for a nudge, or leave the task
   in the user's queue. If acquisition or work is genuinely blocked, record
   the actual blocker and next actor through the normal handoff; do not force
   takeover or falsely retain a user-review wait. An explicit user instruction
   to stop/wait/only answer still takes precedence.
5. **Preserve evidence before the next item.** The note in step 3 records
   each reply containing an answer or decision, combining
   answers delivered together. Name the frozen artifact, item identifiers,
   verdicts, any decision/authorization scope, and the remaining items or next
   action. Record completed reviews using the required plain `review:` shape
   before acting on them, without acquiring just to write that note. Do not
   treat each item of one unfinished review as a separate round. The standing
   park names the whole phase; the latest evidence note carries its cursor,
   without rewriting the park at each item. Verify a failed write before retry
   to avoid duplicates, and stop progression if evidence cannot be saved.
   After interruption, reconstruct from park plus notes and revalidate the
   frozen artifact; ask again only for missing evidence. No inferred approval
   or replay of an irreversible action.

**Identifying own claims.** `tasks claims` is host-wide and has no caller marker.
For the release check, inspect `session` on rows with `live: true` (including
unverifiable rows). A nonempty `TASKS_SESSION` selects that exact opaque identity;
do not strip prefixes or infer ownership from a PID. Otherwise use the concrete
`CLAUDE_CODE_SESSION_ID` or `CODEX_THREAD_ID`, not a Codex root ID for a worker.
Apply the guard's `held` matching: Codex matches the bare ID and `codex:<id>`;
Claude matches the bare ID, `claude-code:<id>` and the existing `codex:<id>`
spelling accepted by that guard. This list describes the current guard, not
permission to invent cross-harness identities. For relay-resolved identity,
verify the actual qualified identity recorded while claimed agrees with the
native/override selection; on mismatch or missing identity, resolve it before
declaring release. `root_session` identifies related worker claims, not claims
the controller owns; do not dispose of them as own. The guard's one-shot Stop
reminder backs up this check at turn end, but cannot protect a native question
that waits without emitting Stop. All release checks precede that tool call.

If a fix interrupts the walkthrough, the findings received so far close review
round n with verdict `revise`, recorded before the fix. Acquire, fix, freeze a
new revision, and park for review round n+1. Remaining items are now explicitly
reviewed against that revision; do not silently combine findings across versions.

The phase permits discussion, observation and evidence appends without exclusive
ownership. Observing a
person's already-authorized action is allowed only if observing it needs no held
resource and supplies no new execution instruction dependent on reserved state.
Before directing a mutation that relies on task ownership or an exclusive hold,
leave this phase and use the normal execution/handoff protocol. In particular,
parking releases a quiet-host or device hold; this design cannot keep one across
human turns. If a procedure needs that reservation, do not present this workflow
as its solution. Preserve the report and design that requirement separately.

## Interruption, ownership and recovery

| Event | Required behavior |
| --- | --- |
| User answers one item | Append evidence and read state in one call. Remain parked only while the next action still belongs to the user; an answer enabling agent work triggers start and execution in the same turn. |
| User says stop, wait, or only answer | Honor it. If already parked, do not start just to park again; if claimed, use the existing park rule. |
| No answer, closed terminal, abandoned session | No claim or exclusive hold remains. The existing park persists as the durable next action; it expires neither into ownership nor approval. |
| Old claim expires during a failed handoff | Claim liveness is governed by existing tasks rules. Verify the actual park/claim state; absence of a live claim does not prove that a requested park was saved. |
| Another agent starts the task | A user-waiting park is not a lock. Ordinary `start` can acquire it; the old conversation stops task-specific progression when it detects the changed state. It never forces takeover to continue its questions. |
| Another start races the evidence append/read | The combined call is not atomic. A note may land after another session acquires the task. Inspect the following show and stop progression if the phase/owner changed; preserve the note as evidence against its named revision, not an execution instruction. Stale-copy refusal is honored. All non-evidence writes still require acquisition and revalidation; a later race cannot grant that authority. |
| Late answer, different session, changed artifact | Read the record and frozen revision. Reuse an answer only where it still applies; otherwise request the missing review. No automatic resume, transfer or permission. |
| Crash around park/start | Use the existing CLI recovery instructions and authoritative record/store. Do not suppress errors, clear stores or assume a successful handoff from text alone. |
| Worker ends its turn | Existing worker handoff applies: finish or park to its controller/person, release, report. This design supplies no retained worker claim and no exception to `SubagentStop`. |
| Controller has related live worker claims or a check running | Existing wait/recovery rules apply. A parked review task cannot conceal another claim, fabricate wake capability or authorize closure of a worker's claim. |

No new timeout is needed because the proposed waiting state holds no ownership.
Existing claim liveness and completion-proven worker recovery are unchanged.

## Acceptance and measurement

**Ruling: manual validation, not the scripted envelope.** Use one supervised
five-item walkthrough on each harness, with private configuration containing the
candidate Tasks/Lore/Flows guidance. Hq prepares and records the runs; the user
supplies answers and controls the session. Tasks reviews the policy verdict.
No four-script driver, fault injector, API proxy, automated prior-guidance arm,
paid judge, worker spawning or 3/3 repetition gate is required.

Why: the changed surface is reversible guidance. Against a compliant batched
cycle, requests and Stop blocks are equal; the gain is fewer lifecycle notes and
commits. Building a cross-harness runner is disproportionate to that gain.
Cost if wrong: races, failed writes and rare abandonment paths may first appear
in real use. Existing CLI refusals and guard tests remain, but neither prevents
all guidance failures; flows cases must judge later real walkthroughs. In
particular, the guard does not catch agent-owned work left parked.

Retain the released phase rather than document only `start && note && park`:
the latter is simpler but preserves the two lifecycle notes per item that
ops-b13db3 asks to eliminate. Retain the decision to omit a prior-guidance arm:
the historical record and protocol table suffice for this bounded choice, with
no causal cost-saving claim.

**Capability check before the walkthrough.** Record exact harness/version,
model, login/billing mode, candidate guidance hash and private hook configuration.
Use the intended interactive route, not print/exec mode merely for automation.
Inspect its exposed tools and perform a bounded availability check for native
questions and root Stop capture; use an existing validated capability fact when
it covers this exact route/version. Record support and the evidence per route.
This check belongs to setup for the two runs, not an additional experiment suite.

- The core case uses ordinary conversational questions and real root Stop
  events. On a route supporting root Stop, broken wiring or absent expected
  delivery is an incomplete run to repair, not a pass or "not applicable".
- A native-question variant applies only where the chosen route exposes the
  tool. On Claude Code, use its interactive native question after release when
  available. If unavailable, record "not applicable" with tool inventory or
  observed refusal and route/version; perform the message-question case.
  Do not infer capability from the harness's name or from another mode.
- No worker episode is required: the unchanged guard explicitly accepts
  `SubagentStop` only for Codex. Keep existing worker-selection, wake and
  completion-proven recovery tests as regression coverage. Do not demand
  Claude worker event support or treat its absence as a failed walkthrough.
- An unsupported optional interface is "not applicable, with evidence", not
  a behavioral pass. Unknown support is unresolved until checked. Never use
  "not applicable" to waive a failed core behavior. Record which interfaces
  the validation covers; a later newly supported interface gets a bounded
  follow-up check before claiming it was validated.

Local help inspected on 2026-10-22 provides no Codex `exec` per-call spending
reservation; Claude's `--max-budget-usd` is a print-mode API session cap. This
design no longer depends on either being a cross-harness per-call limiter.
Native-question availability itself has not been probed by this design task.

**Run once per harness.** Prepare five numbered items in a disposable task and
artifact, with exact revisions, from the shape of obs-175f84 at obs
`faa1c97a24c188cb9b63f1a5730750d92e83f785` and report commit `f9beaba`.
Use sanitized fixture content, not private transcripts. This is a five-item
adaptation, not an exact replay of the ten-item historical record. Freeze the
items, planned answers and candidate guidance before either run. Include the
flow-on wording in the candidate bundle, but do not execute unrelated flow
work or trial allocation against the user's real registry.

Launch the private harness, its hooks and restarted session with the same
scratch `HOME`, `XDG_CONFIG_HOME` and `XDG_STATE_HOME`, so fixture commands and
the guard use one disposable tasks registry; retain these under the run's
persistent storage through restart. Clear inherited `GIT_*` variables, set
an isolated `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_NOSYSTEM=1`, and verify the
fixture's git top level and git directory before its first write. Never
repoint live launchers, config includes or symlinks. In the private bundle,
provide `~/.agents/bin/trial-arm` as a fixture stub that prints `not enrolled`
and exits 0 without reading live trials or invoking tasks; record its path,
hash and substitution in the run record, and use it for every post-start call,
including after restart. This validates the walkthrough, not trial allocation.

1. Start with one claimed disposable task, prepare a frozen review artifact,
   and let the agent release it before asking item one. Use the native question
   here when supported, otherwise an ordinary question. The user withholds the
   answer briefly while checking there is no own live claim or exclusive hold.
2. Answer item one, then item two. The latter supplies a small authorized fix.
   Evidence must be saved while parked; the partial review closes as round n,
   verdict revise. The agent acquires and fixes in that same turn, freezes the
   new revision, and parks for round n+1 without a nudge.
3. Restart the session before answering item three, after its question and park
   exist. Preserve the disposable task record and prior notes. The resumed
   agent must recover the known answers and remaining items from those records,
   revalidate the revision and avoid repeating the fix. One controlled restart
   per harness is included; no 104-minute idle wait is required.
4. Answer items three and four against the new frozen revision. The ordinary
   path is one evidence-note-and-show call, then the next question, without
   additional park/start pairs or omitted evidence.
5. Answer item five and complete the review. The agent records the required
   review note, acquires and performs the remaining authorized artifact/report
   work in that turn. A turn end that leaves executable agent-owned work parked
   waiting on the user is a failure, even if a later nudge produces completion.

The user follows the prepared answers without coaching lifecycle commands.
Hq's recording session, distinct from the walkthrough agent, judges the trace
and record against the following criteria; the walkthrough agent never grades
itself. The user confirms that any nudge or intervention is reported, not edited
out. Save the complete task record, per-question claim snapshots, tool-call/turn trace,
revision/round notes, and captured native root Stop inputs and outputs. Missing
required evidence means unverified. Do not substitute fabricated hook events.

**Threshold:** two completed walkthroughs, one per harness; **1/1 passes on each**
with every applicable core criterion below met. No automatic repetitions.

- Release precedes every question; evidence notes preserve answers while parked.
- No unnecessary lifecycle cycle on the ordinary answer path, and no avoidable
  review-induced Stop refusal. Initial release, the mid-review fix and final
  execution are legitimate boundaries and are counted separately.
- The fix and final execution occur in their enabling answer's turn; the parked
  user queue does not silently retain an agent-owned action.
- Restart recovers saved answers and revision/round boundaries without invented
  approval, repeated changes or lost evidence.
- Native-question release passes where supported, or has an evidenced
  not-applicable disposition; unsupported worker events are not exercised.

A core failure stops activation for that route, is retained in the record and
is corrected before a separately scheduled rerun. It does not start an open-ended
loop. Parent/step release, races and note/park failures remain policy obligations
and flows eval cases, but are not additional live episodes in this small check.
The rare-path coverage limit is accepted explicitly, not relabelled as tested.
Run existing guard tests through ops's test front door to retain unchanged policy
coverage; they do not demonstrate agent adherence to the new text.

**Time and usage reporting.** The scope is two five-item runs and at most two
controlled restarts: at most four session launches, no workers or automatic
expansion. Report at 15 minutes or 30 observed model requests per walkthrough,
including its restart; these are checkpoints, not pass/fail caps. The supervising
user controls whether to continue or stop. Record actual counts, elapsed time
and that decision, with preparation/review time separate. Before launch, choose
a tracked execution timeout with headroom for setup, human answers and restart;
any hard timeout or user-set limit still stops the run and leaves it incomplete.
Use a native per-session cap only where the selected mode actually supports it.
No per-call reservation or new metering service is required.

The former $10/$30 proposal is withdrawn; it grants no spending permission.
Use existing authorized interactive access, with the user present. Before each
run, disclose its selected model and billing mode. Any additional paid API use
needs the user's approval of that concrete run; do not switch providers or
buy capacity under this design. Subscription use consumes allowance and is not
reported as free or assigned an invented dollar charge. Report actual usage and
cost when exposed, otherwise "dollar cost unavailable". Between runs, compare
usage/time and the highest observed per-run cost (when available) with any
user-set allowance before scheduling the next. A measured maximum is an estimate,
not a guaranteed worst case; if the user requires a hard dollar cap that the
chosen route cannot enforce, use another approved route or stop. No harness run
or activation is authorized merely by preparing this design.

**Counts.** The historical matched window contains **21 lifecycle notes**
(11 resumes + ten parks), plus two decision notes. With its already-existing
park reused, the revised rule predicts **one lifecycle note**, the final resume:
20 redundant transitions removed, ten answer checkpoints including both decisions
retained, and all verdicts still written to the report only after acquisition.
That is 23 versus 11 notes in this window before any additional required review
notes, not a token or cost prediction. Counting the earlier entry park in both
histories gives 22 versus 2 lifecycle notes. A fresh already-claimed review phase
uses one new park plus one resume; it is a different boundary condition.

For an ordinary answered item needing no fix, predict model requests from the
written protocols, counting the request that issues the tool call and the one
that consumes its result and replies:

| Protocol | Predicted requests per item | Scope |
| --- | --- | --- |
| Reactive per-item start, reply, blocked retry, park | 4 | Structural reconstruction of the reported pattern, not a measured request count |
| Compliant per-item start + evidence note + park batched in one call, then reply | 2 | Stronger comparison; already avoids the Stop retry |
| Proposed evidence note + show in one call, then reply | 2 | Same request count as the compliant baseline; fewer lifecycle writes, not a further round saving |

The previous revision's separate show/note/show calls would cost four requests;
that is explicitly rejected. Check actual request counts from the rehearsal,
including any observed failures, final-answer execution, review notes, native
tool waits and recovery overhead; these exceptional turns are not covered by the two-request
prediction. No claim that fewer lifecycle notes alone means fewer requests.

A proactive per-item park/start baseline can already produce zero avoidable Stop
blocks. The proposed rule cannot improve that zero. The ten blocks remain the
historical hq measurement, attributed to the content-matched session; fresh
rehearsal block counts come from captured hook output. Report evidence-note,
lifecycle-note, tool-call and commit counts separately so evidence overhead is
not hidden by lifecycle savings.

**Obs attribution control.** Obs's cost spec
`docs/specs/2026-09-21-cost-per-outcome-design.md` and `views/task_windows.sql`
were checked at `faa1c97`. Start/resume opens a claim window; park closes it.
After park, the current view infers a continuation through the last later note,
gate or run before the next stamped transition on that task or session. With no
later evidence, there is no continuation. Notes restore coverage only up to their
endpoint: intervals are half-open, so the response at the note's timestamp and
later turns may remain unattributed. A transition on another task or a new
session can also prevent coverage. Ten notes do not guarantee complete capture.

Obs is an explicit measurement consumer. For each rehearsal, compare its
dedicated session/turn manifest and raw token/tool/hook totals independently
of task attribution. Join that manifest to obs's attributed turns and report
window, continuation and unattributed counts, including the final response,
restart and same-time note boundaries. Competing-session attribution remains
unexercised by this manual check; do not claim coverage of it. Every captured
turn must reconcile exactly once to attributed or unattributed; discrepancies
fail the measurement check, not behavioral acceptance or guidance activation.
Publish task-cost comparisons only with equal,
complete matched coverage; otherwise report session totals and the coverage
gap, suppressing a task-cost saving claim. Do not add artificial gate/heartbeat
notes to extend a window or count idle wall time as model cost. An attribution
change, if needed, belongs to separate obs work, not a silent SQL change here.
The historical record does not supply matched raw turn costs, so it supports
the note baseline only; do not turn this proposed-guidance-only rehearsal into
a measured cost-saving claim. Missing attribution may block publication of a
task-cost comparison, but does not block correct guidance that passes the hq
behavioral gate and receives the separate activation approval.

## Delivery and owners

This task produces only the reviewed design. After user acceptance, update the
source brief and add the finding to ops-b13db3 before merging the design branch.
Retain ops-e76969 and ops-f99904 as dropped source reports.

Proposed follow-on pieces, to be filed on acceptance with dependencies:

1. **Tasks:** clarify `skills/tasks/SKILL.md`, Session protocol step 5's
   "`start` resumes it", to distinguish execution from parked discussion and
   evidence appends. Add the policy amendment beside the accepted
   `docs/specs/2026-10-08-claim-guard-policy-design.md`, preserving that document's
   historical accepted revision. Own the acceptance assertions above and review
   hq's behavioral results. No Rust change or CLI extension is presumed.
2. **Lore:** clarify `instructions/fragments/decisions.md`'s "park the task
   with that step as the next action and resume it" and regenerate its delivered
   instructions through normal assembly. Neither it nor the tasks sentence
   literally mandates start on every reply; clarify the ambiguous boundary.
   Preserve `instructions/fragments/outcome-notes.md`'s exact review-note shape,
   trial arming, user-stop and worker handoff rules.
3. **Flows:** check `skills/flow/SKILL.md` rule 5, "Park before you stop", against
   the standing-park exception and evidence/gate distinction. Its present text
   appears compatible; add a focused clarification only if needed. Gate notes
   and state transitions still require acquisition. Include flow-on guidance in
   the manual walkthrough when that path reaches the harness.
   Register the new scenarios in `evals/cases/` using its existing subject,
   version, input, rubric and evidence fields. Keep `stall-repeated-park.md`
   unchanged as a regression case: its expected zero agent-owned stalls applies
   both to rehearsals and later real-session judgments. Add the final-answer
   and fix-answer no-nudge conditions to the new case rubrics. Hq still owns the
   live rehearsal; the corpus makes its guidance assertions reusable.
4. **Hq:** prepare and record the two supervised private walkthroughs above,
   using the proposed Tasks/Lore/Flows text before rollout. Check capability
   applicability first and record supported/not-applicable evidence. Record fixture,
   instruction and harness revisions with pass/fail evidence; verify the
   delivered route after separately approved guidance activation. Own native
   event collection and hook judgments, not policy or cost attribution.
5. **Obs:** check `views/task_windows.sql`, `views/task_turns.sql` and
   `views/cost.sql` consumers using the rehearsal manifest and existing reports;
   own the coverage reconciliation and any qualified cost comparison above.
   No new metrics system or automatic attribution change is proposed.

Tasks policy precedes dependent guidance; passing the private hq behavioral
rehearsal precedes activation. Obs reconciliation gates only publication of a
task-cost comparison, not activation. Global instruction activation remains a
separate approved external action. Keep source feedback open until the delivered
workflow is checked. No source text in other projects is edited by this design.

Ops supplies this coordination design; no executable or `cli.toml` change is
needed for the recommendation. If review chooses a new command or retained
conversation claim instead, revise this spec first and land any CLI vocabulary
in ops before its implementation. Actual follow-on implementation plans receive
their own review; design acceptance is not approval to change live instructions,
hooks or harness state.
