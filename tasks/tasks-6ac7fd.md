---
id: tasks-6ac7fd
title: Design shared tag definitions from the tag vocabulary brief
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-10-02T15:13:22Z
updated: 2026-10-02T15:13:22Z
depends: []
parent: tasks-cea445
tags: []
agent: codex
---

Why: tasks-eb2b4e requests common tag meanings while preserving standalone use. Local dictionaries already work; authority, cross-project collisions, aliases, and offline behavior remain unresolved.

Where to start: docs/notes/2026-10-02-tag-vocabulary-brief.md; src/repo.rs::tag_dictionary, src/commands/tags.rs::meaning_of, src/commands/check.rs, src/config.rs, tests/cli.rs::tags_carry_the_dictionary_meaning_and_check_holds_open_work_to_it, and docs/specs/2026-09-11-tag-dictionary-design.md. Verify the linked incident reports and mindful's actual tag/export support during design; this scoping pass inspected neither.

Bound: Produce a design spec and then an implementation plan, each reviewed by the user. Compare rendering shared entries into existing project tables with a shared local dictionary; evaluate an optional mindful import before committing to a live provider or bidirectional synchronization. Settle precedence, local narrowing, rename/alias behavior, multi-project display conflicts, enforcement with no local table, and explicit missing/malformed-source behavior. Do not implement the feature in this task.

Done: Reviewed spec and plan identify acceptance checks for standalone/local behavior, duplicate meanings, global-only entries, malformed inputs, offline use, feedback-generated tags, and unchanged tag filtering. File implementation children with explicit process and complexity under the cluster goal before closing.

Ideas it wakes: On completion, add the design finding and approved artifact paths to tasks-eb2b4e with tasks note in the same commit, and update docs/notes/2026-10-02-tag-vocabulary-brief.md.
