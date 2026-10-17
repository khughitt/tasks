---
id: tasks-d56b72
title: "Notes are append-only with no way to amend one: a mislabelled structured note (concerns:, review:, run:) can only be contradicted by a later free-text note that parsers don't link back"
status: idea
priority: 2
reviewed: true
created: 2026-10-05T09:10:39Z
updated: 2026-10-17T17:37:44Z
depends: [tasks-1f6d08]
parent: tasks-ea2a79
tags: [feedback, idea, "from:material"]
agent: claude-code/claude-opus-5-5
---

Case: a structured note written under the wrong rule stays counted by outcome measures; the only fix is a free-text 'correction:' note that consumers cannot tie to the original. Proposal: 'tasks note <id> --amends <note-ref> <text>' keeps the original byte-for-byte and attaches the amendment to it (in show/JSON as note.amendments[]), and consumers read the latest amendment (e.g. a 'retract' or replacement text). Depth: recommend flat, two levels only: an amendment targets an original note, never another amendment; correcting an amendment means adding another amendment to the same original, latest wins, full history kept in order. That gives a bounded shape (note plus an ordered list) with no tree to walk and no question of where to stop. Open: whether generated lifecycle markers may be amended (suggest no: they are provenance) and what note-ref is (timestamp plus index is stable under append-only).

## Notes

- 2026-10-06T09:45:47Z (main): scope: briefed; reused tasks-1f6d08 for structured-note amendments; retain flat supersession as a proposal pending selector, protected-history and consumer review; brief: docs/notes/2026-09-30-note-integrity-brief.md
