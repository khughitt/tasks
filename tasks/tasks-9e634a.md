---
id: tasks-9e634a
title: Refuse a flagless edit without a terminal instead of stalling in $EDITOR
status: doing
priority: 2
size: s
complexity: low
process: direct
owner: main
created: 2026-10-11T12:31:33Z
updated: 2026-10-24T13:36:42Z
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
