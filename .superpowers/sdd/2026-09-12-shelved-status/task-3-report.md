# Task 3 report

## Changes

- `start` refuses shelved tasks with an `invalid_transition` directing the caller to `tasks unshelve`.
- `park` applies the same guard.
- Flag and editor status changes refuse transitions into `shelved` and direct callers to `tasks shelve "<wake condition>"`.
- Edits that leave an already-shelved record shelved continue to work.
- Added CLI coverage for all four paths and the allowed shelved-record edit.

## RED

Command: `cargo test --test cli shelved`

The two new tests failed as expected before implementation: `start`/`park` returned exit code 0 instead of 1, and the editor transition returned exit code 0 instead of 1.

## GREEN

Command: `cargo test --test cli shelved`

Result: 4 passed, 0 failed.

Command: `just test`

Result: 167 unit tests and 270 CLI tests passed, 0 failed.

## Checks

Command: `just check`

Result: formatting, ops checks, clippy with warnings denied, and `tasks check` passed with no errors or warnings.

Self-review found no unrelated changes or production contract changes beyond the requested guards.
