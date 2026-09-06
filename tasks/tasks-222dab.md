---
id: tasks-222dab
title: "Provenance: record the model and key parameters behind a task's implementation"
status: idea
priority: 2
created: 2026-09-06T21:51:32Z
updated: 2026-09-06T21:51:32Z
depends: []
tags: [provenance, observability]
---

Capture which model was responsible for a task's implementation (e.g. claude-fable-5-1) plus key parameters, as observability/provenance on the task record. Today only owner (session identity) and note authorship are recorded. Decide where it lives (a field set at start/done, or a note convention), how the agent supplies it (env var like TASKS_SESSION?), and what counts as a key param. JSON shape change, so it needs a task before any field is added.
