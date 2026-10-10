---
id: tasks-d6f0ed
title: Clarify that automatic record commits exclude configuration writes
status: todo
priority: 3
size: xs
complexity: low
process: direct
created: 2026-10-10T17:51:21Z
updated: 2026-10-10T17:51:21Z
depends: []
tags: [docs]
source: "tasks-6939f6:final-review:M2"
agent: codex
---

Deferred final-review Minor M2 from tasks-6939f6: README and the tasks skill include configuration in the automatic-write commit list, but the journal records task records and attachment changes, and project administration has no automatic commit verb. Remove configuration from the automatic list or explicitly assign it to the tasks commit backlog sweep. Verify both prose surfaces against the engine.
