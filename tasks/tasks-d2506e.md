---
id: tasks-d2506e
title: completion_offers_task_ids_open_first_with_descriptions flakes 1 run in 16
status: todo
priority: 1
size: xs
created: 2026-09-06T10:21:25Z
updated: 2026-09-06T10:21:25Z
depends: []
tags: [testing]
---

tests/cli.rs asserts that completing the 5-char prefix `&open[..5]` (i.e. "sci-<first hex digit>") returns exactly the open id. Both ids in the fixture are random 6-hex, so they share that first digit 1/16 of the time and the closed id matches the prefix too, failing the assertion. Observed twice in four full-suite runs on 2026-09-06: sci-71ea14/sci-76dca7 and sci-61cd49/sci-6d0bae. Predates the parallel-candidates branch; introduced by d7a9806. Fix by making the fixture ids deterministic or by asserting on a prefix long enough to be unique (or by asserting the open id is present rather than sole).
