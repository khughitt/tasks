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

Measured 2026-10-17 from `tasks show` on every reviewed idea, classifying each idea by
its `depends_on` and by the task ids in its latest `scope:` note:

| | ops | tasks |
|---|---|---|
| Reviewed ideas | 59 | 21 |
| An open dependency edge | 31 | 4 |
| All edges closed | 2 | 0 |
| No edge; latest scope note names an open follow-up | 22 | 12 |
| No edge; the named follow-up has closed | 1 | 1 |
| No edge; nothing named | 0 | 2 |
| No edge; a pending `proposal:` (drop) | 3 | 2 |

Registry-wide, 139 reviewed ideas carry no edge and 111 carry one.

- **Prose waits.** Most "no edge" ideas wait on a follow-up named only in the
  audit note, for example ops-79f5bd on ops-cc435c and tasks-2bb0ba on tasks-1f6d08.
  They predate tasks-a309e0 (done 2026-10-13), which added `tasks dep <idea> --on
  <follow-up>` to scope §5. Each will be stranded when its follow-up closes, unless that
  follow-up's waker remembers the note.
- **A missed wake.** ops-5bb3e5 waited in prose on ops-36e093, which closed on
  2026-10-15 without a wake note. tasks-d4e73d's rollout note counted 108 legacy
  "Ideas it wakes" bodies that use a plain `note`. That note leaves the idea reviewed
  and only warns (review-state design §3.3, §7.3). A waker that closes with `done`
  and writes no note gets no warning at all.
- **Closed edges re-reviewed.** The report's two "follow-ups finished without the wake
  note" (ops-8fdaf6, ops-1a552c) were re-briefed after their dependencies closed
  (scope notes 2026-09-29 and 2026-10-05). They are not missed wakes. They wait on
  nothing recorded, so they are stranded the same way.
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
   plain `done` still strands its idea.
2. **Option 1, plus warnings at the moment.** `done` and `drop` warn when a reviewed
   same-project idea depends on the closing task and now has every edge closed. The
   warning names the `note --no-reviewed` to run. `check` warns on a reviewed idea
   with no recorded wait. The queue's meaning is unchanged, but the warnings can be
   ignored, and the `check` warning repeats on every commit until it is fixed.
3. **Derived re-entry.** `needs_review` becomes "an idea that is not reviewed, or is
   reviewed but waits on nothing recorded". Waiting means an open dependency, a
   pending proposal, or a park waiting on the user. When the last edge closes, the
   idea re-enters the queue with no waker action. A pass must leave a wait (or shelve
   or scope the idea) for the idea to leave the queue. This needs dependency status
   and park reads in row building, and it changes what the JSON field means.

Lean: 3, with option 1's skill rule and backfill as its rollout. It fixes the report's
complaint in the queue itself, and missed wakes stop depending on wakers'
wording. Option 2's `done` warning remains a cheap addition if row-time dependency
reads prove too costly.

## Unanswered questions

1. Does a park waiting on the user count as a recorded wait, given that parks are
   host-local? The alternative is a synced marker on the record. — design, then the
   user at spec review.
2. Does the derived rule read pending proposals from notes? That would reverse
   review-state §1's "without reading notes" for this case. — design, with
   tasks-22f407's shared helper.
3. Should a stranded idea rank after never-reviewed ideas in scope's default
   selection? — the user.
4. Should the legacy-prose backfill run in every registered project or only on
   request? The 139 edge-less reviewed ideas registry-wide include proposals and true
   user waits. — the user.

## Proposed decomposition

- tasks-1a3280 — Design wait-aware review state from the brief (P2, m, high, planned),
  under tasks-5408fe. It settles the four questions and the backfill. tasks-163055
  depends on it.
- tasks-163055 waits on that design. It returns to the queue when the design's
  completion wakes it.
