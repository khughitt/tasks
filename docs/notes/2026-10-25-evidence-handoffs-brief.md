# Reliable evidence handoffs

## Problem

Three reports from the same evidence workflow need bounded next actions:
tasks-9a7ac8 needs lossless transport of one over-cap manifest; tasks-056222 asks
whether record commits can avoid absorbing concurrent Git staging; tasks-041759
needs a required brief in another project to remain authoritative and portable.

## Current behaviour and evidence

Checked against main at 3813d1d on 2026-10-25; code and tests were read, not run.

- `src/commands/attach.rs::attach` reads one source through `read_capped`;
  `src/attachments.rs` defaults to 2 MiB. The ledger tracks individual files,
  with storage checks and cleanup on save failure (c7d640e, 44b5212).
  No multipart split, catalog, reconstruction or export contract exists.
- tasks-b9b2e2 specifies atomic multi-file attach, preserving the per-file cap
  agreed with the user on 2026-10-13. Its several-small-files case does not
  cover splitting one large manifest or verifying reconstruction.
- `src/record_commit.rs::commit_paths_in` serializes tasks writers and checks
  staged paths before staging and immediately before a plain commit.
  The approved record-commit design §§3.3/3.7 deliberately permits staging
  arriving while hooks run. Pathspec commits were rejected because hook-generated
  files left stale staging in the real index. The reported overlap fits this
  exception; it does not establish failure of the initial staged-work check.
- `src/refs.rs::location` rejects absolute paths and reserves `@`.
  The reference design defers cross-project references. tasks-d09e87 already owns
  that design, using docs/notes/2026-10-19-reference-extensions-brief.md.
  tasks-041759 adds a concrete required-input case: reuse a foreign follow-up,
  retrieve the current authoritative brief, and avoid duplicate snapshots.
- All three reports have no creation note or provenance stamp in `show`.
  Filing-event outcome: unavailable (no stamp); no older origin session is named.
  Their original bodies remain evidence reports, not verified human requests.
  Origin remains unknown. No attached draft is present; existing attachment,
  scratch-evidence and reference-extension briefs were checked.

## Constraints

Keep caps, readable evidence, exact byte identity, ledger/storage safety and normal
content checks. A multipart route must not become an unbounded cap bypass or hide
text in archives. Preserve the current hook regeneration behavior, staged partial
versions and cleanup semantics. Fixtures isolate Git selectors and global config,
and build subprocess environments key by key. No host resources are declared.
Any new CLI surface starts with ops vocabulary rows. A brief grants no spec or
implementation-plan approval.

## Alternatives

1. Keep manual splitting/catalogs and dated brief snapshots; serialize all Git
   writes. Available now, with reconstruction and snapshot-staleness costs.
2. Investigate narrowly, then extend existing mechanisms only where warranted.
   Lean: establish multipart boundaries and commit-isolation feasibility; reuse
   tasks-d09e87 for portable required inputs.
3. Raise caps, add a general artifact store or replace record transactions.
   These reports do not justify those changes; do not assume them as remedies.

## Unanswered questions

- What smallest supported workflow retains and reconstructs an oversized manifest
  exactly while preserving caps and checks? The multipart investigation compares
  documented manual parts with a narrow supported route, including partial failure.
- Can commits exclude concurrent staging while preserving hook-derived files and
  the user's staged state? The commit investigation tests the documented exception
  and recommends a remedy or an explicit retained limitation.
- How does a foreign follow-up identify and read the authoritative required brief
  across ordinary/detached/linked roots, aliases and unavailable projects?
  tasks-d09e87 answers in its existing reviewed-design workflow.

## Proposed decomposition

Create one ordinary goal under tasks-0e432f, with these three previously unparented
ideas. File one P2/s/mid/direct multipart investigation waking tasks-9a7ac8 and one
P2/s/high/direct commit-isolation investigation waking tasks-056222. Reuse
tasks-d09e87 (P3/m/high/planned, under tasks-7102c1), preserving its existing inputs
and adding tasks-041759 to its wake list. Record every wait as a dependency.
Each follow-up updates this brief and wakes its named ideas with `--no-reviewed`
before integration. No duplicate design or research task is needed.

