---
id: tasks-45bf05
title: "Task review metadata cannot be corrected through the CLI: edit rejects reviewer-only corrections because notes are append-only, but outcome measures consume the original reviewer field."
status: idea
priority: 2
created: 2026-09-30T10:40:35Z
updated: 2026-10-06T16:52:03Z
depends: []
parent: tasks-ea2a79
tags: [feedback, gap, "from:material"]
agent: codex
---

Reported symptom: reviewer-only corrections are rejected by append-only note invariants, while outcome measures continue consuming the original reviewer metadata.

Next action: reuse tasks-1f6d08's structured-note amendment design under the note-integrity lane. Preserve the original event and source, agree effective-review interpretation with obs, and verify that a corrected reviewer updates that same review instead of adding a round. Keep this report as an idea until the reviewed remedy supplies its disposition.

## Notes

- 2026-10-06T16:52:02Z (main): organization: joined tasks-ea2a79; tasks-1f6d08 owns the shared structured-amendment contract and corrected-reviewer acceptance case.
