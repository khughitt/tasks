---
id: tasks-2e7d03
title: "tasks add records no creating-session provenance and no requester/origin marker, so an agent picking up a task cannot find the session or ask that led to it"
status: idea
priority: 2
created: 2026-10-25T11:03:09Z
updated: 2026-10-25T11:03:09Z
depends: []
tags: [feedback, gap, "from:obs"]
agent: claude-code/claude-opus-5-5
---

Action: tracing which session created a task, and whether the user asked for it, to direct design questions correctly. Observed: add stamps only agent (harness/model) and an opaque source; no harness_session as start/park/done carry, no creation note, commit trailers absent, and the creating session may leave the record for another session's sweep commit, so commit time misleads. Workaround: grep session stores for the id and pick the rollout whose first hit matches created, then read the preceding human turn (about a minute by hand). Expected: add stamps the creating harness_session like lifecycle notes do, and optionally records whether the user's own prompt asked for the task versus an agent-originated filing (follow-up, feedback, decomposition, scope pass), generalizing the user-relayed marker already being designed for feedback.
