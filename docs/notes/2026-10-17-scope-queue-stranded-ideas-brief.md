# Brief: reviewed ideas that nothing will wake

Scope pass, 2026-10-17. Idea: tasks-163055 (feedback from ops). Cluster goal:
tasks-5408fe (review-pass follow-through), in lane tasks-46d207.

## Problem

A scope pass sets `reviewed` on a briefed idea, and the idea returns to
`--needs-review` only when someone writes `note --no-reviewed`. When nothing recorded
will prompt that note, the idea never comes back, and default scope passes cannot reach
it. The report counted 25 of 59 reviewed ideas in ops in that state. The person
running scope wants every reviewed idea to wait on something recorded, and wants an
idea whose wait has ended to come back on its own.

## Current behaviour and evidence

Measured 2026-10-17 between 09:14Z and 09:19Z from `tasks show` on every reviewed
idea. Each idea is classified by its `depends_on` and by the task ids in its latest
`scope:` note. The counts are a snapshot: at 09:25Z an ops pass dropped five of these
ideas (ops-3dbbe6, ops-7933db, ops-27f2a3, ops-8fdaf6, ops-1a552c), so a re-measure
will not match.

| | ops | tasks |
|---|---|---|
| Reviewed ideas | 59 | 21 |
| An open dependency edge | 31 | 4 |
| All edges closed before the last review | 1 | 0 |
| All edges closed after the last review: a missed wake | 1 | 0 |
| No edge; latest scope note names an open follow-up | 22 | 12 |
| No edge; the named follow-up woke it with a plain note | 1 | 0 |
| No edge; waits on the user or names no task | 0 | 3 |
| No edge; a pending `proposal:` (drop) | 3 | 2 |

Registry-wide, 139 reviewed ideas carry no edge and 111 carry one.

- **Prose waits.** Most "no edge" ideas wait on a follow-up named only in the
  audit note, for example ops-79f5bd on ops-cc435c and tasks-2bb0ba on tasks-1f6d08.
  They predate tasks-a309e0 (done 2026-10-13), which added `tasks dep <idea> --on
  <follow-up>` to scope §5. Each will be stranded when its follow-up closes, unless that
  follow-up's waker remembers the note.
- **Missed wakes, failing two ways.**
  - ops-1a552c waited on a recorded dependency. The pass that wrote its 2026-10-05
    scope note added `tack-a558e8` (now hq-a558e8) to its `depends` (ops `dc0a36c`,
    2026-10-04). hq-a558e8 closed on 2026-10-09 and nothing wrote a wake note. So an
    edge records the wait but still wakes nothing: tasks-a309e0's fix alone would not
    have saved this idea.
  - ops-5bb3e5 waited in prose on ops-36e093. On 2026-10-15 ops-36e093 wrote its
    finding on the idea as a plain `note` instead of `note --no-reviewed`, so the idea
    stayed reviewed. This is a live case of the plain-note problem: tasks-d4e73d's
    rollout note counted 108 legacy "Ideas it wakes" bodies that use a plain `note`,
    which leaves the idea reviewed and only warns (review-state design §3.3, §7.3).
    The user wrote the missed wake with `--no-reviewed` on 2026-10-17, readmitting it.

  Option 3 covers both: ops-1a552c through its edge, and ops-5bb3e5 once the backfill
  adds its missing edge. A waker that closes with `done` and writes no note gets no
  warning at all today.
- **Closed edges re-reviewed.** ops-8fdaf6 was re-briefed (2026-09-29) after its only
  dependency had closed. It is not a missed wake. It waits on nothing recorded, so it
  is stranded the same way.
- **User waits.** tasks-f3910b ("once Q1 is answered") and tasks-2b5461 ("no
  follow-up until [the user's scope call]") wait on a person. Neither is parked, so
  `list --parked --waiting-on user` does not show them. tasks-00bac8 names "a bounded
  comparison" but no task. tasks-b65dbc appears to be that comparison, unlinked.
- **Proposals.** Pending drop proposals wait on the user, and tasks-22f407
  (`list --pending`) will give them a read. They are out of this gap.

Code: `src/review.rs::needs_review` (an idea needs review iff `!reviewed`);
`TaskSummary::of` computes it from fields the row already has, with no dependency
status, park or note read.

## Constraints

- Review-state design (`docs/specs/2026-10-11-review-state-design.md`): §1 says the
  queue is read "without reading notes"; §3.3 makes incidental notes keep ideas
  reviewed; §6 keeps pending proposals note-based. `needs_review` is a JSON row field,
  so a change to its meaning is a contract change that needs this task's authority.
  No reader outside tasks was found: on 2026-10-17 nothing in tasks-tui, hq, obs,
  familiar, flows, ops, relay or crew references it. That makes the change cheap.
- Parks live in a host-local store, and records sync between hosts. A rule that reads
  parks can mark an idea differently on different hosts.
- Dependency status across projects needs registry reads, so `list` would pay a
  dependency lookup per reviewed idea.
- tasks-22f407 moves `pending_proposal` to a shared helper. Any rule here that reads
  proposals uses that helper.
- tasks-d4e73d is checking whether completion follow-ups need native hooks. A
  done-time wake is in its territory.
- Scope §6 forbids writes outside the fixed root, and any backfill must respect claims
  and stale copies (review-state §7.2 set the precedent for a frozen, unchanged-record
  backfill).

## Alternatives

1. **Skill rule and one backfill.** Scope requires `briefed` and `question` to leave a
   recorded wait: an edge to a follow-up, or `park --waiting-on user --reason decision`
   when only the person can answer. Otherwise the verdict is `shelved` with a wake
   condition. A one-time judged pass turns prose waits into edges, wakes missed ones
   and parks user waits. This needs no CLI change. A future waker that closes with
   plain `done` still strands its idea, edge or no edge: ops-1a552c had its edge and
   was stranded anyway.
2. **Option 1, plus warnings at the moment.** `done` and `drop` warn when a reviewed
   same-project idea depends on the closing task and now has every edge closed. The
   warning names the `note --no-reviewed` to run. `check` warns on a reviewed idea
   with no recorded wait. The queue's meaning is unchanged, but the warnings can be
   ignored, and the `check` warning repeats on every commit until it is fixed.
3. **Derived re-entry.** `needs_review` becomes "an idea that is not reviewed, or is
   reviewed but waits on nothing recorded". Waiting means an open dependency, a
   pending proposal, or a recorded wait on the user (question 1). When the last edge
   closes, the idea re-enters the queue with no waker action. A pass must leave a
   wait (or shelve or scope the idea) for the idea to leave the queue. This needs
   dependency status in row building, and it changes what the JSON field means.

Lean: 3, with option 1's skill rule and backfill as its rollout, once questions 1 and
2 are settled. It fixes the report's complaint in the queue itself. It is the only
option that would have saved ops-1a552c, whose wait was already a recorded edge.
Missed wakes stop depending on what a waker remembers to write. Option 2's `done`
warning remains a cheap addition if row-time dependency reads prove too costly.

Question 1 gates option 3. Until a synced wait-on-user marker exists, a `question`
verdict leaves no recorded wait, so every question idea would come back on every pass.

## Questions and the user's answers

The user answered these on 2026-10-17, in review of this brief. The design adopts them
or argues against them at spec review.

1. Does a park waiting on the user count as a recorded wait? **No.** A park is
   host-local, so the same idea would be queued on one host and not on another. The
   wait goes on the synced record. A declared need might fit (material declares
   `owner`: "The owner supplies or judges an image"), but needs exist to claim
   resources, not to wait, so it may not. Open for the design: which synced marker.
2. Do pending drop proposals count as a wait? **Yes**, read through tasks-22f407's
   shared `pending_proposal` helper. Otherwise every pending drop would be re-scoped on
   every pass until the user rules on it. That justifies one narrow exception to
   review-state §1's "without reading notes".
3. Where do stranded ideas sort? **In two tiers.** An idea whose last dependency
   closed after its review has new evidence, as a woken idea does, and sorts with
   never-reviewed ideas. A legacy idea that waits on nothing and has nothing new sorts
   after them. The user's proposed test: `note --reviewed` and `edit --reviewed` also
   store a `reviewed_at` timestamp on the record, which syncs with it. An idea counts
   as freshly woken when one of its dependencies has a `completed` later than its
   `reviewed_at`. `updated` will not serve, because incidental notes bump it, and the
   scope note's own timestamp would mean reading notes again. Ideas reviewed before
   the field exists cannot be told apart, so the backfill treats them as legacy
   stranded. Settled with it:
   - **A dropped dependency is new evidence** (the user, 2026-10-17). A drop ends the
     wait as done does. A follow-up is dropped because it was covered elsewhere, its
     premise went away, or it was abandoned, and each changes what the idea should
     become. If drops did not count, an idea whose only follow-up was dropped would
     be stranded with no way back. Today `transition()` (`src/commands/mod.rs`)
     stamps `completed` only on a move to `done`, so a drop carries no close time. The
     fix stamps one on any move to `done` or `dropped`, either in `completed` or in a
     separate `closed`, and leaves `done` behaving as it does today.
   - **A moved dependency is followed, not closed** (the user, 2026-10-17). A `moved`
     tombstone names its successor (`moved_to`), and `check` reports such edges as
     `moved_dep` with the command to re-point them. The rule waits on the successor.
     `Status::is_open` counts `moved` as closed (`src/model.rs`), so a rule built on
     it would wake every idea waiting across a `tasks move`, such as a retiring
     project moving its open tasks to a successor. A prefix rename writes no
     tombstones (`src/commands/rename.rs`); its ids resolve through the alias.

   - **A new `closed` field, not `completed`** (the user's input, 2026-10-17; the
     choice is the design's). `closed` is stamped on every move to `done` or
     `dropped`, so a done task carries both fields, and the wake rule reads only
     `closed`. `completed` keeps meaning done-only. Readers checked on 2026-10-17:
     - ops `bin/obs-index` computes lead time from `started` to `completed` only
       inside `status == "done"`, so it is safe either way.
     - obs copies `completed` into `task_records.completed_ms` (`join.py`).
       `views/dashboard/tasks.sql` labels it `closed_ms`, and `views/cost.sql` and
       `dashboard.py::closed_roots` select `status IN ('done','dropped') AND
       completed_ms >= since`. So obs already reads `completed` as a close time that
       includes drops, and because drops never carry it, a `since`-filtered view
       silently omits them (filed as obs-1d6eb4). Stamping `completed` on drops would shift those views
       unannounced. A separate `closed` lets obs adopt it deliberately.

     The cost is one more frontmatter field with `reviewed_at`'s rollout. Tasks closed
     before it exists have no `closed`. That is harmless, since ideas reviewed before
     `reviewed_at` exists count as legacy stranded anyway.

   Still open for the design:
   - `reviewed_at` is cleared wherever `reviewed` is.
   - A new frontmatter key has the same every-host-first rollout as `reviewed`
     (review-state §7.0).
4. Does the backfill cover every project? **Yes, run per project from its own
   checkout**, as scope's fixed root requires, not as one registry-wide pass. A scope
   note that names exactly one open follow-up can become an edge automatically. Only
   the ambiguous cases need judgement: proposals, user waits, and notes naming several
   tasks or none. Of the 139 registry-wide, that is probably a few dozen.

## Proposed decomposition

- tasks-1a3280 — Design wait-aware review state from the brief (P2, m, high, planned),
  under tasks-5408fe. It takes the answers above as inputs, chooses the synced
  wait-on-user marker, settles the `reviewed_at` tier test (a `closed` stamp on
  done and drop, moves followed), and specifies the backfill. tasks-163055
  depends on it.
- tasks-163055 waits on that design. It returns to the queue when the design's
  completion wakes it.
