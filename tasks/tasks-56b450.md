---
id: tasks-56b450
title: Decide where claim-guard's claim policy lives; read the wake facts from harness support
status: todo
priority: 2
size: s
complexity: mid
process: planned
created: 2026-10-04T10:54:36Z
updated: 2026-10-06T10:07:12Z
depends: []
tags: []
source: ops-cb9749
agent: claude-code
---

Why: ops hooks/claim-guard decides whether a turn may stop given live claims and running children (tasks policy) and carries CHILD_WAKES, WAKING and IN_FLIGHT (capability facts). The agent layer split (ops docs/specs/2026-10-03-agent-layer-split-design.md §4.4, §5.3) dissolves hooks to their owners and leaves this decision to tasks.

Done: a recorded decision whether the claim rule moves into tasks or stays in ops until tasks takes it, and the facts read from harness support's consolidated view rather than the constants in the hook, in whichever order tasks chooses.

## Notes

- 2026-10-06T10:07:12Z (investigate/tasks-1ece46): tasks-1ece46 capture separates worker claim identity from controller Stop aggregation: current hook matches only own live Stop session; pure injected-event check blocks controller on own claim and allows child with dead claim. Identity repair alone will not aggregate separately owned child claims; reuse this planned policy task. Evidence: docs/notes/2026-09-30-delegated-identity-brief.md.
