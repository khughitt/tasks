---
id: tasks-37d48a
title: Correct linked-project resolver documentation details
status: doing
priority: 3
size: xs
complexity: low
process: direct
owner: main
created: 2026-10-07T11:24:50Z
updated: 2026-10-09T00:50:11Z
started: 2026-10-09T00:50:11Z
depends: [tasks-9b0a2e]
parent: tasks-a7460c
tags: []
source: tasks-9b0a2e
agent: codex
---

Deferred Minor findings from linked-projects whole-branch review: README holds_records must describe registered-root common Git directory identity as well as a regular config tracked in HEAD (including registered unborn repositories); project-resolution design §4.4 must account for reachable registered config reads during index construction. Documentation-only correction; verify statements against code and run just check.

## Notes

- 2026-10-09T00:50:11Z (main): started
  provenance: {"harness_session":"claude-code:4b1d7eef-4783-40e0-b224-bba892d8a7e3","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
