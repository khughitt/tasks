# Front door for tests. Focused: `just test-one <runner args>`. Fast: `just test-fast`.
# Full suite: `just test`. Gates: checks at pre-commit, check + suite at pre-push.
# Hooks time mandatory hygiene once; broad children record their own policy decisions.
# Leaf recipes run through tools/tt (source: ops bin/tt); compositions are untimed.
# Design: ops docs/specs/2026-09-28-test-ci-act-design.md.

set quiet
set positional-arguments

tt := "python3 tools/tt"

# Commands, each written once. Recipes and hooks all run these, so a hook can
# never drift from the gate it is supposed to be. Avoid single quotes inside them.
# This is a single Rust crate, so there is no affected-only selection; the two grains
# it does have are the ones the baseline showed being used. `test-one` accepts runner
# arguments; `test-fast` runs the fixed non-ignored set: the exhaustive rename::classify
# enumeration alone costs ~17 s in debug. `test` runs everything, ignored included.
#
# Every test command first stops git discovery at the temp directory, so a stray `.git`
# in $TMPDIR (some host process leaves an empty one) cannot fail the fixtures' temp
# projects; tests that set their own ceiling still override it.
test_env := "ceiling=$(realpath -e -- \"${TMPDIR:-/tmp}\") && GIT_CEILING_DIRECTORIES=$ceiling"
fast_cmd := test_env + " cargo test --no-fail-fast"
test_cmd := test_env + " cargo test --no-fail-fast -- --include-ignored"
check_hygiene_cmd := "python3 tools/ops-check && python3 tools/ops-docs check && tasks check"
ci_hygiene_cmd := "python3 tools/ops-check && python3 tools/ops-docs check"
check_code_cmd := "cargo fmt --check && cargo clippy --all-targets -- -D warnings"

# README and design documents stay on the full check; only the agent guide skips Rust checks.
docs_paths := "AGENTS.md"
docs_check_cmd := check_hygiene_cmd

# Local gate retained until the hosted pilot is accepted and policy activation approved.
ci_suite_refs := ""
ci_remote := "origin"
one_cmd := test_env + " cargo test"

# Keep generated directories off the synced tree on hosts with work-link.
work-link *names:
    {{tt}} work-link -- sh -c 'if command -v work-link >/dev/null; then work-link --ensure {{names}}; fi'

# Hydrate a fresh checkout or worktree; safe to rerun after dependency changes.
setup:
    {{tt}} setup -- sh -c 'just work-link target && cargo fetch --locked'

# The inner loop: the whole non-ignored suite, with no arguments.
test-fast:
    {{tt}} test-fast -- sh -c '{{fast_cmd}}'

# A name filter or cargo runner arguments, forwarded unchanged; at least one is required.
test-one +args:
    {{tt}} test-one -- sh -c '{{one_cmd}} "$@" 2>&1' test-one "$@"

# The full suite, ignored tests included.
test:
    {{tt}} test -- sh -c '{{test_cmd}}'

# Deliberate full-suite verification; only the inner test target records timing.
test-full:
    TT_FORCE=1 just test

# Mandatory local hygiene, including detached task-record consistency.
check-hygiene:
    {{tt}} check-hygiene -- sh -c '{{check_hygiene_cmd}}'

# Hosted runners have no private detached task records.
ci-check-hygiene:
    {{tt}} ci-check-hygiene -- sh -c '{{ci_hygiene_cmd}}'

# Broad Rust static checks across all targets.
check-code:
    {{tt}} check-code -- sh -c '{{check_code_cmd}}'

check:
    just check-hygiene
    just check-code

gate: check test

# Regenerate the identity regions in README.md and AGENTS.md from identity.toml. The only
# thing that edits those regions; `check` refuses a commit while they are out of date.
# Design: ops docs/specs/2026-09-22-project-guides-design.md §5.
docs:
    {{tt}} docs -- python3 tools/ops-docs write

# Link every skills/* directory into ~/.claude/skills and ~/.agents/skills. Idempotent;
# re-run after a pull adds a skill.
install-skills:
    {{tt}} install-skills -- sh -c 'set -e; for dest in "$HOME/.claude/skills" "$HOME/.agents/skills"; do mkdir -p "$dest"; for skill in "{{justfile_directory()}}"/skills/*; do [ -d "$skill" ] || continue; name=$(basename "$skill"); ln -sfn "$skill" "$dest/$name"; echo "$dest/$name"; done; done'

# Hook timers cover mandatory hygiene only; broad children record their own decisions.
# Split-hook durations are incomparable with the earlier whole-hook series.
hook-pre-commit:
    {{tt}} hook-pre-commit -- sh -c '{{check_hygiene_cmd}}'
    just check-code

# Guide/task-only changes retain hygiene and task consistency checks.
hook-pre-commit-docs:
    {{tt}} hook-pre-commit-docs -- sh -c '{{docs_check_cmd}}'

# Uncovered pushes retain the same children as gate.
hook-pre-push:
    {{tt}} hook-pre-push -- sh -c '{{check_hygiene_cmd}}'
    just check-code
    just test

# CI carries the full suite only when every pushed ref is covered on ci_remote.
hook-pre-push-fast:
    {{tt}} hook-pre-push-fast -- sh -c '{{check_hygiene_cmd}}'
    just check-code
    just test-fast
