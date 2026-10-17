---
id: tasks-d2cdfe
title: "Wait-aware review: deferred review minors"
status: todo
priority: 3
size: s
complexity: mid
process: direct
created: 2026-10-17T12:37:08Z
updated: 2026-10-17T17:38:28Z
depends: []
parent: tasks-5408fe
tags: []
source: tasks-aeeef1
agent: claude-code/claude-opus-5-5
---

Minor findings the wait-aware review branch deferred (per-task reviews and the final whole-branch review, which triaged every one keep-deferred).

Needs a spec decision first:
- review::apply Review::Set keeps a stale awaiting_user on a re-review (note --reviewed plus a new edge leaves the idea hidden after the edge closes). Spec §3.1 lists what clears the marker and omits --reviewed; decide whether Review::Set clears it unless --awaiting-user is also given.

Before the backfill Rollout:
- tools/wait-backfill freeze: the "changed during the freeze" detail says "rerun after the next pass", but a rerun loads the snapshot and never re-freezes; say "left out of this snapshot; scope it in the next pass".

Wording and comments:
- The plain-note warning and scope skill §1 ("incidental notes … neither readmit it nor change its wait") are wrong when the note answers a pending proposal, which ends that wait.
- src/commands/lanes.rs doc comment (~564) still promises a dependency named only by closed/shelved/paused work cannot fail the view; stale since Deps::of_scan propagates resolve errors.
- Unwrapped prose: skills/scope/SKILL.md (two spots), skills/curate/SKILL.md (Pending proposals), the 2026-10-17 amendment line in docs/specs/2026-10-11-review-state-design.md.
- Misleading comment in tests/review.rs stranded_ideas_and_the_review_time_test.

Code and cost:
- prime: elsewhere_deps uses ? so one foreign checkout view error fails all of prime, unlike claimed_elsewhere and parked recorded rows; warn and drop the row (never hand it a view lacking its edges, or Deps::state panics).
- parked sourced_rows builds a second view of all in prime/next; pass deps in. list resolves edges twice. Cost only.
- wait-backfill: non-Usage errors traceback with exit 1 (inherited from review-backfill); no fsync before snapshot rename; dep targets not id-validated; changed-entry review_reason re-reads list per entry.

Tests:
- RED never observed for several Task 2 tests and the corrupt-snapshot backfill test (could assert sci.json still reads {not json).
- No test for the spec §8 case 6 woken-after-answer variant, prime non-parked woken/stranded rows, a multi-target dep decision, or an interrupted edit --awaiting-user; show-test assertions are loose (contains "yes (", awaiting row not line-anchored).
- check.rs wording for awaiting_user-without-reviewed differs from review::not_reviewed (plan-mandated texts).

## Notes

- 2026-10-17T12:51:56Z (main): also: wait-backfill auto classification and judge proposals should drop named tasks that are the idea's own ancestor (freeze on 2026-10-17 found 27 such names among judge entries and 1 auto entry, mind6-573a5b, overridden to leave); see tasks-9c18e7
