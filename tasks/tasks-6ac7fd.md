---
id: tasks-6ac7fd
title: Design shared tag definitions from the tag vocabulary brief
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-10-02T15:13:22Z
updated: 2026-10-09T18:30:29Z
depends: []
parent: tasks-cea445
tags: []
agent: codex
---

Why: tasks-eb2b4e requests common tag meanings while preserving standalone use. Local dictionaries already work; authority, cross-project collisions, aliases, and offline behavior remain unresolved.

Where to start: docs/notes/2026-10-02-tag-vocabulary-brief.md; src/repo.rs::tag_dictionary, src/commands/tags.rs::meaning_of, src/commands/check.rs, src/config.rs, tests/cli.rs::tags_carry_the_dictionary_meaning_and_check_holds_open_work_to_it, and docs/specs/2026-09-11-tag-dictionary-design.md. Verify the linked incident reports and mindful's actual tag/export support during design; this scoping pass inspected neither.

Bound: Produce a design spec and then an implementation plan, each reviewed by the user. Compare rendering shared entries into existing project tables with a shared local dictionary; evaluate an optional mindful import before committing to a live provider or bidirectional synchronization. Settle precedence, local narrowing, rename/alias behavior, multi-project display conflicts, enforcement with no local table, and explicit missing/malformed-source behavior. Also settle whether optional per-tag styles belong in the vocabulary and how local/shared styles, tags without styles, conflicting cross-project styles and missing/malformed inputs behave. This covers the existing color request tasks-b49ef8; do not choose a separate style store or palette before resolving that contract. Do not implement the feature in this task.

Done: Reviewed spec and plan identify acceptance checks for standalone/local behavior, duplicate meanings, global-only entries, malformed inputs, offline use, feedback-generated tags, and unchanged tag filtering. File implementation children with explicit process and complexity under the cluster goal before closing.

Ideas it wakes: On completion, add the design finding and approved artifact paths to both tasks-eb2b4e and tasks-b49ef8 with tasks note in the same commit, and update docs/notes/2026-10-02-tag-vocabulary-brief.md.

## Notes

- 2026-10-03T16:55:30Z (main): consider an optional per-tag style (color) in the shared definition; tasks-b49ef8 wants tag colors sourced from the vocabulary, possibly from mindful
- 2026-10-08T10:16:45Z (main): Scope clarification: promote the existing optional-style note into the design boundary and add tasks-b49ef8 to Ideas it wakes; record style-source semantics before color rendering is scoped. Its rendering prerequisite is done, but the tags command bypasses present::tags, so acceptance must cover every requested view.
- 2026-10-09T18:30:29Z (main): Data point (2026-10-09): only 3 of 28 registered projects keep a [tags] table; quick-add and cross-project are filed into the others undefined. Both are candidates for shared definitions.
