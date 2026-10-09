# Task note integrity

Scoping handoff, updated 2026-10-08. Goal: tasks-ea2a79. Not an approved design.

## Problem

Notes should avoid machine details and permit meaningful corrections. Publishing
sensitive history and correcting structured measurements are different needs:
redacting the current record alone does not sanitize a repository's Git history.
They share note selection and protected-history constraints, but their decisions
and delivery need not wait on each other.

This pass adds tasks-45bf05's corrected-reviewer case and dispositions the duplicate
structured-correction report tasks-c62c8d using the existing design task.

## Current behaviour and evidence

- `src/commands/mod.rs::append_note` normalizes trailing whitespace;
  `src/commands/edit.rs::check_invariants` allows whitespace-only historical repairs
  (tasks-7c6fcd, done). Other historical edits remain protected.
- `Ctx::takeover_summary` omits host, PID and absolute-worktree fields and names
  numeric Unix fallback identities (`sid:<pid>`) only as `Unix session`
  (tasks-46f99f). Logical session identifiers remain named in takeover notes.
  Detailed diagnostics remain local. Park notes already exclude store host/worktree.
- Feedback recurrence errors now name the single-line --body rule (tasks-fcf6f7, done).
- `src/model.rs::Note` has whole-second timestamps and no distinct note ID.
  Append-only history blocks removal (tasks-cc9a31) and machine-readable correction
  (tasks-2bb0ba, tasks-d56b72). Downstream outcome-measure readers were not inspected.
- tasks-d56b72 proposes flat amendments targeting original notes, latest correction
  effective, without amendment chains. Its full proposal remains in the task body;
  selectors, schema and protected-marker rules are not approved.
- **Reviewer correction (tasks-45bf05).** Reviewer metadata is text within the
  `review:` convention, not a typed field in `Note`. Current `check_invariants`
  preserves count, order, timestamps, authors and harness provenance, permitting
  only trailing-space/tab removal while retaining attachment-ledger meaning.
  `the_editor_may_only_strip_trailing_whitespace_from_notes` covers the general
  refusal. A new `note` appends plain text without a correction link. These are
  source-inspection findings; this pass ran no new reproduction and did not inspect
  obs's reader. The design already requires a corrected effective reviewer on the
  same original review round, with the original audit retained.
- tasks-c62c8d supplies no additional body or distinct acceptance case beyond the
  structured-correction proposal in tasks-d56b72 and reviewer case in tasks-45bf05.

## Constraints

Preserve original reports and unrelated notes. Lifecycle markers/provenance and the
attachment ledger carry machine-readable meaning. A timestamp alone cannot select
a note safely; consider equal timestamps, concurrent changes and redaction. Tasks
must not interpret run arithmetic or flow states.

Current-record redaction leaves earlier Git commits intact and therefore does not
by itself meet tasks-cc9a31's publishing goal. Historical publication cleanup needs
an explicit separate route even if a redaction command is adopted. Shared selection
work is a reason to coordinate design, not to block the tasks-local publishing
choice on obs's correction-reader work.

## Alternatives

1. **Audited current-record redaction.** Select and replace permitted text without
   copying sensitive content into the audit. Justify the mutation contract by its
   actual value; do not promise it removes sensitive Git history.
2. **Append-only correction.** Select an original note and append supersession,
   including the flat proposal. Coordinate obs readers so one effective run, review
   or concern remains after correction; this cannot remove sensitive historical text.
3. **Manual publishing exceptions for removal.** Retain the CLI's mutation rule and
   handle publication sanitization explicitly outside it. Compare this route before
   deciding redaction deserves a feature; prose-only corrections still miss readers.

Current lean: decide publishing/redaction value within tasks first, with manual
publishing exceptions a viable alternative. Design shared selection constraints,
then settle correction and its reader contract with obs. Stage delivery separately.
Use the shared correction contract for reviewer fixes; a reviewer-specific field or
second correction design would duplicate the existing work without settling selection
or downstream interpretation.

## Unanswered questions

- Does current-record redaction provide enough value to justify mutation, given that
  publication also needs Git-history cleanup? tasks-1f6d08 settles this first.
- How should callers select a stable note and reject stale targets while preserving
  lifecycle and attachment meaning, and what can a redaction audit safely retain?
- Can correction readers count exactly one effective run, review and concern, through
  replacement or supersession? tasks-1f6d08 coordinates this later stage with obs.
  For reviewer corrections, verify that the reader changes the effective reviewer
  without adding a review round or rewriting protected harness provenance.

## Proposed decomposition

- tasks-7c6fcd and tasks-fcf6f7: done; whitespace and feedback diagnostics are delivered.
- tasks-0238e4 and tasks-46f99f: delivered separate-field and Unix-identity privacy
  for new takeover notes; historical notes remain publishing work.
- tasks-1f6d08: P2/m/high/planned, staged within the existing design task. First choose
  redaction versus manual publishing handling without waiting on obs; share note
  selection constraints, then coordinate structured correction. Review design and
  implementation plans before code; delivery stages may proceed independently.
- tasks-cc9a31, tasks-2bb0ba and tasks-d56b72 remain ideas. Findings wake each in the
  same commit and update this brief; preserve the full amendment proposal and sources.
- tasks-45bf05 remains briefed as the corrected-reviewer acceptance case.
  tasks-1f6d08 already includes it in both the design checks and completion wake list;
  no follow-up task is added. tasks-c62c8d was dropped with user approval on
  2026-10-08 as a duplicate of tasks-d56b72 and tasks-45bf05, covered by
  tasks-1f6d08; its original report remains in history.
