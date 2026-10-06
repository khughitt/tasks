# Task records across checkouts: readers and recovery

Scoping handoff, updated 2026-10-06. Goal: tasks-984769. Not an approved design.

## Problem

Dependency decisions and recovery instructions need to agree with the record that
contains the work. Four reports cover missing worktree dependency records
(tasks-70f0bc, tasks-562c53), stale-copy merge advice (tasks-a20a64), and a closed
record still shown as parked (tasks-6b2867). Read authority and local-state recovery
need independent decisions.

## Current behaviour and evidence

Probe scripts, transcripts, versions and binary digests are attached to the task named
in each bullet; original incident captures remain absent.

- **Dependency reads (tasks-d0d520).** `src/resolve.rs::Resolver::resolve_task` reads
  the current checkout's records for its own prefix (so inside a worktree, same-project
  dependencies read the worktree copy) and the registered checkout for another prefix.
  `src/commands/dep.rs::add_dependencies` rejects unresolved targets and `ensure_acyclic`
  walks the same resolver. Show's claim/park fallback (`show.rs::recorded_elsewhere`)
  does not make dependency reads worktree-aware.

  | Upstream state | Direct foreign `show` | Dependency behavior | Downstream readiness |
  | --- | --- | --- | --- |
  | Claimed, record only in worktree | `doing`, with claim-based fallback warning | New edge refuses `unresolvable_id`; writes nothing | Ready because failed link created no edge |
  | Open record published to registered checkout | `doing`, registered copy | Edge creation succeeds even with foreign upstream claim; dependency shows `doing` | Blocked, locally and with `--all-projects` |
  | Completion only in worktree | Still `doing`, registered copy | Dependency still shows `doing` | Still blocked in both scopes |
  | Completed record published | `done`, registered copy | Dependency shows `done` | Ready in both scopes |

  Publication was `git restore --source=work -- tasks/<id>.md` plus a record-only commit
  in the registered checkout; a runtime sentinel stayed `live`. Published cycles,
  foreign-owner closure, missing authority and malformed config stayed guarded. The
  probe removed its worktree without merging the branch. A plain-git check since then
  shows that merge hits an add/add conflict on the record whenever the published copy
  differs from the branch's, and merges cleanly when they are identical. `restore
  --source` also overwrites the registered copy, so it is safe only when that copy is
  unchanged since the branch point.
- **Stale-copy recovery (tasks-29f71f).** `src/stale.rs::compare` refuses newer siblings
  and equal-stamp/different-byte forks. A one-sided union keeping the source stamp
  preserves both histories but still refuses against the different sibling bytes, and
  `stale.rs::refusal` blames the fork on same-second writes even when Git resolution
  created it. Committing the resolved union with the authoritative header and
  integrating it into both equal-stamp siblings permits a CLI retry that advances
  `updated` and keeps every note. A behind-only copy needs only an ordinary merge.
- **Closed parks (tasks-6b2867).** Claim/park state is host-local under
  `XDG_STATE_HOME` (`src/claims.rs::ClaimStore::path_with`), independent of task files
  shared through Git or file sync. Two isolated-store probes (attached to tasks-b9b10e):
  park under store A, close under store B, and A's `prime` still lists the closed task.
  Plain `done` under A on the closed non-recurring record exits 0, changes only
  `updated` (completed, model and notes unchanged) and clears A's park. Neither probe
  removed a Git worktree or exercised recurrence or a foreign live claim. This shows a
  mechanism, not the original incident's host sequence.
- Park clearing: `claim_guard` records a release intent without persisting;
  `save` removes the park when the destination is done, dropped or shelved
  (`src/commands/mod.rs:532`, `:1554`). The report's claim that no command clears it is
  therefore incorrect; cleanup-failure guidance already recommends repeating done.
- `prime` and `list --parked` disagree on a closed park: `list_parked` hides a park whose
  record is closed unless a status filter is given (`src/commands/list.rs::list_parked`,
  `is_open`), while prime's `parked::rows` lists it. The shelved case is kept visible
  deliberately "for cleanup".
- The record-home spec is accepted and implemented under tasks-9949f3; use it and
  current code rather than the older brief's pre-implementation behavior.

## Constraints

Preserve stale-copy refusal, notes, ownership, cycle checks and JSON contracts. A
completed dependency does not grant rollout authority. Investigations isolate config
and host state; park probes compare two stores. No live-store editing, host pointers
or hand-edited stamps. Do not erase a legitimate recurring or live-owned obligation
when reconciling history. Related halt-audit and linked-project tasks address different
questions.

## Alternatives

**Dependency read authority**

1. Publish the needed task record to registered authority before another project
   depends on it or observes completion; improve explicit publication guidance.
2. If publication is insufficient, design shared worktree-aware dependency resolution
   with reviewed source selection, cycles and ownership, distinct from runtime rollout.

Recommendation, option 1: keep registered authority. Publish the upstream record before
creating a foreign dependency, and publish its completion before expecting downstream
readiness. Direct `show` fallback is a diagnostic, not publication. Publish linked
documents/attachments too when the record uses them. Overwrite the registered copy only
when it is unchanged since the branch point; otherwise merge it, using the recovery
procedure below for divergent copies. The guidance must also cover the later branch
merge, whose add/add conflict on a differing record is so far unexercised with the CLI.
Publication records progress; it does not authorize runtime activation. Tasks owns
CLI/skill guidance; upstream owners publish their records and keep their own rollout
gates. tasks-70f0bc covers this guidance. No shared-resolver change is justified unless
its branch-merge check shows publication cannot be made routine.

**Recovery**

1. Use existing lossless Git integration and repeated done/drop where demonstrated;
   document their exact effects and limits.
2. Design a narrowly scoped recovery change only if those paths cannot handle the
   reproduced case safely. Avoid automatic cleanup that removes valid foreign work.

Recommendation, option 1: no timestamp edit or stale-copy override. For divergent
histories:

1. Commit both sides and resolve their Git conflict with both note histories intact,
   keeping the authoritative frontmatter rather than inventing a timestamp.
2. Integrate the resolved record into every sibling with the same or a newer stamp; a
   same-stamp sibling must have identical bytes before retrying.
3. Retry the original CLI command in its intended checkout; the CLI advances the stamp.
   Coordinate with active owners before integrating their checkout.

tasks-a20a64 covers guidance and regression protection for this existing contract.
Closed-park recovery waits on tasks-b9b10e. A recovery remedy is not an alternative
read authority.

## Unanswered questions

- Does repeated done stay safe once the park's worktree has been removed, on a recurring
  record, and against a foreign live claim? tasks-b9b10e answers with its two-store
  probe.
- Should `prime` hide a park on a closed record as `list --parked` does, or should
  `list --parked` show it as a cleanup row, as it already does for shelved parks?
  tasks-b9b10e records both outputs and recommends one; changing either is a JSON/output
  decision for the user.
- Is the branch merge after publication clean when the final bytes were published, and
  how is the add/add conflict resolved when they were not? tasks-70f0bc extends the
  tasks-d0d520 probe before writing guidance.

## Proposed decomposition

- tasks-d0d520 (done): record-only publication suffices for reads in its fixture.
  tasks-70f0bc: guidance follow-up, including the branch-merge check. tasks-562c53:
  proposed drop, covered by tasks-70f0bc.
- tasks-29f71f (done): lossless Git recovery verified. tasks-a20a64: guidance follow-up;
  preserve the refusal, explain identical same-stamp siblings, stop blaming every fork
  on write timing.
- tasks-b9b10e: closed-park safeguards and the `prime`/`list --parked` disagreement.
  tasks-6b2867 waits on it.
- Findings update this brief and wake named ideas in the same commit. Changed routing,
  recovery or output contracts require a reviewed design and implementation plan.
