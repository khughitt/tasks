---
id: tasks-60cc95
title: tasks check reports unlinked_step for every heading of a plan whose goal was moved to another project
status: idea
priority: 2
created: 2026-10-25T17:19:48Z
updated: 2026-10-25T17:19:48Z
depends: []
tags: [feedback, friction, "from:ops"]
agent: claude-code/claude-opus-5-5
origin: agent
---

After tasks move carried a planned goal and its step children to another project, the source's moved tombstone kept its plan link, and tasks check in the source warned unlinked_step for each heading of the plan copy left behind (8 warnings), which blocks tools that require a silent check. The only route was deleting the source's plan copy. Deleting it in a worktree did not clear the warning; check still found the file through the main checkout. Expected: move leaves no warning behind (it flags or carries the plan copy, or check skips headings of a plan linked only by a moved tombstone), or the warning names the tombstone and the fix.

## Notes

- 2026-10-25T17:19:48Z (feedback): created
  provenance: {"harness_session":"claude-code:d825d1b2-617b-4da6-a70a-92adf0d4852f","harness_session_source":"CLAUDE_CODE_SESSION_ID","origin":"agent"}
