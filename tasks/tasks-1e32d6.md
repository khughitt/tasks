---
id: tasks-1e32d6
title: Trim trailing periods from CLI task-ID inputs
status: doing
priority: 2
size: s
complexity: low
process: direct
owner: main
created: 2026-10-01T16:30:37Z
updated: 2026-10-01T16:35:58Z
started: 2026-10-01T16:35:58Z
depends: []
tags: [cli]
agent: codex
---

Why: a task ID copied from the end of a sentence can include a trailing period. Today tasks show tasks-2578c3. fails with invalid_id.

Done:
- Strip one or more trailing ASCII periods from CLI task-ID inputs before strict validation and registry alias canonicalization.
- Apply this consistently to positional task IDs and ID-valued flags, including --parent, --depends, dep --on/--rm, and feedback --recur.
- Do not trim embedded dots, arbitrary punctuation, or whitespace; empty or otherwise malformed IDs still fail.
- Do not relax TaskId::parse, task-file/frontmatter validation, claim keys, filenames, or output IDs. Persist and emit canonical full IDs only.
- Full IDs with trailing periods work outside a project too. When shorthand support lands in tasks-dda2ea, period trimming composes before its local-prefix expansion.

Where to look: src/commands/mod.rs::parse_id and its callers, src/filter.rs::TaskFilter::parse, src/model.rs::TaskId::parse, src/complete.rs, tests/cli.rs. The shared CLI parser already separates user input from stored IDs, so this can be implemented independently of shorthand without a new parser or dependency.

Verification: focused CLI cases for one and multiple trailing periods, positional and flag inputs, outside-project routing, retired-prefix canonicalization, invalid punctuation, and strict on-disk rejection; run just test-one for these cases and just test-fast before committing. Update README.md and skills/tasks/SKILL.md with the accepted input rule.

Process: direct; the normalization rule and verification are settled. Related request split from tasks-dda2ea during scoping.

## Notes

- 2026-10-01T16:35:58Z (main): started
