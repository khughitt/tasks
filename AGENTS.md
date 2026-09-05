# tasks — agent guide

Rust CLI (`tasks`) that tracks work as one markdown file per task under `tasks/`.
This repo tracks itself with the same tool. Design: `docs/specs/2026-08-29-tasks-design.md`.

## Session protocol

- Start with `tasks prime`; pick from `tasks ready` (or `tasks next`); `tasks start <id>` before changing code.
- `tasks note <id> "<one line>"` when scope or understanding changes.
- `tasks done <id> "<what landed>"` in the same commit as the code. `tasks check` before every commit.
- Decompose goals with `--parent`; close a goal from `prime`'s closeout list.
- Never edit `tasks/*.md` by hand; the binary is the only writer. Full protocol: `skills/tasks/SKILL.md`.
- File tool friction with `tasks feedback`; in this repo, review uncommitted feedback files before committing them.
- Demoing or smoke-testing `tasks init`? Use `XDG_CONFIG_HOME=$(mktemp -d)` so the scratch
  project does not leave a permanent entry in the real registry.

## Gates

    just gate

`just check` is the seconds-long part (`cargo fmt --check`, `cargo clippy --all-targets
-- -D warnings`, `tasks check`); `just test` is `cargo test`. Every recipe runs through
the vendored timing wrapper `tools/tt`, which records the run for the cross-project test
and CI audit (ops `docs/specs/2026-09-04-test-ci-audit-design.md`). The git hooks in
`.githooks/` run `check` at pre-commit and `gate` at pre-push; on a fresh clone, run
`git config core.hooksPath .githooks` once.

Rebuild and reinstall after CLI changes so the tracker used above is the code under test:
`cargo install --path .`

## Layout

- `src/` — `main.rs` / `cli.rs` (clap), `commands/` (one module per subcommand), `model.rs`
  (task record), `frontmatter.rs`, `repo.rs` (tasks/ dir), `registry.rs` (`~/.config/tasks/projects.toml`),
  `resolve.rs` (spec/plan links), `query.rs`, `output.rs` / `format.rs` (JSON default, `--pretty`),
  `hierarchy.rs` (parent validation, subtree walks, forest).
- `tests/cli.rs` — end-to-end tests against the built binary in temp repos.
- `justfile`, `tools/tt`, `.githooks/` — the test front door and its timing wrapper (see Gates).
- `skills/tasks/SKILL.md` — the agent skill shipped to other projects; keep it in step with CLI changes.
- `docs/specs/`, `docs/plans/` — design and plan docs; tasks link to them with `--spec` / `--plan --step`.

## Rules

- JSON output is the contract; `--pretty` is for humans. Never change JSON shapes without a task.
- Fail early with a typed error; no silent fallbacks.
- Conventional commits; no AI-attribution trailers.
