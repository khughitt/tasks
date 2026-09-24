---
id: tasks-f17488
title: Spec references cannot attach an existing design document stored under docs/plans
status: idea
priority: 2
created: 2026-09-11T12:17:29Z
updated: 2026-09-24T13:07:13Z
depends: []
tags: [feedback, friction, "from:atoms"]
---

edit --spec rejected both a document stem and an explicit relative path to a design document stored under docs/plans, the reporting repository's established convention. Expected an explicit path to identify the document without moving historical files.

Configurable roots (0f7d904, 2026-09-03) predate this report: listing docs/plans in `spec_dirs` in tasks/.config.toml attaches the document, and a directory may sit in both `spec_dirs` and `plan_dirs` (verified on a scratch project, check clean). The rejection, `spec "…" must be under docs/specs/ or …/`, does not name that key.

## Notes

- 2026-09-24T13:07:13Z (main): curate: stale; body records that spec_dirs (0f7d904) already attaches a design under docs/plans; proposal: drop citing 0f7d904, or retitle to "the must-be-under rejection names spec_dirs/plan_dirs in tasks/.config.toml" and keep as a small error-message fix
