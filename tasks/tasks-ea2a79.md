---
id: tasks-ea2a79
title: Note integrity
status: todo
priority: 3
size: m
complexity: high
process: planned
lane: true
created: 2026-09-30T09:53:56Z
updated: 2026-10-06T16:49:56Z
depends: []
tags: []
source: docs/notes/2026-09-30-note-integrity-brief.md
agent: codex
---

Make task history correctable and usable for publishing without erasing protected provenance or misleading downstream measures. First milestone: tasks-1f6d08 settles note selection and protected-history rules, with separate decisions for publishing redaction and structured corrections.

Coordinate the scoped note-input, takeover-privacy, and feedback-diagnostic fixes with a design for historical redaction and structured run-note corrections. Done when the direct fixes land and the waiting ideas have reviewed, explicit dispositions and any agreed implementation is complete. Handoff: docs/notes/2026-09-30-note-integrity-brief.md.

Include tasks-45bf05's reviewer-metadata correction in the structured-amendment acceptance checks already shared by run, review, and concern notes. Coordinate consumer interpretation with obs; tasks remains ignorant of measurement-specific arithmetic and state names. A current-record redaction does not sanitize Git history. Preserve sources and existing child trees. Done when retained work has reviewed decisions and verification or explicit dispositions.

## Notes

- 2026-10-06T16:49:55Z (main): organization: adopted as a lane from the user's approved 2026-10-06 backlog organization; existing child records and their review gates remain intact.
