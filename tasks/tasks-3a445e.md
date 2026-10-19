---
id: tasks-3a445e
title: "Design task references: one explicit, ordered list of the documents and artifacts a task cites"
status: doing
priority: 2
size: m
complexity: high
process: planned
owner: task-references
created: 2026-10-19T11:01:01Z
updated: 2026-10-19T11:04:06Z
started: 2026-10-19T11:03:39Z
depends: []
tags: []
source: "user:2026-10-19 task references discussion"
agent: claude-code
---

Why: a task's documents are scattered across spec, plan, source, attachments and prose. Prose citations dominate: about 2,800 docs/plans, 1,700 docs/specs and 1,700 docs/notes mentions across project records, with 156 hand-written 'Where:' sections (count 2026-10-19). None can be queried, checked, or rewritten when a document moves, and an agent picking up a task has no single answer to 'what must I read first'. Example: crew-5aa683's Where: section.

Settled in discussion (2026-10-19, with a codex review):
- Entries are TaskReference: handle, ref, relationship (context | input | output), optional kind, optional locator (section), description. Kind is descriptive only; no type dictionary or filename inference until something behaves differently by kind. Location (repo path, attachment, URL, external scheme, task id) is its own dimension, not a kind.
- spec and plan keep their fields and workflow roles (phase, step checks, list --plan, doc); source stays opaque provenance (its duplicate key is source plus title). A referenced spec is not the task's governing spec.
- Inventory is not required reading: an explicit required-review marker, separate from relationship, selects the reading list; entry order is reading order. Sources and attachments appear in the inventory, not automatically in the reading list. JSON exposes the same effective list as pretty output.
- Prose cites with a reserved form ([ref:<handle>], optionally with a section); the reference stores the location and the sentence says why it matters. check errors on a citation without an entry. No routine warning for raw paths in prose; promotion suggestions belong to curate or an explicit audit.
- Attachments are referenced, not copied. Locating and reading are separate: the resolver reads UTF-8 bodies (Resolver::read), so binary artifacts resolve to a location, and reading text is a separate capability.
- Code locations are out of scope; they get their own design (path plus symbol, drift).

Open for the spec: storage (a frontmatter extension, which today holds only scalars and flat string lists, versus a second binary-managed body section after the '## Notes' precedent and its reserved-heading cost, including edit --body); expected versus produced outputs, so an unwritten output does not fail an input's existence check; mutable path versus pinned revision for evidence and reviews; validation that reports unavailable and unchecked references distinctly without fetching on list; the resolution contract for which project/root and which version a reference selects.

First increment: explicit references, an ordered required-reading list, one JSON and pretty view that includes the existing spec and plan roles, and deliberate retrieval (doc by handle). Richer artifact handling follows when needed.

Coordinate with tasks-dc16eb's decision on document roots without waiting for its relocation rollout; branch-only and linked-checkout resolution remain open in tasks-a73dce, tasks-82bd08 and tasks-6360ac.

Done: a reviewed design spec and an implementation plan for the first increment.

## Notes

- 2026-10-19T11:01:06Z (main): proposal reviewed before filing by codex/gpt-6-astra (relayed by the user): adopted inventory vs required reading, separated dimensions, [ref:] citations, locate vs read, expected vs produced outputs, revision pinning; storage left open for the spec
- 2026-10-19T11:03:39Z (main): started
  provenance: {"harness_session":"claude-code:c665bb98-6b0b-4918-9280-20b2b8db99f7","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-19T11:04:06Z (task-references): resumed
  provenance: {"harness_session":"claude-code:c665bb98-6b0b-4918-9280-20b2b8db99f7","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
