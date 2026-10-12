---
id: tasks-45bf05
title: "Task review metadata cannot be corrected through the CLI: edit rejects reviewer-only corrections because notes are append-only, but outcome measures consume the original reviewer field."
status: idea
priority: 2
reviewed: true
created: 2026-09-30T10:40:35Z
updated: 2026-10-12T18:22:32Z
depends: []
parent: tasks-ea2a79
tags: [feedback, gap, "from:material"]
agent: codex
---

Reported symptom: reviewer-only corrections are rejected by append-only note invariants, while outcome measures continue consuming the original reviewer metadata.

Next action: reuse tasks-1f6d08's structured-note amendment design under the note-integrity lane. Preserve the original event and source, agree effective-review interpretation with obs, and verify that a corrected reviewer updates that same review instead of adding a round. Keep this report as an idea until the reviewed remedy supplies its disposition.

Scoping evidence (2026-10-08): src/model.rs::Note stores at, by, text and optional harness provenance; reviewer is text within the review: convention, not a typed Note field. src/commands/edit.rs::check_invariants permits only removal of trailing spaces/tabs while retaining note order, authors, timestamps, provenance and ledger meaning. src/commands/status.rs::note appends; it creates no correction link. The existing test the_editor_may_only_strip_trailing_whitespace_from_notes covers the general refusal. This pass inspected source, not a new runtime reproduction or the downstream obs readers.

Handoff: retain this distinct corrected-reviewer acceptance case under tasks-1f6d08, which already names this idea in its completion wake list. Require the same original review round with corrected effective reviewer, unchanged original audit history and protected harness provenance. Keep the report as an idea until the user-reviewed correction contract and consumer checks establish its remedy; do not introduce a reviewer-specific field or a separate design task.

## Notes

- 2026-10-06T16:52:02Z (main): organization: joined tasks-ea2a79; tasks-1f6d08 owns the shared structured-amendment contract and corrected-reviewer acceptance case.
- 2026-10-08T10:21:32Z (main): scope: briefed; confirmed text-only reviewer metadata and append-only editor guard; retained same-review-round acceptance and protected provenance; reuse tasks-1f6d08, whose wake list already includes this idea; downstream reader behavior remains unverified here; brief: docs/notes/2026-09-30-note-integrity-brief.md
