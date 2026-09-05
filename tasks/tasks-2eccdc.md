---
id: tasks-2eccdc
title: note and dep on a task created via --project need cwd in that project; add --project to note/dep/edit
status: idea
priority: 2
created: 2026-09-05T01:11:08Z
updated: 2026-09-05T01:11:08Z
depends: []
tags: [feedback, gap, "from:ops"]
---

tasks add --project X creates the task, but the following tasks note <id> and tasks dep <id> --on ... from the hub cwd fail with task_not_found. Expected the id prefix to resolve the project the way tasks show does.
