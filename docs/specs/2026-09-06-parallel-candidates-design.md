# Parallel candidates: marking tasks safe to hand out at once

Status: designed (2026-09-06)
Task: tasks-cf1bda

## Problem

A session that wants to dispatch several agents at once has no way to know which ready
tasks can safely run together. `ready` sorts by priority then size and says nothing about
whether two rows would collide.

Work claims solved the adjacent problem: `start` records a claim outside git, so two live
sessions cannot take *the same* task, and `ready` omits what someone else holds. Nothing
addresses two *different* tasks that both land in `output.rs`. The dispatcher either picks
by hand — reading each task and guessing at its blast radius — or discovers the conflict
as a merge.

The judgement itself is not hard for whoever scoped the task. It is simply not written
down anywhere the tool can read.

## Approach

### A per-task boolean, not a conflict relation

"No shared files or state with the rest of the ready list" is a relation between tasks,
which argues for lanes: a group label per task, with two tasks compatible when their
groups differ. Lanes were rejected.

They pay off when the ready list is long enough that "which two of these twelve" is a real
combinatorial question. Here it runs one to five tasks, and the person setting the marker
has already made the conflict judgement in their head. Lanes would also demand a label on
every task to be useful, which is a tax on every `add` for a benefit that shows up rarely.

A boolean is a strict subset of the lane design: `parallel: true` with no lane is exactly
"compatible with everything else marked". If the ready list ever grows to where lanes earn
their keep, they can be added beside the boolean without changing what it means.

The cost of the simpler model is that the flag is a promise about *the whole marked set*,
not about a pair. It has to be set conservatively — self-contained, touching nothing
central. That constraint belongs in the docs, not in the schema; the next section states
exactly what it covers.

### A first-class field, not a reserved tag

The marker could be the tag `parallel`, which needs no schema change at all: `--tag
parallel` and `list --tag parallel` work today.

Rejected. `ready` would have to ascribe fixed meaning to a freeform user string. Tags are
uninterpreted everywhere else in the tool, so one magic value would be the single
exception — silently changing behaviour for anyone who tagged `parallel` meaning something
else, with no validation and nothing for `check` to catch. Fail-early and explicit-over-
implicit both point at a real field.

### What the promise covers

`parallel: true` asserts one thing: **this task, as currently scoped, can run beside any
other task marked `parallel` without touching the same files or state.**

It is silent about everything else, and the gaps matter more than the guarantee:

- **Unmarked tasks.** Nothing is claimed against them. A marked task may well collide with
  an unmarked one; that is not a violation of the promise.
- **Work already in flight.** `is_ready` requires `todo`, so anything `doing` is absent
  from `ready` by construction and takes no part in the marked set's mutual guarantee. A
  filtered list is safe *against itself*, not against what is already running. A
  dispatcher must read `prime`'s `doing` list before treating the list as safe to hand
  out. `ready` additionally omits tasks another live session claims, but a stale claim is
  no protection.
- **Blocked tasks.** A `blocked` task is not ready and so is never dispatched, but it
  keeps its marker. It re-enters the marked set on `unblock` without anyone re-examining
  it, which is one of the ways a marker goes stale below.
- **Other projects.** Under `ready --all-projects` the marked set spans registered
  projects, which are separate roots, so file collisions across them are impossible by
  construction. The residual risk is shared external state — the registry, the claim
  store, a shared library — not the tree.

**Lifetime.** The marker describes the task *as scoped when it was set*, so anything that
changes the scope invalidates it: a widening note, a new parent, a different spec, a
dependency closing and revealing more work. Whoever changes the scope re-examines the
marker in the same breath — `tasks note` on a scope change is already the protocol hook
for this, so the rule adds a habit rather than a mechanism. Nothing enforces it: a marker
is an assertion by a person, and a stale one is a wrong assertion, not a bug.

### Hand-set, with no inference

The originating idea floated inferring the flag from disjoint `spec`/`plan` references.
Rejected as confidently wrong in both directions: two tasks against one spec routinely
touch disjoint files, and two against different specs both land in `output.rs`. A
heuristic that is wrong either way is worse than an honest blank, because a marker that
sometimes lies cannot be trusted for the one job it has.

`check` gains nothing either. With hand-set semantics there is no drift for it to detect
beyond what the parser already rejects.

## Schema

`Task` gains `parallel: bool`. The frontmatter key sits after `size` — both describe the
shape of the work rather than its state — and is written only when true:

    ---
    id: tasks-98f569
    title: graph --format is a hand-parsed value set with no completion
    status: todo
    priority: 3
    size: s
    parallel: true
    created: 2026-09-05T23:00:36Z
    updated: 2026-09-06T00:32:54Z
    depends: []
    tags: [cli]
    ---

`KEYS` in `format.rs` grows to 15. Reading:

| value in file | result |
|---|---|
| key absent | `false` |
| `parallel: true` or `parallel: "true"` | `true` |
| `parallel: false` or `parallel: "false"` | `false`, and the key is dropped on the next write |
| anything else | parse error: `parallel must be true or false` |

Both spellings are accepted because the parser cannot tell them apart: `parse_scalar`
takes the quoted branch for `"true"` and returns `Value::Scalar("true")`, byte-identical
to what the bare word yields. Quoting is discarded at parse time, so no parser change is
needed and none is proposed.

Emission is canonical and unquoted, which does require care: `needs_quotes` quotes the
literal `true`, so `Value::Scalar` would write `parallel: "true"`. Use `Value::Raw("true")`,
as `priority` already does. That is a readability choice, not a correctness one — the
quoted form would round-trip correctly, it would just be ugly and drift from how every
other scalar in the file is written.

## CLI

- `FieldArgs` gains `--parallel`, a valueless flag shared by `add` and `edit`. On `add` it
  sets the field. On `edit`, present means set to true and absent means leave alone — the
  rule `--tag` already follows.
- `EditArgs` gains `--no-parallel`, `conflicts_with = "parallel"`, mirroring
  `--parent`/`--no-parent` and `--tag`/`--no-tags`.
- `ready` gains `--parallel`, which keeps only marked tasks. It composes with the existing
  `--size` and `-n`, applied after the readiness test and before the limit.

Completion needs no work. A valueless flag is completed by name from the `Cli` definition,
so `complete.rs` is untouched.

## Output

`TaskSummary` gains `parallel: bool`. This is the JSON contract change; `list`, `ready`,
`prime`, and `tree` rows all carry it.

`show` and `next` need no output work. Both serialize `Task` directly for JSON, and
`show_text` renders `format::serialize_task`, so the field appears in both shapes as soon
as the model carries it.

Pretty tables get a marker column between status and date, rendered as ASCII `||`:

    tasks-98f569  P3 s  todo    || 2026-09-05  graph --format completion [cli]
    tasks-a14f0d  P3 xs todo    || 2026-09-04  Validate TASKS_FORMAT ... [cli]
    tasks-120a02  P3 s  todo       2026-09-05  Completion scope contradiction [cli]

The column is present only when something in the output is marked. Rendering it
unconditionally would widen every `list` and `prime` row by two characters in service of a
flag that is usually unset. ASCII rather than `∥` keeps pretty chrome to the ASCII range
it uses everywhere else.

**Visibility is decided once per command output, by the caller, not inside `table`.**
`table` cannot decide for itself, because it is not always given the whole picture:
`tree_text` calls it one node at a time (`output.rs:497`), and `prime`'s roadmap calls it
per childless root (`output.rs:361`). A per-call decision would give a marked task a
wider row than its unmarked sibling, shifting dates and titles between adjacent lines.

So `table` takes the visibility as a parameter, and each pretty branch computes it once
over every summary it is about to render:

- `list` and `ready` — over their rows.
- `tree` — over the whole forest, recursively through `TreeNode::children`.
- `prime` — over `closeout`, `roadmap` (recursively), `ready`, and `doing` together.

Per *output* rather than per section, so that `prime`'s four blocks keep a single column
layout; they align today only because every width is fixed, and a per-section decision
would break that.

This needs two helpers next to `table`: one over `&[TaskSummary]` and one over
`&[TreeNode]` that recurses into children.

## Not in scope

- `is_ready` and `ready_order` are untouched. Parallelism is a filter, never a sort key:
  floating marked tasks up would perturb the default order for the ordinary
  single-agent case, which is the common one.
- `next` gains no flag. It is the single-task entry point; dispatching is a list operation.
- No lanes, no inference, no `check` rule. Each is argued against above.

## Migration

Unknown frontmatter keys are a hard parse error, so a `tasks` binary built before this
change rejects any file carrying `parallel:`. Run `cargo install --path .` before marking
tasks in a project tracked from another checkout. Existing files need no migration: the
absent key reads as `false`.

## Testing

Test-first, in this order:

1. `format.rs` units — round-trip a task with `parallel: true`; `"true"` and `"false"`
   read the same as their bare spellings; `parallel: maybe` is a parse error;
   `parallel: false` reads as false and is dropped on write; `true` is emitted unquoted.
2. `output.rs` unit — the marker column appears when a row is marked and is absent from
   every row when none is. It must cover **mixed siblings**: a tree whose marked and
   unmarked nodes sit at the same depth, asserting that both rows carry the column and
   their dates and titles stay in the same character positions. That is the case a
   per-call decision inside `table` would get wrong, so it is the case that pins the
   design down.
3. `tests/cli.rs` end-to-end — `add --parallel`; `edit --no-parallel` clears it;
   `ready --parallel` filters and still honours `-n`; the `parallel` key is present on
   summary JSON.

## Docs to update in the same change

- `docs/specs/2026-08-29-tasks-design.md` — four places: the frontmatter example, the
  field table, the `add`/`edit`/`ready` synopses, and the `Task` and `TaskSummary` JSON
  shapes.
- `skills/tasks/SKILL.md` — a line under "Recording work" on marking parallel candidates
  and dispatching with `ready --parallel -n N`, carrying the two rules from "What the
  promise covers": check `prime`'s `doing` list before dispatching, and re-examine the
  marker whenever a task's scope changes. AGENTS.md requires the shipped skill stay in
  step with CLI changes.
- `README.md` — "## Use" carries examples rather than a field table, so it needs a line
  only if the dispatch flow reads as worth showing there.
