---
id: tasks-476c6b
title: prime shows live claims on tasks that exist only in a worktree
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-09-25T02:36:08Z
updated: 2026-09-29T20:50:25Z
depends: []
parent: tasks-c4ad8e
tags: []
source: ai-21ea5d
agent: claude-code/claude-opus-5-5
---

Why: prime builds doing from the local scan only (src/commands/list.rs, prime: all.iter().filter(status == Doing || claims.live)), so a live claim on a task whose record exists only on a worktree branch is dropped with no warning, from plain prime and from --all-projects. Found in the ai turn-boundary-gate spec review 2026-09-24 and reproduced live (ai-2f1271); source tack-21ea5d (formerly ai-21ea5d). tasks claims already answers the claim question; this is about prime's other readers.

Done: for each live claim whose id is absent from the local scan, prime resolves the record by scanning claim.worktree exactly as park design §5.3 resolves a store-only park (reuse or generalise resolve_recorded in src/commands/parked.rs: same prefix check, unavailable on a missing checkout or file) and lists it under doing, warning '<id> is claimed in <worktree>; resume it from that checkout'. An unresolvable claim still warns ('<id> is claimed in <worktree>, which is unavailable'), never silent. No JSON shape change: rows are TaskSummary, which already carries the claim's worktree. Store-only claims are never next/ready candidates (park §5.3 rule).

Check: an integration test in tests/cli.rs mirroring park's 'park a task that exists only in a new worktree, inspect main' case, but with start instead of park: prime in main lists it under doing with the warning; remove the worktree and it warns unavailable. Update the park or work-claims design doc where it states the §5.3 rule. Brief: docs/notes/2026-09-29-cross-checkout-records-brief.md.

## Notes

- 2026-09-29T20:50:25Z (main): scope: scoped; todo P2 s/mid/direct, body rewritten with the §5.3 resolution approach and test, parented to tasks-c4ad8e; brief: docs/notes/2026-09-29-cross-checkout-records-brief.md
