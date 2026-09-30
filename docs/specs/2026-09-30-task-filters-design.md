# One task filter for list and ready — design

**Status:** draft, awaiting review. Task: tasks-964c95.

## Outcome

`tasks list` and `tasks ready` select tasks by priority, size, complexity, and process,
alongside the filters they have today. One type decides whether a task matches, and every
command that filters on record fields calls it. That includes `list --parked`, which
currently keeps its own copy of the checks.

## What exists

- `list` filters on `--status`, `--tag`, `--owner`, `--source`, and `--parent`. Its
  `retain` closure over `Task` (`src/commands/list.rs`, `list`) is copied almost line for
  line in `list_parked` over `ParkedRow`.
- `ready` has `--size` (one exact value) and `--parallel`. Both are separate `retain` calls
  that run after the ready pool is built, once for ready tasks and once for deferred ones.
- `ready` and `next` have `--max-complexity`. It is a picker cutoff with its own home in
  `src/complexity.rs`: it compares against the effective rating, hides unassessed tasks,
  warns with counts, and is read from `TASKS_MAX_COMPLEXITY`.
- Nothing filters on priority or process. The process design settled "no process filter"
  for that change (`2026-09-13-task-process-design.md`), and this design revisits it.

## Flags

The new flags are one clap `Args` group, `FilterArgs`, flattened into `list` and `ready`
in the same way `ScopeArgs` is shared today:

| flag | values | matches |
|---|---|---|
| `-p`, `--priority <n>` | `0`–`4`, repeatable | the record's priority |
| `--size <s>` | `xs s m l xl none`, repeatable | the record's size; `none` is unsized |
| `--complexity <c>` | `low mid high none`, repeatable | the effective rating; `none` is unassessed |
| `--process <p>` | `direct planned none`, repeatable | the record's process; `none` is unassessed |
| `--tag <t>` | repeatable | carries every given tag (unchanged) |
| `--owner <o>` | one value | owner equals it (unchanged) |
| `--source <ref>` | one value | source equals it byte for byte (unchanged) |
| `--parent <ref>` | one value | direct child of that task (unchanged) |
| `--parallel` | flag | marked parallel |

`--status` stays on `list` only, because `ready` defines its own status pool.

Repeats of one flag widen (OR), and different flags narrow (AND). This matches the shared
CLI vocabulary for repeatable filters. For example, `tasks list -p 0 -p 1 --size s --size xs`
lists P0 or P1 tasks that are small or extra-small.

`--tag` is the exception: today it requires every tag given (all-of), and this change keeps
that. It is a known departure from the vocabulary. See **Boundaries**.

The value `none` selects records where the field is unset. It combines with the others:
`--complexity low --complexity none` selects tasks rated low or not yet rated. Priority is
always set, so it has no `none`. Before choosing `none`, I considered separate
`--no-size`-style flags. They cannot be combined with a value of the same field, and they
would double the flag count.

Complexity filters on the **effective** rating: the record's rating or a parked escalation,
whichever is higher (`complexity::effective`). This is the same number `--max-complexity`
compares and the one list rows show through `escalation`. `--complexity` is a plain
selection: it is silent and never reads `TASKS_MAX_COMPLEXITY`. On `ready`, both flags
apply and their results intersect.

`ready --size` goes from one value to a repeatable set that accepts `none`. An existing
single-value call behaves the same as before.

## The filter type

A new module, `src/filter.rs`:

- `TaskFilter` holds the parsed selection: `statuses: Vec<Status>` and
  `priorities: Vec<u8>`; `sizes`, `complexities`, and `processes` as `Vec<Option<_>>`, where
  `None` means `none`; `tags`; `owner`, `source`, and `parent` as `Option<_>`, with `parent`
  as a canonical `TaskId`; and `parallel: bool`. An empty `Vec` or `None` means that field
  is not constrained. `TaskFilter::parse(args, statuses, registry)` does the parsing. Parse
  failures cannot happen at runtime, because clap's `ValueSet` has already rejected unknown
  values.
- `Fields<'a>` is the view the filter reads: status, priority, size, effective complexity,
  process, tags, owner, source, canonical parent, and parallel. Two constructors build it:
  `Fields::of_task(&Task, &ClaimSnapshot, &Registry)` and
  `Fields::of_row(&ParkedRow, &Registry) -> Option<Fields>`. The row constructor returns
  `None` for an unresolved park, which has no record, and computes effective complexity
  from the row's `complexity` and `escalation` through `complexity::higher`.
- `TaskFilter::matches(&Fields) -> bool` and `TaskFilter::is_empty() -> bool`.

The two row types meet in a small concrete view rather than a trait implemented on both.
This keeps the predicate in one function over one input, and `ParkedRow` and `Task` stay
unchanged. I rejected keeping the source `Task` next to each parked row: the row can be
built from another checkout's copy (`recorded_or_fallback`), so threading the `Task`
through would widen `parked::rows_preferring` and its callers for no gain.

The check that `--parent` names a task in scope moves into one helper,
`filter::check_parent`, used by `list`, `list --parked`, and `ready`. For `list --parked` it
keeps today's rule: the parent is also accepted when only a parked row's parent names it.

## Command behaviour

- **`list`:** each command still applies its default pool when `--status` is not given.
  That pool is open tasks except shelved ones, or every status with `--periodic`, and after
  it comes `TaskFilter::matches`. `--periodic` and `--deferred` stay list-mode flags,
  because they also change the sort and the date column.
- **`list --parked`:** rows go through `Fields::of_row`. An unresolved row is still shown
  only when the filter is empty. This extends today's rule ("shown unless any filter is
  given") to the new flags.
- **`ready`:** the filter moves to the start of `ready_tasks`, so a candidate that does not
  match is never considered. As a result, the claim-omission, parked-on-user, halt-hidden,
  complexity-cutoff, and deferred-count warnings describe only tasks the caller asked about.
  Today `--size` and `--parallel` filter after those warnings, so a call such as
  `ready --size s` warns about claimed tasks of every size. That output changes to count
  matching tasks only. Dependency resolution still reads every scanned task. `next` and
  `prime` pass an empty filter and behave as before.

## Boundaries

- The JSON shape and pretty output do not change.
- `next`, `prime`, `sample`, `tree`, `graph`, `tags`, and `quiet` get no new flags. Each
  can gain a filter later by flattening `FilterArgs`, but no current need asks for it.
- The default-visibility rules differ from command to command on purpose. `list` hides
  shelved tasks; `graph` and `tags` keep them; `tree` shows shelved tasks under a shown
  parent; `sample` draws from `idea`, `todo`, and `blocked`. Each rule defines that
  command's pool, not a user filter, so this design leaves them where they are.
- **The all-of rule for `--tag` stays.** Changing it to any-of would break existing callers
  and does not belong in this change. I'll file a follow-up to decide it against the
  vocabulary.
- **Ranges are out of scope.** Examples are `-p ..1` or a size range. Repeats cover the
  current need over sets of 5, 5, 3, and 2 values.

## CLI inventory

`tools/cli.toml` is vendored from ops and changes there first. The `list` and `ready` rows
gain the new local options: `--priority`/`-p` as a repeatable enum, `--size`,
`--complexity`, and `--process` as repeatable enums that include `none`, and the moved
`--parallel`. The `ready` row also gains `--tag` (shared, filter, repeatable), `--owner`,
`--source`, and `--parent`. The ops edit is committed on ops `main`, and then
`just vendor-cli` in ops copies it here. The surface conformance test proves that the two
match. Completion gets candidate functions that add `none` to the size, complexity, and
process sets.

## Documentation

- The `README.md` list examples gain one filter example.
- `skills/tasks/SKILL.md` notes that `list` and `ready` take these filters, and that
  `--complexity` is a selection, not the session cutoff. A session under a cutoff still
  picks only through `ready` and `next`.
- The flag help text states the OR-within and AND-across rule once, on `--priority`.

## Verification

Unit tests in `src/filter.rs` cover:

- each field's match;
- OR within a field and AND across fields;
- `none` alone and mixed with values;
- effective complexity, where an escalation above the record wins;
- `of_row` returning `None` for an unresolved row.

End-to-end tests in `tests/cli.rs`:

- `list` with each new flag, with repeats, and with `none`;
- `list --complexity high` finding a task rated `mid` and escalated to `high`;
- `list --parked` with new flags, including an unresolved row that disappears once a filter
  is given;
- `ready --size s --size none`, `ready -p 0`, `ready --tag`, and `ready --parent`;
- `ready --size s` with a claimed task of size `m`, which produces no claim-omission warning;
- a single-value `ready --size` call that behaves as before.

The existing `list`, `ready`, and conformance tests pass unchanged, apart from the
cli.toml rows.
