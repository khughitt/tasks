---
id: tasks-1675ac
title: Dependency checks flag an intentionally shelved future goal stage
status: idea
priority: 2
created: 2026-10-22T21:18:57Z
updated: 2026-10-22T21:18:57Z
depends: []
tags: [feedback, friction, "from:ops"]
agent: codex
---

A goal deliberately depends on a later stage that is shelved until an earlier prerequisite produces evidence. The shelving note explicitly retains the goal dependency. Validation reports shelved_dep and tells the caller to unshelve or drop the dependency, but either would undo the recorded sequencing decision. Consider distinguishing this intentional future-stage case or giving a disposition route; the observed warning is retained rather than changing another goal.
