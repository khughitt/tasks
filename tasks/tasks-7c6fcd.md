---
id: tasks-7c6fcd
title: Normalize trailing note whitespace and allow whitespace-only repairs
status: done
priority: 2
size: s
complexity: mid
process: direct
owner: fix/note-whitespace
created: 2026-09-18T15:31:41Z
updated: 2026-09-30T14:44:36Z
started: 2026-09-30T14:40:30Z
completed: 2026-09-30T14:44:36Z
depends: []
parent: tasks-ea2a79
tags: [feedback, friction, "from:obs"]
agent: codex
---

Why: a trailing space or tab in a note can fail whitespace gates, while the editor currently refuses even a whitespace-only repair.

Done: validate the original input for CR/LF before trimming trailing ASCII spaces and tabs in the shared append_note path; reject a result with no text. Preserve leading and internal whitespace. Permit an explicit editor repair only when each changed note text equals its original text with trailing spaces/tabs removed; note count, order, timestamp, author, provenance, and all other text must remain unchanged. Do not silently rewrite historical notes on unrelated commands. Update the note/editor contract in the core spec and relevant user documentation.

Where to look: src/commands/mod.rs::append_note (also used by lifecycle, feedback, shelf, and attachment writers), src/format.rs::validate_note_text, src/commands/edit.rs::check_invariants, tests/cli.rs, docs/specs/2026-08-29-tasks-design.md sections 3.2 and interactive edit.

Verification: focused checks cover new notes with trailing spaces/tabs, a lifecycle message through the same writer, blank and CR/LF rejection, an explicit whitespace-only repair preserving metadata, and rejection of any other historical-note edit. Existing attachment-ledger checks must remain valid; run just test-one for the focused checks and just test-fast before the implementation commit.

Original report: The note command accepts a single-line value ending in a space. A whitespace-check gate then fails on the generated record. Editing only that trailing space through the editor command is rejected as an append-only note change. Trim trailing whitespace at insertion or allow this normalization through the CLI.

## Notes

- 2026-09-30T09:53:58Z (main): scope: scoped; shared insertion normalization plus a narrowly defined explicit whitespace-only repair; s/mid/direct; brief: docs/notes/2026-09-30-note-integrity-brief.md
- 2026-09-30T14:40:30Z (main): started
  provenance: {"harness_session":"claude-code:d94a2e3c-5d2c-4340-8f21-b32c0525bded","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T14:40:31Z (fix/note-whitespace): resumed
  provenance: {"harness_session":"claude-code:d94a2e3c-5d2c-4340-8f21-b32c0525bded","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T14:44:36Z (fix/note-whitespace): done
  provenance: {"harness_session":"claude-code:d94a2e3c-5d2c-4340-8f21-b32c0525bded","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T14:44:36Z (fix/note-whitespace): every new note is stored without trailing spaces/tabs (CR/LF checked first, whitespace-only rejected); the editor accepts removing trailing whitespace from existing notes with count, order, stamps, authors, provenance and ledger entries unchanged; spec §3.2/§5.3 and the tasks skill updated
  provenance: {"harness_session":"claude-code:d94a2e3c-5d2c-4340-8f21-b32c0525bded","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
