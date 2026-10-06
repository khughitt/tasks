---
id: tasks-1f6d08
title: Design note redaction and correction semantics from the brief
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-09-30T09:53:57Z
updated: 2026-10-06T16:54:21Z
depends: []
parent: tasks-ea2a79
tags: []
agent: codex
---

Why: tasks-cc9a31 needs removal of sensitive historical note text, while tasks-2bb0ba needs a corrected run total to count as the same attempt. Correction is not solved by an arbitrary extra note; current-record redaction alone does not meet a repository-publishing need because Git history retains the text. tasks-d56b72 adds a concrete flat amendment proposal for concerns:, review: and run: corrections; retain it as an alternative rather than an approved JSON contract.

Additional report tasks-c62c8d describes the same structured-note correction gap and uses this design rather than a separate remedy.

Related report tasks-45bf05 needs a corrected reviewer to replace the effective review metadata consumed by outcome measures. Include it in the same structured-note amendment design and consumer checks, retaining the original review event rather than creating an extra review round.

Where to start: docs/notes/2026-09-30-note-integrity-brief.md; src/model.rs::Note, src/commands/edit.rs::check_invariants, src/format.rs, src/attachments.rs, and the lifecycle provenance and attachment contracts. Coordinate with obs on the run-note, review-round and concern reader contracts; its current implementation has not been inspected in this pass.

Bound: stage the design within this task, with design and implementation-plan review before code. First settle whether current-record redaction is useful enough to justify a mutation contract for tasks-cc9a31; compare manual publishing exceptions and explicitly describe required Git-history cleanup without promising the CLI performs it. Make this tasks-local decision without waiting on obs. Share note selection and protected-history constraints, then coordinate structured correction semantics and consumer validation with obs. The publishing decision and correction delivery may proceed separately. Compare an explicit audited text-replacement operation with append-only supersession, including the flat original-note-plus-amendments proposal in tasks-d56b72, whether generated lifecycle/attachment notes are protected, and how a target note is selected when timestamps repeat, how concurrent changes are rejected, how lifecycle and attachment-ledger meaning survives, and what data a redaction audit retains. Keep tasks ignorant of run: arithmetic and flow state names. Historical Git cleanup is outside this CLI feature.

Expected result: a reviewed decision on mutation versus supersession, necessary consumer coordination and checks, and executable child tasks only after plan review. A correction check must show one attempt with a corrected total and one effective review/concern after amendment, including a corrected reviewer consumed by the downstream measure, retaining the original audit history; if redaction is selected, its check removes selected text from the current record without copying it into an audit event or damaging protected history, and explicitly shows why that alone does not sanitize repository history for publication.

Ideas it wakes: on completion, add a finding note to tasks-cc9a31, tasks-2bb0ba, tasks-d56b72, tasks-45bf05 and tasks-c62c8d in the same commit and update the brief; then rerun scope on them.

## Notes

- 2026-10-06T09:45:47Z (main): scope follow-up: tasks-d56b72 contributes flat original-note amendments; extend consumer verification beyond run totals to review and concern records without choosing the proposed schema
- 2026-10-06T10:39:15Z (main): review: spec round 1 — verdict: revise; findings: Important 3, Minor 1; reviewer: human
- 2026-10-06T10:41:53Z (main): Review disposition: decide publishing/redaction value first within tasks, including the manual publishing option and unchanged Git-history exposure; coordinate obs only for structured corrections. Share selectors without coupling delivery milestones.
- 2026-10-06T10:59:44Z (main): review: spec round 2 — verdict: accept; findings: none; reviewer: unspecified
- 2026-10-06T16:52:02Z (main): scope: tasks-45bf05 joins the existing structured-amendment design; verify corrected reviewer metadata as the same effective review, sharing note selection and protected-history rules rather than creating a separate correction API.
- 2026-10-06T16:54:20Z (main): scope: tasks-c62c8d arrived during organization and joins the existing structured-note correction evidence; reuse this design without adding another API or investigation.
