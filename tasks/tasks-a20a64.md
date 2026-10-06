---
id: tasks-a20a64
title: Clarify stale-copy recovery guidance for divergent Git merges
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-05T09:08:21Z
updated: 2026-10-06T11:17:16Z
depends: []
parent: tasks-984769
tags: [feedback, friction, "from:material"]
agent: codex
---

Original report: The stale-copy refusal recommends merging records, but a union of the newer notes remains rejected at the source stamp; the manual merge also needs a fresh merge-time stamp, which the message does not explain.

Finding: tasks-29f71f disproved the fresh-stamp requirement. Git conflict resolution can retain the authoritative header and both histories; integrating that resolved record into both equal-stamp siblings permits a CLI retry that advances updated itself. The attached disposable probe verifies this. A one-sided union remains refused, correctly.

Why: Same-stamp diagnostic text blames every fork on same-second writes and recommends only a one-sided merge, so an otherwise lossless resolution still leaves the writer blocked.

Done: Clarify the same-stamp refusal and recovery documentation: resolve divergent histories through Git, integrate the resolved record into the equal-stamp siblings before retrying in the intended checkout, and coordinate active owners. Explain that equal stamps with differing bytes can result from Git conflict resolution. Preserve the typed refusal, ownership and JSON contracts; no timestamp editing or bypass. Retain ordinary behind-only merge guidance.

Where: src/stale.rs::refusal and its formatter tests; tests/cli.rs stale-copy recovery cases; README.md, skills/tasks/SKILL.md, docs/specs/2026-09-30-record-home-design.md. Evidence: tasks-29f71f attachments and docs/notes/2026-10-06-record-continuity-brief.md.

Verification: Exercise a CLI-authored divergent Git fixture: one-sided source-stamped union refuses without writes, identical resolved siblings permit a retry, and both histories remain intact. Check the diagnostic without changing stale-copy selection. Use just test-one filters and just test-fast before committing code.

## Notes

- 2026-10-06T09:45:48Z (main): scope: briefed; grouped record visibility and recovery evidence under tasks-984769; bounded reproductions precede a routing or recovery change; brief: docs/notes/2026-10-06-record-continuity-brief.md
- 2026-10-06T11:17:14Z (docs/tasks-29f71f): Finding from tasks-29f71f: source-stamped union correctly refuses against different sibling bytes; integrating the lossless resolved union into both siblings permits CLI retry and advances updated without hand-edited timestamps.
- 2026-10-06T11:17:14Z (docs/tasks-29f71f): scope: scoped; tasks-29f71f established existing lossless recovery; narrow diagnostic/documentation fix with a concrete Git regression, no new recovery mechanism; brief: docs/notes/2026-10-06-record-continuity-brief.md
