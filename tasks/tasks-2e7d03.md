---
id: tasks-2e7d03
title: "tasks add records no creating-session provenance and no requester/origin marker, so an agent picking up a task cannot find the session or ask that led to it"
status: idea
priority: 2
created: 2026-10-25T11:03:09Z
updated: 2026-10-25T11:13:05Z
depends: []
tags: [feedback, gap, "from:obs"]
agent: claude-code/claude-opus-5-5
---

Action: tracing which session created a task, and whether the user asked for it, to direct design questions correctly. Observed: add stamps only agent (harness/model) and an opaque source; no harness_session as start/park/done carry, no creation note, commit trailers absent, and the creating session may leave the record for another session's sweep commit, so commit time misleads. Workaround: grep session stores for the id and pick the rollout whose first hit matches created, then read the preceding human turn (about a minute by hand). Expected: add stamps the creating harness_session like lifecycle notes do, and optionally records whether the user's own prompt asked for the task versus an agent-originated filing (follow-up, feedback, decomposition, scope pass), generalizing the user-relayed marker already being designed for feedback.

Scope evidence (2026-10-25, 79b8253): the creation/session gap is confirmed in src/commands/add.rs and commands::create; native provenance already exists for stamped notes. The original report above is retained. [ref:brief] carries the ordinary-add requirements alongside the existing feedback design, including the distinction between filer identity and user intent. Reuse tasks-a58a90 to settle the shared representation and optional requester/origin vocabulary before implementation; its existing feedback requirements remain intact. This idea waits on that design, not on a duplicate research task.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-20-feedback-ask-preservation-brief.md — shared creation provenance and requester/origin questions
