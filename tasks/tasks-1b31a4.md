---
id: tasks-1b31a4
title: Support grouping by project in tasks list --pretty --all-projects
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-24T11:23:30Z
created: 2026-10-24T11:07:36Z
updated: 2026-10-24T11:23:30Z
depends: [tasks-6fcb58]
parent: tasks-eceb1c
tags: [feedback, idea, "from:tasks"]
agent: codex
---

User request: add project grouping to tasks list --pretty --all-projects so tasks from each project can be read together. The user proposed three approaches: (1) treat project as another sortable field, noting that this limits sorting within projects unless multiple sort fields are supported; (2) add a separate grouping switch, with the selected sort applied within each project; or (3) make project grouping the default for this view. Current CLI inspection confirms that --sort accepts a single value: priority, updated, or created. Scoping should choose among these interfaces and settle ordering within project groups; the user has not selected an approach. This captures the feature request for later scoping and implementation.

Scope handoff: [ref:brief] records verified list/tree behavior and the joint design choices. Wait on tasks-6fcb58 for a reviewed contract covering this request and its interaction with the other list display request.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-24-list-organization-brief.md — evidence and choices for list organization

## Notes

- 2026-10-24T11:23:30Z (main): scope: briefed; checked list/tree selection and ordering at 392155e39d3f48a5b2832d201d1e56d67688eece; preserved the original request; grouped under tasks-eceb1c and waiting on design tasks-6fcb58; brief: docs/notes/2026-10-24-list-organization-brief.md
