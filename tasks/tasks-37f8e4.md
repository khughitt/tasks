---
id: tasks-37f8e4
title: Decide whether re-reviewing an idea clears awaiting_user
status: idea
priority: 2
created: 2026-10-17T17:45:11Z
updated: 2026-10-17T17:45:29Z
depends: []
tags: []
source: tasks-aeeef1
agent: claude-code/claude-opus-5-5
---

note --reviewed (Review::Set) on an idea already awaiting the user keeps the marker, so a re-scope that adds a new edge leaves the idea hidden after that edge closes: the failure the wait-aware spec §7.3 warns about, reached through the normal writer. Spec §3.1 lists what clears the marker and omits --reviewed, so the code is compliant; the question is whether Review::Set should clear it unless --awaiting-user rides along. Raised by the final whole-branch review; split out of tasks-d2cdfe because it needs a spec decision.

## Notes

- 2026-10-17T17:45:29Z (main): concerns: tasks-aeeef1 change — whether a re-review should clear awaiting_user, which the shipped rule keeps
