# tasks — agent guide

Rust CLI (`tasks`) that tracks work as one markdown file per task under `tasks/`.
This repo tracks itself with the same tool. Design: `docs/specs/2026-08-29-tasks-design.md`.

## Session protocol

- Start with `tasks prime`; pick from `tasks ready` (or `tasks next`); `tasks start <id>` before changing code.
- `tasks note <id> "<one line>"` when scope or understanding changes.
- `tasks park <id> "<next step>" [--waiting-on user]` when setting work down or waiting on the user; `start` resumes it.
- `tasks shelve <id> "<wake condition>"` for work to keep out of sight; `unshelve` brings it back.
- `tasks done <id> "<what landed>"` in the same commit as the code. `tasks check` before every commit.
- Recurring sweeps use `--every 30d`; `start` before closing each later occurrence.
- Decompose goals with `--parent`; close a goal from `prime`'s closeout list.
- Use `/scope [<id>... | --tag <tag>] [--project <prefix>]` for a deliberate idea review.
- Never edit `tasks/*.md` by hand; the binary is the only writer. Full protocol: `skills/tasks/SKILL.md`.
- File tool friction with `tasks feedback`; in this repo, review uncommitted feedback files before committing them.
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

For either code path, reuse the task's isolated worktree or create one with
`git worktree add` under .worktrees/; planned work does this before drafting its
spec. Run just setup immediately after creation when the justfile defines it.
Read-only investigation and task-record maintenance alone need no new worktree.
An explicit user instruction to work in place wins.

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

- `src/` — `main.rs` / `cli.rs` (clap), `src/commands/` (one module per subcommand), `model.rs`
  (task record), `frontmatter.rs`, `repo.rs` (tasks/ dir), `registry.rs` (`~/.config/tasks/projects.toml`),
  `claims.rs` (out-of-git work claims: the per-prefix store, liveness, and mutation lock),
  `resolve.rs` (spec/plan links), `complete.rs` (shell completion candidates; best-effort, never errors), `query.rs`, `output.rs` / `format.rs` (JSON default, `--pretty`),
  `hierarchy.rs` (parent validation, subtree walks, forest).
- `tests/cli.rs` — end-to-end tests against the built binary in temp repos.
- `justfile`, `tools/tt`, `.githooks/` — the test front door and its timing wrapper (see Gates).
- `skills/tasks/SKILL.md` — the agent skill shipped to other projects; keep it in step with CLI changes.
  `skills/curate/SKILL.md` — the curation pass (`tasks sample`, then bounded edits); same rule.
  `skills/scope/SKILL.md` — the deliberate idea scoping pass; same rule.
- `docs/specs/`, `docs/plans/` — design and plan docs; tasks link to them with `--spec` / `--plan --step`.

## Rules

- JSON output is the contract; `--pretty` is for humans. Never change JSON shapes without a task.
- Fail early with a typed error; no silent fallbacks.
- Conventional commits; no AI-attribution trailers.
