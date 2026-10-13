---
id: tasks-ec3ff9
title: "sample draws uniformly, so a pass spends draws on tasks a scope pass briefed days ago while long-untouched tasks wait"
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-08T20:35:39Z
updated: 2026-10-13T09:55:49Z
depends: []
parent: tasks-5408fe
tags: [feedback, idea, "from:ops"]
agent: claude-code/claude-opus-5-5
---

Why: sample draws uniformly from the pool, so a pass spends draws on ideas a scope pass briefed a week or two ago while tasks untouched for weeks wait.
Ruling (user, 2026-10-13): weight the draw by time since last activity.
Done: src/commands/sample.rs draws without replacement with each pool member's weight proportional to the time since its `updated` stamp (the same last activity list rows show), in whole days with a floor so a task at the window edge keeps a nonzero chance. The pool definition, warnings, seeded determinism, `--limit`/`--older-than` and the JSON shape stay unchanged. The module doc, `tasks sample --help`, skills/curate/SKILL.md (if it says uniform), and docs/specs/2026-09-08-task-curation-design.md (selection rule, and the non-goal that deferred weighting) describe the new rule. With `--older-than 0d` a future-dated record gets the floor weight, not an error.
Check: a seeded test over a fixture with one long-untouched and several recent tasks shows the old one drawn far more often across seeds; existing sample tests stay green under `just test-fast`; reinstall with `cargo install --path .`.
Where to look: docs/notes/2026-10-13-review-pass-followthrough-brief.md; src/commands/sample.rs (partial Fisher-Yates at the draw); tests/cli.rs sample tests; docs/specs/2026-09-08-task-curation-design.md.

Original report: In a pass of 5 (small sample), the two tasks untouched for five weeks yielded a stale count, a question that is still open, and an unrecorded agenda; the three briefed 9 days earlier mostly yielded a restatement of the brief. Options: weight the draw by age since last update or review, or treat a scope note like a curate note for the age window.

## Notes

- 2026-10-13T09:21:13Z (main): scope: briefed; parented under goal tasks-5408fe; brief compares an age-weighted draw (lean), a longer window for reviewed ideas, and a wider default --older-than; waits on the user's ruling; brief: docs/notes/2026-10-13-review-pass-followthrough-brief.md
