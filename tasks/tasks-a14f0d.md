---
id: tasks-a14f0d
title: Validate TASKS_FORMAT even when --pretty overrides it
status: done
priority: 3
size: xs
complexity: low
process: direct
owner: a14f0d-format
created: 2026-09-04T02:03:12Z
updated: 2026-09-22T13:07:58Z
started: 2026-09-22T13:07:57Z
completed: 2026-09-22T13:07:58Z
depends: []
tags: [cli]
model: "claude-opus-5[1m]"
---

`--pretty` wins before `TASKS_FORMAT` is read (the `(true, _)` arm of the format match in main.rs), so `tasks --pretty ...` with `TASKS_FORMAT=xml` succeeds while the same variable without `--pretty` is a config error. Fail-early says an invalid value is an error whenever it is set, still true as of 2026-09-09.

Done: any invocation with `TASKS_FORMAT` set to something other than `json` or `pretty` exits 1 with the existing config error, with or without `--pretty`, and a test in tests/cli.rs covers the `--pretty` case. This turns a currently succeeding command into an error, which is why the colour design deferred it.

## Notes

- 2026-09-09T11:05:38Z (design/curation): curate: refined; body now states the done condition and the behaviour change; facts unchanged
- 2026-09-13T16:47:44Z (main): Complexity low: main.rs still selects the (true, _) format arm before rejecting invalid TASKS_FORMAT. Validation before override selection is established, and the task specifies the failing value, exit code, and regression check.
- 2026-09-14T11:46:16Z (main): Process direct: the body fixes the arm to change, the error and exit code to reuse, and the regression test to add; no design choice remains.
- 2026-09-22T13:07:57Z (a14f0d-format): started
  provenance: {"harness_session":"claude-code:3b265943-33e2-4466-947a-581d11a9cbb0","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-22T13:07:57Z (a14f0d-format): Rebased the stranded a14f0d-format worktree onto main: main gained --json, so Format::resolve takes (json, pretty, TASKS_FORMAT) and validates the variable under either flag.
- 2026-09-22T13:07:58Z (a14f0d-format): done
  provenance: {"harness_session":"claude-code:3b265943-33e2-4466-947a-581d11a9cbb0","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-22T13:07:58Z (a14f0d-format): Format::resolve validates TASKS_FORMAT whenever it is set; --json and --pretty override a valid value, not the check. Unit test over the flag/variable matrix and a cli.rs case for both flags.
  provenance: {"harness_session":"claude-code:3b265943-33e2-4466-947a-581d11a9cbb0","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
