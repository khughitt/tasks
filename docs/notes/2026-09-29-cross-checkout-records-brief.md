# Cross-checkout task records brief

Scoping handoff, 2026-09-29. Goal: tasks-c4ad8e. This is not an approved design.

**Status, 2026-09-30.** tasks-476c6b (b784901) and tasks-2325a1 (dca3e62) are done, and
tasks-ace27b (ecceb96) extended the main-checkout document fallback from `check` to `show`,
`add`, and `edit`. Design task tasks-ab8d2d has a draft spec under review,
`docs/specs/2026-09-30-record-home-design.md` (on its branch until it merges). The user's
2026-09-30 decision recorded there supersedes this brief's current lean for writes: see
§Alternatives. The evidence below describes the code as it stood when this brief was written.

## Problem

A task's record exists once per checkout: the main checkout and every worktree carry their
own `tasks/<id>.md`. Someone standing in one checkout can write to a copy that is no
longer the live one, fail to see a task that exists only on a worktree branch, or be
blocked by a document the worktree cannot carry. Writes should land in the copy the work
will merge from, reads should find worktree-only work, and a fresh worktree should pass
its own gate.

## Current behaviour and evidence

- **Writes.** `save` in `src/commands/mod.rs` calls `warn_on_newer_sibling_copies` before
  writing (commit 465b778, tasks-76671b). It warns only when a sibling's `updated` stamp is
  newer than the loaded one, never refuses, and falls quiet after the first write because
  the new stamp wins. tasks-2c0a1d asks for a refusal, or for the worktree to be named
  before the write. tasks-bb53e5 records the silent case: once a worktree exists with an
  equal copy, a write in the main checkout says nothing. The only signal comes at the
  worktree's next write. That happened on tasks-142d2f on 2026-09-25: `start` ran in main,
  a note was added in main after `git worktree add`, and the note was replayed by hand.
- **Home checkout.** Claims and parks record `worktree` (`src/claims.rs`, `Claim`, `Park`).
  In the prescribed order (commit the record, then `git worktree add`), `start` runs in
  main, so the claim names main while the work happens in the worktree. Nothing names the
  worktree until the worktree writes for the first time.
- **Reads.** Park design §5.3 (`docs/specs/2026-09-09-park-design.md`) resolves a
  store-only park by scanning its recorded worktree (`resolve_recorded` in
  `src/commands/parked.rs`), and it warns `resume it from that checkout`. `show`
  (`src/repo.rs`, `TaskNotFound`) and plain `list` read only the local scan (tasks-fbc32b).
  `prime` builds `doing` from the local scan only (`src/commands/list.rs`, `prime`), so a
  live claim on a worktree-only task is dropped with no warning (tasks-476c6b).
- **Documents.** `check` tests `ctx.project.root.join(path).is_file()` for open tasks'
  spec and plan (`src/commands/check.rs`, `doc_missing`). A spec kept out of git through
  `.git/info/exclude` exists in main and not in a new worktree, so that worktree's
  pre-commit gate fails (tasks-2325a1). The global instructions leave whether a project
  commits its specs to the project's profile, so uncommitted specs are a supported setup.

## Constraints

- Work-claims design §Warnings and §Known gaps (`docs/specs/2026-09-05-work-claims-design.md`):
  the sibling check is a signal by design ("never refuses"), and content comparison was
  rejected as noise. Changing either reverses a documented decision.
- The fresh-worktree brief (`docs/notes/2026-09-15-fresh-worktree-brief.md`) rejects
  copying records between checkouts, so the CLI stays a record manager.
- The JSON output is the contract. Adding worktree-only rows to `list` or a `checkout`
  field to `show` changes its shape and needs its own task.
- Park §5.3 already fixes store-only entries as never `next` candidates, and says a task
  present in both checkouts is read from the local copy.

## Alternatives

1. **Refuse on a newer sibling.** Turn the existing warning into a typed `newer_copy` error
   that names the checkout, with `--force` to override. This closes tasks-2c0a1d cheaply,
   but not tasks-bb53e5's equal-stamp case.
2. **A home checkout.** Treat the worktree recorded by the live claim or park as the
   record's home. A write from another checkout refuses and names it, and `show` reads from
   it when the local scan lacks the id. This needs a handoff when work moves into a new
   worktree, since `start` usually runs in main first. A re-`start` in the worktree could
   move the claim, or `git worktree add` could be wrapped.
3. **Stay a signal, add a read-side route.** Keep writes as they are. Close the read gaps
   (tasks-476c6b, tasks-fbc32b) with the §5.3 resolution pattern, and rely on the workflow
   rule of committing before branching and writing only in the worktree afterwards.

Current lean: option 3 now (tasks-476c6b is scoped on its own), then options 2 and 1
together as the durable fix. *Superseded on 2026-09-30: the user decided that a write from a
copy that is behind refuses for every task, claimed or not, with no override; the record-home
spec records this and rejects the claimed-or-parked-only scope below.* The lean was: the live
claim or park names the record's home. `start` is
the handoff: a re-`start` in a new worktree moves the claim there, and it refuses when a
sibling copy is newer, so the handoff cannot drop writes made in main. Any other write
from a checkout that is not home refuses with a typed error that names the home and gives
the exact `tasks -C <home> ...` retry. It never routes the write on its own, because a
redirect is a silent fallback. A task with no claim and no park has no home, and the
newer-sibling rule stays a warning for it. `show` falls back to the home copy with a
warning, and plain `list` stays local. The user has not decided this.

For documents, the user decided on 2026-09-29: `check` also looks in the main checkout
(the first entry of `git worktree list`), and a doc found there is a warning naming that
checkout, not a `doc_missing` error.

## Unanswered questions

- Should a write from a non-home checkout refuse, or warn before writing? Answered
  2026-09-30: a write from a copy that is behind refuses (record-home spec §2). Design task
  tasks-ab8d2d resolves the rest.
- How does home move to a worktree created after `start`? A re-`start`, a new command, or
  inference from the branch? This is for the design task.
- Should plain `list` include worktree-only tasks, marked with their checkout, or should
  only `show` route? This is for the design task, and it touches the JSON contract.

## Proposed decomposition

- Goal tasks-c4ad8e: keep a task record coherent across checkouts. It is the parent of
  all five ideas.
- tasks-476c6b (done, b784901): `prime` resolves live claims on worktree-only tasks from
  the claim's worktree.
- Design task (tasks-ab8d2d): the home checkout for writes and reads; spec drafted and under
  review. tasks-2c0a1d, tasks-bb53e5 and tasks-fbc32b wait on it. tasks-94b9c7, a later
  report of the tasks-bb53e5 flow, was dropped as its duplicate.
- tasks-2325a1 (done, dca3e62): `check` looks for missing docs in the main checkout, as
  the user decided on 2026-09-29.
