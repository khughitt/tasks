---
id: tasks-9e634a
title: Refuse a flagless edit without a terminal instead of stalling in $EDITOR
status: doing
priority: 2
size: s
complexity: low
process: direct
owner: fix/tasks-9e634a
created: 2026-10-11T12:31:33Z
updated: 2026-10-24T13:49:54Z
started: 2026-10-24T13:36:42Z
depends: []
tags: [feedback, friction, "from:ops", cli]
agent: codex
---

Why: `tasks edit <id>` with no field flags opens `$EDITOR` through `sh -c` and waits. An automation caller (an agent's shell: pipes, no terminal) that forgets its flags gets a terminal editor that never draws and a command that never returns, with no output. Fail early instead.

Approach (chosen at scoping): editor mode refuses when stdin is not a terminal (`std::io::IsTerminal`), before the temp copy is written, with kind `editor` and a message that says a flagless edit opens `$EDITOR`, which needs a terminal, and names the way out: pass field flags (`tasks edit --help`), or `--editor` to open the editor anyway. A new `--editor` switch forces editor mode without a terminal and conflicts with every field flag; it keeps scripted editors (the test suite's `editor_script`, a GUI `EDITOR="code --wait"`) working explicitly. Rejected: an environment variable as the override (invisible at the call site, and the refusal could not name it as plainly); probing `/dev/tty` (passes or fails by host, so the suite would differ between a terminal and CI).

Done: a flagless edit with stdin not a terminal exits non-zero with kind `editor`, writes no temp file and does not run `$EDITOR`; `--editor` runs it as today; `--editor` with any field flag is a usage error. Existing editor tests in tests/cli.rs pass `--editor`; new RED→GREEN tests cover the refusal (no temp file left, EDITOR script not run) and the forced path. Update skills/tasks/SKILL.md (the `tasks edit <id>` with no flags line) and README's edit reference. `just test-one`, `just test-fast`, `just check`; `cargo install --path .`.

Where to look: src/commands/edit.rs::run (the `has_flags` check that calls `editor`) and `editor`; src/cli.rs EditArgs; src/error.rs Editor; tests/cli.rs `editor_script` and its callers.

Original report (preserved): The task CLI launches an interactive editor for a flagless edit even when the automation caller uses pipes, so an accidental invocation can stall silently instead of refusing with an actionable message. The caller mistakenly omitted edit flags; the CLI then launched the configured editor from a piped subprocess and waited indefinitely without output. Explicit edit flags are the recovery. Consider refusing interactive editor mode when the caller has no usable terminal, with a diagnostic naming editor mode and the required explicit flags.

## Notes

- 2026-10-13T10:48:22Z (main): scope: scoped; chose refuse-when-stdin-not-a-terminal plus an explicit --editor override (rejected env var and /dev/tty probe); editor path is edit.rs::run -> editor, no terminal check today (checked at d698a36); P2/s/low/direct
- 2026-10-24T13:36:42Z (main): started
  provenance: {"harness_session":"codex:01a16e1f-1714-7cb0-8a41-c0e31ca677e2","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-24T13:37:12Z (fix/tasks-9e634a): resumed
  provenance: {"harness_session":"codex:01a16e1f-1714-7cb0-8a41-c0e31ca677e2","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-24T13:43:05Z (fix/tasks-9e634a): Process direct in .worktrees/tasks-9e634a; baseline just test passed 1666 tests. RED: flagless piped edit ran EDITOR and left its failed-edit temp; explicit editor tests fail because --editor is unknown. Scope includes parser, editor dispatch, five integration files, README, tasks skill and ops-678532 option declaration.
- 2026-10-24T13:48:15Z (fix/tasks-9e634a): GREEN: refusal test 1, explicit editor/conflict tests 2, parser-surface test 1; fixed four caller/expectation migrations found by fast suite and each passed focused. just test-fast passes 1668 tests (one ignored exhaustive test); just check passes. PTY smoke proves flagless edit with terminal stdin still runs EDITOR. Reference skill retrieval failed before wording and passed after. Ops option declaration merged at 1002e3a and adopted byte-identically; other adopted rows belong to already-landed upstream vocabulary.
- 2026-10-24T13:48:27Z (fix/tasks-9e634a): Verification count correction: fast.log totals 1665 passed, 0 failed and 4 ignored across 14 targets; prior note miscounted the ignored set. Full pre-change baseline remains 1666 passed, 0 ignored (just test includes ignored cases).
- 2026-10-24T13:49:04Z (fix/tasks-9e634a): attached: baseline.log (123246 bytes): Full pre-change baseline: 1666 tests passed including ignored cases
- 2026-10-24T13:49:06Z (fix/tasks-9e634a): attached: red-refusal.log (972 bytes): RED: flagless nonterminal edit incorrectly launched EDITOR
- 2026-10-24T13:49:07Z (fix/tasks-9e634a): attached: red-explicit.log (2444 bytes): RED: explicit editor option was unrecognized
- 2026-10-24T13:49:09Z (fix/tasks-9e634a): attached: fast.log (120809 bytes): Final fast suite: 1665 passed, four ignored, zero failures
- 2026-10-24T13:49:12Z (fix/tasks-9e634a): attached: check.log (145 bytes): Final hygiene, formatting, clippy and task validation passed
- 2026-10-24T13:49:54Z (fix/tasks-9e634a): detached: baseline.log: Replace raw tool output with the same evidence after redacting machine-specific paths
