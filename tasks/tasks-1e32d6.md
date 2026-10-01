---
id: tasks-1e32d6
title: Trim trailing periods from CLI task-ID inputs
status: done
priority: 2
size: s
complexity: low
process: direct
owner: trim-period
created: 2026-10-01T16:30:37Z
updated: 2026-10-01T16:57:41Z
started: 2026-10-01T16:35:58Z
completed: 2026-10-01T16:47:32Z
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
- 2026-10-01T16:36:19Z (trim-period): resumed
- 2026-10-01T16:36:19Z (trim-period): took over session sid:2486550 (owner main, stale)
- 2026-10-01T16:47:32Z (trim-period): done
- 2026-10-01T16:47:32Z (trim-period): CLI id inputs now strip trailing sentence periods: TaskId::parse_input trims trailing ASCII dots before validation and alias canonicalization, wired through parse_id (all positional ids and id-valued flags, including --parent, --depends, dep --on/--rm, feedback --recur, list --parent) and completion's subject parse. On-disk ids, TaskId::parse, and outputs stay strict and canonical. Integration test covers single/multiple periods, flags, outside-project routing, retired aliases, rejected punctuation, and on-disk rejection; README and skills/tasks/SKILL.md document the rule.
- 2026-10-01T16:57:41Z (trim-period): review: impl round 1 — verdict: accept; findings: none; reviewer: codex
