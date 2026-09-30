---
id: tasks-cff04e
title: "A write in the same second as its sibling's leaves an equal stamp, so the documented start-in-main-then-worktree sequence forks the record"
status: done
priority: 2
size: s
complexity: low
process: direct
owner: fix/same-second-stamp
created: 2026-09-30T14:19:22Z
updated: 2026-09-30T14:32:54Z
started: 2026-09-30T14:31:14Z
completed: 2026-09-30T14:32:54Z
depends: []
parent: tasks-c4ad8e
tags: [feedback, friction, "from:tack"]
model: claude-opus-5-5
agent: claude-code/claude-opus-5-5
---

Cause: save stamps updated = now() at second precision. start in main at T, commit, git worktree add, start in the worktree at T: both copies carry stamp T with different bytes, and record-home §3.1's same-stamp rule refuses every later write from the worktree as a same-second fork, although the worktree copy strictly extends main's.

Fix: a write stamps strictly after the stamp it loaded: updated = max(now, loaded + 1s). The §3.1 check has already proved no sibling is newer than the loaded copy (under the mutation lock), so the written copy becomes the unique newest and the stale copy refuses by the ordinary newer-stamp rule. Covers save and the feedback recurrence path (check whether its same-second keep still holds). Amend record-home spec §3.1's claim that every write sets a new stamp.

Verification: an integration test that runs start in main, worktree add, start in the worktree within one second (or with a frozen clock if the test harness has one), then note from the worktree succeeds and a note from main refuses stale_copy with the handoff remedy. just test-fast.

Original report: followed the documented sequence (start, commit, worktree add, start in worktree) in one shell command; every later note from the worktree refused with stale_copy until the worktree copy was reset to main's, losing the resumed note.

## Notes

- 2026-09-30T14:31:07Z (main): concerns: tasks-9949f3 defect — same-second writes in two checkouts leave equal stamps, so the record-home stale_copy refusal fires on the documented start-then-worktree sequence
- 2026-09-30T14:31:07Z (main): scope: scoped; cause traced to second-precision stamps in save; fix is a strictly increasing stamp per write; P2 s/low/direct under the cross-checkout goal, which stays open until it lands
- 2026-09-30T14:31:14Z (main): started
  provenance: {"harness_session":"claude-code:4471e860-d17f-47d3-a634-479664d60702","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T14:31:16Z (fix/same-second-stamp): resumed
  provenance: {"harness_session":"claude-code:4471e860-d17f-47d3-a634-479664d60702","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T14:32:54Z (fix/same-second-stamp): done
  provenance: {"harness_session":"claude-code:4471e860-d17f-47d3-a634-479664d60702","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T14:32:54Z (fix/same-second-stamp): Writes stamp strictly after the stamp they loaded (time::after: now, or loaded + 1s), in save and the feedback recurrence, so a same-second write in a second checkout leaves its copy the unique newest instead of an equal-stamp fork; record-home spec §3.1 amended; regression test covers start in main, branch, start in the worktree in the same second.
  provenance: {"harness_session":"claude-code:4471e860-d17f-47d3-a634-479664d60702","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
