# Successor task migration

Scoping handoff, 2026-10-23. Idea: tasks-5ab7e8. Lane: tasks-a7460c.
This brief frames decisions; it is not an approved design.

## Problem

When a project gives way to a successor, review its open tasks once: carry the work
that still applies, drop obsolete work with a shared reason, and close work already
done elsewhere. The requested application is one transaction that preserves carried
tasks' notes, parents and dependency edges, leaves source tombstones, and permits the
old project to retire. Repeated manual moves do not cover that whole outcome.

## Current behaviour and evidence

- At code revision `60cad3f`, `src/moving/preflight.rs::prepare` refuses a source
  with a parent, and `validate_childless` refuses a goal with children. The implemented
  `docs/specs/2026-10-08-task-move-design.md` deliberately excludes subtree moves.
- Single-task moves preserve notes and attachments, allocate a new destination id,
  and leave a read-only tombstone. Incoming dependency evaluation follows its pointer;
  dependent owners receive repair guidance rather than automatic foreign edits.
  See `src/commands/move_.rs`, `src/moving/` and `src/tombstone.rs`.
- The existing protocol journals a move across two repositories and host-local state
  with forward recovery. It does not provide atomic rollback across those stores.
- `src/commands/retire.rs` rejects live claims and open records across the project's
  checkouts. `--successor` records succession after settlement; it does not migrate work.
- The open-task and brief search found no existing bulk-successor design or research
  follow-up. Earlier individual-move work is complete; tasks-183bfa owns diagnostics.
  This pass inspected code and records, without running a new migration experiment.

## Constraints

Retain the source material on tasks-5ab7e8. Reuse the existing migration lane and
single-task machinery where its guarantees apply. Preserve notes, provenance,
attachments, ownership, stale-copy refusal, destination validation and cycle checks.
Account for destination needs, document references and detached records roots.
Do not silently detach goals or make dependents ready by dropping unfinished work.
CLI additions require ops vocabulary rows before parser changes. No live migration
or retirement is authorized by this brief.

## Alternatives

1. A triage view followed by existing per-task commands: least new machinery, but it
   leaves manual hierarchy repair and does not meet the requested single transaction.
2. A reviewed batch decision manifest with whole-graph preflight, destination id
   mapping and resumable forward application: reuse existing recovery patterns while
   making partial progress visible. Its exact guarantees still need design.
3. An all-or-nothing batch with rollback across records, attachments and local state:
   closest to a strict reading of transaction, with a larger recovery contract.

Lean toward option 2 for design, rather than extending individual move with implicit
detach/re-parent behavior. Treat any relaxation of the requested transaction guarantee
as a decision for the user's design review, not an assumption this pass settles.

## Unanswered questions

- Does transaction mean all-or-nothing rollback, or one reviewed operation with
  explicit forward recovery? The design author proposes precise failure cases for
  the user's review.
- What happens when a parent is carried but a child is dropped, closed or retained,
  or when destination goals already exist? The design must define the graph mapping
  and demonstrate dependency readiness through interruption and retry.
- What policy did the historical incident require? Source `forge-0e720e, 2026-10-22`
  and the related ops project-generations idea were outside this fixed-checkout pass.
  Their actual task graph, outcomes and additional requirements remain unverified;
  the design author retrieves that context before treating it as evidence.

## Proposed decomposition

- tasks-5ab7e8 remains **briefed**, P3, under tasks-a7460c, waiting on tasks-f80a97.
- tasks-f80a97, **Design successor task migration from the brief**, is P3/m/high/planned
  under the same lane. Produce a user-reviewed design and then a user-reviewed plan;
  cover triage, graph preservation, transaction semantics, safe retries and retirement
  ordering. Do not implement or migrate records in that task. No separate research
  task duplicates the design decisions already identified here.
- Before the design result's branch merges, update this brief and run
  `tasks note tasks-5ab7e8 --no-reviewed "<design finding and review outcome>"`.
  The dependency edge also readmits the idea when the follow-up closes.
