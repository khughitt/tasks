---
id: tasks-479a6f
title: list --pretty gives no sign that a task is parked waiting for an idle host
status: idea
priority: 2
created: 2026-09-23T17:07:01Z
updated: 2026-09-23T17:07:01Z
depends: []
tags: [feedback, idea, "from:material"]
agent: claude-code/claude-opus-5-5
---

Parked-quiet work (park --reason quiet, with --needs and --minutes) shows up only in 'tasks quiet' and 'list --parked'. Plain 'tasks list --pretty' shows it like any other doing task. Suggest a column or a coloured tag in list/ready/prime pretty output, e.g. 'quiet idle 40m' or 'quiet headless 50m', so it stands out in the everyday views. Also consider having 'park --reason quiet' print a hint that 'tasks quiet' (across all projects) is the end-of-day queue, since neither the person nor the agent that parked it may know the command exists.
