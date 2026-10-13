---
id: tasks-eec7d5
title: Make the test suite immune to a stray .git above TMPDIR
status: todo
priority: 2
size: s
complexity: low
process: direct
created: 2026-10-08T21:02:30Z
updated: 2026-10-13T10:48:39Z
depends: []
tags: [feedback, friction, "from:tasks", tests]
agent: codex
---

Why: the fast suite's fixtures create their projects under `$TMPDIR` (tempfile::tempdir). When something on the host leaves an empty `.git` in `$TMPDIR` (tasks-fa0276's report: recreated every minute or two), git rejects it and linked discovery's marker scan (src/link.rs::GitTree::of) turns the miss into an `io` error, so every `init` in a temp project fails until the marker goes: 68 failures in one run, then a clean rerun. The suite should not depend on what sits above its temp directories.

Established (checked at d698a36): tasks-fa0276 (done) makes the error name the marker path; tasks-084a9f (done) makes the marker scan stop where git's discovery stops, at `GIT_CEILING_DIRECTORIES` and at the mount point git names. Neither stops the flake: `$TMPDIR` is below its mount point, so git and the scan both examine `$TMPDIR/.git`. The test helpers also inherit the caller's own `GIT_CEILING_DIRECTORIES`. No `$TMPDIR/.git` was present at scoping; the run log (~/.local/share/ops/runs.jsonl) keeps exit codes but not output, so later fast-suite failures cannot be attributed to this cause. The host process that creates the marker is not investigated and is not this task's to fix.

Approach: the test helpers set `GIT_CEILING_DIRECTORIES` to the canonicalized `std::env::temp_dir()` on every command they spawn (tests/common/mod.rs TestEnv::cmd and ::raw, and any other `cargo_bin("tasks")` or git spawns in tests/), so neither git nor the marker scan looks at `$TMPDIR/.git`. Tests that set their own ceiling (tests/linked.rs) keep overriding it. Canonicalize: git compares ceiling entries after realpath only when they follow an empty entry.

Done: with `TMPDIR=$(mktemp -d)` and an empty `$TMPDIR/.git`, `just test-fast` passes; without the change that same setup reproduces the init git-discovery failures (record both runs on this task). Existing ceiling and marker tests in tests/linked.rs still pass. `just test-fast`, `just check`.

Where to look: tests/common/mod.rs (TestEnv::cmd, ::raw), tests/linked.rs ceiling tests, src/link.rs GIT_CEILING_DIRECTORIES handling, tasks-fa0276 and tasks-084a9f notes.

Original report (preserved): Fast-suite CLI fixtures intermittently reject init in non-Git temporary directories with a git-discovery error: 68 failures in one run, the named focused test and an unchanged full rerun then pass.

## Notes

- 2026-10-09T00:42:57Z (fa0276-git-marker): Likely cause: tasks-fa0276's report says a host process recreates an empty .git in TMPDIR every minute or two, and git-discovery io on init in temp dirs is that failure; intermittent because the marker comes and goes. fa0276 makes the error name the marker path; it does not stop the flake.
- 2026-10-13T10:48:39Z (main): scope: scoped; fa0276 (diagnostic) and 084a9f (ceiling/mount stop) landed but don't cover $TMPDIR/.git below the mount; fix is a GIT_CEILING_DIRECTORIES=temp_dir in the test helpers, checked with an empty $TMPDIR/.git under a scratch TMPDIR; host process creating the marker left uninvestigated; P2/s/low/direct
