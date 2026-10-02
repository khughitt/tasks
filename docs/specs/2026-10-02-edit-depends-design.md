# Additive `edit --depends`

Status: draft for review (tasks-e9af16, goal tasks-671956)
Brief: `docs/notes/2026-10-02-dependency-editing-brief.md`

## 1. Problem

`tasks edit <id> --depends <x>` replaces the task's whole dependency list with the ids
given. `edit --tag` appends. An agent that meant to add one dependency dropped five
(tasks-8efda8). Nothing warns. The flag's help ("Depend on another task (repeatable)")
does not say it replaces, and `add` shares the flag through `FieldArgs`, where appending
and replacing look the same because a new task has no dependencies.

The replacement lives in `src/commands/mod.rs::apply_fields`
(`task.depends = dependencies`). It canonicalizes the ids supplied and stores them in
place of the old list, so every edge the call does not name is dropped, along with its
stored spelling. Additions keep the rename contract's rule that untouched stored
references keep their spelling (`docs/specs/2026-09-08-prefix-rename-design.md` §4).

## 2. Evidence about replacement users

Nothing found relies on replacement:

- This checkout: one test passes `edit --depends` (`tests/cli.rs`, the
  `open_dependencies` check on `edit --status done`). It asserts a refusal that holds
  under either semantics, because the added dependency is open.
- Docs: the tasks skill and README list `--depends` among the edit flags without
  saying it replaces. The curate skill allows `--depends` "when the link is missing or
  wrong". A wrong link is now fixed in one call with `dep --on <new> --rm <old>`
  (tasks-45e400).
- Other registered projects' tooling (bin/, src/, skills/, hooks/ in the hub, harness,
  relay, obs, mindful, science and tasks-tui checkouts): no `--depends` caller.
- Task corpus across projects: no recorded `edit … --depends` replacement. One plan
  note asks for a task to be set to depend "on nothing", which the CLI cannot do today:
  `--depends ""` fails with `invalid_id`.

Replacement callers outside these checkouts are unknown. The change moves them toward
keeping more edges, which is the safe direction: a stale edge shows up in `ready` and
`check`, whereas a lost one disappears without trace.

## 3. Contract

`edit` treats dependencies the way it treats tags:

| Call | Effect |
|---|---|
| `edit <id> --depends X` | Appends X unless the task already depends on it under any spelling. Existing edges and their stored spellings stay. |
| `edit <id> --no-depends` | Clears the list. |
| `edit <id> --no-depends --depends X …` | Replaces the list with X …, the explicit form of today's behaviour. |
| `dep <id> --rm X` | Removes one dependency (unchanged; `edit` gets no `--rm-depends`). |
| `add … --depends X` | Unchanged: the new task starts with those dependencies. |
| `edit <id>` (editor) | Unchanged: the list in the editor is the list saved. |

The rules:

1. **Identity.** An added id is compared with the stored list by canonical identity, as
   `dep --on` does. Adding one already present (live or retired spelling) changes
   nothing, keeps the stored spelling, and adds the same warning `dep --on` gives
   (tasks-2942cb). Repeats within one call are silently deduplicated. A new edge is
   stored in canonical form.
2. **Validation.** Each added id is checked as now: a self-dependency is a `cycle`
   error, an unresolvable id `unresolvable_id`, a malformed one `invalid_id`. Whenever
   `--depends` is supplied, the final list is checked with `dep::ensure_acyclic`, even
   when every id named is already present, as `dep --on` does: a duplicate-only call on
   a task with a stored unreachable dependency fails with `unresolvable_id`. Any
   failure leaves the record unchanged, because `edit` saves once at the end.
3. **Clearing.** `--no-depends` alone walks no graph, so a stored dependency pointing at
   an unreachable task can always be cleared, as with `dep --rm`. `--no-depends` runs
   before additions, so `--no-depends --depends X` validates only the new list.
4. **Output.** JSON shapes are unchanged; the only new output is the warning in
   `warnings`.

`--no-depends` sits on `EditArgs` next to `--no-tags` and does not conflict with
`--depends`, so the pair means replace. `edit` counts it as a flag, so
`edit <id> --no-depends` alone does not open the editor.

## 4. Code shape

One helper in `src/commands/dep.rs` holds the add logic that `dep --on` and
`apply_fields` both use:

    pub fn add_dependencies(ctx: &mut Ctx, task: &mut Task, values: &[String]) -> Result<()>

It parses each value, rejects a self-dependency and an unresolvable id, appends the
missing edges, warns on edges the task held before the call, and finishes with
`ensure_acyclic` when `values` is not empty. `dep::run` keeps its overlap check and
removals, then calls the helper. `apply_fields` calls it in place of the replacement
block. `edit::run` clears `task.depends` for `--no-depends` beside the `--no-tags`
clear, before `apply_fields`.

The help text for `FieldArgs::depends` becomes: "Depend on another task (repeatable).
On `edit` this appends; see `--no-depends` and `dep --rm`."

## 5. Docs

- `docs/specs/2026-08-29-tasks-design.md`: the `edit` entry describes `--depends` as
  appending and `--no-depends`, in the sentence that covers `--tag`/`--no-tags`; the
  cycle-detection paragraph names `edit --depends` with `dep --on` and `add --depends`.
- README: one example line beside the `--tag` example.
- `skills/tasks/SKILL.md`: add `--no-depends` to the edit flag list and say that
  `--depends` appends.
- `skills/curate/SKILL.md`: a missing link is added with `edit --depends`; a wrong one is
  replaced with `dep <id> --on <right> --rm <wrong>`.

## 6. Acceptance checks

CLI tests in `tests/cli.rs`:

1. A task with five dependencies gains a sixth through `edit --depends`; all six remain,
   in their original order with the new one last (the tasks-8efda8 regression).
2. `edit --depends` naming a stored dependency by its retired spelling, and the reverse,
   warns, keeps one edge, and keeps the stored spelling in the file.
3. `edit --no-depends` clears the list, including a stored unreachable reference.
4. `edit --no-depends --depends X` leaves exactly X.
5. `edit --depends X` that would close a cycle fails with `cycle` and leaves the file
   byte-identical, as does an unresolvable id in a list that also names a valid one.
6. `edit --depends X`, where X is already present and the task also has a stored
   unreachable dependency, fails with `unresolvable_id` and leaves the file
   byte-identical.
7. `add --depends` and the editor path keep their current behaviour (existing tests
   pass unchanged).

Then `just test-fast` and `just check`.

## 7. Alternatives rejected

- **Keep replacement and document it.** The smallest change, but protection would still
  depend on someone reading the help, and the reported loss came from a reasonable
  reading of a flag that appends elsewhere in the same command.
- **A separate replace command or `--set-depends`.** It makes replacement explicit but
  adds surface that `--no-depends --depends` already covers through an existing pattern.
- **`edit --rm-depends`.** It would duplicate `dep --rm`, which already removes one edge
  with the identity rules this design relies on.

## 8. Follow-up

On completion of this design, tasks-8efda8 gets a note recording this contract and is
re-scoped as its implementation task.
