---
id: tasks-d40e8e
title: "Lifecycle hooks: run something on done or after a merge"
status: idea
priority: 2
created: 2026-09-07T10:25:11Z
updated: 2026-09-07T10:39:15Z
depends: []
tags: [quick-add, cli]
source: "mindful:thought:ec1726a8595c47f78ddaad57f65eb89e"
---

Let a project hang a hook off task lifecycle events. Motivating case: after `tasks done <id>`, or after a merge into `main`, ask "are there any follow-up tasks we should create, or update?"

Open questions: where hooks are configured (tasks/.config.toml?), which events fire, what the hook receives (the task JSON on stdin?), whether a hook may write tasks itself or only emit a prompt, and how a failing hook affects the command that triggered it.

## Notes

- 2026-09-07T10:39:15Z (main): Shares a design fork with tasks-5caeae: if hooks land first, recurrence may be a hook consumer rather than a model field. Whichever is scoped first constrains the other.
