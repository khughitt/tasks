---
id: tasks-beb617
title: Editor-script tests no longer race sibling forks into ETXTBSY
status: done
priority: 2
size: xs
complexity: low
process: direct
owner: fix/rejection-hints
created: 2026-09-24T13:48:24Z
updated: 2026-09-24T13:49:30Z
started: 2026-09-24T13:48:32Z
completed: 2026-09-24T13:49:30Z
depends: []
tags: [testing]
agent: claude-code/claude-opus-5-5
---

Why: `just gate` failed once on 2026-09-24 in editor_reopen_keeps_the_completed_stamp_and_transition_clears_it with `editor.sh: /bin/sh: bad interpreter: Text file busy` (exit 126); the next run passed. `editor_script` in tests/cli.rs writes an executable script that `tasks edit` then execs through `sh -c "$EDITOR \"$1\""`; a sibling test thread forking while the write descriptor is open makes that execve fail. The same race was fixed for the relay harness shim on 2026-09-22 (tasks-634c4a).

Done when `editor_script` returns an EDITOR value that has sh read the script (`sh '<path>'`) rather than exec it, so no test executes a file it just wrote, and the 48 callers pass unchanged. Where: `editor_script` in tests/cli.rs. The fake git in the worktree-list test (tests/cli.rs, executed through PATH) carries the same exposure; it is out of this fix and noted on close.

## Notes

- 2026-09-24T13:48:32Z (fix/rejection-hints): started
  provenance: {"harness_session":"claude-code:c3b66540-0acc-4861-917d-8c10dfaea3bd","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-24T13:49:30Z (fix/rejection-hints): done
  provenance: {"harness_session":"claude-code:c3b66540-0acc-4861-917d-8c10dfaea3bd","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-24T13:49:30Z (fix/rejection-hints): editor_script now returns sh '<path>', so sh reads the script and no test execs a file it just wrote; 48 callers unchanged, five consecutive clean suite runs. The fake git in the worktree-list test still execs a freshly written file through PATH.
  provenance: {"harness_session":"claude-code:c3b66540-0acc-4861-917d-8c10dfaea3bd","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
