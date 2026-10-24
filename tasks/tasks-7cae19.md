---
id: tasks-7cae19
title: "The project resolver (tasks where / linked lookup) fails in a repository with no commits yet when GIT_TRACE is set: git ls-tree HEAD's trace lines on stderr seem to defeat the no-HEAD classification, so the lookup errors instead of answering"
status: doing
priority: 2
size: xs
complexity: low
process: direct
owner: fix/tasks-7cae19-git-trace
created: 2026-10-17T11:24:25Z
updated: 2026-10-24T11:46:17Z
started: 2026-10-24T11:42:16Z
depends: []
tags: [feedback, friction, "from:ops"]
agent: claude-code/claude-opus-5-5
---

Why: `where` and linked lookup error on a repository with no commits whenever GIT_TRACE (or GIT_TRACE2) is set, so a pre-commit hook running under trace fails on a fresh `git init`.

Established (scope 2026-10-17, reproduced with git in a scratch repo): `src/link.rs::tracked_config` runs `git ls-tree -z HEAD`, which fails on an unborn HEAD and falls back to `unborn()`. `unborn()` requires `git for-each-ref` to succeed with empty stdout *and empty stderr* (link.rs:361). Under GIT_TRACE=1 git writes `trace: built-in: git for-each-ref ...` to stderr, so the check reads as not-unborn and the ls-tree failure becomes an `io` error. No other site in src/ tests stderr emptiness.

Done: both git closures in `tracked_config` and `unborn` run with every `GIT_TRACE*` variable removed from the child environment (keep the stderr-empty guard: it is part of the corrupt-ref classification). An unborn repository answers `holds_records: false` under GIT_TRACE=1 and GIT_TRACE2=1; the corrupt-ref cases still error.

Where: src/link.rs (tracked_config, unborn); tests/linked.rs: the `no HEAD tracks nothing` assertion near line 684 and `a_corrupt_head_ref_is_an_error_not_an_empty_repository`. Verify with `just test-one --test linked <name>` (a regression run of `where` with GIT_TRACE=1 set on the child), then `just test-fast`.

Original report (preserved): Repro: git init a scratch repo, then run a client that asks the resolver about it (here a pre-commit hook) under GIT_TRACE=1: error 'io: git ls-tree HEAD in <repo> failed: <trace line> trace: built-in: git ls-tree -z HEAD -- tasks/.config.toml'. Without GIT_TRACE, or once HEAD exists, it answers. Expected: an unborn HEAD is recognised regardless of git trace output (e.g. check rev-parse --verify HEAD, or run git with GIT_TRACE* unset).

## Notes

- 2026-10-17T17:49:59Z (main): scope: scoped; root cause reproduced: unborn() in src/link.rs:361 requires empty for-each-ref stderr, which GIT_TRACE fills; fix strips GIT_TRACE* from those git calls, regression in tests/linked.rs
- 2026-10-24T11:42:16Z (main): started
  provenance: {"harness_session":"codex:01a16db5-709f-7583-9487-e278f2c67739","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-24T11:42:50Z (fix/tasks-7cae19-git-trace): resumed
  provenance: {"harness_session":"codex:01a16db5-709f-7583-9487-e278f2c67739","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-24T11:46:17Z (fix/tasks-7cae19-git-trace): Direct process in .worktrees/tasks-7cae19-git-trace. Confined README install: CARGO_TARGET_DIR under worktree, cargo install --locked --path . --root target/setup. Baseline just test passed: 1663 tests, 0 failures. Scope: the two src/link.rs Git closures and linked-discovery regression coverage; preserve corrupt-ref errors and JSON shapes.
