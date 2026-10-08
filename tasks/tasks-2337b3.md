---
id: tasks-2337b3
title: "stale_copy 'same stamp, different content' fires after merging the main copy into a branch: the git conflict resolution keeps the newer updated stamp, so the merged superset ties and every write refuses with no recovery path except hand-editing updated"
status: idea
priority: 2
created: 2026-10-08T02:12:40Z
updated: 2026-10-08T09:43:56Z
depends: []
parent: tasks-984769
tags: [feedback, friction, "from:material"]
agent: claude-code/claude-opus-5-5
---

Merged main's record into a worktree branch (conflict on updated: + notes, resolved as union with main's stamp); next tasks note refused stale_copy same-stamp. Expected: a record whose notes are a superset of the other copy's is accepted, or the error names how to bump the stamp.

## Notes

- 2026-10-08T09:43:56Z (main): scope: drop; matched the source-stamped union already reproduced by tasks-29f71f (4095f53); lossless recovery integrates resolved bytes into equal-stamp siblings before CLI retry, without editing timestamps; brief: docs/notes/2026-10-06-record-continuity-brief.md; proposal: drop as covered by tasks-a20a64
