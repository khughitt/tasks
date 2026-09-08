# Task 4 report

Implemented the stale local checkout guard.

## Changes

- Added `commands::reject_stale_local`, comparing the local project prefix with `Registry::canonical_prefix`.
- Applied it to write contexts and the local read context.
- Added the CLI regression covering both `list` and `add` from a checkout whose prefix was retired.
- Closed `tasks-bdde89` through the `tasks` CLI.

## Files

- `src/commands/mod.rs`
- `tests/cli.rs`
- `tasks/tasks-bdde89.md`

## TDD evidence

- RED: `cargo test --test cli -- a_checkout_still_using_a_retired_prefix_refuses` failed because `list` exited 0 with `{"tasks":[],"warnings":[]}`.
- GREEN: the same focused test passed after the guard was added.

## Validation

- `just gate`: passed; 108 unit tests and 159 CLI tests passed.
- `cargo install --path .`: passed and replaced `/home/keith/.cargo/bin/tasks` with this worktree build.
- `tasks check`: passed as part of `just gate` and again before commit.
- `just check`: passed before commit.

## Self-review

The guard is called only for the local checkout path. Explicit registered project scopes and `--all-projects` retain existing routing behavior. No known concerns.
