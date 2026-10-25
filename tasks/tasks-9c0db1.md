---
id: tasks-9c0db1
title: Close the provenance test gaps the reviews deferred
status: todo
priority: 2
complexity: low
process: direct
created: 2026-10-25T13:57:13Z
updated: 2026-10-25T13:57:13Z
depends: []
tags: []
source: tasks-053e89
agent: claude-code
---

Deferred review minors from goal tasks-053e89 (all behaviour verified by hand, coverage missing): tests/refs.rs editor_saves_validate_added_entries and the tests/external_docs.rs editor edit no longer exercise a record without a ## Notes section (legacy records); no CLI test that tasks check rejects an invalid origin or continuation key; occurrence_keys_reject_bad_forms asserts is_err() only, not which rule refused; no test of validate_task on an in-memory bad occurrence or of the JSON shape (origin after agent, note keys omitted when absent); no explicit harness_session-absent assertion under unknown identity with --origin; no test of a bad feedback --origin-session; no test that edit --editor with --origin/--no-origin is a usage error; pretty show note-header origin labels untested; recur_origin_follows_the_table misses (agent, agent) and (mixed, agent); resolve_root_from with equal ids untested; feedback_outside_projects_writes_creation_note does not assert from:unregistered; the recurrence test sleeps about 2.2s.
