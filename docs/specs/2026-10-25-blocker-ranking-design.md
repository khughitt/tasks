# Ranked downstream blockers

Status: proposed for user review. Task: tasks-e637a1. This is a design decision
record, not documentation of shipped behaviour. All choices below are the author's
recommendations until the user approves this revision.

## Intent and boundaries

The user's originating ask was to surface and prioritize tasks by their direct or
indirect downstream impact, and by persistent blocking across time or scope passes.
The originating brief is [blocker ranking](../notes/2026-10-25-blocker-ranking-brief.md).
This design delivers the first measure: a deterministic, explainable ranking of
unmet prerequisites. tasks-71c74d owns prospective history/confirmed-wait collection
and its persistence report. This report neither infers age from task dates nor
counts arbitrary scope notes as confirmed passes. Those measures remain unknown.

Success: a person can find a prerequisite affecting many open tasks, see why it
ranks there, distinguish dependency relief from permission to execute, and notice
when the prerequisite needs the person's attention or the scan is incomplete.
No priorities, task records, parks, claims or registry entries are written by the
report. No paid calls, foreign-task mutations or scheduler changes are authorized.

## Approaches and recommended choice

1. **Dedicated `tasks blockers` read, recommended.** Rank observed downstream
   impact and explain the graph and metadata limits. Leave ready/next unchanged.
   It is useful independently of historical collection and allocator policy.
2. **Also display inherited urgency in ready.** Keep eligibility and order intact,
   but show declared priority, temporary urgency, active beneficiary and path.
   This needs crew-a0952b's explicitly active outcomes, propagation through unmet
   dependencies, lifecycle stopping rules and recomputation. An inactive P1 idea
   cannot confer urgency. Defer this surface: tasks has no active-beneficiary input
   contract, and deriving it from every open dependent would invent a second policy.
3. **Reorder ready/next.** More intervention, with allocation and starvation
   consequences. Defer pending an explicit reviewed choice; this spec grants none.

These are author decisions, not a prior user choice. Report priority tie-breaks
below use the blocker's own declared priority, never an inherited priority.

## Existing contracts to reuse

Source checked at 06a2f03, before document changes:

- `commands/list.rs::resolve_dependency` prefers the captured scan, canonicalizes
  prefix aliases, then follows `moved_to` with `tombstone::follow`. `ready_tasks`
  tests the resulting record's `!status.is_open()`. Done and dropped satisfy;
  shelved does not. Recurring done satisfies even when due, until restarted.
  Missing targets and move loops are unresolved, not satisfied.
- `query::ready_order` uses declared priority, size, creation time and full ID.
  `readiness` and callers add goals, deferrals, parks, claims, halts and needs gates.
  `show.resolved` means found; neither it nor absence from ready proves satisfaction.
- `Scope::open_all`, `Scope::scan` and `moving::inbound` provide registry-wide
  scanning and an incoming-edge precedent. tasks-e8d471 owns sharing the reverse
  scan with show/next; reuse that scan/index rather than another project walker.
- Structural goals are lanes or records with children (`hierarchy::is_goal`). A
  childless hub whose `depends` are its cross-project pieces is indistinguishable
  from an ordinary prerequisite consumer in today's schema.
- `ClaimSnapshot` reads host-local claims and parks. A park can say `waiting_on:
  user`, with optional reason approval/review/decision/etc. No park is evidence of
  no recorded park on this host; it cannot establish authorization on all hosts.

Implementation must share canonical dependency satisfaction with tasks-e90b4f
(under tasks-606dd3, originating tasks-1bbe49), not copy a status rule into the
new command. A common internal resolver/verdict is sufficient; calling another
CLI process for every edge is unnecessary. This design does not choose that
task's public JSON shape or change its semantics.

## Scope and snapshot

Proposed surface: `tasks blockers`, with the existing scope arguments `--project`,
`--group`, `--all-projects`, global JSON/pretty/color/headers arguments and
`-n/--limit N`. Default is the invoking project; cross-project scanning is explicit.
No status/priority filters, new sort modes, prime section or detail subcommand in
the first delivery. `--limit` defaults to 20, accepts nonnegative integers (0 gives
counts/coverage only), and truncates after ranking. Omit it for 20; request a larger
N to retrieve more. All candidates are evaluated before truncation.

The dependent universe is every record scanned in the requested projects,
including shelved and closed records needed for classification. Local reads retain
the invoking worktree; explicit wide scopes use registered checkouts just like
existing reads. Never silently replace a scanned record with its main-checkout
copy or claim-holder worktree. Capture task files once per project and memoize
resolution per canonical ID. This is a best-effort read, not an atomic snapshot
across projects; report the start timestamp as `observed_at`.

Resolve each scanned open record's dependencies even when a target is outside the
dependent universe. Such a resolved foreign prerequisite can be a ranked row;
only in-scope dependents contribute counts. Do not recursively import out-of-scope
records as dependents or traverse their own outgoing edges. For complete chains
through foreign dependents, request `--all-projects`. Report all such boundary
target IDs, so a local count is never presented as portfolio-wide impact.

## Graph and counts

Use `dependent -> prerequisite` below. Build one edge for each distinct canonical
resolved pair whose dependent and target are both open. An unresolved reference
is retained separately. Closed dependents contribute no live edges. A closed
prerequisite ends traversal even if its historical dependencies remain open.
Moved tombstones contribute no retained historical edges: resolve to their final
live identity. Alias spellings, old move IDs and the destination ID deduplicate
to that identity. Retain original reference IDs in unresolved diagnostics.

For open prerequisite B, reverse-walk these edges with a visited set:

- Direct dependents are unique open in-scope records with an edge to B.
- Transitive dependents are unique open in-scope records reaching B in one or
  more edges, including direct dependents. A diamond counts its endpoint once.
  Always exclude B itself, even in a cycle. Do not add parent/child or lane edges.
- Partition both sets into `work` (not a structural goal) and `goals`. Ideas,
  blocked, deferred and shelved work still count; these are affected records,
  not runnable jobs. Known goals remain visible separately and contribute zero
  to work counts, but explicit dependency paths through them are still traversed.
- A childless cross-project closeout hub is included as work, since there is no
  reliable structural discriminator. Every response and pretty footer explains
  this limitation. Never guess from prose, title, tags or cross-project fan-out.
  This is a known inflation risk; explicit goal/edge-purpose metadata would need
  its own reviewed change. The fixture below shows both identifiable and
  unidentifiable closeout consumers.
- `dependencies_cleared_direct_count` counts direct **work** dependents whose
  dependency set would be wholly satisfied if only B became done at observation
  time. Every other dependency must positively satisfy the shared verdict.
  Duplicate spellings of B count once. Unknown and other open dependencies prevent
  inclusion. Do not recursively complete anything. This count does not check
  lifecycle, permission, claims or resources, and must never be labelled ready,
  actionable, or jobs unblocked. A shelved dependent can qualify and remain shelved.

Rank every resolved open prerequisite with at least one direct dependent (work or
goal). Sort by transitive work count descending, direct work count descending,
declared priority ascending, creation timestamp ascending, canonical full ID
ascending. Goal counts, dependency-cleared counts, park state and unknown history
do not break ties. Emit consecutive 1-based ranks before limiting. Goal-only
rows remain visible at the end; zero counts are explicit. Counting is per record,
not estimated effort, value or sum of priorities.

Cycles from synced records must terminate. Emit sorted cycle components (including
self-loops), exclude self from all counts, and retain observed impact; a cyclic
row's `cycle` flag requires investigation, not a launch recommendation. A move
loop is an unresolved reference rather than a dependency cycle. No graph repair.

## Honest coverage and errors

Coverage names requested and scanned project prefixes, unreachable projects and
out-of-scope resolved target IDs. `coverage.complete` means every requested project
was scanned and every referenced target resolved; it says nothing about projects
outside the request, other hosts' parks or a globally atomic snapshot. Unknown
references make it false even if all project scans succeeded. Counts then remain
observed lower bounds within the requested universe, and rank is **observed rank**:
missing projects/edges can change the ordering. A local complete scan still has
only local dependent coverage, printed in its heading.

Use existing typed errors for invalid args, unknown groups, malformed config,
corrupt task records, I/O errors and malformed claim stores; do not silently drop
parse failures into a successful partial report. The existing wide-scan
unreachable-project warnings permit a partial result. A missing/unavailable target
or move loop produces an unresolved diagnostic, never a fictitious open task.
Diagnostics carry dependent ID, stored reference ID, canonical ID if known, and
the shared resolver's available reason; use `unresolved` when it cannot distinguish
missing from a move loop, rather than promising new diagnostic precision.
Unresolved references appear outside ranked task rows, sorted by dependent then
reference ID. No unknown target receives an invented title, priority or rank.

## User-held blockers and authorization limits

All ranked rows carry stored status, needs, future deferral when present, structural
goal flag and an observed park when present. Load local claim/park evidence for
all resolved ranked prefixes, including foreign boundary targets, using the
existing store. Report the observation host; no remote store probing or writes.

An explicit park `waiting_on: user` yields `attention: user` with its actual reason
and next step, including approval, review, quiet or environment. An unparked
record with `awaiting_user: true` also yields user attention with that source;
otherwise `attention: unknown`. A park waiting on agent is shown as evidence but
does not prove authorization; no row ever claims definitely agent-actionable.
Claims/holds are not an alternative authorization source or ranking input.

The owner must record a gate before the report can identify it. The motivating
crew-ba0a02 is an unstarted todo with an approval bound only in its body: the
report returns todo/attention unknown and the general metadata caveat. A later
owner-recorded `tasks park crew-ba0a02 "Obtain authorization for the prepared
bounded calls" --waiting-on user --reason approval` would expose user attention;
this is an example, not an instruction to mutate that task in this work.

A declared need says what resource is required, not that approval exists or is
missing. For example, an owner-agreed `paid-calls` need would be shown by name
and would leave attention unknown unless a user-wait
signal is also recorded. There is no current vocabulary declaration to adopt here.
On another host with no shared park evidence, the same task remains unknown even
if this host once observed approval waiting. Never extract a gate from free prose.

## Initial output contract

JSON is a new response type; existing reads keep their shapes. All collections
below are present even when empty, and counts include explicit zeroes. Optional
row facts are omitted when unset, matching existing task summaries.

```text
BlockersOut {
  observed_at: UTC timestamp,
  coverage: {
    mode: "local" | "project" | "group" | "all_projects",
    requested_projects: [prefix], scanned_projects: [prefix],
    unreachable_projects: [prefix], boundary_targets: [canonical task ID],
    complete: bool
  },
  park_evidence: { host: string, scope: "host_local" },
  goal_detection: "structural_only",
  ranking: "transitive_work,direct_work,priority,created,id",
  total: integer, shown: integer, omitted: integer, limit: integer,
  blockers: [{
    rank: integer, id: canonical task ID, title: string, status: existing status,
    priority: integer, goal: bool | null, cycle: bool,
    direct_work_count: integer, transitive_work_count: integer,
    direct_goal_count: integer, transitive_goal_count: integer,
    dependencies_cleared_direct_count: integer,
    attention: "user" | "unknown",
    awaiting_user: bool, needs: [existing need name],
    // optional: future defer date and existing ParkInfo-shaped observed park
    // optional: up to three direct dependent IDs, sorted by canonical ID
  }],
  unresolved: [{ dependent: task ID, reference: task ID,
                 /* optional canonical ID */ reason: string }],
  cycles: [[canonical task ID]],
  warnings: [string]
}
```

`total` is the full ranked row count; `shown + omitted = total`. Limit applies only
to ranked rows; diagnostics are complete and may be large, which is an explicit
first-version payload limit, not silent truncation. No notes/bodies or transitive
ID lists are emitted. Name the optional sample `direct_examples`; omit when empty.
Declare optional fields `defer` (existing date format) and `park` (existing
`ParkInfo`, including waiting_on/reason/next_step when set). Needs retain names;
configured meaning can be looked up through existing project configuration, not
a new response schema. Do not emit unsupported blocking-since or pass-count fields
with zeroes; those metrics are unavailable, stated in warnings and help.
The row's `goal` is null for an out-of-scope target unless its lane flag proves
it is a goal; its project's child records were not scanned. For in-scope targets,
use structural goal detection. Counts partition only scanned dependents, whose
child relationships are available. A false goal value never rules out a
prose-only cross-project closeout hub.

Pretty output uses the same sorted rows and limit: heading with dependent scope
and complete/partial coverage; columns rank, ID, declared priority, status,
transitive work, direct work, dependency sets cleared, goals (direct/transitive),
attention, title. `--headers` uses existing conventions. Below a user-attention row,
print its park reason/next step or stored awaiting-user source, and print declared
need names when present; below a cyclic row,
print investigate cycle. Print unresolved references and cycle components after
the table. Footer always explains structural-only goal detection, host-local park
limits, unknown history and that dependency sets cleared does not imply ready.
A partial result also warns that ranks may change when missing coverage returns.
Color comes from existing project colors. JSON uses full canonical IDs throughout.

## Verification cases and expected verdicts

Use disposable fixtures, no changes or calls to real foreign tasks. In this table
all records are open work unless stated. A needs B means A declares B in depends.

| Record | Priority | Dependencies / structural fact |
| --- | --- | --- |
| aa-000001 (B) | 2 | none |
| aa-000002 (L) | 3 | B |
| bb-000003 (R) | 1 | B |
| bb-000004 (D) | 1 | L, R |
| bb-000005 (M) | 2 | B, aa-000006 (X) |
| aa-000006 (X) | 2 | none |
| aa-000007 (G) | 0 | B; has a closed child, so structural closeout goal |
| bb-000008 (H) | 0 | B; childless cross-project closeout hub, purpose in prose only |

With both projects scanned, B has direct work 4 (L,R,M,H), transitive work 5
(L,R,D,M,H), direct/transitive goals 1 (G), dependency sets cleared 3 (L,R,H).
D counts once despite its diamond; M remains blocked by X. X has direct/transitive
work 1 (M) and sets cleared 0. L and R each affect D, and clear 0 because the other
remains. Ranking is B, R, X, L under the stated ties. G's P0 does not boost B;
H illustrates unavoidable work-count inflation without typed closeout metadata.
No parent edge is invented from G to its closed child.

Additional cases the implementation plan must retain:

- Local aa scope counts B's L as work and G as a goal; only L clears. Foreign
  dependents disappear with explicit local coverage. A foreign target of a local
  dependency still gets a boundary row. All-projects restores the full diamond.
- Equal counts and priority tie by created, then full canonical ID, independently
  of filesystem/registry iteration. `-n 0`, default 20, limit beyond total, empty
  graph and goal-only blockers preserve totals/arrays and deterministic output.
- Duplicate alias/old-move/destination references count once. Done/dropped targets
  and historical dependencies of closed consumers add no edges. Recurring done
  satisfies even when due; restarting it creates blocking edges again. An alias
  is not an extra task; a move chain ending open is one prerequisite.
- Missing target, unavailable project, unregistered target prefix and move loop
  produce diagnostics/partial coverage without a fabricated task. A malformed
  scanned record/config/store is a typed failure. Partial ranks are labelled.
- A dependency cycle and self-loop terminate, exclude self and expose cycles;
  closed nodes break paths. Shelved/idea/deferred dependents count impact without
  claiming eligibility; structural goals are counted separately through paths.
- Synthetic equivalent of crew-ba0a02: prose-only approval gate gives attention
  unknown; explicit user/approval park gives attention user and explanation,
  without changing its impact or rank. Its consumer remains affected in both.
  A declared need alone remains unknown. No park on another host remains unknown;
  an unreadable existing store errors. Also cover awaiting_user and agent-wait parks.
- Graph and status fixtures assert dependency-set relief separately from actual
  ready membership (multiple blockers, user-held consumer, shelf, deferral, claim,
  goal). Existing ready/next order and JSON remain unchanged.

## Implementation handoff and review gates

After user design approval, write an implementation plan for separate user review.
It must explicitly sequence the shared incoming scan (tasks-e8d471) and canonical
dependency-verdict work (tasks-e90b4f) before report graph integration. Reuse their
accepted internal contracts; if absent, retain explicit dependencies on the report
implementation steps, without blocking this design or duplicating those features.
New command/option rows must first land in ops's cli.toml with its own task and
review process, then be adopted by the vendored tool before parser changes here.
Do not edit the vendored table by hand or treat this spec as rollout approval.

The plan covers graph/ranking, CLI/output, fixtures and current-behaviour guidance
(README and skills/tasks), including focused tests, fast pre-commit checks and
the full-suite baseline at execution Setup. No performance claim is established
by this design; one scan plus memoized resolution and visited-set graph walks are
the initial approach. Profile before caching or a more complex graph engine.

Record the reviewed result in the brief and on tasks-e637a1; accepted plan steps
remain its children. This task stays open through design and plan review and any
retained implementation. tasks-6166d6 closes only after the report and independent
history/confirmation deliverables actually ship with evidence.
