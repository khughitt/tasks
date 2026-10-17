---
id: tasks-4fa71b
title: Say that a planned task keeps the written design under brainstorming's own sizing
status: done
priority: 2
size: xs
complexity: low
process: direct
owner: planned-sizing
created: 2026-10-17T11:38:22Z
updated: 2026-10-17T12:27:53Z
started: 2026-10-17T11:49:17Z
completed: 2026-10-17T12:27:53Z
depends: []
tags: []
source: lore-5b6cab
agent: claude-code/claude-opus-5-5
---

Why: superpowers v7.0.0 rebuilds brainstorming so it sizes the work itself after its conversation (do it now, agree a small change in chat, or write a design document), and when the user opts out of its questions 'the skill is done and the gate goes with it'. The tasks skill's Process and workspace section says the recorded process decides whether brainstorming runs, and that planned means a written design spec reviewed before the plan. Under v7 a planned task can end with no spec: brainstorming sizes it as a chat change, or the user skips the questions.

Change: in the Process and workspace section (planned bullet), say that under process: planned brainstorming's sizing step takes the written-design size and its questions opt-out does not waive the spec; when its conversation shows the work is smaller, record the evidence and change the process to direct with tasks edit before skipping the design, the mirror of the existing direct-to-planned rule. The With superpowers bullet for brainstorming needs no other change: --spec attachment and children are unchanged.

Evidence: lore docs/notes/2026-10-15-superpowers-v7-upgrade-brief.md (reference review, 2026-10-17); superpowers v7.0.0 skills/brainstorming/SKILL.md, sections Size the Work and HARD-GATE. Lands before or with lore's superpowers v7.0.0 upgrade, which depends on it.

## Notes

- 2026-10-17T11:49:17Z (main): started
  provenance: {"harness_session":"claude-code:0bf9fcb5-39fd-48a3-8c22-b689a52e7a3b","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T11:49:24Z (planned-sizing): resumed
  provenance: {"harness_session":"claude-code:0bf9fcb5-39fd-48a3-8c22-b689a52e7a3b","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T11:51:35Z (planned-sizing): parked (waiting on user, approval): On approval: tasks done on branch planned-sizing (.worktrees/planned-sizing, commit 48fcfac, test-fast 1496 passed), merge into main (live: ~/.claude/skills/tasks), remove the worktree.
  provenance: {"harness_session":"claude-code:0bf9fcb5-39fd-48a3-8c22-b689a52e7a3b","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T12:27:50Z (planned-sizing): resumed
  provenance: {"harness_session":"claude-code:0bf9fcb5-39fd-48a3-8c22-b689a52e7a3b","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T12:27:52Z (planned-sizing): approval: the user approved the merge 2026-10-17
- 2026-10-17T12:27:53Z (planned-sizing): done
  provenance: {"harness_session":"claude-code:0bf9fcb5-39fd-48a3-8c22-b689a52e7a3b","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T12:27:53Z (planned-sizing): Process and workspace's planned bullet now says brainstorming's own sizing takes the written design, skipping its questions does not waive the spec, and a smaller size needs the process changed to direct first (48fcfac).
  provenance: {"harness_session":"claude-code:0bf9fcb5-39fd-48a3-8c22-b689a52e7a3b","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
