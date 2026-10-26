# Surface blockers by impact and persistent delay

## Problem

The user asks to surface and prioritize blockers by direct/indirect downstream
impact and by how long or through how many scope passes they have blocked work.
These are two deliverables: a ranked downstream report, and evidence collection
for persistent blocking. The report reads data; prospective history and scope
confirmation require writes. Collection should ship early because its coverage
starts only at deployment. Neither design waits for the other or for git backfill.

Revision 2 addresses the review of 7085e03 (verdict: revise; reviewer identity
supplied as claude-opus-5.5, harness unspecified). One existing goal now has two
substantive children; no additional records are needed.

## Current behaviour and evidence

Code checked at 7085e03, unchanged by this revision; no runtime pilot was run.

- `src/commands/show.rs::describe` exposes outgoing dependencies and children.
  tasks-e8d471 adds incoming dependents using local or explicit cross-project scans;
  `src/moving/mod.rs::inbound` already uses `Scope::open_all` and canonical IDs.
  Reuse that work. Its implementation is not a prerequisite for either design.
- `src/query.rs::ready_order` uses declared priority, size, creation and ID.
  `src/commands/graph.rs` draws one project's edges; neither ranks blocker impact.
- `Task.depends` is an ID list. `src/commands/dep.rs::add_dependencies` serves
  `dep --on`, `add --depends` and `edit --depends`; no edge timestamp is recorded.
  Current task dates cannot prove continuously unmet age. Removal, clearing,
  replacement, re-addition and later lifecycle changes affect history semantics.
- crew-1c3624 still names crew-8b52fb and autonomy-1aac03. crew-9b4b47 has four
  scope notes naming crew-ba0a02: two on October 15, one on October 19 and one on
  October 25. These prove observations, not continuous edge history or necessarily
  four distinct passes. The scope skill has no fixed confirmed-wait record yet.
- crew-ba0a02 is an unstarted todo with authorization requirements in its body,
  no declared need and no observed user-wait park. A status-only report cannot
  recognize that prose-only gate. This pass changes no foreign task or host park.
- crew-a0952b records allocator-owned inherited urgency and active beneficiaries.
  tasks-298107 examines priority meanings; tasks-606dd3/tasks-1bbe49 address
  dependency satisfaction. No overlapping blocker-history research or handoff was
  found in the original document, attachment and cross-project task searches.

Origin remains **mixed**. Filing session
claude-code:75e5d0a1-29d8-4f5c-8267-a25b055879ed contains the user's own request at
2026-10-25T21:29:33.202Z, message UUID e6c8baae-a7c8-4416-a6d1-bbd718fd15ca.
The uniquely matching add call is toolu_019n9n6vmJWzyw3xEcaaa8Xy; its result names
tasks-e637a1, action created, at 2026-10-25T21:31:04Z. The filing response labels
urgency inheritance, edge events, exclusions and CLI placement as agent proposals.
These native IDs and content/time anchors replace fragile line-number locators.
Stage 1 is matched; no older origin_session is named (stage 2 not applicable).
The full original capture remains in tasks-e637a1 under Original capture.

## Constraints

“Prioritize” means an explicitly ranked report of blockers, not just an unsorted
count. The recommended first version leaves ready/next ordering unchanged; this
is a design choice the user can override, not a settled narrowing of the ask.
Keep declared priorities and admission gates intact. Inherited urgency must name
its active beneficiary and cause rather than treating every inactive P1 idea as
an urgency source; coordinate with crew's existing rule.

Reuse dependency satisfaction, alias/move resolution and checkout authority;
`show.resolved` means found, not satisfied. Incomplete scans and historical gaps
must be explicit. Cross-project scans stay explicit; prior cost measurements in
tasks-e8d471 were not repeated. JSON changes need an implementation task, and new
CLI vocabulary lands in ops before adoption. No shared needs are declared here.
A need describes a required resource, not authorization to spend or execute.

For user-only blockers, prefer an owner-recorded park with --waiting-on user
--reason approval when approval is actually needed. A declared need is an
alternative only with an agreed vocabulary and explicit meaning. Parks are
host-local: missing park evidence, including on another host, cannot prove the
work is agent-actionable. The report must expose metadata limits instead of
classifying authorization from prose. This brief grants no spending or live calls.

## Alternatives

1. **Current recommendation:** rank downstream impact in a focused report and
   independently design/ship forward dependency events plus scope-confirmed waits.
   Define deterministic order/ties; distinguish unique impacted work from work
   immediately made ready. Keep age unknown until evidence supports it. Git
   reconstruction is optional backfill, not a gate on either first delivery.
2. **Middle option to assess in the report design:** also display inherited
   urgency and its cause in ready, without changing its sorting or eligibility.
   This makes a P2 prerequisite holding up active P1 work visible. It expands the
   read surface and needs explicit active-beneficiary/coverage semantics.
3. Reorder ready/next by inherited urgency, or require a combined ranking/history
   design before shipping either half. Defer these choices: scheduler changes
   need explicit approval, and combined delivery delays both the cheap report
   and the start of historical coverage. They remain choices the user can revisit.

## Unanswered questions

- **Downstream design, tasks-e637a1:** settle unique direct/transitive counts,
  diamond deduplication, deterministic ranking, goals/closeout edges, cycles,
  aliases/moves, incomplete scope and the first JSON/pretty read contract.
  Count impacted work separately from work made ready when multiple blockers
  remain. Decide on display-only urgency explicitly; do not silently reorder.
- **Evidence design, tasks-71c74d:** settle prospective event schema, coverage
  start, atomic persistence and every add/edit/dep/clear/replace path. Cover
  failed/no-op writes, removal/re-addition, reopen/recurrence and aliases/moves.
  Separate edge age from continuous unmet intervals. Design optional git backfill
  without making pre-deployment history appear known or delaying collection.
- The same evidence track considers a fixed scope observation, for example
  waits: crew-ba0a02. Record only waits actually reconfirmed; identify distinct
  passes, deduplicate repeated notes, and report “reconfirmed N times since D”
  with first/last observation. This can start without reconstructing edge age;
  it does not establish uninterrupted duration. Define changing/closed waits too.
- **Verification:** the downstream design includes a cross-project diamond,
  mixed priorities, multiple blockers, goals, moved/closed/missing targets and
  unknown history. Include crew-ba0a02's prose-only approval gate, then its
  explicitly user-held park and a declared-need variant, plus unavailable park
  evidence. The evidence design tests all mutation paths, interval changes and
  distinct versus duplicate/unrelated scope confirmations. Design authors bring
  specs and plans to user review; no new research-only task is required.

## Proposed decomposition

- tasks-6166d6 remains the P2/m/high/planned coordination goal under Work
  discovery and resumption (tasks-46d207). Done means a usable ranked downstream
  report, shipped event/confirmation collection and a persistence report of
  supported intervals/reconfirmed passes with honest coverage. Only git backfill
  is optional and gets an explicit disposition. An idea's disposition is
  not a completion criterion. Remove its redundant brief source; keep the ref.
- tasks-e637a1 is now **scoped**, P2/s/high/planned, **Design ranked downstream
  blocker reporting**. Preserve its full original capture. Remove its waits on
  tasks-71c74d and tasks-e8d471 so this design is independent; retain the latter
  as an implementation input and establish implementation ordering in the plan.
- tasks-71c74d is **rescoped**, P2/m/high/planned, **Design early dependency events
  and confirmed-wait recording**. Start this track early; its first shipping
  milestone is collection, independently of the ranked report and optional
  historical reconstruction; its later report surfaces supported duration and
  confirmed-pass measures. Scope-skill changes are explicit work in this track.

Both design tasks retain accepted implementation steps as children. Replace the
obsolete Ideas it wakes instruction: there is no waiting idea after promotion.
Keep findings on the tasks and in this brief; goal closeout requires the actual
retained deliverables. This revision approves no design, plan or implementation.

## Downstream design review handoff

tasks-e637a1 now has a proposed [ranked blocker design](../specs/2026-10-25-blocker-ranking-design.md),
initially frozen for user review at c45fc7f. A read-only builder check by
codex/gpt-6.1-sol returned "nothing important left"; it grants no user approval.
The design recommends a dedicated impact-ranked read, preserves ready/next, keeps
dependency relief separate from execution permission, and documents structural-only
goal detection, local coverage and host-local approval evidence. Display-only
inherited urgency remains an explicit deferred alternative, under crew's active
beneficiary policy. No product code or CLI vocabulary changed.

The next step is the user's design review, followed by a separately reviewed
implementation plan. That plan must sequence the shared incoming scan and canonical
internal dependency-verdict extraction before integration. History collection remains independent
under tasks-71c74d. This design task and its coordination goal remain open.

Review round 3 (claude-code/claude-opus-5-5) returned revise with three Important
and five Minor findings against c45fc7f. Revision 2 addresses all eight: foreign
paths cut by local/group scope mark coverage partial with named unfollowed targets;
rows show their own open/unresolved dependencies; shared internal rule extraction
can ship here without waiting for tasks-e90b4f's public design. It also names the
review/open_deps callers, explains the goal-child attribution cost, limits
awaiting_user to reviewed ideas, distinguishes task_needs from park.needs and
shows observed live/unverifiable claim holders. Ranking still weights all open
work equally, with per-status counts making the composition visible. Compact
pretty rows move explanations below the row. New fixtures cover each change.
The task notes and next review request name the new frozen revision; c45fc7f is
superseded. This revision is not approved and no implementation plan was drafted.

Fresh builder review round 4 of frozen d2940d0 by codex/gpt-6.1-sol accepts for
planning with no Important or Minor findings. Full reviews and the updated ledger
are attached to tasks-e637a1. The user still reviews the design before planning.

The supplied round-5 review (claude-code/claude-opus-5-5) accepts d2940d0 with one
nonblocking Minor: the bb-scope fixture also has boundary X, while only L is
unfollowed. Following the user's delegation to accept and proceed, the controller
accepted the design and preserved its frozen bytes. The
[implementation plan](../plans/2026-10-25-blocker-ranking.md) corrects that fixture
and explicitly parameterizes lookup authority for the shared internal helper.
The plan has three blocked implementation steps plus a blocked ops vocabulary
prerequisite; their review gates must be released deliberately after plan approval.
The plan still needs its separate user review. No product code has changed.
