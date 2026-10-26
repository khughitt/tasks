---
id: tasks-71c74d
title: Design early dependency events and confirmed-wait recording
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-10-25T21:37:44Z
updated: 2026-10-26T01:55:48Z
depends: []
parent: tasks-6166d6
tags: []
source: tasks-e637a1
agent: codex
origin: agent
---

Why: blocking duration needs persisted dependency history or reconstruction, while repeated scope confirmation can be collected independently of edge age. [ref:brief] separates these evidence writes from the downstream report. Prospective coverage begins only when collection ships, so design and ship collection early without waiting for fan-out reporting or optional backfill.

Done: prepare a written design spec for user review, then an implementation plan for user review. The first shipping milestone records forward dependency additions/removals and scope-confirmed waits; a persistence-report milestone then surfaces supported blocking intervals and confirmed-pass counts with explicit coverage. Only git reconstruction is optional backfill; neither it nor the later report may delay collection. Define versioned evidence, coverage start, atomicity with edge writes, no-op/failed writes, every add/edit/dep/clear/replace path, removal/re-addition, reopening/recurrence, and alias/move continuity. Separate edge age from continuously unmet age; leave unobserved history unknown.

Define a fixed scope-note observation such as waits: <canonical blocker ids>, recorded only for waits that the pass actually checked. Count distinct confirmed passes and their first/last observation; do not equate them with uninterrupted blocking duration or count repeated notes as separate passes. Settle pass identity/deduplication and changing or closed waits. Scope observations can ship without historical edge reconstruction. Keep changes to skills/scope/SKILL.md and any required parser/model contract explicit in the plan.

Verification: additions through add/edit/dep, removals/clear/replace, failed and no-op mutations, re-add and reopen intervals, recurrence, aliases/moves, absent pre-deployment evidence, and two distinct scope confirmations plus a duplicate/unrelated note. Confirm count/coverage survives normal persistence. No implementation is authorized by this scope revision.

Where to look: [ref:brief]; src/commands/dep.rs (add_dependencies), src/commands/mod.rs (shared field changes), src/commands/add.rs, src/commands/edit.rs, src/model.rs, src/frontmatter.rs, src/format.rs, src/rename/, src/moving/, src/review.rs, skills/scope/SKILL.md and tests/cli.rs. Coordinate the later report with tasks-e637a1, without a dependency between the two design tracks.

Closeout: record the design/plan result on this task and in the brief. File accepted implementation steps under this task so tasks-6166d6 closes only after the actual history/observation deliverables are shipped. There is no waiting idea to wake after this revision.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-25-blocker-ranking-brief.md — evidence, alternatives, metric questions and verification cases

## Notes

- 2026-10-25T21:37:44Z (add): created
  provenance: {"harness_session":"codex:01a174fb-0fa0-7571-93ce-b3218da174d1","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
- 2026-10-25T22:02:32Z (main): scope: rescoped; split the combined design from 7085e03 into P2/m/high/planned early dependency-event and confirmed-wait recording; collection first, independently of report/backfill, then supported duration/reconfirmation reporting; cover all mutation paths and interval/identity changes, distinct checked scope passes, unknown coverage and optional git reconstruction; replaced obsolete Ideas it wakes because tasks-e637a1 is now a todo sibling, not a waiting idea; code premises rechecked against 7085e03; brief: docs/notes/2026-10-25-blocker-ranking-brief.md
  provenance: {"harness_session":"codex:01a174fb-0fa0-7571-93ce-b3218da174d1","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-26T01:55:48Z (main): tasks blockers shipped (merge 074672f) with blocking-since and confirmed-pass metrics reported as unknown and a permanent warning saying so. When this collection lands, the report needs those fields (spec d2940d0 forbids zero-filled placeholders), and the permanent warning should be reconsidered: the final reviewer noted it duplicates the pretty footer on every run.
