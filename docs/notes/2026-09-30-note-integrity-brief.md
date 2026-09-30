# Task note integrity

Scoping handoff, 2026-09-30. Goal: tasks-ea2a79. This is not an approved design.

## Problem

Task notes need to pass repository gates, avoid publishing generated machine details,
and support correcting mistakes without corrupting the history other tools read. Five
reports share the note-writing path, but prevention and diagnostics can proceed before
deciding how historical redaction and corrections work.

## Current behaviour and evidence

- `src/commands/mod.rs::append_note` validates and stores text verbatim. All command
  note writers, including lifecycle and feedback writers, use it. Trailing spaces/tabs
  survive; `src/commands/edit.rs::check_invariants` rejects any change to existing notes
  (tasks-7c6fcd).
- `src/commands/status.rs::start` copies takeover warnings into notes. Their formatter,
  `Ctx::describe_claim`, includes host, PID, and absolute worktree. In contrast,
  `src/commands/park.rs` keeps host/worktree in the local `Park` record and writes only
  the waiting reason and next step to the note (tasks-0238e4). Lifecycle provenance
  stamping landed in `8f13112`; it is independent of these text diagnostics.
- `src/commands/feedback.rs::recur_into` accepts a single-line body and rejects multiline
  detail before writing. Creation accepts multiline bodies. The feedback design section
  3 explicitly requires this distinction, and
  `feedback_recurs_on_exact_titles_and_refuses_to_guess_on_similar_ones` checks the
  rejection kind; the generic error does not explain it (tasks-fcf6f7).
- `src/model.rs::Note` has timestamp, author, text, and optional provenance, with no
  distinct note ID. The append-only editor guard prevents redaction (tasks-cc9a31) and
  run-total correction (tasks-2bb0ba). The latter report says a second `run:` line counts
  as another attempt; the downstream reader was not inspected in this checkout.

## Constraints

Preserve original reports and sources. Existing notes remain unchanged on unrelated
writes. A narrowly defined, explicit trailing-space/tab repair does not authorize general
history editing. The core design documents the append-only rule; changing it requires
an explicit contract.

Notes carry machine-readable meaning: lifecycle markers/provenance and the attachment
ownership ledger (`src/attachments.rs`, attachment design section “Ownership ledger”).
Timestamps have whole-second precision, so a timestamp alone cannot safely select a
note. Tasks should not interpret run arithmetic or flow states. Redaction from the
current record does not erase prior Git commits. The existing checkout-coherence design
task tasks-ab8d2d is related concurrency context, not a duplicate of this work.

## Alternatives

1. **Explicit audited replacement:** select an existing note unambiguously, reject stale
   targets, replace permitted text, and record the operation without repeating removed
   sensitive text. This can serve redaction, but needs rules protecting structured notes.
2. **Append-only supersession:** identify the original note and append a correction that
   readers resolve. This preserves history but cannot remove sensitive text and requires
   reader changes before a corrected run counts once.
3. **Keep the current rule:** record prose corrections and handle publishing exceptions
   manually. This leaves both reported gaps unresolved.

Current lean: explicit redaction for removal, with correction semantics chosen after
checking reader behavior. Avoid a generic event framework. Ship the three bounded fixes
independently; do not make them wait for a new note model.

## Unanswered questions

- How should a caller select a note and prove it has not changed? The design follow-up
  must compare selectors against same-second notes and concurrent edits.
- Which edits preserve lifecycle and attachment meaning, and what does the audit retain?
  The design follow-up and user review settle the contract.
- Would the run reader correctly re-read replaced text, or does it need supersession and
  cache invalidation? Coordinate with obs during design; this pass establishes no finding
  about that implementation.

## Proposed decomposition

- tasks-7c6fcd: scoped, priority 2, small/mid/direct. Normalize new note endings and permit
  only explicit trailing-space/tab removal from existing note text with metadata intact.
- tasks-0238e4: scoped, priority 2, small/mid/direct. Persist a concise takeover summary;
  keep machine details in local diagnostics/store. Ordinary park already does this.
- tasks-fcf6f7: scoped, priority 2, extra-small/low/direct. Identify `--body` in recurrence
  errors and document its single-line constraint, for automatic and explicit recurrence.
- tasks-1f6d08: priority 2, medium/high/planned. Design historical redaction and correction
  semantics, then obtain design and plan reviews before implementation. Its findings wake
  tasks-cc9a31 and tasks-2bb0ba, which remain ideas. No separate research task is needed
  before this bounded design investigation.
