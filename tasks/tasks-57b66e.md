---
id: tasks-57b66e
title: "stale_copy tells me to merge another checkout's copy of a record by hand, but notes are append-only and the stamp check refuses a merged copy with the same stamp, so the merge needs a hand-bumped updated field; a merge command (or accepting a superset copy) would avoid editing records directly"
status: dropped
priority: 2
created: 2026-10-10T13:27:34Z
updated: 2026-10-13T10:20:17Z
depends: []
tags: [feedback, gap, "from:ops"]
agent: claude-code/claude-fable-5-1
---

## Notes

- 2026-10-13T10:12:46Z (main): scope: drop; covered: tasks-29f71f (4095f53) established that no hand-bumped updated stamp is needed: resolve divergent histories through Git, integrate the resolved record into both equal-stamp siblings, and a CLI retry advances the stamp itself; it also found a one-sided superset copy is correctly refused; tasks-a20a64 (todo) fixes the stale_copy message and docs that misdirected this report, with no new recovery mechanism or bypass; proposal: close as covered by tasks-a20a64 with tasks-29f71f's finding
- 2026-10-13T10:20:17Z (main): dropped
  provenance: {"harness_session":"claude-code:c8f37d6a-e93e-42ac-86c6-6c700be1db4b","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-13T10:20:17Z (main): Accepted by the user: covered by tasks-a20a64 with tasks-29f71f's finding (4095f53); no hand-bumped stamp is needed for a Git-resolved record
  provenance: {"harness_session":"claude-code:c8f37d6a-e93e-42ac-86c6-6c700be1db4b","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
