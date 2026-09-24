---
id: tasks-c53b01
title: an_add_waiting_through_a_rename_never_writes_the_old_prefix flakes under the full suite
status: doing
priority: 3
size: s
complexity: mid
process: direct
owner: main
created: 2026-09-23T11:17:32Z
updated: 2026-09-24T12:43:35Z
started: 2026-09-24T12:43:35Z
depends: []
tags: [test]
agent: "claude-code/claude-opus-5-5[1m]"
---

Failed once in a full just gate run on 2026-09-23 (cli suite, while working on tasks-962300); passed 3/3 in isolation and on the next full gate. Likely a timing assumption in the rename-wait path under load. Reproduce with repeated full-suite runs, then find the race.

## Notes

- 2026-09-24T12:36:18Z (fix/closed-doc-links): Recurred 2026-09-24 in a full test-fast run on the closed-doc-links branch (panic at tests/cli.rs:10353); passed alone and on the immediate full rerun
- 2026-09-24T12:43:35Z (main): started
  provenance: {"harness_session":"claude-code:a06e4c8f-324c-47ec-98f1-1a947f1b8716","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
