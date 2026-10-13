# Task records across checkouts: readers and recovery

Scoping handoff, updated 2026-10-09. Goal: tasks-984769. Not an approved design.

## Problem

Dependency decisions and recovery instructions need to agree with the record that
contains the work. Four reports cover missing worktree dependency records
(tasks-70f0bc, tasks-562c53), stale-copy merge advice (tasks-a20a64), and a closed
record still shown as parked (tasks-6b2867, repeated by tasks-2c7979). Read authority and local-state recovery
need independent decisions.

The latest pass adds tasks-955c2a: recording a finding while another session holds
the newer worktree copy. tasks-2337b3 repeats the stale-merge report; its remedy is
already covered by tasks-a20a64. The related resume warning, tasks-ef69c0, duplicates
tasks-9de6aa in the parent lane and needs no new implementation task.

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
  `updated` (completed, model and notes unchanged) and clears A's park.
- **Closed-park safeguards (tasks-b9b10e, 2026-10-09).** A two-host probe (script and
  full log attached to tasks-b9b10e; tasks 0.2.0 at 42ea5ab, git 2.56.0) built twelve
  fresh fixtures: host A parks from a Git worktree, the parked record is integrated,
  host B closes it, and the closed record reaches A either through the shared checkout
  or through `git pull` from B's own clone. Results:
  - Non-recurring, worktree removed through Git: bare `done` under A exits 0, the record
    diff is the one `updated` line, and A's park is gone. Identical for both arrivals and
    for a closer with the parking session's identity or a distinct one.
  - An explicit message (`done <id> "<text>"`) also appends that note; a different
    `TASKS_MODEL` leaves `model` and `completed` untouched. tasks-2c7979's restamping
    report is not reproduced; its added note matches an explicit-message retry.
  - Dropped record: `done` refuses `invalid_transition` and writes nothing; `drop <id>
    "<why>"` clears the park, appending its note.
  - Recurring record: `done` from the main checkout refuses (`validation`: "has work in
    <wt>; finish that occurrence in its owning checkout") with the worktree present and
    after its removal, writing nothing; `park` refuses `invalid_transition`. In the
    worktree, after merging main, the retry warns "already completed; releasing its
    claim without recording another occurrence" and changes only `updated`. After
    removal no command clears it: `start` then `done` records a spurious occurrence
    (new `completed`, a "completed; next due" note, `model` reset). tasks-6a0050 fixes this.
  - Foreign claim under A: a live one makes the retry refuse `claimed`, record and store
    unchanged; a stale one is taken over with a warning, the record diff again only
    `updated`. prime's stale-claim hint suggests `start --force`, which a closed
    non-recurring record refuses (source inspection, not run).
- **Repeated closed-park report (2026-10-08 scope).** tasks-2c7979 reports the same
  view mismatch and says repeating done adds a note and restamps model. Current
  `src/commands/mod.rs::transition` stamps model/completed only on a fresh completion;
  `src/commands/status.rs::close` can append an explicitly supplied message on a
  non-recurring retry. The attached bare-retry probe preserved both metadata and notes.
  The report does not capture its exact invocation or binary, so tasks-b9b10e will
  compare bare retries, explicit messages and a changed `TASKS_MODEL`; no second
  investigation or new cleanup command is justified by this report.
- Park clearing: `claim_guard` records a release intent without persisting;
  `save` removes the park when the destination is done, dropped or shelved
  (`src/commands/mod.rs:532`, `:1554`). The report's claim that no command clears it is
  therefore incorrect for non-recurring records (verified above); cleanup-failure
  guidance already recommends repeating done. It holds for a recurring record whose
  worktree is gone.
- `prime` and `list --parked` disagree on a closed park: `list_parked` hides a park whose
  record is closed unless a status filter is given (`src/commands/list.rs::list_parked`,
  `is_open`), while prime's `parked::rows` lists it. The shelved case is kept visible
  deliberately "for cleanup". The probe confirms it for both arrivals: prime lists the
  done or dropped task; `list --parked` omits it; `list --parked --status done` shows
  a done one. tasks-912279's "list --parked keeps showing" was not reproduced without
  a status filter.
- The record-home spec is accepted and implemented under tasks-9949f3; use it and
  current code rather than the older brief's pre-implementation behavior.
- **Foreign notes (2026-10-08 source inspection, not a new runtime probe).**
  `src/commands/status.rs::note` loads through the stale-copy guard, appends in memory,
  then saves the entire serialized record through `src/repo.rs::write_task`. Append-only
  notes do not make writes to stale whole records safe. CLI writers share the per-prefix
  host mutation lock; `concurrent_notes_and_a_status_change_lose_nothing` tests that
  serialization. `a_note_from_a_foreign_session_leaves_the_claim_alone` covers a foreign
  note in the same checkout without refreshing or moving the owner's claim. Neither
  establishes the complete two-checkout workflow requested by tasks-955c2a.
  `occupants` deliberately chooses wait/merge guidance for an occupied checkout.
- **Repeated merge report.** tasks-2337b3 describes the same one-sided source-stamped
  union demonstrated by tasks-29f71f (4095f53). Its attached transcript says recovery
  passed with both histories intact after integrating identical resolved siblings;
  hand-editing `updated` is unnecessary. The diagnostic remains unchanged in current
  `src/stale.rs`, so tasks-a20a64 still owns the fix.

## Constraints

Preserve stale-copy refusal, notes, ownership, cycle checks and JSON contracts. A
completed dependency does not grant rollout authority. Investigations isolate config
and host state; park probes compare two stores. No live-store editing, host pointers
or hand-edited stamps. Do not erase a legitimate recurring or live-owned obligation
when reconciling history. Related halt-audit and linked-project tasks address different
questions.

The host mutation lock does not establish safety against another host, manual file
edits or Git operations. Explicit targeting and silent routing are different decisions:
the accepted record-home contract rejects silent routing. Preserve the queued-note
proposal verbatim in tasks-955c2a without treating it as an approved design. Note
redaction/correction (tasks-1f6d08) and halt-audit ordering (tasks-10c968) remain separate.

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
A recovery remedy is not an alternative read authority.

**Closed parks**

Recovery: keep the repeated close. It is safe for non-recurring records (only `updated`
changes, a live foreign claim refuses) and needs no new command. The one gap is a
recurring record whose recorded worktree is gone: tasks-6a0050 lets the existing guard
release a park or claim whose worktree no longer exists on this host, keeping the
refusal while it does. Claims and parks are host-local, so absence on this host is
decisive.

View: change prime, not `list --parked`. `list --parked` is the picker's feed and rightly
hides closed work; prime shows the closed task as work waiting, which three reports read
as uncleared live state. tasks-081099 moves such an entry from prime's parked rows to a
warning naming the clearing command, as prime already does for stale claims. Making
`list --parked` show them instead would put closed work in a picker's feed.

**Notes sent to an active task**

1. Coordinate an explicit `tasks -C <current-checkout> note` through the existing CLI
   lock, if the two-session probe establishes the supported workflow and limits.
2. Keep wait/merge or owner-mediated delivery when explicit targeting is unsuitable.
3. Design queued delivery only if those routes leave a demonstrated gap; it needs
   durable delivery, retry semantics, visibility, and a policy for absent owners.

Current lean: test option 1 before introducing a second note store. Reject a blanket
stale-copy exemption: appending a note still rewrites fields and earlier history.
tasks-a984f1 supplies the evidence; this pass does not authorize automatic routing.

## Unanswered questions

- Is the branch merge after publication clean when the final bytes were published, and
  how is the add/add conflict resolved when they were not? tasks-70f0bc extends the
  tasks-d0d520 probe before writing guidance.
- Does explicit targeting preserve all notes, fields and the owner's claim during
  concurrent CLI writes, and what happens on retry? tasks-a984f1 runs the isolated
  two-session probe and states where the host lock stops providing protection.

## Proposed decomposition

- tasks-d0d520 (done): record-only publication suffices for reads in its fixture.
  tasks-70f0bc: guidance follow-up, including the branch-merge check. tasks-562c53:
  proposed drop, covered by tasks-70f0bc.
- tasks-29f71f (done): lossless Git recovery verified. tasks-a20a64: guidance follow-up;
  preserve the refusal, explain identical same-stamp siblings, stop blaming every fork
  on write timing.
- tasks-b9b10e (done): closed-park safeguards verified; see the Closed parks
  alternative. tasks-6a0050 (P2/s/mid/direct): recurring release after worktree removal.
  tasks-081099 (P2/s/low/direct): prime warns instead of listing closed parks.
  tasks-6b2867 waits on both; tasks-2c7979 and tasks-912279 are proposed drops as its
  duplicates, original reports preserved.
- tasks-a984f1 (P2/s/mid/direct): establish safe foreign-note delivery; tasks-955c2a
  remains an idea awaiting its finding. No queue implementation or design task yet.
- tasks-2337b3: dropped with user approval on 2026-10-08 as covered by tasks-a20a64
  and the completed recovery probe. tasks-ef69c0: dropped with the same approval as
  covered by tasks-9de6aa. Both retain their original reports.
- Findings update this brief and wake named ideas before the result's branch merges. Changed routing,
  recovery or output contracts require a reviewed design and implementation plan.
