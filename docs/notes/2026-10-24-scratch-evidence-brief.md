# Preserve task evidence when closing work from scratch

## Problem

Files prepared off the synced tree need a disposition before a task closes. Two
reports from the same source describe adjacent obstacles: `attach` failed from a
scratch directory beneath an empty `.git`, and `done` left a reproduction script
and verification output in per-task scratch without any record naming them.

## Current behaviour and evidence

Checked at `89e0371` on 2026-10-24; these are code findings, not a reproduced incident.

- `src/commands/mod.rs::open_id_ctx_unwarned` uses the registry only after
  `Error::NoProject`. Both `show` and `open_id_write_ctx` use this function; `attach`
  and `note` use the latter. There is no attachment-specific routing difference.
- `src/link.rs::GitTree::of` returns an IO error for an obstructing `.git` marker;
  `obstruction` offers conditional `rmdir` guidance only for a confirmed empty
  directory. Commit `328790d` introduced the diagnostic. The report's assertion
  that `show` succeeds needs checking under the same cwd, environment and binary.
- `attach::attach` opens a source path as supplied, copies it into the resolved
  project's records, and records a ledger note. The reviewed attachment design
  (`docs/specs/2026-09-28-task-attachments-design.md`) requires prefix routing,
  storage validation, caps and cleanup on save failure. Preserve these protections.
- `src/commands/status.rs::close` loads, transitions and saves a task. No source
  code reads `WORK_ROOT`; no close-time scratch warning exists.
- `tasks-037538` and its open design `tasks-57425c` concern parked resume inputs.
  Their brief (`docs/notes/2026-10-16-park-resume-inputs-brief.md`, updated in
  `3d46b2b`) explicitly excludes close-time scratch checks. They do not cover this
  new request. Open attachment filing research `tasks-4216a2` concerns creating
  records with attachments, not routing existing IDs from a scratch directory.

## Constraints

Records remain CLI-managed. No automatic deletion, arbitrary cleanup commands,
recursive scratch inventory or new artifact schema. Paths under `WORK_ROOT` are
host-local. For the bounded warning, use the current canonical project prefix as
the project component of `$WORK_ROOT/<project>/<task-id>/`; do not search old
prefixes or other hosts. This is a selected convention, not an existing CLI contract.
Keep warning text in the existing warnings array; preserve JSON shapes, completion
and output-reference gates, claims, and checkout selection. No new CLI options are
needed for the warning. This project declares no shared resource needs.

## Alternatives

1. Keep manual attachment/disposition guidance alone. It is available now but did
   not catch the reported unrecorded evidence at close.
2. Add a non-blocking warning on successful `done` when the conventional per-task
   scratch directory is nonempty, and investigate the routing report separately.
   **Lean:** this directly covers both asks without coupling close to park design.
3. Introduce record-wide artifacts and shared park/close auditing. The parked-input
   brief considered this; it adds synced schema and host identity decisions that
   the requested warning does not need. Do not expand the existing park design.

## Unanswered questions

- With the same binary, cwd and environment, do `show`, `note` and `attach` differ
  under a confirmed empty ancestor `.git`? What happens without that marker? The
  routing investigation answers with an isolated fixture matrix and binary identity.
- If marker obstruction is the common cause, should full IDs bypass an unrelated
  empty marker or should the diagnostic name the checkout/absolute-file workaround?
  The investigation recommends a remedy while preserving real broken-repository
  errors and local/worktree precedence; an implementation requiring a policy change
  receives a reviewed design rather than an attachment-only fallback.

## Proposed decomposition

- `tasks-b34076`: cluster goal inside the existing Record authority and recovery
  lane (`tasks-0e432f`); no new lane.
- `tasks-7cbc1b`: scope directly, P2 / s / mid / direct. Warn on successful `done`
  (including an already-done cleanup retry) for a nonempty conventional scratch
  directory; name the path and a checkout-based `attach` command using an absolute
  source path. No warning for unset/empty `WORK_ROOT`, absent or empty directory;
  diagnose an unreadable or invalid scratch path instead of calling it empty.
  Inspect only directory entries. Never remove or automatically attach anything.
- `tasks-4b68c8`: keep as an idea pending a bounded P2 / s / mid / direct routing
  investigation `tasks-e147b9`. Record the wait with a dependency and carry findings back using
  `--no-reviewed`. Do not duplicate the existing attachment-filing or park research.

The routing investigation is a child of the cluster goal. Its `Ideas it wakes`
requires a finding note on `tasks-4b68c8` before integration. Neither the original
source reports nor their provenance is replaced.
