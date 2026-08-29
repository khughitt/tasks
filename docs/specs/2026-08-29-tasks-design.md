# tasks — design

**Status:** approved design, not yet implemented (2026-08-29).

## 1. Purpose

A fast, file-based task tracker for software projects, used mostly by coding agents and
occasionally by humans. It complements — does not replace — the markdown design specs and
implementation plans that projects already keep. Each task is one markdown file checked
into the project's repo; the CLI is the only writer.

Goals:

- Simple, discoverable CLI: `tasks add`, `tasks show`, `tasks list`, `tasks ready`, …
- Tasks serialized as markdown with YAML frontmatter; readable with `cat`, diffable, mergeable.
- Agent-first: JSON output by default, `tasks prime` for session context, `tasks ready` for
  "what can I work on now", cheap to call hundreds of times per session.
- First-class links to design specs and implementation plans, with drift detection.
- Task dependencies, including across projects.
- Safe under parallel work: two agents in two worktrees never collide on ids or files.

Non-goals (deliberately):

- No database, cache, or index. Every command scans `tasks/*.md`.
- No hierarchy (epics/subtasks), no `split` command, no scheduling math.
- No locking or atomic claims; claims are advisory and git is the arbiter.
- No delete; `drop` closes a task and keeps its history.

## 2. Storage layout

Per repository, checked in:

```
tasks/
  .config.toml        project prefix and doc directories
  sci-4f2a.md         one file per task; filename == id
  sci-91be.md
docs/
  specs/              design specs   (YYYY-MM-DD-<topic>-design.md)
  plans/              implementation plans (YYYY-MM-DD-<topic>.md)
```

`docs/specs` and `docs/plans` are a fixed convention, not configurable. `tasks init`
creates them if absent. Projects that keep specs/plans elsewhere are expected to move them
(a one-time `git mv`); see §9.

Per machine, not checked in: `~/.config/tasks/projects.toml`, the project registry (§6).

## 3. Task file format

```markdown
---
id: sci-4f2a
title: Bank the holdings ledger
status: todo
priority: 2
size: m
owner: keith
created: 2026-08-29T14:02:11Z
updated: 2026-08-29T14:02:11Z
depends: [sci-91be, fam-0c3d]
tags: [world-index, cut-12]
spec: docs/specs/2026-08-24-world-index-holdings-design.md
plan: docs/plans/2026-08-24-holdings.md
step: "Task 3: emit the ledger row"
---

Free-form markdown body.

## Notes

- 2026-08-29T15:10:44Z (keith): started; the spec's §4 assumption no longer holds.
- 2026-08-29T16:41:02Z (slice-12): split the emitter into sci-a7d1.
```

### 3.1 Fields

| Field      | Type                | Required | Notes |
|------------|---------------------|----------|-------|
| `id`       | `<prefix>-<hex4>`   | yes      | Immutable. Must equal the filename stem. |
| `title`    | string              | yes      | One line. |
| `status`   | enum                | yes      | `idea`, `todo`, `doing`, `blocked`, `done`, `dropped`. |
| `priority` | int 0–4             | yes      | 0 = most urgent. Default 2. |
| `size`     | enum                | no       | `xs`, `s`, `m`, `l`, `xl`. |
| `owner`    | string              | no       | Advisory claim; set by `start`. |
| `created`  | RFC 3339 UTC        | yes      | Set once by `add`. |
| `updated`  | RFC 3339 UTC        | yes      | Set by every write command. |
| `depends`  | list of ids         | yes      | May be empty. Foreign prefixes allowed (§6). |
| `tags`     | list of strings     | yes      | May be empty. The only grouping mechanism. |
| `spec`     | repo-relative path  | no       | A file under `docs/specs/`. |
| `plan`     | repo-relative path  | no       | A file under `docs/plans/`. |
| `step`     | string              | no       | Exact text of a heading inside `plan`. Requires `plan`. |

Unknown keys are an error. Times are UTC RFC 3339 with second precision.

### 3.2 Body and notes

Everything between the frontmatter and the `## Notes` heading is the body, editable by
`edit`. The `## Notes` section is owned by the tool: it holds only bullets of the form
`- <timestamp> (<owner>): <text>`, appended by `note` and by `start`/`done`/`drop` messages.
Any other content after `## Notes` is a `check` error. The section is created on the first
note.

### 3.3 Lifecycle

```
idea ──► todo ──► doing ──► done
           │        │
           ▼        ▼
        blocked  dropped   (blocked/dropped reachable from idea/todo/doing)
```

- `idea`: unscoped; never appears in `ready`. Promote with `edit --status todo`.
- `blocked`: explicit human/agent judgment, distinct from "has open dependencies".
- `done` and `dropped` are terminal. Both count as closed for dependency purposes.

`ready` = status `todo` and every entry in `depends` is closed.

## 4. Identity and collisions

Ids are `<prefix>-<hex4>`: the repo's prefix from `.config.toml` plus four random lowercase
hex digits (65,536 per project). `add` regenerates on a local collision. There is no
counter and no index file, so parallel `add`s in different worktrees produce distinct files
and merge cleanly. Ids never change; titles may.

The prefix is the project name for cross-project references: `fam-0c3d` in a science task
means task `0c3d` in the project registered as `fam`.

## 5. CLI

Every command locates the project by walking up from the current directory to the nearest
`tasks/.config.toml`; `-C <path>` overrides. Output is JSON by default; `--pretty` (or
`TASKS_FORMAT=pretty`) renders tables and full text for humans. TTY detection is not used.
Write commands print the affected id and nothing else.

```
tasks init [--prefix P]
    Create tasks/.config.toml, docs/specs/, docs/plans/ (if absent), and register the
    project in ~/.config/tasks/projects.toml. Prefix defaults to the first three letters
    of the repo directory name; errors if the prefix is already registered to another path.

tasks add <title> [-b|--body TEXT] [--status idea|todo] [-p N] [--size S]
          [--tag T]... [--depends ID]... [--spec NAME] [--plan NAME] [--step TEXT]
    Create a task. Default status todo, priority 2. --spec/--plan accept either a
    repo-relative path or a bare name resolved as the unique docs/<kind>/*NAME*.md
    (error on 0 or >1 matches). --depends ids and --step headings are validated before
    anything is written.

tasks show <id>
    The full task, with spec/plan resolved to absolute paths, each dependency's title and
    status, and whether the step heading still resolves.

tasks list [--status S]... [--tag T]... [--owner O] [--all-projects]
    Default: open tasks (not done/dropped), sorted by priority then updated desc.
    --all-projects walks the registry.

tasks ready [--size S] [-n N]
    Actionable tasks: todo with all dependencies closed. Sorted by priority, then size
    (xs first, unsized last), then created.

tasks edit <id> [same field flags as add] [--body -]
    With flags: update those fields. Without flags: open the file in $EDITOR, then
    re-validate it (invalid edits are rejected and the original restored).

tasks note <id> <text>
    Append a timestamped bullet under ## Notes.

tasks start <id>
    status=doing, owner=$TASKS_OWNER, else the current git branch name, else $USER.

tasks done <id> [message]
    status=done; message appended as a note. Refuses when any dependency is open,
    unless --force.

tasks drop <id> [message]
    status=dropped; message appended as a note.

tasks dep <id> --on <id>...  |  tasks dep <id> --rm <id>...
    Add or remove dependencies. --on rejects cycles and unresolvable ids.

tasks graph [--format mermaid|dot] [--all]
    Dependency graph of open tasks (--all includes closed). Nodes carry id, title,
    status, priority, size.

tasks check
    Validate every task file: frontmatter schema, filename == id, timestamps, dangling
    depends/spec/plan, missing step heading, dependency cycles, malformed notes section.
    Exit 1 on any error. Unresolvable foreign ids (unregistered prefix) are warnings.

tasks prime
    Agent session context: prefix, counts by status, the ready list, and doing tasks
    with owners. Intended to be run at the start of every agent session.
```

## 6. Configuration and registry

`tasks/.config.toml` (checked in):

```toml
prefix = "sci"
specs = "docs/specs"   # informational; the convention is fixed
plans = "docs/plans"
```

`~/.config/tasks/projects.toml` (per machine; written by `init`, hand-editable):

```toml
[projects]
sci = "~/d/science"
fam = "~/d/familiar"
```

A foreign id `<p>-<hex>` resolves to `<projects.p>/tasks/<p>-<hex>.md`. The registry points
at each project's main checkout, so a worktree reading a foreign dependency sees that
project's main branch — accepted as the right approximation. Unregistered prefixes are a
warning in `show`/`check`/`list` and an error in `dep --on` and `add --depends`.

## 7. Spec and plan linkage; drift

- A task may point at a spec (`spec`), a plan (`plan`), and a heading inside that plan
  (`step`). These are validated on write and re-validated by `check`.
- `check` fails when: a linked file is missing; a `step` heading no longer appears verbatim
  in its plan; `step` is set without `plan`.
- Running `check` in a project's test or pre-commit path turns doc drift under open tasks
  into a build failure, which is the intended coupling: when a plan step is renamed or
  removed, the task must be updated in the same change.

## 8. Agent guidance (SKILL.md)

The repo ships `skills/tasks/SKILL.md`. It is installable at either level; both are
documented in the README:

- **User level** (applies to every project): copy or symlink `skills/tasks/` into
  `~/.claude/skills/tasks/` and/or `~/.agents/skills/tasks/` (whichever harnesses are in
  use). Recommended, since the tool is meant to be used across many projects.
- **Project level**: copy into `<repo>/.claude/skills/tasks/` (or the harness's project
  skills dir), or reference it from the project's CLAUDE.md/AGENTS.md. Use this when a
  project needs to pin a specific skill version or a contributor doesn't have the user-level
  install.

`tasks init` prints the install hint if no skill is found at either level.

Skill content:

1. **Session protocol**: run `tasks prime`; choose from `ready`; `tasks start` before
   changing code; `tasks note` when scope or understanding changes; `tasks done <id> "…"`
   with a message in the same commit as the code. Never edit `tasks/*.md` by hand.
2. **Recipes**: recording an idea vs. a scoped task; splitting a task (add the pieces,
   `dep <original> --on <pieces>` or drop the original, and leave a note); blocking on
   another project's task.
3. **Superpowers integration** (applies when the superpowers plugin is present):
   - Brainstorming writes specs to `docs/specs/YYYY-MM-DD-<topic>-design.md`. After
     approval, add one task per major deliverable with `--spec <topic>`.
   - writing-plans writes plans to `docs/plans/YYYY-MM-DD-<topic>.md`. Add one task per
     `Task N:` heading with `--plan <topic> --step "Task N: …"` and `--depends` mirroring
     the plan's order.
   - executing-plans and subagent-driven-development call `tasks start`/`done` per step.
   - `tasks check` in CI is the drift limiter (§7).

## 9. Adoption in existing projects

Manual, documented steps — the tool does not move files:

1. Move specs to `docs/specs/` and plans to `docs/plans/` (for example, `git mv docs/designs
   docs/specs`, `git mv docs/superpowers/plans/* docs/plans/`), fixing links in the README.
2. `tasks init --prefix <p>`.
3. Install the skill (user level preferred) and add a line to CLAUDE.md pointing at it.
4. Add `tasks check` to the test script or pre-commit hook.

Projects with an incompatible docs layout (e.g. an mkdocs tree) can still `init`; only
`--spec`/`--plan` bare-name resolution assumes the convention, and explicit paths always work.

## 10. Implementation

- Rust, single static binary, edition 2024. Crates: `clap` (derive), `serde` +
  `serde_yaml`, `toml`, `time`, `rand`, `thiserror`/`anyhow`.
- Modules: `model` (Task, Status, Size, id parsing), `format` (parse/serialize frontmatter
  + body + notes, round-trip exact), `repo` (locate project, scan, write), `registry`,
  `query` (ready, graph, cycle detection), `check`, `cli` (clap + JSON/pretty rendering).
- Writes: build the full file in memory, write `tasks/.<id>.tmp`, rename over the target.
  No partial files.
- Errors: a single typed error enum; JSON `{"error": {"kind", "detail"}}` with exit 1;
  clap usage errors exit 2.
- Tests: unit tests for format round-trip, id generation, ready/cycle logic; integration
  tests via `assert_cmd` + `tempfile` running the binary against temp repos with a fake
  `HOME` for the registry, including a two-project cross-reference case.

## 11. Open questions

None blocking. Candidates for later, only if they prove necessary: a `promote` verb,
an `xs`-only "quick wins" view, `tasks graph` critical-path annotation, a fifth hex digit for
very large projects.
