# Task 5 report

## Changes

- Added `scope:` as an accepted prefix for pending proposals in `tasks sample`.
- Updated the module and helper documentation.
- Added an end-to-end regression test covering pending, answered, and proposal-free scope notes.

## TDD evidence

- RED: `cargo test --test cli sample_treats_a_scope_proposal` failed; sampled IDs contained three tasks instead of the expected two, so the `scope:` proposal was not excluded.
- GREEN: the focused test passed after the parser change.

## Verification

- `just test`: 167 unit tests and 275 CLI tests passed.
- `just check`: passed formatting, ops checks, clippy with warnings denied, and `tasks check`.
- `git diff --check`: passed.

## Self-review

The implementation reuses the existing parser and adds only the requested alternate prefix. Later notes still clear pending status, and notes without a proposal remain eligible. No concerns.
