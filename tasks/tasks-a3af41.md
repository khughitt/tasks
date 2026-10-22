---
id: tasks-a3af41
title: Cross-project document handoffs need a configured provider even when both projects are registered
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-22T22:35:22Z
created: 2026-10-22T20:36:45Z
updated: 2026-10-22T22:35:22Z
depends: []
tags: [feedback, gap, "from:ops"]
agent: codex
---

A required document reference on a newly filed follow-on was accepted, but tasks doc could not resolve its project scheme because no provider was configured on the host. The workaround is an immutable document attachment on each consuming task. Registered project paths are available, but cannot directly serve this cross-project document handoff without host configuration.

## Notes

- 2026-10-22T22:35:22Z (main): scope: drop; duplicate of tasks-b841bc and its existing planned design tasks-d09e87; checked 6698ca8: external refs are shape-validated at filing and read through host docs.schemes, while project-qualified references remain deferred by the implemented reference spec; registered projects do not configure document providers; the existing brief already covers native cross-project resolution and attachment-copy limits; brief: docs/notes/2026-10-19-reference-extensions-brief.md; proposal: drop tasks-a3af41 as covered by tasks-b841bc/tasks-d09e87
