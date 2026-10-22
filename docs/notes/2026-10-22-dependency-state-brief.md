# Dependency satisfaction and intentional waits

## Problem

Consumers need to know whether dependencies are satisfied without treating absence
from `ready` as a dependency verdict (tasks-1bbe49). Maintainers also need guidance
that preserves an intentionally shelved future stage (tasks-1675ac). Both concern
explaining a dependency's effect without changing the recorded work.

## Current behaviour and evidence

Checked in the main checkout at `d0564d1` on 2026-10-22; no runtime reproduction was
performed. Neither report links its originating study or goal, so those concrete
examples remain unverified; the corresponding code paths are established.

- `src/commands/show.rs::describe` and `src/output.rs::DepInfo` expose the stored
  dependency's id, title, status and `resolved` (found). They expose no satisfaction
  verdict and do not follow a move for that row. Commit `0c4ff24` introduced this
  dependency view; `show_resolves_local_and_foreign_dependencies` tests it.
- `src/commands/list.rs::resolve_dependency` prefers the captured scan, resolves
  aliases, and follows `moved_to` via `src/tombstone.rs::follow`. `ready_tasks` then
  evaluates `!status.is_open()`: done and dropped satisfy; shelved stays open;
  unreachable records and move cycles cannot satisfy. A recurring record still in
  done satisfies even when its next occurrence is due. After `start`, it is open
  again. `src/query.rs::readiness` and later filters also exclude goals, deferred,
  claimed, user-held, paused and otherwise inadmissible work.
- `src/commands/check.rs` warns `shelved_dep` for an open, unshelved dependent of a
  shelved task. Its advice is only "unshelve it or drop the dependency"; the test
  `check_warns_when_open_work_depends_on_shelved_work` asserts that exact text.
  Commit `4a7bc5b` introduced this warning. It does not inspect the shelf's reason.

## Constraints

Keep dependency satisfaction separate from permission to start. Preserve the JSON
meaning of `resolved`, snapshot/registered-checkout authority, move forwarding and
recurrence semantics. The move design (`docs/specs/2026-10-08-task-move-design.md`
§3.4) and periodic design (`docs/specs/2026-09-09-periodic-design.md`) remain inputs.
New CLI commands or flags must land in ops's vocabulary before adoption here.

Related open work was checked: tasks-984769 concerns record authority, tasks-e8d471
reverse dependencies, and tasks-96c00c summary size, listing speed and identity.
None answers the dependency-verdict contract. Existing tasks-52bd39 adds warnings
at dependency/shelf writes; its eventual wording must incorporate the intentional
wait disposition. It remains outside this pass. No matching brief, attached draft
spec or open research was found. No shared needs are declared.

## Alternatives

1. Add a dependency verdict to an existing read, with explicit unavailable cases.
   Current lean for the consumer need: reuse tracker resolution and preserve its
   contract; the design must choose aggregate versus per-edge output.
2. Add a dedicated dependency query. Consider only if the consumer's batching or
   payload requirements justify another command; do not assume they do.
3. Keep caller-side inference and current warning wording. Reject this as the
   target: callers would duplicate move/recurrence rules, and the warning omits
   the valid choice to retain an intentional wait.

For shelf guidance, retain the warning and explain that a recorded intentional
wait can keep the edge and shelf. Unshelve or remove the edge only when that wait
is no longer intended. Prefer this bounded text correction over a new suppression
field, inferred intent from notes, or changed readiness semantics.

## Unanswered questions

- The dependency-query design author, with the consumer and user review, settles
  the exact JSON shape, aggregate/per-edge choice, empty dependencies, missing and
  moved targets, errors, and single-task versus batch needs. The existing rule is
  established; another research-only task is unnecessary.
- The shelf-guidance implementer checks wording against tasks-52bd39 before either
  change lands. Warning detection, severity and blocking remain unchanged. The
  scope does not promise automatic acknowledgement or warning suppression.

## Proposed decomposition

Goal tasks-606dd3 (P2/m/high/planned) is an ordinary child of Work discovery and
resumption (tasks-46d207), with both selected members beneath it.

- tasks-1bbe49 — **briefed**, with a dependency on tasks-e90b4f,
  **Design dependency satisfaction reporting from the brief**
  (P2/s/high/planned). The design updates this brief and wakes the idea with
  `tasks note tasks-1bbe49 --no-reviewed "<finding>"` before its branch merges.
- tasks-1675ac — **scoped**, P2/s/low/direct: revise the warning, regression
  assertion and user guidance while preserving detection and blocking. Coordinate
  wording with tasks-52bd39; this pass does not alter that task.

Neither selected idea had a parent or source to replace. No design or implementation
plan is approved by this handoff, and no implementation was started.
