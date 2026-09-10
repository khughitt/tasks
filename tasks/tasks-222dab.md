---
id: tasks-222dab
title: "Provenance: record the model and key parameters behind a task's implementation"
status: doing
priority: 2
owner: main
created: 2026-09-06T21:51:32Z
updated: 2026-09-10T12:11:06Z
depends: []
tags: [provenance, observability]
source: "mindful:thought:455894a15b734fdf93dec0d9d84e20ab"
spec: docs/specs/2026-09-10-model-provenance-design.md
plan: docs/plans/2026-09-10-model-provenance.md
---

Capture which model was responsible for a task's implementation (e.g. claude-fable-5-1) plus key parameters, as observability/provenance on the task record. Today only owner (session identity) and note authorship are recorded. Decide where it lives (a field set at start/done, or a note convention), how the agent supplies it (env var like TASKS_SESSION?), and what counts as a key param. JSON shape change, so it needs a task before any field is added.

## Notes

- 2026-09-07T08:29:07Z (main): 2026-09-07: the source field (docs/specs/2026-09-06-task-source-design.md) is the precedent for an opaque, uninterpreted string on the record; model-id provenance likely takes the same shape (set by the agent at start/done, never resolved by tasks).
- 2026-09-10T10:32:11Z (main): scoped with user: one optional model field, stamped only by done, supplied only via TASKS_MODEL env (absent = no stamp, silent); corrections via edit --model/--no-model; additive JSON key incl. summary rows; deferred: list --model filter, params, stamping start/notes
- 2026-09-10T10:42:08Z (main): parked (waiting on user): spec docs/specs/2026-09-10-model-provenance-design.md awaiting user review; then writing-plans, then implement
- 2026-09-10T11:13:39Z (main): spec review round 1: recovery test now plants a lingering claim/park and retries under C; completion-stamps-last precedence over same-invocation --model/--no-model; query example uses list --status done; tests/common builders will scrub TASKS_MODEL
- 2026-09-10T11:13:47Z (main): parked (waiting on user): spec revised after review round 1; awaiting user re-review, then writing-plans
