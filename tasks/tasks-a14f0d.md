---
id: tasks-a14f0d
title: Validate TASKS_FORMAT even when --pretty overrides it
status: todo
priority: 3
size: xs
created: 2026-09-04T02:03:12Z
updated: 2026-09-09T11:05:38Z
depends: []
tags: [cli]
---

`--pretty` wins before `TASKS_FORMAT` is read (the `(true, _)` arm of the format match in main.rs), so `tasks --pretty ...` with `TASKS_FORMAT=xml` succeeds while the same variable without `--pretty` is a config error. Fail-early says an invalid value is an error whenever it is set, still true as of 2026-09-09.

Done: any invocation with `TASKS_FORMAT` set to something other than `json` or `pretty` exits 1 with the existing config error, with or without `--pretty`, and a test in tests/cli.rs covers the `--pretty` case. This turns a currently succeeding command into an error, which is why the colour design deferred it.

## Notes

- 2026-09-09T11:05:38Z (design/curation): curate: refined; body now states the done condition and the behaviour change; facts unchanged
