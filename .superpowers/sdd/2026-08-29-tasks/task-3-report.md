# Task 3 report

## Implementation

- Added `src/format.rs` with canonical task markdown parsing, serialization, body/notes handling, field validation, owner validation, and normalized documentation-path validation.
- Registered the module in `src/main.rs`.
- Kept timestamps emitted as exact unquoted RFC3339 values while adapting them for the strict frontmatter parser on input.

## RED / GREEN

- RED attempt: `cargo test format` was blocked before compilation because the default Cargo cache is read-only and `assert_cmd` was not cached.
- GREEN focused: `CARGO_HOME=/mnt/ssd2/uv-cache/cargo cargo test format` — 4 passed.
- GREEN full: `CARGO_HOME=/mnt/ssd2/uv-cache/cargo cargo test` — 13 passed.
- Formatting: `rustfmt src/format.rs` completed successfully.

## Files

- `src/format.rs`
- `src/main.rs`
- This report

## Self-review

- Exact field ordering and optional-field omission are centralized in serialization.
- Unknown keys, malformed values, reserved body delimiter, malformed notes, traversal/non-canonical paths, multiline values, invalid owners, invalid times, and invalid dependencies are rejected.
- Round-trip tests cover full and minimal canonical files.

## Concerns

- `parse_task` receives a file label but does not enforce filename/id equality; repository-level checks can apply that when the filesystem context is available.
- The strict frontmatter subset rejects bare colon-containing scalars, so task parsing quotes timestamps internally and serializer uses raw values to preserve the required canonical output.

## Fix Round 1

Validated the task ID and every dependency ID inside `validate_task`, covering public callers that construct `Task` values directly. Added focused coverage for invalid task and dependency IDs.

- `CARGO_HOME=/mnt/ssd2/uv-cache/cargo cargo test format` — 5 passed.
- `CARGO_HOME=/mnt/ssd2/uv-cache/cargo cargo test` — 14 passed.

Commit: `fix: validate task and dependency ids`
