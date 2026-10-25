# Recovery hints for an unclaimed worktree-only task

## Problem

An existing task can appear lost when its only record is on an unmerged worktree
branch and neither its own claim nor park names that checkout. tasks-399eef asks
for a read-only way to locate it without starting or parking untouched work.
The report says the user asked for history and prevention; its creation has no
session stamp, so the originating ask cannot be independently verified. Keep
origin unset. The reported incident is evidence supplied by the record, not a
reproduction performed by this scope pass.

## Current behaviour and evidence

Code inspected at `220bb14` on 2026-10-25:

- `src/commands/show.rs::read_record` reads locally first. On absence,
  `recorded_elsewhere` tries the exact task's live claim, then its park. With
  neither, it returns `task_not_found`. A parent's park is not considered.
- `bd7cb18` introduced this fallback. The tests beginning with
  `show_reads_a_worktree_only_task_from_the_checkout_its_claim_or_park_names`
  cover claim/park recovery, unavailable checkouts and local precedence.
- `src/repo.rs::Project::sibling_task_copies` already enumerates Git worktrees
  for write-side stale-copy checks. It distinguishes found and unreadable copies,
  but skips `NotFound`, including a missing worktree directory. Reuse needs an
  explicit decision about what an incomplete scan can establish.
- Record-home design sections 6.1 and 8 explicitly retain this visibility limit.
  tasks-f3910b covers a newer sibling of a record present locally, not this case.
  tasks-70f0bc covers publication for dependency authority, not locating an
  unclaimed record. No open research task found answers this exact question.

## Constraints

Keep local reads, claim/park precedence, registered dependency authority and
write ownership unchanged. Do not copy or merge records, activate runtime code,
or scan arbitrary clones, other hosts or Git history. Detached records have one
shared records home; stale historical code-tree records must not become recovery
candidates. Nested in-tree projects and inaccessible worktrees need explicit
treatment. This project declares no shared resource needs.

## Alternatives

1. Keep manual `git worktree list` and explicit-checkout reads as guidance only.
   Cheap, but plain `show` still gives no location evidence.
2. Enrich the missing-record error with read-only worktree diagnostics and an
   explicit `tasks -C <checkout> show <id>` retry for a unique eligible match.
   **Lean:** preserve the error kind and JSON shape; report ambiguity and incomplete
   search honestly. No automatic selection or write-routing extension.
3. Automatically return a unique sibling record. This expands read authority and
   requires deciding which records and documents a scan may treat as current.
   The reported ask can be met by alternative 2 with fewer contract changes.

## Unanswered questions

- Which worktrees are eligible under in-tree, nested, linked and detached layouts?
  The design follow-up must settle this against current discovery and pairing code.
- How should absent, unreadable, malformed and multiple candidates affect the
  diagnostic and its uniqueness claim? The design must define bounded scan and
  race behavior; a hint cannot guarantee the record remains available at retry.
- Can the existing sibling helper meet that contract without changing stale-copy
  behavior? Trace its callers during design and specify regression checks.

The follow-up produces a written design for user review, then a reviewed plan
before implementation. This brief and its lean do not grant those approvals.

## Proposed decomposition

Reuse tasks-984769 within the tasks-0e432f lane; create no new goal or lane.
Keep tasks-399eef as an idea, waiting on tasks-217fbb, the P2 / s / high / planned
design task from this brief. Its completion must record the finding on tasks-399eef with
`--no-reviewed` before integration. Acceptance cases include no match, unique and
multiple matches, incomplete scans, local/claim/park precedence, detached records,
nested projects and unchanged writes. No separate research task is needed: the
current failure path is established, while the diagnostic contract needs design.
