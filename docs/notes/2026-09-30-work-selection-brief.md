# Work selection and waiting work

Scoping handoff, 2026-09-30; refreshed 2026-10-08. Goal: tasks-46d207.
This is not an approved design. This pass reviews tasks-e34b99 and tasks-00bac8.

## Problem

Make useful work visible: distinguish ideas that still need review, discover tasks
that could use an idle host, and notice when waiting work can resume. These are
separate improvements within the existing work-discovery lane.

## Current behaviour and evidence

- **Idea review (tasks-e34b99):** `skills/scope/SKILL.md` excludes an idea by its
  latest note: `scope:` or a pending `proposal:` excludes it; a later note readmits
  it. `src/model.rs::Task`, `src/filter.rs::TaskFilter` and
  `src/output.rs::TaskSummary` have no review flag. `src/commands/sample.rs` parses
  pending proposals separately. This pass needed individual `show` calls to find
  five eligible ideas among 24. The task's 2026-10-08 note records the user's choice
  of a top-level boolean and a left-most `*` for ideas not yet reviewed. The name
  `reviewed`, explicit wake reset and one-time backfill are agent proposals in that
  note, not a reviewed implementation contract.
- **Unprepared host work (tasks-00bac8):** `src/commands/quiet.rs` calls
  `parked::rows_preferring` with `Reason::Quiet` and `Prefer::Recorded`. Its rows
  originate from park entries, so a need-bearing todo without a park cannot appear.
  `src/filter.rs` already matches named needs for list/ready. The quiet-queue design
  sections 4-5 specifies prepared resume recipes and `ParkedRow` JSON; the
  lanes/needs/groups design section 11 explicitly deferred unprepared-task discovery
  and merging task needs with park recipes. Source inspection confirms the boundary;
  the follow-up will compare actual filtered command outputs.
- **Delivered context:** tasks-77dbc6 and tasks-e02860 delivered groups, lanes and
  shared needs under tasks-ece1e2 and
  `docs/specs/2026-10-03-lanes-needs-groups-design.md`. The old focus idea
  tasks-9bdd68 is dropped in favor of lanes. tasks-479a6f delivered pretty-table
  quiet markers. Commits `e160aff` and `d78b1ef` introduced task needs and lanes;
  `9896564` moved quiet/parked presentation onto the table module.
- **Calendar waits (tasks-44b889):** doing deferral remains excluded; park has no
  resume date. Deferral filtering (`80a27e2`) and due visibility (`9e9b12b`) remain
  the starting point for tasks-157d05, not evidence that in-progress waits work.
- **Historical sources:** the original groups thought lookup
  `mindful --json show 5ccf9506e36842cf8e635e55248040a9` found no match;
  correction (2026-10-08, tasks-bde218): that command omitted the `thought:` type.
  The canonical `thought:5ccf9506e36842cf8e635e55248040a9` lookup succeeds and the
  archived source is available. The historical miss does not establish missing context. The September 30 local `sample --limit 0 --older-than 0d`
  check returned no warnings, not an exhaustive cross-project decision count.

## Constraints

Preserve reports, sources and existing parents. Reuse tasks-46d207 and this brief;
no new goal or lane is needed. Review state is distinct from status, executable
readiness, plan approval and pending human decisions. Existing JSON is a contract:
a boolean and filter need explicit design, migration and consumer checks.

Prepared quiet work has a next step, estimate and idle/headless recipe. A named
exclusive device need alone proves neither idle-host suitability nor preparation.
Discovery must distinguish blocked, deferred, user-held and claimed work, respect
resource holds, and never authorize launching it. The project declares no needs;
these follow-ups require neither a host reservation nor a new need vocabulary.
Existing groups, calendar and cross-project designs retain their own review gates.

## Alternatives

1. Keep note-derived eligibility and use existing named-need filters beside the quiet
   queue. The filters may suffice for host-work discovery, but the user has chosen
   explicit review state for ideas.
2. Add the requested boolean and unread marker; explicitly reset it when evidence
   warrants another review. First compare existing need filters with the host-work
   use case. This is the current lean: separate designs, no combined state machine.
3. Add a richer review-state enum and mix all exclusive-need todos into `quiet`.
   The boolean request gives no reason for the enum, and exclusivity does not establish
   a prepared idle-host run. Do not adopt this without additional evidence.

## Unanswered questions

- **Review design author, then user review:** unset/false/true persistence and JSON;
  non-idea transitions and return to idea; filter naming; atomic note-plus-reset;
  safe rerunnable backfill; proposal handling versus user-waiting parks. Preserve
  audit history and specify whether incidental notes can still readmit ideas.

Resolved 2026-10-11: the accepted review-state spec and reviewed implementation
plan use `reviewed` on ideas, explicit atomic `note --no-reviewed` wakes, and
`needs_review` for unreviewed ideas and unassessed leaf todo/blocked rows. Incidental
notes keep an idea reviewed; pending proposals remain note-based. The left-most
`*` mark follows the same predicate as JSON and `--needs-review`. Implementation
is tasks-b8f3c9, following `docs/plans/2026-10-11-review-state.md`; rollout requires
upgrading every reading host before the first flag write and pausing writers for
the frozen-snapshot backfill. The current note rule remains active until rollout.
- **Quiet-discovery investigator:** do list/ready need filters answer preparation
  and idle-session selection, including TASKS_WITHOUT, claims and holds? If not,
  identify the missing view and its rows without inventing estimates or recipes.
- **Calendar design author, then user review:** choose synced task date versus park
  date or todo-plus-defer; specify due visibility, recurrence, early resume and
  re-park. A date must not authorize user-held work.

## Proposed decomposition

- **tasks-a9b8d3** — P2/m/high/planned: design explicit idea review state, then a
  reviewed implementation plan. Wakes **tasks-e34b99** with the approved finding
  and a brief update in the same commit. Original report and input note stay intact.
- **tasks-b65dbc** — P3/s/mid/direct: bounded command/fixture comparison for
  unprepared quiet-host work. Wakes **tasks-00bac8** with evidence and a recommendation
  in the same commit as the result and brief update. No speculative design child yet.
- **tasks-157d05** — existing P2/m/high/planned calendar design; wakes tasks-44b889.
- **tasks-91c622** — existing cross-project scope design, under tasks-ffdbaa and its
  separate review brief; coordinate eligibility wording with tasks-a9b8d3. This pass
  neither duplicates nor changes it. tasks-325e79 separately investigates safe draft
  plan linking; idea review must not silently become plan approval.
- **tasks-22f407** — previously shelved decisions queue. Its existing wake condition
  remains repeated missed proposals or an inadequate proposal view; this pass does
  not reopen or re-shelve it.

Both selected ideas remain `idea` with `briefed` verdicts. Follow-up completion must
add finding notes to the named waiting ideas so the current scope rule readmits them.
