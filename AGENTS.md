# tasks — agent guide

Rust CLI (`tasks`) that tracks work as one markdown file per task under `tasks/`.
This repo tracks itself with the same tool. Design: `docs/specs/2026-08-29-tasks-design.md`.

## Session protocol

- Start with `tasks prime`; pick from `tasks ready` (or `tasks next`); `tasks start <id>` before changing code.
- `tasks note <id> "<one line>"` when scope or understanding changes.
- `tasks park <id> "<next step>" [--waiting-on user]` when setting work down or waiting on the user; `start` resumes it.
- `tasks shelve <id> "<wake condition>"` for work to keep out of sight; `unshelve` brings it back.
- `tasks done <id> "<what landed>"` on the task's branch before it merges. `tasks check` before every commit.
- Recurring sweeps use `--every 30d`; `start` before closing each later occurrence.
- Decompose goals with `--parent`; close a goal from `prime`'s closeout list.
- Use `/scope [<id>... | --tag <tag>] [--project <prefix>]` for a deliberate idea review.
- Never edit `tasks/*.md` by hand; the binary is the only writer. Full protocol: `skills/tasks/SKILL.md`.
- File tool friction with `tasks feedback --project tasks`; in this repo, review feedback ideas without a triage note and redact with `tasks edit`.
- Demoing or smoke-testing `tasks init`? Use `XDG_CONFIG_HOME=$(mktemp -d)` so the scratch
  project does not leave a permanent entry in the real registry.

## Process and workspace

This repo adopts the process policy in `skills/tasks/SKILL.md`. The task's `process`
field, not a generic Superpowers trigger, decides whether brainstorming runs.
`direct` executes the scoped task or its reviewed plan without invoking brainstorming
or creating new design/plan documents. `planned` requires a written design spec
reviewed by the user, then a written implementation plan reviewed by the user,
before implementation. Reuse existing artifacts after verifying their contents and
review state; their presence alone is not approval. Both paths retain applicable
debugging, testing, verification, and code-review skills.

Before implementation, state the chosen process and workspace. When process is
unassessed, inspect the task and relevant code, record the choice with
`tasks edit <id> --process direct|planned`, and note the reason. Ideas still need
scoping. Discovery beyond a direct task's scope requires a note and reassessment
to planned before continuing implementation.

For either code path, commit the task record before git worktree add, then reuse its
isolated worktree or create one under .worktrees/; in a new worktree, run `tasks start <id>`
before any other `tasks` command for it; planned work does this before drafting
its spec. Immediately after creation, hydrate from the first declared source:
just setup when the justfile defines it; else the setup command the root guide names,
or one its README or CONTRIBUTING names that writes only inside the worktree; else the
frozen install of a lockfile that names its package manager and installs inside the
worktree (package-lock.json: `npm ci`; uv.lock: `uv sync --frozen`). A bare manifest
(pyproject.toml, package.json or requirements.txt without a lockfile), or a lockfile
whose install can leave the worktree (Poetry's default cache environment), declares no
usable installer: never guess one, and report the missing setup route before the first
commit. Hydrate before that commit. A checkout that declares no dependencies needs no
setup. Read-only investigation and task-record maintenance alone need no new
worktree. An explicit user instruction to work in place wins.

## Gates

    just gate

Tests: `just test-one <name>` for a test-name filter, or `just test-one --test cli <name>`
for one integration test; runner arguments are forwarded intact and at least one is
required. Run `just test-fast` before committing; it accepts no arguments and runs the
non-ignored suite. `just test` includes the slow `#[ignore]`d exhaustive enumeration.
No CI workflow runs the full suite on push (`ci_suite_refs` is empty), so the pre-push
hook carries it. Run `just test` yourself only if hooks are not installed or the fast
set does not cover affected behavior. Never run `cargo test` directly: the recipe records
the run and stops git discovery at `$TMPDIR`, so a stray `.git` there cannot fail the
fixtures. `just check` runs hygiene, `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`, and `tasks check`. Every recipe runs through
the vendored timing wrapper `tools/tt`, which records the run for the cross-project test
and CI audit (ops `docs/specs/2026-09-04-test-ci-audit-design.md`). The git hooks in
`.githooks/` run the check at pre-commit, or hygiene and `tasks check` alone when every
staged path matches `AGENTS.md` or `tasks/*.md` (renames count on both sides and failed
classification runs the full check). Pre-push runs the full gate unless every pushed
ref is covered by full-suite CI on `ci_remote`; on a fresh clone, run
`git config core.hooksPath .githooks` once.

Rebuild and reinstall after CLI changes so the tracker used above is the code under test:
`cargo install --path .`

## Layout

- `src/` — `main.rs` / `cli.rs` (clap), `src/commands/` (one module per subcommand, including `resolver.rs` for ids and paths), `model.rs`
  (task record), `frontmatter.rs`, `repo.rs` (tasks/ dir), `registry.rs` (`~/.config/tasks/projects.toml`),
  `locations.rs` (registry location history, path normalization, and the location index behind `resolve`),
  `claims.rs` (out-of-git work claims: the per-prefix store, liveness, and mutation lock),
  `attachments.rs` (tasks/files/<id>/ storage, the attached:/detached: ledger, and the audit behind show and check),
  `clipboard.rs` (wl-paste input for attach),
  `resolve.rs` (spec/plan links), `complete.rs` (shell completion candidates; best-effort, never errors), `query.rs`, `output.rs` / `format.rs` (JSON default, `--pretty`),
  `hierarchy.rs` (parent validation, subtree walks, forest).
- `tests/cli.rs` — end-to-end tests against the built binary in temp repos.
- `tests/attachments.rs` — end-to-end tests for attach, detach, show, check, and rename of task files.
- `justfile`, `tools/tt`, `.githooks/` — the test front door and its timing wrapper (see Gates).
- `tools/cli.toml` — the vendored CLI surface from ops (see `tools/README.md`): the shared
  vocabulary plus a row for every in-scope command and option. `src/surface.rs` builds the
  live parser surface, and its `parser_surface_equals_table` test asserts it matches the
  rows here.
- `skills/tasks/SKILL.md` — the agent skill shipped to other projects; keep it in step with CLI changes.
  `skills/curate/SKILL.md` — the curation pass (`tasks sample`, then bounded edits); same rule.
  `skills/scope/SKILL.md` — the deliberate idea scoping pass; same rule.
- `docs/specs/`, `docs/plans/` — design and plan docs; tasks link to them with `--spec` / `--plan --step`.

## Rules

- Any CLI change (a new subcommand, command, or option) starts on ops: land its rows in
  ops's `cli.toml` first, then run
  `python3 "$(tasks --pretty root ops-88a0fa)/bin/vendored" adopt cli.toml` in this
  checkout and commit the vendored copy together with the parser change. Never edit
  `tools/cli.toml` in place (ops-check refuses it) and never hand-copy a table into it:
  `parser_surface_equals_table` fails until the adopted table matches the parser.
- JSON output is the contract; `--pretty` is for humans. Never change JSON shapes without a task.
- Fail early with a typed error; no silent fallbacks.
- Conventional commits; no AI-attribution trailers.
