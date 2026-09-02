# tasks — design

**Status:** implemented (2026-08-29; spec roots extended 2026-09-02); see
docs/plans/2026-08-29-tasks.md.

## 1. Purpose

A fast, file-based task tracker for software projects, used mostly by coding agents and
occasionally by humans. It complements — does not replace — the markdown design specs and
implementation plans that projects already keep. Each task is one markdown file checked
into the project's repo; the CLI is the only writer.

Goals:

- Simple, discoverable CLI: `tasks add`, `tasks show`, `tasks list`, `tasks ready`, …
- Tasks serialized as markdown with YAML-like frontmatter; readable with `cat`, diffable, mergeable.
- Agent-first: JSON output by default, `tasks prime` for session context, `tasks ready` for
  "what can I work on now", cheap to call hundreds of times per session.
- First-class links to design specs and implementation plans, with drift detection.
- Task dependencies, including across projects.
- Safe under parallel work: agents in separate worktrees share no counter or index file, so
  concurrent additions merge without conflict except in the id-collision case defined in §4.

Non-goals (deliberately):

- No database, cache, or index. Every command scans `tasks/*.md`.
- No hierarchy (epics/subtasks), no `split` command, no scheduling math.
- No locking or atomic claims; claims are advisory and git is the arbiter.
- No delete; `drop` closes a task and keeps its history.

## 2. Storage layout

Per repository, checked in:

```
tasks/
  .config.toml        project prefix
  sci-4f2a9c.md       one file per task; filename == id
  sci-91be03.md
docs/
  specs/              design specs   (YYYY-MM-DD-<topic>-design.md)
  designs/            alternative accepted spec root
  superpowers/
    specs/            alternative accepted spec root
    designs/          alternative accepted spec root
  plans/              implementation plans (YYYY-MM-DD-<topic>.md)
```

A task's `spec` must be under `docs/specs/`, `docs/designs/`,
`docs/superpowers/specs/`, or `docs/superpowers/designs/`; its `plan` must be under
`docs/plans/`. This is enforced on every write and by `check`; there is no configuration
to relocate them. `tasks init` creates the default `docs/specs/` and `docs/plans/`
directories if absent. Historical docs need not move unless they fall outside every
accepted root and need structured `spec`/`plan` links (§9).

Per machine, not checked in: `~/.config/tasks/projects.toml`, the project registry (§6).

## 3. Task file format

```markdown
---
id: sci-4f2a9c
title: Bank the holdings ledger
status: todo
priority: 2
size: m
owner: keith
created: 2026-08-29T14:02:11Z
updated: 2026-08-29T14:02:11Z
depends: [sci-91be03, fam-0c3d7e]
tags: [world-index, cut-12]
spec: docs/specs/2026-08-24-world-index-holdings-design.md
plan: docs/plans/2026-08-24-holdings.md
step: "Task 3: emit the ledger row"
---

Free-form markdown body.

## Notes

- 2026-08-29T15:10:44Z (keith): started; the spec's §4 assumption no longer holds.
- 2026-08-29T16:41:02Z (slice-12): split the emitter into sci-a7d1e2.
```

### 3.1 Fields

| Field      | Type                | Required | Notes |
|------------|---------------------|----------|-------|
| `id`       | `<prefix>-<hex6>`   | yes      | Immutable. Must equal the filename stem. |
| `title`    | string              | yes      | One line; no newlines. |
| `status`   | enum                | yes      | `idea`, `todo`, `doing`, `blocked`, `done`, `dropped`. |
| `priority` | int 0–4             | yes      | 0 = most urgent. Default 2. |
| `size`     | enum                | no       | `xs`, `s`, `m`, `l`, `xl`. |
| `owner`    | string              | no       | Advisory claim; set by `start`; `[A-Za-z0-9._/@+-]+`. |
| `created`  | RFC 3339 UTC        | yes      | Set once by `add`. Immutable. |
| `updated`  | RFC 3339 UTC        | yes      | Set by every write command. |
| `depends`  | list of ids         | yes      | May be empty. Foreign prefixes allowed (§6). |
| `tags`     | list of strings     | yes      | May be empty. The only grouping mechanism. |
| `spec`     | repo-relative path  | no       | Must be an existing file under an accepted spec or design root (§2). |
| `plan`     | repo-relative path  | no       | Must be an existing file under `docs/plans/`. |
| `step`     | string              | no       | Exact text of a heading inside `plan`. Requires `plan`. |

Unknown keys are an error. Every scalar and list item is a single line. Times are UTC RFC
3339 with second precision.

### 3.2 Body and notes

The line `## Notes` (exactly: level-2 heading, that text, nothing else on the line) is
reserved as the delimiter. Everything between the frontmatter and that line is the body,
editable by `edit`; a body containing the reserved line is rejected on write and by `check`.
The serializer's one structural newline after the closing frontmatter delimiter and trailing
serialization whitespace are not body content; all other leading whitespace and blank lines are.

The section after the delimiter is owned by the tool and holds only bullets of the form
`- <timestamp> (<owner>): <text>`, appended by `note` and by the messages of
`start`/`done`/`drop`/`block`. Note text is a single line; `note` rejects text containing
newlines. Any other content after the delimiter is a `check` error. The section is created
on the first note.

### 3.3 Lifecycle

Statuses are *open* (`idea`, `todo`, `doing`, `blocked`) or *closed* (`done`, `dropped`).
Closed counts as satisfied for dependency purposes.

Allowed transitions, enforced by every write path including `edit --status` and the
editor path:

| From      | To                                   |
|-----------|--------------------------------------|
| any open  | any other open status                |
| any open  | `done` (subject to the open-deps rule), `dropped` |
| `done`, `dropped` | `todo` only (reopen)         |

Open-deps rule: a task may become `done` only when every dependency is closed, unless
`--force` is given. It applies to `done` and to `edit --status done` alike.

- `idea`: unscoped; never appears in `ready`.
- `blocked`: explicit judgment, distinct from "has open dependencies".

`ready` = status `todo` and every entry in `depends` is closed.

## 4. Identity and collisions

Ids are `<prefix>-<hex6>`: the repo's prefix from `.config.toml` plus six random lowercase
hex digits (16,777,216 per project). `add` regenerates on a collision with an existing
local file.

Collision policy: the retry cannot see ids created concurrently in another worktree or
branch. The probability that two concurrent additions choose the same id is 1/16,777,216
per pair; it is small, not zero. If it happens, git reports an add/add conflict on the same
`tasks/<id>.md` path at merge time. Resolution is manual: keep one file, rename the other
to a fresh id (`tasks reid <old> <new>` is not provided; edit the filename and the `id`
field, then fix any `depends` references — `check` reports the dangling ones). Ids never
change otherwise; titles may.

The prefix is the project name for cross-project references: `fam-0c3d7e` in a science task
means task `0c3d7e` in the project registered as `fam`.

## 5. CLI

Every command locates the project by walking up from the current directory to the nearest
`tasks/.config.toml`; `-C <path>` overrides. Output is JSON by default; `--pretty` (or
`TASKS_FORMAT=pretty`) renders tables and full text for humans. TTY detection is not used.

```
tasks init [--prefix P]
    Create tasks/.config.toml, docs/specs/, docs/plans/ (if absent), and register the
    project in ~/.config/tasks/projects.toml. Prefix defaults to the first three letters
    of the repo directory name; rerunning with the same prefix is an idempotent repair and
    a different prefix is an error.
    Prints the skill install hint (§8) if no skill is found at user or project level.

tasks add <title> [-b|--body TEXT] [--status idea|todo] [-p N] [--size S]
          [--tag T]... [--depends ID]... [--spec NAME] [--plan NAME] [--step TEXT]
    Create a task. Default status todo, priority 2. --spec/--plan accept either a
    repo-relative path under an accepted directory or a bare name resolved as the unique
    match across the accepted directories (error on 0 or >1 matches). --depends ids and
    --step headings are validated before anything is written.

tasks show <id>
    The full task with resolved spec/plan paths, each dependency's title and status,
    and whether the step heading still resolves.

tasks list [--status S]... [--tag T]... [--owner O] [--all-projects]
    Default: open tasks, sorted by priority then updated desc. --all-projects walks the
    registry.

tasks ready [--size S] [-n N]
    Actionable tasks: todo with all dependencies closed. Sorted by priority, then size
    (xs first, unsized last), then created.

tasks edit <id> [same field flags as add] [--status S] [--body -] [--force]
    With flags: update those fields. Without flags: open an editable copy in $EDITOR
    (§5.2). Either way the result is validated against §3 and the invariants in §5.3
    before it replaces the original.

tasks note <id> <text>
    Append a timestamped bullet under ## Notes.

tasks start <id>
    status=doing, owner=$TASKS_OWNER, else the current git branch name, else $USER.

tasks done <id> [message] [--force]
    status=done; message appended as a note. Refuses under the open-deps rule.

tasks drop <id> [message]
    status=dropped; message appended as a note.

tasks block <id> [message]
tasks unblock <id>
    status=blocked (message appended as a note) / status=todo.

tasks dep <id> --on <id>...  |  tasks dep <id> --rm <id>...
    Add or remove dependencies. --on rejects cycles (§6.1) and unresolvable ids.

tasks graph [--format mermaid|dot] [--all]
    Dependency graph of open tasks (--all includes closed).

tasks check
    Validate every task file: frontmatter schema, filename == id, timestamps, dangling
    depends/spec/plan, paths outside the accepted directories, missing step heading,
    dependency cycles, reserved delimiter in body, malformed notes section. Exit 1 on any
    error. Unresolvable foreign ids (unregistered or unreachable prefix) are warnings.

tasks prime
    Agent session context: prefix, counts by status, the ready list, and doing tasks
    with owners. Intended to be run at the start of every agent session.
```

### 5.1 Output contract

- stdout carries the result; stderr carries nothing on success in JSON mode. In `--pretty`
  mode, warnings go to stderr.
- Errors: a single JSON object `{"error": {"kind": "<snake_case>", "detail": "<text>"}}` on
  stderr, exit 1. Clap usage errors exit 2. `check` exits 1 when it has errors, 0 with
  warnings only.
- Warnings are part of the success payload in JSON mode (`"warnings": [...]`, always
  present, possibly empty) so agents see them without parsing stderr.

Shapes (all fields always present; optional fields are `null`):

```
Task = {
  id, title, status, priority, size, owner, created, updated,
  depends: [string], tags: [string], spec, plan, step,
  body: string,
  notes: [{ at: string, by: string, text: string }]
}

TaskSummary = { id, title, status, priority, size, owner, updated, tags, depends }

show   -> { task: Task,
            spec_path: string|null,    absolute
            plan_path: string|null,    absolute
            step_found: bool|null,     null when no step
            depends_on: [{ id, title: string|null, status: string|null, resolved: bool }],
            warnings: [string] }
list   -> { tasks: [TaskSummary], warnings }
ready  -> { tasks: [TaskSummary], warnings }
graph  -> { format: "mermaid"|"dot", text: string, warnings }
check  -> { errors: [{ id: string|null, file: string, kind, detail }],
            warnings: [{ id, file, kind, detail }] }
prime  -> { prefix, counts: { idea, todo, doing, blocked, done, dropped },
            ready: [TaskSummary], doing: [TaskSummary], warnings }
init   -> { prefix, root, warnings }
add, edit, note, start, done, drop, block, unblock, dep
       -> { id, warnings }
```

`--pretty` renders the same data as tables (`list`, `ready`, `prime`), the file text plus a
resolved-dependencies footer (`show`), and the bare id for write commands.

### 5.2 Interactive editing

`edit <id>` with no flags:

1. Copy `tasks/<id>.md` to a unique temporary file in `tasks/` (`.<id>.<random>.edit.md`)
   and record a hash of the original.
2. Open the temporary file in `$EDITOR`.
3. Parse and validate the result (§3 and §5.3). On failure, print the error and the path
   of the temporary file so the edit is not lost; the original is untouched.
4. Re-hash the original. If it changed since step 1, error `concurrent_modification`
   and keep the temporary file; nothing is written.
5. Set `updated`, write the final content through the unique, exclusively claimed sibling
   described in §10, rename over the original, and remove the edit temporary file.

The original file is never written except by the final rename.

### 5.3 Edit invariants

Validation of an edited task (flags or editor) compares against the original:

- `id` and `created` must be unchanged.
- `status` changes must be allowed by the §3.3 table; a change to `done` obeys the
  open-deps rule (`--force` applies to the flag path; the editor path has no force and
  fails with `open_dependencies`).
- `updated` in the edited content is ignored and replaced.
- The notes section must be unchanged (notes are append-only via `note`).
- Everything else is validated as on `add`.

## 6. Configuration and registry

`tasks/.config.toml` (checked in):

```toml
prefix = "sci"
```

`~/.config/tasks/projects.toml` (per machine; written by `init`, hand-editable):

```toml
[projects]
sci = "~/d/science"
fam = "~/d/familiar"
```

A registry path may start with `~/`; it expands to the user's home directory. A foreign id
`<p>-<hex>` resolves to `<projects.p>/tasks/<p>-<hex>.md`. The registry points at each
project's main checkout, so a worktree reading a foreign dependency sees that project's main
branch — accepted as the right approximation.

A prefix is *unreachable* when it is not registered, or its path or the task file does not
exist. Unreachable ids are warnings in `show`/`list`/`check` and errors (`unresolvable_id`)
in `dep --on` and `add --depends`.

### 6.1 Cycle detection

`dep --on`, `add --depends`, and `check` detect cycles by depth-first traversal of
`depends` starting from the task being written (or every local task, for `check`),
following foreign ids through the registry into other projects' task files. The traversal
therefore reads foreign projects' `depends` too.

If any id reached during traversal is unreachable, `dep --on` and `add --depends` fail with
`unresolvable_id` naming it (a cycle cannot be ruled out, so the link is not created);
`check` emits a warning for it and reports only the cycles it could prove.

## 7. Spec and plan linkage; drift

- A task may point at a spec (`spec`), a plan (`plan`), and a heading inside that plan
  (`step`). These are validated on write and re-validated by `check`.
- `check` fails when: a linked file is missing; a path lies outside the accepted spec
  roots (§2) or `docs/plans/`; a `step` heading no longer appears verbatim in its plan;
  `step` is set without `plan`.
- Once automation has a pinned Tasks install, running `check` in a project's test or
  pre-commit path turns doc drift under open tasks into a build failure, which is the
  intended coupling: when a plan step is renamed or removed, the task must be updated in
  the same change. Never make the gate conditional on the binary being present.

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
   another project's task; resolving an id collision (§4).
3. **Superpowers integration** (applies when the superpowers plugin is present):
   - Brainstorming writes specs under one of the accepted roots in §2. After approval,
     add one task per major deliverable with `--spec <topic>`.
   - writing-plans writes plans to `docs/plans/YYYY-MM-DD-<topic>.md`. Add one task per
     `Task N:` heading with `--plan <topic> --step "Task N: …"` and `--depends` mirroring
     the plan's order.
   - executing-plans and subagent-driven-development call `tasks start`/`done` per step.
   - `tasks check` in CI is the drift limiter once CI has a pinned install (§7).

## 9. Adoption in existing projects

Manual, documented steps — the tool does not move files:

1. Keep historical docs in place. Structured spec links accept every root in §2 and plan
   links accept `docs/plans/`. Move only linked docs outside those roots, fixing links;
   projects may reference other historical layouts in task bodies, and `init`
   intentionally creates the default directories alongside them.
2. `tasks init --prefix <p>`.
3. Install the skill (user level preferred) and add a line to CLAUDE.md pointing at it.
4. Require `tasks prime` at session start and `tasks check` before completion. Add
   `tasks check` to the test script or pre-commit hook once the binary has a pinned install
   source; never silently skip it when unavailable.

## 10. Implementation

- Rust, single Rust binary, edition 2024. Crates: `clap` (derive), `serde` +
  `serde_json`, `toml`, `time`, `fastrand`, `thiserror`. Frontmatter uses a strict in-tree
  subset parser; no YAML crate is used. `serde_yaml`, `rand`, and `anyhow` are not used.
- Modules: `model` (Task, Status, Size, id parsing, transition table), `format`
  (parse/serialize frontmatter + body + notes, round-trip exact), `repo` (locate project,
  scan, atomic write), `registry`, `query` (ready, graph, cycle detection across
  projects), `check`, `cli` (clap + JSON/pretty rendering).
- Writes: build the full file in memory, claim a unique same-directory sibling
  (`.<target-name>.<random>.tmp`) with exclusive creation and at most 16 collision retries,
  write it completely, then rename over the target. Task, project-config, and registry writes
  share this path; a claimed sibling is cleaned up after write/rename failure where safe. No
  partial target files.
- Errors: a single typed error enum whose variants map to the `kind` strings in §5.1.
- Tests: unit tests for format round-trip, id generation, the transition table,
  ready/cycle logic; integration tests via `assert_cmd` + `tempfile` running the binary
  against temp repos with a fake `HOME` for the registry, covering: two projects with a
  cross-project A→B→A cycle rejected; a dependency on an unregistered prefix; editor path
  with `EDITOR` set to a script that writes invalid content, valid content, and a
  concurrent modification of the original.

## 11. Open questions

None blocking. Candidates for later, only if they prove necessary: a `promote` verb,
`tasks reid`, an `xs`-only "quick wins" view, `tasks graph` critical-path annotation.
