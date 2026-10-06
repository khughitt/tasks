# Task records across checkouts: readers and recovery

Scoping handoff, 2026-10-06. Goal: tasks-984769. This is not an approved design.

## Problem

Dependency decisions and recovery instructions need to agree with the record that
contains the work. Four reports describe cross-project readers missing worktree work
(tasks-70f0bc, tasks-562c53), stale-copy advice that does not lead to a writable merge
(tasks-a20a64), and a closed task left in prime's parked view (tasks-6b2867).
These questions share checkout authority, but need independent verdicts.

## Current behaviour and evidence

- `src/resolve.rs::resolve_task` reads local records for its own prefix and the
  registered checkout for another. `src/commands/dep.rs::add_dependencies` rejects
  unresolved targets and `ensure_acyclic` walks the same resolver. `show` separately
  finds a missing record through its claim/park checkout, but uses the local copy
  when present. That fallback does not make dependency reads worktree-aware.
- The two dependency reports contain no captured commands or fixture. Current paths
  support their visibility account, but the intended publication boundary is unresolved.
- `src/stale.rs::compare` refuses newer sibling stamps and equal-stamp/different-byte
  forks. `refusal` recommends merging the named copy; a union that retains the same
  stamp but changes bytes can still refuse. tasks-a20a64 has no exact merge capture.
  No supported fresh-stamp operation was established here.
- `src/commands/mod.rs::claim_guard` schedules `clear_park` for done/drop and `save`
  calls `ClaimStore::remove_park`. Cleanup failure emits recovery guidance after the
  record write. `src/commands/parked.rs::rows` can show local store entries against
  closed registered records; ordinary list filters differ. This contradicts a blanket
  claim that done never clears parks. The incident's binary version, cleanup result
  and whether closure arrived through a CLI write or git integration are unknown.
- The record-home spec is accepted and implemented under tasks-9949f3. Its older
  cross-checkout brief has superseded pre-implementation behavior and is historical
  context; the current code and accepted spec are the starting point.

## Constraints

Preserve stale-copy refusal, notes, claim ownership, cycle validation and JSON
contracts. A dependency becoming done does not grant activation or rollout authority.
Investigations use disposable git checkouts and isolated config/state; no live store
editing, host pointer changes, or hand-edited task timestamps. Do not reopen the closed
record-home goal or file an implementation before the new evidence establishes a gap.
No overlapping open research was found; the halt-audit design tasks-10c968 and linked
project layout task tasks-9b0a2e address different questions.

## Alternatives

1. **Explicit record publication.** Integrate the needed task record before other
   projects depend on it or observe its completion. Keep registered authority and
   improve diagnostic/workflow guidance if this satisfies the intended boundary.
2. **Shared worktree-aware resolution.** Propose a reviewed rule for dependency reads,
   source selection and cycles if publication is insufficient; preserve explicit
   ownership and distinguish code rollout from record visibility.
3. **Repair evidenced recovery gaps.** Establish a lossless merge remedy and whether
   a closed park needs retry, lifecycle repair or explicit reconciliation. Avoid
   automatic cleanup that could erase an active occurrence in another checkout.

Current lean: test the existing publication and cleanup paths first, then scope the
smallest demonstrated gap. The read and recovery investigations can settle separately.

## Unanswered questions

- Should dependency creation/readiness use registered publication or an active
  worktree's record? tasks-d0d520 compares worktree-only creation and newer completion,
  before and after record integration, and recommends a boundary.
- Which exact merge makes a divergent task writable while preserving both histories?
  tasks-29f71f compares normal git integration and a union retaining the source stamp.
- Does current done leave a park, or did failed cleanup, older code or a merged closed
  record produce it? tasks-b9b10e tests cross-checkout closure, removed worktrees and
  existing recovery commands; the original incident capture remains unavailable.

## Proposed decomposition

- tasks-70f0bc and tasks-562c53: briefed under tasks-984769; await tasks-d0d520.
- tasks-a20a64: briefed under the same goal; awaits tasks-29f71f.
- tasks-6b2867: briefed under the same goal; awaits tasks-b9b10e.
- All three follow-ups are P2/s/mid/direct investigations with explicit fixture bounds,
  no product-code changes, and finding notes to wake their named ideas in the same
  commit. Update this brief with observed results before rerunning scope. Any changed
  routing or recovery contract requires reviewed design and plan work afterward.
