# Multi-project support — design

**Status:** implemented 2026-09-04; see docs/plans/2026-09-04-multi-project.md. Task: tasks-3029be.

## 1. Problem

`tasks` works one project at a time. The registry (`~/.config/tasks/projects.toml`) already
lets a task depend on a task in another project, `show` already reads a foreign id, and
`feedback` already writes into another project, but there is no way to ask "what should I
work on next across everything", no view of the registry itself, and no home for work that
spans several projects. Cross-cutting goals (a test-suite audit across every active project,
say) therefore have no task at all, and getting back into development after time away means
visiting each checkout in turn.

## 2. Decisions

- **A hub project holds cross-project goals.** It is an ordinary private repository,
  registered like any other project, with the prefix `ops`. Nothing in the tool knows the
  prefix; the hub is a convention, not a feature. It is separate from this repository
  because this repository is public and a hub goal with per-project pieces would publish
  the names of every project and what is being done in them.
- **A hub goal links to its pieces by dependencies only.** The goal in `ops` depends on one
  task per affected project. `show` already resolves each dependency's title and status
  through the registry, and the goal leaves `ready` while any piece is open and returns
  when the last one closes, which is the moment to verify and close it. `parent` stays
  local. `graph` also stays local and omits edges to tasks outside its scan, so the hub
  goal's dependency picture is `show`, not `graph`. Foreign parents were considered and rejected: the hub
  could only find its foreign children by scanning every registered project on every
  `done`, `drop`, and `prime`, with no way to know in advance which projects to look in.
- **Registry-wide scope makes the local project optional.** A command run with
  `--all-projects` reads the registry and never locates a local project, so its output is
  the same from any directory. The existing `list --all-projects`, which today fails
  outside a project with `no_project`, changes to this rule. There is one flag, the
  existing spelling, on every command that gains the scope.
- **One scope, opened once.** Read commands take a `Scope` that is either the local project
  or the set of reachable registered projects. Reachability is decided in one function, so
  `list`, `ready`, `prime`, `tree`, `next`, `tags`, and `projects` cannot drift in how they
  treat a missing root. `--all-projects` is only defined on the read commands. Write
  commands keep a mandatory local project, with exactly one exception: `add --project`
  names its target explicitly and locates no local project (§4.3). Dispatch must therefore
  open the local project lazily for `add`, after the flag is known, rather than eagerly
  for every command as it does today.
- **`next` answers "what do I do now" in one call.** It is the head of `ready` in the
  `show` shape, per project or across all of them.
- **Tags get visibility, not a vocabulary.** `tags --all-projects` shows which tags are
  actually shared; enforcing a shared list waits until that table has stabilized.
- **JSON stays additive.** Rows do not gain a `project` field because the id already
  carries the prefix. The one non-additive change, `prime`'s `prefix` becoming null, only
  occurs under the new flag.

Deferred, with the trigger that would revive each: per-project weights or a paused mark
(a dormant project crowding `next --all-projects`); foreign parents (missing the tree view
of a hub goal); a cross-project `graph` (the dashboard, a separate project consuming the
JSON, covers it); a shorthand that creates a piece and its dependency in one command (the
second command is cheap and the explicit link reads better in a transcript).

## 3. Scope

### 3.1 Opening a registered project by prefix

The three checks that `show` and `feedback` each perform by hand today become one function
used by `show`, `feedback`, `root`, `add --project`, and the scope opener. The dependency
resolver deliberately returns no task for an unregistered or config-less prefix, then uses
the same function so malformed config and prefix mismatch retain the strict errors below:

1. The prefix must be registered. Otherwise: `unresolvable_id` when the caller came in
   with a task id, `config` when it came in with a prefix (`--project`), each naming the
   prefix.
2. The root must contain `tasks/.config.toml`. Otherwise the same kinds as in 1, with the
   message naming the root and saying to run `tasks init` there.
3. The config's prefix must equal the registry key. Otherwise `config`: "registry maps
   `<key>` to `<root>`, whose prefix is `<prefix>`; fix the registry".

### 3.2 Registry-wide scope

With `--all-projects` the command reads the registry and handles each entry in one of
three ways:

- root and config present, prefix matching: the project is opened and joins the set;
- root or config missing: one warning, `project <prefix> at <root> is unreachable`, and
  the entry is skipped;
- config present but malformed, or its prefix disagrees with the key: a `config` error.
  A broken registered project is a fault to fix, not something to paper over.

No local project is located. Two edge cases warn rather than fail: an empty registry
(`registry is empty`), and the current directory lying inside a project that is not
registered (`current project <prefix> is not registered`), which would otherwise vanish
silently from its own portfolio view. That second check is the only time a wide command
looks at the current directory, and it is read-only.

Projects are visited in registry order (the registry is a sorted map, so alphabetical by
prefix). The scope hands commands the union of every project's tasks in one slice, and
also the per-project slices it was built from. Ids are globally unique through their
prefix, so the hierarchy, ready, and sort code runs on the union unchanged. Dependency
resolution looks in the union first and then in the registry, as `ready` already does; its
registry lookup uses the same config-file and prefix-mismatch checks as the scope opener.
Ordering is whatever the command uses locally; a wide `ready` is priority, then size, then
created, then id, exactly as a local one, with no project grouping or weighting (the id
tiebreak orders by prefix only among tasks equal on everything else). The one exception
is `tree`, which groups by project
(§4.1): it runs the forest builder once per project slice and concatenates, because the
builder sorts every root globally in ready order and a single run over the union would
interleave projects.

Warnings are reported per project, prefixed with the prefix, in the same `warnings` array
as today: unreachable projects, unreachable dependencies, and uncommitted task files.

### 3.3 Local scope

Unchanged. The project is located by walking up from the current directory or `-C`, and
its absence is `no_project`.

## 4. Commands

### 4.1 Existing commands gaining `--all-projects`

```
tasks list  [--status S]... [--tag T]... [--owner O] [--parent ID] [--all-projects]
tasks ready [--size S] [-n N] [--all-projects]
tasks tree  [<id>] [--all] [--all-projects]
tasks prime [--all-projects]
```

- `list`: as today, except it now works outside a project.
- `ready`: the union's ready tasks in ready order.
- `tree`: each project's forest, built per project and concatenated in registry order
  (§3.2). `<id>` and `--all-projects` conflict (the subtree of an id is read from that
  id's own project).
- `prime`: counts, ready, doing, roadmap, and closeout over the union. Each list keeps its
  local order. `prefix` is null and the new `projects` field lists the prefixes in scope.
  The uncommitted-files warning is emitted per project.

### 4.2 New commands

```
tasks next [--all-projects]
    The first task of `ready`, in the `show` shape: body, dependencies with their titles
    and statuses, resolved spec and plan paths, parent, children. Nothing ready is not an
    error: the task is null, warnings are still reported, exit 0.

tasks root <id>
    The registered root of the id's project, resolved strictly by §3.1 (it came in with
    an id, so unregistered or config-less is `unresolvable_id`; mismatched is `config`).
    Runs outside a project. The task file is not checked; the root is what the caller
    wants, and a missing file is `show`'s to report. A successful lookup emits the
    unregistered-current-project warning when applicable; an empty registry cannot
    produce a successful root lookup.

tasks tags [--status S]... [--all-projects]
    Every tag in scope with the number of tasks carrying it (a task counts once per tag,
    however many times it lists the tag) and, per project, the same count there. Open
    tasks only unless `--status` is given, so a tag that survives only on done tasks does
    not look current. Sorted by count descending, then tag.

tasks projects
    Every registry entry with its root, whether it is reachable (§3.2, the first two
    outcomes), and, when reachable, the same status counts `prime` reports. Runs outside a
    project. An unreachable entry is a row, not a warning: here the row is the report. A
    malformed reachable project is a `config` error here as everywhere. The two shared
    warnings of §3.2 (empty registry, unregistered current project) are emitted.
```

`show` is unchanged: a foreign id already routes through the registry. `graph` stays local.

### 4.3 `add --project`

```
tasks add <title> [existing flags] [--project <prefix>]
```

Creates the task in the named registered project exactly as `add` would there. Every field
is validated against the target: `--parent` and `--depends` must resolve from the target,
`--spec`, `--plan`, and `--step` resolve against the target's doc roots, the id takes the
target's prefix, and the owner recorded on any note is derived from the target. The output
is the usual `add` output. No local project is located, so it runs from anywhere. An
explicit prefix always targets its registered root, including when the current checkout
has the same prefix; in a linked or displaced checkout this intentionally differs from
plain `add`. As with every `add`, nothing is committed.

### 4.4 `feedback` on top of it

`feedback` keeps its fixed target (`tasks`), its `feedback`, `<category>`, and
`from:<prefix>` tags, its matching, and `--recur`/`--new`. The creation of a new entry goes
through the same code path as `add --project`, so the two cannot diverge in how a file is
created in another project. `feedback` still requires a local project: the `from:` tag is
its provenance, and feedback without provenance is not accepted
(docs/specs/2026-09-03-feedback-design.md §3).

## 5. Output

```
next     -> { next: ShowFields | null, warnings }
            ShowFields = the `show` object without its `warnings`
root     -> { prefix, root, warnings }
tags     -> { tags: [{ tag, count, projects: { <prefix>: count } }], warnings }
projects -> { projects: [{ prefix, root, reachable, counts: Counts | null }], warnings }
prime    -> existing fields + projects: [prefix]; prefix: string | null
```

- `tags.projects` is always a map, with one key in local scope, so the shape does not
  depend on the flag.
- `projects[].counts` uses the same object as `prime.counts`.
- `root.warnings` is normally empty; it is present because every success payload carries
  `warnings` (docs/specs/2026-08-29-tasks-design.md §5.1).
- `prime.projects` is always present; locally it is the one-element list. `prime.prefix`
  is null only under `--all-projects`.
- `list`, `ready`, `tree`, `show`: unchanged.

`--pretty`:

- `next`: the `show` rendering, or the line `nothing ready`.
- `root`: the path alone, so `cd "$(tasks root fam-0c3d7e --pretty)"` works.
- `tags`: a table of tag, count, and a per-project breakdown column in wide scope.
- `projects`: a table with one row per entry; `unreachable` in place of counts.
- `prime` wide: the header names the projects in scope.
- Task tables get no project column; the id shows the prefix.

## 6. Errors

No new error kinds. `tree <id> --all-projects` is a clap conflict. `--all-projects` on a
command that does not define it is a clap error. Prefix resolution follows §3.1. `next`
and `projects` fail only for what they cannot read, never for what they find.

## 7. Skill and documentation

- `skills/tasks/SKILL.md`: `tasks next` (and `tasks next --all-projects`) as the way back
  into a session when there is no task in hand; and the hub pattern: a goal in the hub
  project, one `tasks add "<piece>" --project <prefix>` per affected project, one
  `tasks dep <goal> --on <piece>` each, closed from the goal when it returns to `ready`.
- `AGENTS.md` here: one line adding `tasks next` to the session protocol.
- `docs/specs/2026-08-29-tasks-design.md` §5 gains the new commands and flags in the same
  change, so the CLI reference stays in one place. §6 notes that `--all-projects` does not
  require a project.

## 8. Testing

End-to-end in `tests/cli.rs`, which already builds temp projects with a fake home for the
registry:

- wide scope from a directory with no project; from inside an unregistered project, with
  its warning; with an empty registry, with its warning;
- one unreachable entry warns and is skipped; one malformed config fails with `config`;
  one prefix mismatch fails with `config`;
- `ready --all-projects` applies ready order across two projects; its shared comparator is
  characterized through the created and final id tiebreaks;
- `next` returns the show shape for a task in another project; `next` with nothing ready
  gives a null task and exit 0;
- `prime --all-projects`: null prefix, the projects list, per-project uncommitted warnings;
- `tree --all-projects` concatenates in registry order; `tree <id> --all-projects` is a
  usage error;
- `root` for a registered and an unregistered prefix, including the shared warning for an
  unregistered current project; `--pretty` prints the path alone;
- `projects` with a reachable and an unreachable row;
- `tags` counts per project, and `--status` widening to closed tasks;
- `add --project` from outside a project; `--parent`, `--spec`, and `--depends` validated
  against the target and not the caller; an unregistered `--project` is `config`;
- `feedback` still lands in `tasks` with its tags after the refactor (existing tests).

Unit tests cover the scope opener's three outcomes, shared config rules for foreign
dependency resolution, the shared prefix resolver, and the created/id ready tiebreaks.

## 9. Follow-through outside this repository

After the code lands: create the hub (`git init`, `tasks init --prefix ops`), file the
test-and-CI audit goal there with one piece per active project, and drop the placeholder
idea tasks-f27f59 here with a note pointing at the new id. Revisit the deferred items in §2
only on their triggers.

## 10. Out of scope

The dashboard (a separate project consuming `list --all-projects` and `projects` JSON), a
shared tag vocabulary, project weights, foreign parents, and any change to how `feedback`
matches or discloses.
