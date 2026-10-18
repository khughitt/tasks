---
id: tasks-6500ec
title: "test-fast stops at the first failing test binary, hiding every integration suite behind a unit failure"
status: doing
priority: 1
size: xs
complexity: low
process: direct
owner: fix/test-hermeticity
created: 2026-10-17T17:45:13Z
updated: 2026-10-18T09:49:04Z
started: 2026-10-18T09:49:04Z
depends: []
tags: [testing]
source: tasks-aeeef1
agent: claude-code/claude-opus-5-5
---

fast_cmd runs cargo test without --no-fail-fast, so one failing unit test (today the 18 record_commit failures of tasks-eeeeda) stops the run before tests/*.rs execute: the baseline and every implementer's test-fast on 2026-10-17 silently skipped all integration suites, and push_fast_cmd inherits the same blind spot. Add --no-fail-fast to fast_cmd (and check test_cmd) so a run always reports every binary; the exit code stays non-zero on any failure.

## Notes

- 2026-10-18T09:49:04Z (fix/test-hermeticity): started
  provenance: {"harness_session":"claude-code:db33ae25-27fd-4537-883b-1b3aa17ecfdf","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
