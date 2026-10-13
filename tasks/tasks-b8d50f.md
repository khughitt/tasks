---
id: tasks-b8d50f
title: Refuse private values at write time so an append-only note or park step never records one
status: todo
priority: 2
size: s
complexity: mid
process: planned
created: 2026-10-13T18:30:32Z
updated: 2026-10-13T18:30:33Z
depends: [ops-554eb9]
tags: []
source: relay-4dfc10 tailnet hostname leak (2026-10-13)
agent: claude-code/claude-opus-5-5
---

Why: a commit-time refusal (ops-554eb9) arrives only after `tasks add/note/park/done` has already written the record. Notes are append-only, so the only recovery is to revert the uncommitted record file. That loses the rest of the write, and for `park` it leaves the shared-store park without its note.

The 2026-10-13 leak happened exactly this way: a park's next-step text carried a host's tailnet DNS name into a committed record. Removing it took a history rewrite.

Approach: before writing, check every new free-text field against the same host-local private-value set that ops-554eb9 defines:
- titles and bodies
- notes, including `--stamp` notes
- park next steps
- done and drop messages
- feedback

On a match, refuse with a typed error that names the value class and the placeholder to use. Never auto-replace. Read the value set through the contract ops-554eb9 settles, rather than inventing a second list.

This prevents writing such text. tasks-1f6d08 designs redaction of text already written, so reference that design and do not duplicate it.

Done:
- Each write path refuses a private value and leaves the record and the shared store unchanged.
- A generic fixture name passes.
- When no private-value file exists, nothing is refused.
- The README documents the error.
