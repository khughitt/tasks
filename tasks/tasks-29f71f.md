---
id: tasks-29f71f
title: Establish a supported recovery from merged stale task records
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-06T09:45:48Z
updated: 2026-10-06T09:45:49Z
depends: []
parent: tasks-984769
tags: []
source: tasks-a20a64
agent: codex
---

Question: Which exact merge operation makes a divergent record writable without losing either note history or bypassing stale-copy checks?

Where to start: docs/notes/2026-10-06-record-continuity-brief.md; src/stale.rs::compare/refusal, src/commands/mod.rs::load/refuse_stale_copy, src/time.rs::after and record-home spec sections 3.1-3.2.

Bound: one disposable git main/worktree pair with isolated config/state; use CLI notes to create divergence, compare normal git integration of the authoritative record with a union merge retaining the source stamp, and record the retry result. Task files are written only by the CLI or git merge/conflict resolution under the repository protocol; do not prescribe a hand-edited timestamp, add an override or change product code.

Expected result: a reproducible, lossless supported recovery and precise diagnostic/doc recommendation, or a concrete contract gap requiring reviewed design; update the brief.

Ideas it wakes: on completion add the finding to tasks-a20a64 with tasks note in the same commit, update the brief, then rerun scope on them.

## Notes

- 2026-10-06T09:45:48Z (main): concerns: tasks-9949f3 defect — Reported merge recovery can remain refused despite preserving the newer notes; reproduce the diagnostic gap before prescribing a fix.
