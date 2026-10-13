# Review-pass follow-through

## Problem

A curate or scope pass should leave the corpus in a state the next reader can act on
without reconstructing the pass. Six reports from ops and hq passes show where it does
not: an idea waiting on research looks unblocked, edits across projects have no commit
rule, a misfiled report can only be proposed for drop, proposals awaiting the human
have no read of their own, sample keeps drawing ideas a scope pass just briefed, and
(already fixed) list rows could not show whether an idea had been reviewed. The
cluster goal is tasks-5408fe, inside the work-discovery lane tasks-46d207.

## Current behaviour and evidence

- tasks-e34b99 (review state visible in list): delivered by `2a32b16`
  (`feat(review): activate reviewed ideas and needs-review markers`) under
  `docs/specs/2026-10-11-review-state-design.md`: a stored `reviewed` flag, the `*`
  marker, and `list --needs-review`. Its remaining asks are the waiting-on id
  (tasks-a309e0) and pending drop proposals (spec §9 leaves them to tasks-22f407).
- tasks-a309e0 (waiting ideas lack an edge): `skills/scope/SKILL.md` §5 tells research
  completion to wake ideas with `note --no-reviewed`, and makes design tasks depend on
  research, but never adds `idea --on research`. A probe on 2026-10-13 showed `dep`
  accepts an idea, `ready` never lists ideas (so the report's "ready shows it
  unblocked" does not hold), and pretty `tree`/`list` show no dependency marker for
  any status; the edge is visible in `show`'s `depends_on` and survives promotion to
  `todo`, where `ready` then honours it.
- tasks-82725f (commits after a pass): neither skill mentions committing. Since
  `1ef0ba5` (2026-10-10, after the report) a project with `[git] commit = true` commits
  each write itself and reports `commit_deferred:`/`commit_failed:` warnings; other
  projects need a manual commit. `tasks commit` sweeps every uncommitted record under
  `tasks/`, which can capture unrelated records. This repository's passes commit by
  hand as `chore(tasks): scope …` (`9806c2e`, `0fc5947`).
- tasks-19d4fe (misfiled reports): scope's verdicts are scoped, briefed, question,
  shelved and drop; the fixed root forbids writes elsewhere. `tasks move --to <prefix>`
  exists, leaves a tombstone, and has `--explain`. Each project declares what it owns
  in `[feedback] scope` of `tasks/.config.toml`.
- tasks-84d2a9 (proposals queue): duplicate of shelved tasks-22f407, whose wake
  condition is "unanswered proposals being missed or the existing view no longer
  suffices". `tasks sample --limit 0 --older-than 0d --all-projects` on 2026-10-13
  reports 13 pending proposals across 8 projects, against 2 when it was shelved.
- tasks-ec3ff9 (sample redraws briefed ideas): `src/commands/sample.rs` draws
  uniformly from open, unclaimed, non-pending tasks not updated within the window
  (default 7 days). A scope audit note bumps `updated`, so a briefed idea re-enters
  after a week; nothing favours tasks untouched for weeks.

## Constraints

- JSON output is the contract; a new list field or filter needs its own task
  (tasks-22f407 already says so for the queue).
- Scope fixes one root per pass and writes nowhere else; cross-project writes are being
  designed in tasks-91c622 ("one fixed root per project").
- Scope and curate never execute `tasks drop`; drops stay proposals.
- The `reviewed` flag is about scope review, not curation (review-state spec §4);
  sample's pending-proposal rule stays note-based.
- A profile or a repository's own instructions decide commits (global rules); a pass
  must not override them.
- tasks-4d7a32 (doing) is editing `skills/scope/SKILL.md` for a re-scope path; the
  skill edits here should land after or merge with it.

## Alternatives

1. Misfiled reports (tasks-19d4fe).
   - A. Add a `refile` verdict that runs `tasks move --to <owner>` when the whole
     remedy belongs to one project whose `[feedback] scope` covers it, and otherwise
     files the remedy there and proposes the drop. **Lean**, since a move is
     reversible and keeps the tombstone.
   - B. Keep the fixed root absolute: record a `proposal: refile to <prefix>` and let
     the human or a cross-project pass (tasks-91c622) move it.
2. Sample's selection (tasks-ec3ff9).
   - A. Weight the draw by days since `updated`, keeping the pool definition and seeded
     determinism. **Lean**: it serves the stated outcome without teaching sample about
     scope.
   - B. Keep reviewed ideas out of the pool for a longer window (e.g. 30 days since
     their last `scope:` note), which reintroduces note parsing.
   - C. Raise the default `--older-than` from 7 days.
3. Proposals queue (tasks-84d2a9): unshelve tasks-22f407 and design its read there,
   rather than keep a second task; `park --waiting-on user` for proposals stays out of
   scope as the review-state spec left it.

## Unanswered questions

- May a scope pass move a record into another project without a ruling (1A), or must a
  refile stay a proposal (1B)? The user decides; tasks-91c622's contract must agree.
- Which selection rule should sample use (2A, 2B, 2C)? The user decides; a few curate
  passes after the change can show whether yield improves.
- Unshelve tasks-22f407 now? The user rules on the tasks-84d2a9 drop proposal.

## Proposed decomposition

- tasks-a309e0 — scoped, direct: scope §5 adds `tasks dep <idea> --on <research>`.
- tasks-82725f — scoped, direct: curate and scope say how a pass commits per project.
- tasks-19d4fe — briefed; waits on the user's ruling on question 1.
- tasks-ec3ff9 — briefed; waits on the user's ruling on question 2.
- tasks-84d2a9 — drop proposed in favour of tasks-22f407 (unshelve it).
- tasks-e34b99 — drop proposed; delivered by `2a32b16`.
