# Front door for tests. Full suite: `just test`. Gates: `just check` (seconds) at
# pre-commit, `just gate` (check + suite) at pre-push. The git hooks in .githooks/ call
# `hook-pre-commit` and `hook-pre-push`, which run the very same commands under their own
# target names so the report can price the hooks separately from runs typed by hand.
# Every recipe runs through the vendored timing wrapper tools/tt (source: ops bin/tt).
# Design: ops docs/specs/2026-09-04-test-ci-audit-design.md.

tt := "python3 tools/tt"

# The three commands, each written once. Recipes and hooks all run these, so a hook can
# never drift from the gate it is supposed to be. Avoid single quotes inside them.
# This is a single Rust crate with one integration-test binary, so there is no finer
# grain to select: the fast target is the suite. The report shows the gap as equal
# durations; a curated subset is the follow-up if that gap costs too much.
fast_cmd := "cargo test"
test_cmd := "cargo test"
check_cmd := "cargo fmt --check && cargo clippy --all-targets -- -D warnings && tasks check"

# The inner loop; here, the whole suite.
test-fast:
    {{tt}} test-fast -- sh -c '{{fast_cmd}}'

# The full suite.
test:
    {{tt}} test -- sh -c '{{test_cmd}}'

# Seconds, not minutes: format, lint, tasks check.
check:
    {{tt}} check -- sh -c '{{check_cmd}}'

gate: check test

# What the pre-commit hook runs: `check`'s command under its own hook target.
hook-pre-commit:
    {{tt}} hook-pre-commit -- sh -c '{{check_cmd}}'

# What the pre-push hook runs: the same commands as `gate`, under one hook target.
hook-pre-push:
    {{tt}} hook-pre-push -- sh -c '{{check_cmd}} && {{test_cmd}}'
