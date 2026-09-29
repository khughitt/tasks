# Front door for tests. Focused: `just test-one <runner args>`. Fast: `just test-fast`.
# Full suite: `just test`. Gates: checks at pre-commit, check + suite at pre-push.
# The git hooks in .githooks/ call
# `hook-pre-commit` and `hook-pre-push`, which run the very same commands under their own
# target names so the report can price the hooks separately from runs typed by hand.
# Every recipe runs through the vendored timing wrapper tools/tt (source: ops bin/tt).
# Design: ops docs/specs/2026-09-28-test-ci-act-design.md.

set quiet
set positional-arguments

tt := "python3 tools/tt"

# The three commands, each written once. Recipes and hooks all run these, so a hook can
# never drift from the gate it is supposed to be. Avoid single quotes inside them.
# This is a single Rust crate, so there is no affected-only selection; the two grains
# it does have are the ones the baseline showed being used. `test-one` accepts runner
# arguments; `test-fast` runs the fixed non-ignored set: the exhaustive rename::classify
# enumeration alone costs ~7 s in debug. `test` runs everything, ignored included.
fast_cmd := "cargo test"
test_cmd := "cargo test -- --include-ignored"
check_cmd := "python3 tools/ops-check && cargo fmt --check && cargo clippy --all-targets -- -D warnings && tasks check"

# README and design documents stay on the full check; only guide/task records skip Rust checks.
docs_paths := "AGENTS.md tasks/*.md"
docs_check_cmd := "python3 tools/ops-check && tasks check"

# No CI workflow: every push retains the full local gate.
ci_suite_refs := ""
ci_remote := "origin"
# Fixed non-ignored set: this single crate has no base-aware affected selection.
push_fast_cmd := fast_cmd
one_cmd := "cargo test"

# The inner loop: the whole non-ignored suite, with no arguments.
test-fast:
    {{tt}} test-fast -- sh -c '{{fast_cmd}}'

# A name filter or cargo runner arguments, forwarded unchanged; at least one is required.
test-one +args:
    {{tt}} test-one -- sh -c '{{one_cmd}} "$@" 2>&1' test-one "$@"

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

# Guide/task-only changes retain hygiene and task consistency checks.
hook-pre-commit-docs:
    {{tt}} hook-pre-commit-docs -- sh -c '{{docs_check_cmd}}'

# What the pre-push hook runs: the same commands as `gate`, under one hook target.
hook-pre-push:
    {{tt}} hook-pre-push -- sh -c '{{check_cmd}} && {{test_cmd}}'

# CI carries the full suite only when every pushed ref is covered on ci_remote.
hook-pre-push-fast:
    {{tt}} hook-pre-push-fast -- sh -c '{{check_cmd}} && {{push_fast_cmd}}'
