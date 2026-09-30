# Work selection and waiting work

Scoping handoff, 2026-09-30. Goal: tasks-46d207. This is not an approved design.

## Problem

Choose work from a useful project set, prefer the goal currently receiving attention,
and notice work that needs a free host or a calendar revisit. Five ideas concern this
daily selection workflow; they need separate changes rather than a new scheduling system.

## Current behaviour and evidence

- **Groups (tasks-77dbc6):** `src/scope.rs` supports local or all-project scope;
  `src/registry.rs` stores projects and retired-prefix aliases, with no groups.
  Completed tasks-3029be and the multi-project design establish the existing union and
  dependency-resolution behavior. The captured motivating example is a named project
  family. `mindful --json show 5ccf9506e36842cf8e635e55248040a9` returned no match;
  the original task body remains the available source.
- **Focus (tasks-9bdd68):** `src/commands/list.rs::ready_tasks` applies readiness and
  claim/park gates, then `ready` applies the complexity cutoff. `next` puts eligible
  parked-agent resumes ahead of ready tasks. The proposed goal preference therefore
  needs an explicit precedence rule, not just another sort key.
- **Quiet visibility (tasks-479a6f):** `TaskSummary` already exposes park fields, but
  the shared pretty table, `src/output.rs::table`, renders no park suffix; it already carries
  defer, due, and claim-owner row markers to follow. `parked_table` already uses
  `claims::describe_stop` to show the reason and recipe. The quiet-queue design section 4
  and `a_quiet_park_records_its_recipe_in_the_entry_the_note_and_every_park_view` cover
  the existing contract. A shared-table display fix needs no JSON change.
- **Calendar waits (tasks-44b889):** the defer design section 3.2 deliberately excludes
  `doing`; status transitions clear dates. Park has no resume date. Existing picker
  deferral filtering landed in `80a27e2`, and due visibility in `9e9b12b`; the gap is
  extending those semantics to in-progress work, not inventing a timer.
- **Decisions queue (tasks-22f407):** `src/commands/sample.rs` reports pending proposals
  from the latest scope/curate note. The September 24 review found two across projects
  and judged the existing command sufficient. This pass ran the local
  `sample --limit 0 --older-than 0d` and received no warnings; it did not recount other
  projects. Deferred, live-claimed, and otherwise ineligible records are excluded by
  sample, so this is not an exhaustive pending-decision query.

## Constraints

Preserve original reports and sources. Scope and preference must retain dependency,
claim, user-waiting, complexity, and deferral gates. The multi-project, park, quiet-queue,
and defer specs are implemented contracts; revisions need explicit design decisions.
Registry and park state are outside task Git history, whereas `defer` is in the record.
A due date must not authorize work parked for a person's review. Existing checkout
ownership design tasks-ab8d2d is related context; it is not a prerequisite for the
display-only fix. No matching open design/research follow-up or existing handoff was
found for these selection decisions.

## Alternatives

1. Keep current commands: per-project reads, priorities, explicit quiet/parked views,
   and the sample-based proposal listing. This is sufficient for the decisions queue
   until evidence shows missed decisions.
2. Make incremental extensions: explicit project groups, opt-in goal preference,
   existing-field quiet markers, and a dated-resume contract. This is the current lean;
   ship the marker independently and review selection and calendar semantics separately.
3. Derive groups from remote organizations and introduce automatic scheduled resumption.
   This adds external configuration coupling and authorization questions unsupported by
   the captured needs; leave it outside these tasks.

## Unanswered questions

- Groups/focus design: explicit registry membership or repeated project flags? How do
  aliases, rename/unregister, overlapping groups, and missing members behave? How does
  focus compare with urgent priorities and parked-agent resumes? Where is focus stored?
- Calendar design: reuse `defer` on parked doing tasks, add a park-store date, or use the
  existing todo-plus-defer sequence? Specify early resume, re-park, recurrence, due
  visibility, and host-local versus synced persistence. The design author proposes;
  the user reviews the resulting contract.

## Proposed decomposition

- tasks-479a6f: scoped, P2, small/low/direct. Add the quiet marker to shared pretty
  tables with focused verification. The original optional discovery hint stays deferred.
- tasks-ece1e2: P2, medium/high/planned. Design groups and goal focus; record decisions
  on waiting ideas tasks-77dbc6 and tasks-9bdd68 and update this brief in the same commit.
- tasks-157d05: P2, medium/high/planned. Design dated resumption; record decisions on
  waiting idea tasks-44b889 and update this brief in the same commit.
- tasks-22f407: shelved independently. Wake when repeated passes expose missed
  proposals or the existing proposal view becomes inadequate. No new queue task.

Both design follow-ups require design and implementation-plan review before code.
