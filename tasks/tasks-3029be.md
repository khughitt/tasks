---
id: tasks-3029be
title: "Multi-project support: registry-wide views, hub goals, cross-project add"
status: todo
priority: 2
size: l
created: 2026-09-04T12:48:03Z
updated: 2026-09-04T16:24:02Z
depends: []
tags: [multi-project]
spec: docs/specs/2026-09-04-multi-project-design.md
plan: docs/plans/2026-09-04-multi-project.md
---

Cross-project awareness on top of the existing registry. Candidates: --all scope on read-only commands (list/ready/prime/tree/show) with a project field per JSON row; tasks next (head of ready, per project and across projects); tasks root <id> so a dashboard or shell alias can jump to a task; tasks add --project <prefix> as the general form of feedback; tags --all frequency table before any shared vocabulary. Cross-cutting work is a goal in a hub project with one child per affected project linked by existing cross-project deps; foreign parents only if the tree view is missed. Decide where the hub lives (this repo vs a small separate ops project). Dashboard is a separate repo consuming the JSON.

## Notes

- 2026-09-04T16:03:30Z (multi-project): multi-project support landed
