---
id: tasks-2eccdc
title: note and dep on a task created via --project need cwd in that project; add --project to note/dep/edit
status: done
priority: 2
owner: feat/cross-project-writes
created: 2026-09-05T01:11:08Z
updated: 2026-09-05T21:02:53Z
depends: []
tags: [feedback, gap, "from:ops"]
---

tasks add --project X creates the task, but the following tasks note <id> and tasks dep <id> --on ... from the hub cwd fail with task_not_found. Expected the id prefix to resolve the project the way tasks show does.

## Notes

- 2026-09-05T21:02:53Z (feat/cross-project-writes): id-taking write commands follow the id prefix to its project; open_id_write_ctx replaces open_write_ctx
