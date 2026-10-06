# Task records across checkouts: readers and recovery

Scoping handoff, updated 2026-10-06. Goal: tasks-984769. Not an approved design.

## Problem

Dependency decisions and recovery instructions need to agree with the record that
contains the work. Four reports cover missing worktree dependency records
(tasks-70f0bc, tasks-562c53), stale-copy merge advice (tasks-a20a64), and a closed
record still shown as parked (tasks-6b2867). Read authority and local-state recovery
need independent decisions.

## Current behaviour and evidence

- `src/resolve.rs::resolve_task` reads local records for its prefix and the registered
  checkout for another. `src/commands/dep.rs::add_dependencies` rejects unresolved
  targets and `ensure_acyclic` walks that resolver. Show's separate claim/park fallback
  does not make dependency reads worktree-aware. Original dependency captures are absent.
- `src/stale.rs::compare` refuses newer siblings and equal-stamp/different-byte forks.
  tasks-29f71f reproduced a one-sided union retaining the source stamp: it preserves
  both note histories but still refuses against the different sibling bytes. The
  diagnostic attributes the fork to same-second writes even when Git resolution
  created it. Normal fast-forward integration of a behind-only copy permits a retry.
- Lossless divergent recovery needs no hand-edited stamp: commit the resolved union
  with the authoritative header intact, integrate that exact record into both
  equal-stamp siblings, then rerun the CLI write. The retry advances `updated` and
  preserves every original note. The isolated probe, command transcript, versions
  and binary digest are attached to tasks-29f71f; its fixture was removed.
- Claim/park state is host-local under XDG_STATE_HOME (`src/claims.rs::ClaimStore::path_with`),
  independent of task files shared through Git or synchronized across hosts. Closing
  on one host does not clear another host's park store.
- The reviewer's isolated two-state-store probe, run at the controller's request,
  parked under store A and closed under store B. After record integration, prime
  under A showed the closed task parked.
  Re-running tasks done <id> under A exited 0 and cleared its park; the probe was deleted.
  This is a demonstrated mechanism, not proof of the original incident's host sequence.
- An additional current-binary probe with shared files and two isolated stores confirms
  that plain done on a closed non-recurring record changes only updated; completed,
  model and notes stay unchanged. Local park and parked rows disappear. Its result is
  attached to tasks-b9b10e; this smaller check did not exercise Git worktree removal.
- `src/commands/mod.rs::claim_guard/save` clear parks on done/drop. Same-status closure
  reaches that release path, and cleanup-failure guidance already recommends repeating
  done. The report's blanket claim that no command clears it is therefore incorrect.
  tasks-b9b10e must verify worktree-removal and recurrence/foreign-claim safeguards.
- The record-home spec is accepted and implemented under tasks-9949f3; use it and
  current code rather than the older brief's superseded pre-implementation behavior.

## Constraints

Preserve stale-copy refusal, notes, ownership, cycle checks and JSON contracts. A
completed dependency does not grant rollout authority. Investigations isolate config
and two host state stores; no live-store editing, host pointers or hand-edited stamps.
Do not erase a legitimate recurring or live-owned obligation when reconciling history.
Related halt-audit and linked-project tasks address different questions.

## Alternatives

**Dependency read authority**

1. Publish the needed task record to registered authority before another project
   depends on it or observes completion; improve explicit publication guidance.
2. If publication is insufficient, design shared worktree-aware dependency resolution
   with reviewed source selection, cycles and ownership, distinct from runtime rollout.

**Recovery**

1. Use existing lossless Git integration and repeated done/drop where demonstrated;
   document their exact effects and limits.
2. Design a narrowly scoped recovery change only if those paths cannot handle the
   reproduced case safely. Avoid automatic cleanup that removes valid foreign work.

Recovery recommendation: use existing Git integration, rather than a timestamp edit
or a stale-copy override. For a behind-only worktree, commit the authoritative record
and merge it into the worktree before retrying. For divergent histories:

1. Commit both sides and resolve their Git merge conflict with both note histories
   intact, retaining the authoritative frontmatter rather than inventing a timestamp.
2. Integrate the resolved record into every sibling with the same or a newer stamp;
   the probe used a fast-forward back to the source branch. A sibling with the same
   stamp must have identical record bytes before retrying.
3. Retry the original CLI command in its intended checkout; the CLI advances the
   stamp itself. Coordinate with active owners before integrating their checkout.

tasks-a20a64 now covers precise guidance and regression protection for this existing
contract; a new recovery mechanism is unnecessary. Dependency publication still needs
tasks-d0d520's probe, and local park recovery still needs tasks-b9b10e's safeguards.
These are separate choices; a recovery remedy is not an alternative read authority.

## Unanswered questions

- Which authority should dependency creation/readiness use? tasks-d0d520 compares
  worktree-only creation and newer completion before/after publication.
- How should the stale-copy diagnostic explain a source-stamped Git union and its
  two-sided integration requirement? tasks-a20a64 implements the scoped guidance fix;
  tasks-29f71f established the recovery without changing the safety contract.
- What does repeated done write on a closed record after closure in another host's
  store: updated/completed/model, notes and local state? tasks-b9b10e checks exact
  changes and warnings, including removed worktrees and protected active occurrences.

## Proposed decomposition

- tasks-70f0bc and tasks-562c53: briefed; await tasks-d0d520 under tasks-984769.
- tasks-29f71f: investigation complete; attached probe verifies lossless Git recovery.
  tasks-a20a64: scoped guidance follow-up under the same goal; preserve the refusal,
  explain identical same-stamp siblings, and avoid blaming every fork on write timing.
- tasks-6b2867: briefed; awaits tasks-b9b10e, with the reviewer's host-local-store and
  repeated-done finding recorded on both tasks. The original incident remains uncaptured.
- All three follow-ups are P2/s/mid/direct investigations. tasks-b9b10e's bound now
  includes two independent host state stores and before/after record comparisons.
  Findings update this brief and wake named ideas in the same commit. Changed routing
  or recovery contracts require reviewed design and implementation plans afterward.
