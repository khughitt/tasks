---
id: tasks-44ec1a
title: Trim trailing commas from CLI task-ID inputs
status: todo
priority: 2
size: xs
complexity: low
process: direct
created: 2026-10-15T17:41:58Z
updated: 2026-10-15T17:41:58Z
depends: []
tags: [cli]
source: tasks-1e32d6
agent: claude-code/claude-opus-5-5
---

Why: an id copied from a list or the middle of a sentence carries a trailing comma (`tasks show tasks-16a48c,`), and double-click selection in zsh and other apps takes it along. tasks-1e32d6 trimmed trailing periods and deliberately left commas rejected; this extends the same input rule.

Done:
- TaskId input parsing strips any trailing run of ASCII periods and commas, in any mix (`id,` `id.` `id.,` `id,,`), before validation and alias canonicalization. One helper owns the trim; the bare-suffix path in src/shorthand.rs uses it instead of its own trim_end_matches('.').
- Every positional id and id-valued flag inherits it through parse_id; no id flag splits on commas (no clap value_delimiter), so nothing collides.
- Embedded punctuation, leading punctuation, whitespace and other trailing punctuation stay invalid_id. On-disk ids, TaskId::parse and output stay strict and canonical.

Where to look: src/model.rs TaskId::parse_input and its unit test (which currently pins `tasks-2578c3,` as rejected), src/shorthand.rs Shorthand::parse, tests/cli.rs trailing_periods_are_trimmed_from_cli_id_inputs (asserts `bogus,` fails; keep that, it is not an id).

Verification: unit cases for comma and mixed runs; CLI cases for a full id, a bare suffix and a flag with a trailing comma; just test-one for these, just test-fast before commit. Update README.md and skills/tasks/SKILL.md's input rule.
