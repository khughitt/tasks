---
id: tasks-d40e8e
title: "Lifecycle hooks: run something on done or after a merge"
status: idea
priority: 2
reviewed: true
created: 2026-09-07T10:25:11Z
updated: 2026-10-12T18:22:32Z
depends: []
parent: tasks-46d207
tags: [quick-add, cli]
source: "mindful:thought:ec1726a8595c47f78ddaad57f65eb89e"
---

Let a project hang a hook off task lifecycle events. Motivating case: after `tasks done <id>`, or after a merge into `main`, ask "are there any follow-up tasks we should create, or update?"

Open questions: where hooks are configured (tasks/.config.toml?), which events fire, what the hook receives (the task JSON on stdin?), whether a hook may write tasks itself or only emit a prompt, and how a failing hook affects the command that triggered it.

## Notes

- 2026-09-07T10:39:15Z (main): Shares a design fork with tasks-5caeae: if hooks land first, recurrence may be a hook consumer rather than a model field. Whichever is scoped first constrains the other.
- 2026-10-08T10:59:00Z (main): scope: briefed; recurrence is delivered independently under tasks-5caeae; tasks-d4e73d will compare explicit closeout, external integration and native hooks for follow-up discovery; original capture/source preserved, Mindful lookup found no matching thought; brief: docs/notes/2026-10-08-completion-followups-brief.md
