---
id: tasks-2942cb
title: "dep --on a retired-prefix id is a silent no-op when the task already depends on it, so repointing a dependency to the renamed prefix by --on then --rm drops it entirely"
status: todo
priority: 2
size: s
complexity: low
process: direct
created: 2026-09-29T20:37:17Z
updated: 2026-10-02T14:22:42Z
depends: []
parent: tasks-671956
tags: [feedback, friction, "from:tasks"]
agent: claude-code/claude-sonnet-5-5
---

Why: dep --on silently deduplicates canonical task identities. An agent mistook that success for a new edge, then used --rm with the old spelling and deleted the existing relationship. Alias equality and preservation of stored spelling are deliberate contracts, not separate dependencies.

Done: When an addition names an already-present canonical dependency, keep successful idempotent behavior and add a warning through the existing warnings array. Name the supplied, stored, and canonical IDs where useful, and explain that --rm with either spelling removes the same relationship; an add/remove pair is not a prefix rewrite. Keep one dependency, preserve stored spelling, and leave the JSON shape unchanged. Document the identity rule and the warning in the README and tasks skill. Do not add a reference-migration command in this task.

Where to look: src/commands/dep.rs::run, Ctx.warnings and id_out in src/commands/mod.rs, src/registry.rs::canonical_id, tests/cli.rs::a_retired_id_is_one_task_for_routing_dedup_and_removal and stored_retired_references_resolve_detect_cycles_and_keep_their_spelling. The governing contract is docs/specs/2026-09-08-prefix-rename-design.md section 3; canonical comparisons landed in 1e7b9bc.

Verification: Extend the CLI checks for an already-present live ID and alias-equivalent ID, including a stored retired spelling. Assert successful output contains the explanation, exactly one dependency remains, and its spelling is retained. Retain alias removal and cycle checks. Run just test-one --test cli <filter>, then just test-fast.

Original report: tasks dep <id> --on tack-d5a56c then tasks dep <id> --rm ai-d5a56c, where ai is a retired alias of tack: the first call reports no warning and changes nothing because both ids resolve to one task, and the second removes the only copy. Expected: --on reports that the dependency already exists, or check's retired_prefix warning names a one-step repoint.

## Notes

- 2026-10-02T14:22:40Z (main): scope: scoped; warn on existing canonical dependencies while preserving idempotence and stored spelling; P2/s/low/direct; original report preserved
