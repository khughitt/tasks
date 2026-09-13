# Task 4 report

## Changes

- Default `list` excludes shelved tasks; explicit `--status shelved` still returns them.
- Parked shelved records remain visible in `prime` and `list --parked`, but cannot feed `next`.
- `tree` shows shelved descendants below visible parents, while `prime` roadmap hides every shelved row, including a shelved root with an active child.
- `check` warns when visible open work depends on a shelved task.
- Added CLI regressions for default views, counts, hierarchy modes, stale park entries, and dependency warnings.

## RED

`cargo test --test cli shelved -- --nocapture` failed 4 of 8 tests:

- default `list` returned the shelved task;
- default `tree` returned the shelved root;
- `next` selected a shelved task with a surviving park entry;
- `check` returned no `shelved_dep` warning.

## GREEN

- `cargo test --test cli shelved -- --nocapture`: 8 passed.
- `just test`: 167 unit and 274 CLI tests passed.
- `just check`: formatting, clippy with warnings denied, and `tasks check` passed.

## Self-review

The root eligibility check runs before descendant fallback, so a shelved ancestor cannot leak into the hidden roadmap. A private `Visibility` enum combines the existing closed-node flag and shelved mode to keep recursion within clippy's argument limit; the requested public `Shelved` interface remains intact. No concerns.
