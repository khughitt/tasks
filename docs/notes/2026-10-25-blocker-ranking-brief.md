# Surface blockers by impact and persistent delay

## Problem

tasks-e637a1 asks which prerequisites hold up the most work and which have held it
up longest or through repeated scope passes. This is one blocker-discovery effort
inside Work discovery and resumption (tasks-46d207), not a new execution lane.

Origin is **mixed**, unchanged. In filing session
claude-code:75e5d0a1-29d8-4f5c-8267-a25b055879ed, the user's own turn (JSONL line 17)
asks for downstream impact, direct or indirect, and prolonged or repeated blocking.
The add call at line 115 and its result at line 116 uniquely match the creation:
tasks-e637a1, action created, 2026-10-25T21:31:04Z. The later response at line 132
labels inherited urgency, edge events, goal exclusions and CLI placement as agent
proposals. No older origin session is named; no intent question remains for this pass.

## Current behaviour and evidence

Checked against code at c8de557 on 2026-10-25; no implementation or runtime pilot.

- `src/commands/show.rs::describe` exposes outgoing dependencies and children.
  Existing tasks-e8d471 (P2/s/mid/direct) adds incoming dependents, locally by
  default and across registered projects on request. Reuse it; design can proceed
  before it lands. `src/moving/mod.rs::inbound` already scans registered projects
  with `Scope::open_all` and canonicalizes dependency IDs.
- `src/commands/graph.rs` draws one project's graph. `src/query.rs::ready_order`
  orders by declared priority, size, creation and ID; neither ranks blocker impact.
- `Task.depends` is an ID list. `src/commands/dep.rs::add_dependencies` serves
  `dep --on`, `add --depends` and `edit --depends`; none records an edge timestamp.
  Removal, clearing, replacement and re-addition also matter to any event design.
  Current task creation/update dates cannot establish uninterrupted blocking age.
- Current records confirm the motivating waits: crew-1c3624's October 25 scope
  note still names crew-8b52fb and autonomy-1aac03; crew-9b4b47's scope notes on
  October 15, 19 and 25 name crew-ba0a02. Its latest note calls the probe unstarted
  and spending-gated. These are observations, not a verified continuous edge history.
- Related crew-a0952b already records allocator-owned inherited urgency, separate
  from stored task priority and admission. tasks-298107 examines priority meanings;
  tasks-606dd3/tasks-1bbe49 address dependency satisfaction. No existing blocker
  brief, attached draft spec or overlapping open research was found in this pass's
  document, attachment and cross-project task searches.

## Constraints

Preserve stored priorities, existing picker ordering and admission gates. Reuse
dependency satisfaction, alias/move resolution and registered-checkout authority;
`show.resolved` means found, not satisfied. Missing or unreachable evidence must
stay explicit, and an incomplete project scan cannot imply a complete ranking.
Cross-project scans stay explicit; tasks-e8d471 records their prior measured cost,
which this pass did not remeasure. Changes to JSON need an explicit implementation
task; new CLI vocabulary lands in ops before adoption here. No shared needs are
declared. A brief authorizes neither implementation nor spending or live calls.

## Alternatives

1. **Current lean, controller's recommendation:** a read-only blocker report with
   separate impact and persistence measures, evidence coverage and the blocker's
   state/next action. Start with one explicit read surface; defer automatic `prime`
   expansion and scheduler changes. Keep unknown history visible. This satisfies
   discovery without redefining which work an agent may start.
2. Add blocker sorting to `list`/`ready` and a `prime` section immediately. This
   couples observation to selection and scan/payload costs before the measures are
   defined; defer it pending the report design and a bounded verification case.
3. Reuse crew's allocator as the only surface. It owns execution allocation and
   inherited urgency, but does not replace general blocker discovery or historical
   reporting. Coordinate its consumption rather than build another scheduler.

## Unanswered questions

The design author resolves these in a written spec for user review; no separate
research task is needed to rediscover the established code paths.

- Define unique direct/transitive open dependents, diamond deduplication, cycles,
  cross-project scope and ordering/ties. Reachable work is not necessarily work
  made ready by completing one prerequisite when other blockers remain.
- How should goals and closeout edges contribute? Organizational links are not
  dependencies; the stored edge has no intent field that safely identifies a
  closeout-only wait. Choose an explicit presentation without guessing intent.
- Define edge age versus continuously unmet age and which scope passes count.
  Compare prospective lifecycle evidence with optional git reconstruction; cover
  all dependency writers, removal/re-addition, reopen/recurrence and aliases/moves.
  Do not count unrelated passes as proven repeated blocking or substitute task age.
- Choose one JSON/pretty read contract, ranking criteria and unavailable-history
  representation. Explain lifecycle/user-held blockers without overriding them.
  Verification needs a small cross-project graph with diamonds, mixed priorities,
  multiple blockers, closed/moved targets and unknown or discontinuous history.

## Proposed decomposition

Goal tasks-6166d6 (P2/m/high/planned) is an ordinary child of tasks-46d207,
holding tasks-e637a1 and tasks-71c74d, **Design blocker impact and persistence
reporting from the brief** (P2/s/high/planned). The design task develops the spec
and subsequent plan through their user review gates. It reads tasks-e8d471 and
the related contracts above;
the existing reverse-dependency implementation need not finish before design.

tasks-e637a1 is **briefed**, retaining its full capture and existing dependency on
tasks-e8d471, with an additional dependency on tasks-71c74d. The design's result
must reach this brief and `tasks note tasks-e637a1 --no-reviewed "<finding>"`
before its result branch merges. The dependency also wakes the idea on completion.
No implementation was started; neither this brief nor its task filings approve a
design or implementation plan.
