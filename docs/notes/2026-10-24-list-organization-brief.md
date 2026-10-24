# Organize filtered task lists by project and parent

## Problem

Tasks-fad561 asks to indent children beneath their goals while retaining list's
filters and sorting. Tasks-1b31a4 asks to keep each project's tasks together in
`list --pretty --all-projects`. Coordinate these two display choices so readers
can combine them without losing selected tasks or changing ordering unexpectedly.

## Current behaviour and evidence

Checked against code at `392155e39d3f48a5b2832d201d1e56d67688eece`
(2026-10-24); the original requests remain in both idea bodies.

- `src/cli.rs::ListArgs` shares list options with search. `--sort` accepts one of
  priority, updated, or created; parked, periodic, and deferred have their own
  ordering and conflict with explicit sort/reverse.
- `src/commands/list.rs::list` filters the scanned union, then sorts it globally.
  `src/query.rs::sort_by_key` orders priority by priority then newest update then
  id; date sorts use newest first then id. `--reverse` reverses the ordinary list.
- `src/output.rs::ListOut` carries a flat `tasks` array; pretty output calls
  `task_table`. Parked output uses a separate `ParkedOut` and table.
- `src/commands/tree.rs` builds each project's forest in scope order.
  `src/hierarchy.rs::forest` sorts roots and siblings by ready order, supports
  arbitrary depth, and retains some unselected ancestors as context. It does not
  implement list's filtering or sorting contract.
- `tests/cli.rs` has `tree_nests_prunes_and_orders`,
  `tree_all_projects_groups_by_project_in_registry_order`,
  `list_sorts_by_priority_updated_or_created_and_prints_the_date`, and
  `search_takes_list_filters_pools_and_order`. These tests were read, not run.

## Constraints

Preserve exact selected membership, existing default output, JSON contracts,
warnings, review markers, project colors, headers, and width handling unless a
reviewed design explicitly changes them. Parents are confined to one project
(`docs/specs/2026-09-03-task-hierarchy-design.md`). Existing wide-list global
ordering and tree's project grouping are documented in
`docs/specs/2026-09-04-multi-project-design.md` §3.2 and §4.1.

Any new CLI option or value must land its rows in ops's `cli.toml` before adoption
and parser changes here. Related open work is distinct: tasks-76d269 proposes a
list limit, tasks-e7424b bounded prime summaries, and tasks-e4ccb6 cross-project
listing performance. No overlapping open design/research or existing handoff for
these two requests was found. Keep this effort inside the existing presentation
lane tasks-1f05a7. No shared needs are declared in this project's configuration.

## Alternatives

1. **Lean: explicit list display options for project grouping and parent nesting.**
   Keep flat defaults and JSON; apply list's order within project groups and among
   siblings. Prefer prefix order for project groups, as tree already uses, and
   hoist children whose parents were filtered out rather than adding unmatched
   rows. Exact spelling and mode interactions remain design work.
2. Add project as a sort key and list filters to tree. This splits the requests
   across commands, and a single project sort cannot also express the existing
   within-project sort. It could reduce display duplication but broadens tree's
   selection contract.
3. Group wide pretty lists by default and show filtered-out ancestors as context.
   This offers structure immediately but changes established reading order and
   adds rows that do not meet the user's filters; explicit opt-in is preferred.

## Unanswered questions

- Design follow-up: choose the option names, their JSON behavior, and whether
  search receives them through shared `ListArgs`.
- Design follow-up: settle missing-parent handling, intermediate missing
  ancestors, arbitrary depth, root/sibling sorting, group ordering, and reverse.
- Design follow-up: settle combinations with local/project/group/all-project
  scopes and parked/periodic/deferred views; cover accepted combinations and
  explicit refusals rather than silently ignoring an option.
- User design review: judge concrete examples of grouping, nesting, and their
  combination, including headers and narrow output, before implementation.

## Proposed decomposition

- **tasks-eceb1c — Organize filtered lists by project and parent:** priority-2
  coordination goal under tasks-1f05a7; ordinary child goal within that lane.
- **tasks-6fcb58 — Design list organization from the brief:** priority 2, size s,
  high complexity, planned process. Both tasks-fad561 and tasks-1b31a4 wait on this
  design. No separate research task is needed for facts already established above.
  Produce a user-reviewed spec, then a user-reviewed implementation plan with
  acceptance cases for membership, ordering, presentation, and unchanged JSON.
  Before its result branch merges, record findings on both ideas using
  `note --no-reviewed` so a later scope pass can reconsider them.
