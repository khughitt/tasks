---
id: tasks-1f6d08
title: Design note redaction and correction semantics from the brief
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-09-30T09:53:57Z
updated: 2026-09-30T09:53:57Z
depends: []
parent: tasks-ea2a79
tags: []
agent: codex
---

Why: tasks-cc9a31 needs removal of sensitive historical note text, while tasks-2bb0ba needs a corrected run total to count as the same attempt. Neither is solved by an arbitrary extra note.

Where to start: docs/notes/2026-09-30-note-integrity-brief.md; src/model.rs::Note, src/commands/edit.rs::check_invariants, src/format.rs, src/attachments.rs, and the lifecycle provenance and attachment contracts. Coordinate with obs on the run-note reader contract; its current implementation has not been inspected in this pass.

Bound: prepare a design and implementation plan for review before code. Compare an explicit audited text-replacement operation with append-only supersession, including how a target note is selected when timestamps repeat, how concurrent changes are rejected, how lifecycle and attachment-ledger meaning survives, and what data a redaction audit retains. Keep tasks ignorant of run: arithmetic and flow state names. Historical Git cleanup is outside this CLI feature.

Expected result: a reviewed decision on mutation versus supersession, necessary consumer coordination and checks, and executable child tasks only after plan review. A correction check must show one attempt with a corrected total; a redaction check must remove the selected text from the current record without copying it into the audit event or damaging protected history.

Ideas it wakes: on completion, add a finding note to tasks-cc9a31 and tasks-2bb0ba in the same commit and update the brief; then rerun scope on them.
