---
id: tasks-adf58d
title: "A recurring task cannot be given a first due date: add refuses --defer together with --every"
status: idea
priority: 2
created: 2026-10-26T14:47:22Z
updated: 2026-10-26T15:35:17Z
depends: []
parent: tasks-46d207
tags: [feedback, gap, "from:obs"]
agent: claude-code/claude-opus-5-5
origin: agent
---

Action: filing a monthly reassessment task whose first check should fall a month from now, using add --every 30d --defer <date>. The add was refused because defer and every cannot both be set, and a new recurrence is ready immediately. Expected a way to start a recurrence on a chosen date. Workaround: a plain deferred task whose body tells each occurrence to reset its status and re-defer by 30 days, in two commands.

## Notes

- 2026-10-26T14:47:22Z (feedback): created
  provenance: {"harness_session":"claude-code:b0f7fde8-95d2-4d12-a287-5daf820d7b6b","harness_session_source":"CLAUDE_CODE_SESSION_ID","origin":"agent"}
