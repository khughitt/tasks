---
id: tasks-c53b01
title: an_add_waiting_through_a_rename_never_writes_the_old_prefix flakes under the full suite
status: done
priority: 3
size: s
complexity: mid
process: direct
owner: main
created: 2026-09-23T11:17:32Z
updated: 2026-09-24T12:55:58Z
started: 2026-09-24T12:43:35Z
completed: 2026-09-24T12:55:58Z
depends: []
tags: [test]
agent: "claude-code/claude-opus-5-5[1m]"
---

Failed once in a full just gate run on 2026-09-23 (cli suite, while working on tasks-962300); passed 3/3 in isolation and on the next full gate. Likely a timing assumption in the rename-wait path under load. Reproduce with repeated full-suite runs, then find the race.

## Notes

- 2026-09-24T12:36:18Z (fix/closed-doc-links): Recurred 2026-09-24 in a full test-fast run on the closed-doc-links branch (panic at tests/cli.rs:10353); passed alone and on the immediate full rerun
- 2026-09-24T12:43:35Z (main): started
  provenance: {"harness_session":"claude-code:a06e4c8f-324c-47ec-98f1-1a947f1b8716","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-24T12:55:58Z (fix/rename-wait-flake): Root cause: the test proved the add was waiting by finding dot.lock in /proc/<pid>/fd, but spawn() can return while the child is still in execve: its fd table then still holds the test's own close-on-exec lock descriptor (6% of immediate reads under 64-way load: false positive, so the fixture races the child's config read) or /proc/<pid>/fd is root-owned (0.2%: the recorded EACCES at the read_dir unwrap, all three failures)
- 2026-09-24T12:55:58Z (fix/rename-wait-flake): done
  provenance: {"harness_session":"claude-code:a06e4c8f-324c-47ec-98f1-1a947f1b8716","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-24T12:55:58Z (fix/rename-wait-flake): The rename-wait test now waits for /proc/locks to list the child as a blocked flock waiter on the lock's inode instead of scanning its descriptor table; stress of the single test under 96-way load: pre-fix 7/9600 failures, fixed 0/19200
  provenance: {"harness_session":"claude-code:a06e4c8f-324c-47ec-98f1-1a947f1b8716","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
