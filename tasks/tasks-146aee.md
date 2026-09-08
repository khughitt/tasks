---
id: tasks-146aee
title: show --pretty panics with a Rust broken-pipe backtrace when stdout is closed early (piped to head)
status: done
priority: 2
size: xs
owner: fix/broken-pipe
created: 2026-09-06T17:44:05Z
updated: 2026-09-08T02:18:45Z
depends: []
tags: [feedback, friction, "from:dots", cli]
---

tasks <cmd> | head panics with 'failed printing to stdout: Broken pipe (os error 32)' instead of exiting quietly. Reproduced: tasks list --all-projects --pretty | head -0.

There is exactly one stdout write in the binary, println! at src/main.rs:77; everything routes through output::render into it. Replace it with a locked write plus an explicit flush -- a piped stdout is block-buffered, so for output under the pipe buffer the failure surfaces at the flush, and the implicit flush at process exit discards it. On ErrorKind::BrokenPipe fall through silently; any other write error still goes through render_error and exits 1, so fail-early is intact.

Falling through rather than exiting immediately keeps the exit code the command earned: tasks check piped to head still exits 1 when it found errors. Otherwise the process ends 0, the ripgrep convention, so an ordinary | head does not fail a pipefail script.

The five eprintln!/eprint! sites in main.rs have the same defect (2>&1 | head in pretty mode reaches them), so the same helper covers all six. On stderr a departed reader is ignored outright: a diagnostic we cannot deliver must not become a panic and must not change the exit code.

No new dependency: restoring SIGPIPE to SIG_DFL would mean adding libc and an unsafe block for a one-site problem.

## Notes

- 2026-09-08T02:18:45Z (fix/broken-pipe): Single stdout write in main.rs now goes through write_to (locked stream, explicit flush -- a piped stdout is block-buffered, so the failure surfaces at the flush the implicit end-of-process one discards). BrokenPipe falls through silently, leaving the exit code the command earned intact, so check | head still exits 1; any other write error reports and exits 1. The five stderr sites route through to_stderr, which ignores a departed reader outright. No new dependency. Documented in the design spec's output contract.
