# Front door for tests. Full suite: `just test`. Gates: `just check` (seconds) at
# pre-commit, `just gate` (check + suite) at pre-push. The git hooks in .githooks/ call
# `hook-pre-commit` and `hook-pre-push`, which run the very same commands under their own
# target names so the report can price the hooks separately from runs typed by hand.
# Every recipe runs through the vendored timing wrapper tools/tt (source: ops bin/tt).
# Design: ops docs/specs/2026-09-04-test-ci-audit-design.md.

set quiet

tt := "python3 tools/tt"

# The three commands, each written once. Recipes and hooks all run these, so a hook can
# never drift from the gate it is supposed to be. Avoid single quotes inside them.
# This is a single Rust crate, so there is no affected-only selection; the two grains
# it does have are the ones the baseline showed being used. `test-fast` takes an
# optional name filter, because every recorded bypass was `cargo test <name>` for one
# test, and it skips tests marked `#[ignore]`: the exhaustive rename::classify
# enumeration alone costs ~7 s in debug. `test` runs everything, ignored included.
fast_cmd := "cargo test"
test_cmd := "cargo test -- --include-ignored"
check_cmd := "python3 tools/ops-check && cargo fmt --check && cargo clippy --all-targets -- -D warnings && tasks check"

# The inner loop: the non-ignored suite, or the tests whose names contain `filter`.
test-fast *filter:
    {{tt}} test-fast -- sh -c '{{fast_cmd}} {{filter}}'

# The full suite, ignored tests included.
test:
    {{tt}} test -- sh -c '{{test_cmd}}'

# Seconds, not minutes: format, lint, tasks check.
check:
    {{tt}} check -- sh -c '{{check_cmd}}'

gate: check test

# Link every skills/* directory into ~/.claude/skills and ~/.agents/skills. Idempotent;
# re-run after a pull adds a skill.
install-skills:
    {{tt}} install-skills -- sh -c 'set -e; for dest in "$HOME/.claude/skills" "$HOME/.agents/skills"; do mkdir -p "$dest"; for skill in "{{justfile_directory()}}"/skills/*; do [ -d "$skill" ] || continue; name=$(basename "$skill"); ln -sfn "$skill" "$dest/$name"; echo "$dest/$name"; done; done'

# What the pre-commit hook runs: `check`'s command under its own hook target.
hook-pre-commit:
    {{tt}} hook-pre-commit -- sh -c '{{check_cmd}}'

# What the pre-push hook runs: the same commands as `gate`, under one hook target.
hook-pre-push:
    {{tt}} hook-pre-push -- sh -c '{{check_cmd}} && {{test_cmd}}'
