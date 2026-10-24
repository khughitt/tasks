---
id: tasks-1b31a4
title: Support grouping by project in tasks list --pretty --all-projects
status: idea
priority: 2
created: 2026-10-24T11:07:36Z
updated: 2026-10-24T11:07:36Z
depends: []
tags: [feedback, idea, "from:tasks"]
agent: codex
---

User request: add project grouping to tasks list --pretty --all-projects so tasks from each project can be read together. The user proposed three approaches: (1) treat project as another sortable field, noting that this limits sorting within projects unless multiple sort fields are supported; (2) add a separate grouping switch, with the selected sort applied within each project; or (3) make project grouping the default for this view. Current CLI inspection confirms that --sort accepts a single value: priority, updated, or created. Scoping should choose among these interfaces and settle ordering within project groups; the user has not selected an approach. This captures the feature request for later scoping and implementation.
