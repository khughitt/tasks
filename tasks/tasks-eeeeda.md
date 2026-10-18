---
id: tasks-eeeeda
title: "Unit tests leak HOME process-wide, so later fixture commits run the host's global git hooks with a temp HOME and fail"
status: doing
priority: 1
size: xs
complexity: low
process: direct
owner: fix/test-hermeticity
created: 2026-10-17T11:24:09Z
updated: 2026-10-18T09:45:23Z
started: 2026-10-18T09:45:12Z
depends: []
tags: [testing]
source: tasks-dd87b6
agent: claude-code/claude-opus-5-5
---

src/registry.rs expands_leading_tilde_on_load does std::env::set_var("HOME", <tempdir>) and never restores it. Unit tests share one process, so every later test that shells out to git (record_commit::tests, about 18 of them) runs under that HOME while git still reads the real global config through XDG_CONFIG_HOME. On a host whose global config declares hook.identity-check (command /usr/bin/python3 $HOME/.local/bin/git-identity-check), those commits fail: 'can't open file <tmp>/.local/bin/git-identity-check'. Order-dependent: just test-fast failed 18 record_commit tests on 2026-10-17; each passes alone. Fix: test tilde expansion without mutating the process environment (inject the home, or a serialized guard that restores it), and make the unit-test git helper hermetic (GIT_CONFIG_GLOBAL to a fixture, GIT_CONFIG_NOSYSTEM=1), as tests/linked.rs's git() already is. Blocks the pre-push gate on hosts with the hook.

## Notes

- 2026-10-18T09:45:12Z (main): started
  provenance: {"harness_session":"claude-code:db33ae25-27fd-4537-883b-1b3aa17ecfdf","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-18T09:45:23Z (fix/test-hermeticity): resumed
  provenance: {"harness_session":"claude-code:db33ae25-27fd-4537-883b-1b3aa17ecfdf","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
