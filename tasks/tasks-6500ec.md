---
id: tasks-6500ec
title: "test-fast stops at the first failing test binary, hiding every integration suite behind a unit failure"
status: todo
priority: 1
size: xs
complexity: low
process: direct
created: 2026-10-17T17:45:13Z
updated: 2026-10-17T17:45:13Z
depends: []
tags: [testing]
source: tasks-aeeef1
agent: claude-code/claude-opus-5-5
---

fast_cmd runs cargo test without --no-fail-fast, so one failing unit test (today the 18 record_commit failures of tasks-eeeeda) stops the run before tests/*.rs execute: the baseline and every implementer's test-fast on 2026-10-17 silently skipped all integration suites, and push_fast_cmd inherits the same blind spot. Add --no-fail-fast to fast_cmd (and check test_cmd) so a run always reports every binary; the exit code stays non-zero on any failure.
