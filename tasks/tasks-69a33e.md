---
id: tasks-69a33e
title: "Point multi-line completion evidence at tasks attach: the single-line refusal names it and the skill documents it"
status: todo
priority: 2
size: s
complexity: low
process: direct
created: 2026-10-18T02:37:48Z
updated: 2026-10-18T09:43:13Z
depends: []
tags: [feedback, gap, "from:beliefs"]
agent: codex
---

Why: `done` and `note` store their text as a single-line note (`format::validate_note_text`, enforced again when a record is parsed), so a multi-line message is refused with "note text must be a single line". A test-latency closeout on 2026-10-18 needed the full `tt-latency verify` output in its completion, as the global closeout rule asks, and kept it by JSON-encoding the newlines into one line. A route that keeps multi-line output intact already exists: `tasks attach <id> - --name <file> --caption "<what it shows>"` reads stdin and works both before and after `done` (checked in a scratch project, 2026-10-18). Neither the refusal nor the skill points to it.

Reported as: "Completion messages accept only one line; preserving full verifier output requires encoding its newlines." (from beliefs, codex)

Decided at scoping: notes stay single-line. Multi-line notes were rejected because the notes section is a one-line-per-entry ledger that every reader parses and `parse_task` validates, and attachments already carry arbitrary bytes behind a ledger note.

Done:
- When `done` or `note` refuses text that contains a line break, the error says to keep the message to one line and attach the full output with `tasks attach <id> - --name <file> --caption "<what it shows>"`. The error kind stays `validation`. Other commands that share `normalize_note_text` get the same hint where the id is known.
- skills/tasks/SKILL.md says, at step 6 (`tasks done <id> "<what landed>"`) or in the attach bullet, that the completion message is one line, and that multi-line evidence such as verifier or test output is attached and named in the message.
- A test in tests/cli.rs checks the refusal text on `done` with a two-line message.

Where to look: src/format.rs (`validate_line`, `validate_note_text`, `normalize_note_text`), src/commands/mod.rs (`append_note`), src/commands/status.rs (`done`, `note`), src/commands/attach.rs; skills/tasks/SKILL.md.

Out of scope: the global closeout rule that asks for the verify output in the done message belongs to lore. It was filed there as lore-e24159.

## Notes

- 2026-10-18T09:43:13Z (main): scope: scoped; todo, s/low/direct, P2; retitled to the attach route: tasks attach - already keeps multi-line output before and after done (scratch check 2026-10-18), so the work is the refusal hint and a skill line; multi-line notes rejected; instruction side filed as lore-e24159
