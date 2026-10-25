# Project routing for filtered reads

Status: approved for planning by Keith on 2026-10-25 at f03f0b4;
round-4 wording clarifications applied without changing the approved policy.
Task: tasks-284d9b.

## Intent and evidence

An ID-valued filter should select its task's project, like a positional ID.
Today `show ops-2fe949` and `tree ops-2fe949` route through the registry outside
every project, while `list --under ops-2fe949` fails with `no_project`.
Inside another project the filtered read fails with `task_not_found`, even
though the full ID identifies an unambiguous destination.

Code inspected at `998e6a4`:

- `src/commands/mod.rs::open_id_ctx_unwarned` retains a matching local checkout,
  follows a foreign prefix through `scope::open_registered`, and falls back to
  registry routing only on `Error::NoProject`.
- `open_id_read_ctx` already adapts that resolver for `tree <id>` and lets an
  explicit read scope win. `open_read_ctx` has no filter inputs.
- `list`, `search` and `ready` carry `FilterArgs.parent` and `.under`; `next`
  carries its own `under`. `lanes`, `prime`, `tags` and `sample` have no ID filters.
- `TaskFilter::parse` canonicalizes ID filters; `check_parent` and
  `resolve_under` validate their targets against the selected scan.
- `hierarchy::validate_parent` requires a parent to share its child's project.
  Foreign hierarchy filters cannot match tasks in the current project.
- `FilterArgs.source` and `TaskFilter::matches` implement literal source equality.
  A source identifies provenance, which can belong to another project.

This proposal changes scope selection for ID filters only. It adds no command,
flag, dependency, stored field or JSON shape. It does not change mutation routing.

## Selected contract

The user approved this contract for planning. The existing Rust CLI and registry
remain the implementation platform.

### Routing precedence

Apply these rules to `list`, `search`, `ready`, and `next`:

1. An explicit `--project`, `--group` or `--all-projects` selects the scope as
   today. ID filters narrow that scope; they never enlarge or replace it.
   A bare suffix outside every project is `invalid_id`, including with explicit
   scope: explicit scope does not define shorthand's local prefix.
2. Without explicit scope, use `--parent` and/or `--under` to select the project
   exactly as a positional ID does. A canonical prefix matching the local project
   keeps that checkout, including `-C`, linked records and worktrees; a foreign
   prefix opens its registered project. Outside every project, use the registry.
3. With neither ID filter, keep the existing current-project lookup. If that
   fails with `no_project`, retain the kind and add guidance naming
   `--all-projects` and `--project <prefix>` to the detail. The shared opener does
   give this same guidance on an unfiltered call; no predicate-specific error
   plumbing is needed. Other discovery failures remain errors, not routing hints.
4. Without explicit scope, parse every supplied ID filter before selecting a
   destination. Existing trailing ASCII period/comma trimming and retired-prefix
   canonicalization apply.
5. If both filters name different canonical prefixes without explicit scope,
   fail with existing `validation`, identifying both flags and prefixes and
   advising IDs from one project, or explicit scope when intended. Apply this
   check inside a project too. Do not pick the first flag or scan both projects.
   Same-prefix filters may name different tasks; their predicates intersect.
6. Resolve the agreed prefix using the existing ID resolver. An unknown prefix
   or unavailable registered destination follows its existing typed error
   contract (`unresolvable_id`, or the more specific existing config/storage
   error). No fallback to a different checkout or registry-wide search.

Explicitly scoped calls retain existing filter validation and error precedence:
a target absent from the selected scan is `task_not_found`, even when its prefix
belongs to a different project. Rule 5 does not apply to explicit scopes; their
filters retain ordinary intersection semantics. A parent in one project and an
ancestor in another can never match the same task, so if both targets exist in
an explicit wide scope the intersection is empty. This preserves existing filter
behavior and the requirement that parents share their child's project. There is no new routing
warning; full IDs select the destination as on `tree <id>`. Existing retirement
and scope warnings remain.

Once routed, each command keeps its own status pool, sorting, limit, claim and
halt checks, resource handling, parked behavior and output. A routed read has the
same JSON shape and warning policy as a read in that registered project.

### Source remains literal

`--source` does not select a project, even when it looks exactly like a full task
ID or contains an unregistered prefix. It is never canonicalized, punctuation
trimmed, resolved, or checked for the existence of a source task.

Consequently, outside every project:

| Invocation | Result |
| --- | --- |
| `tasks list --under ops-2fe949` | Read descendants in registered `ops`; validate that target exists. |
| `tasks list --parent ops-2fe949` | Read direct children in registered `ops`; validate that target exists. |
| `tasks list --source ops-2fe949 --status todo` | Still `no_project`, with `--all-projects` / `--project <prefix>` guidance: provenance alone supplies no scope. |
| `tasks list --all-projects --source ops-2fe949 --status todo` | Find exact-source matches across reachable, non-retired registered projects. |
| `tasks list --project ops --source ops-2fe949 --status todo` | Find exact-source matches in registered `ops` only. |
| `tasks list --under ops-2fe949 --source other-000001` | Route to `ops`; apply the source string literally there. |

The original `--source` example therefore needs an explicit scope, named in the
error's detail without changing its JSON envelope. This is a deliberate limitation:
routing to the source ID's project can exclude the tasks
filed elsewhere from that source. Implicitly searching every project would
also introduce a broader and potentially slower default just because of cwd.
Neither behavior is selected here. The README and shipped tasks skill must
explain the explicit all-projects form alongside the new filter routing.

### Other reads and completion

`tree <id>` retains its existing execution behavior: a positional ID routes even
inside another project, unless explicit scope wins. Filters now follow that
same policy. `tree` without an ID, `lanes`, `prime`, `tags`,
`sample`, and `quiet` have no relevant ID filter and retain current behavior.
String filters such as `--plan` and `--ref` never infer a project.

`complete::scoped` is shared by the read filters and positional `tree <id>`.
Keep it shared: without explicit scope, use `local_or_foreign` so a typed foreign
prefix such as `ops-` offers IDs from its registered project both inside and
outside another project. A matching prefix retains local/worktree candidates;
an empty fragment offers local IDs only and never enumerates every project.
No bare suffix is offered outside a project. Explicit project/group/all-projects
completion stays scoped as today. This intentionally improves positional `tree`
completion too, bringing it into line with its existing execution routing.
Best-effort completion failures stay silent; write completion is unchanged.

## Implementation boundary

Add one shared filtered-read context opener in `src/commands/mod.rs`, taking the
scope plus the optional parent and under references. Dispatch `list`, `search`,
`ready`, and `next` through it; `next` supplies only under. Explicit scope
delegates directly to `open_id_read_ctx`, preserving existing validation order.
Otherwise parse the supplied filters with `parse_id`, `Shorthand` and registry
canonicalization, check prefix agreement, then pass one original filter reference
to `open_id_read_ctx` (or None when neither is supplied). This reuses
`open_id_ctx_unwarned` for the destination and preserves shorthand's original
local meaning. Decorate only a returned `NoProject` with the scope guidance above.
Keep command-specific filter evaluation in `TaskFilter` and existing read functions.

The builder may choose a small extraction to avoid repeated discovery/parsing,
provided positional routing's behavior stays intact. No generic resolver
framework, compatibility layer or new policy mode is needed.

Affected surfaces are dispatch/context selection and its no-project detail,
shared read-filter/tree completion, integration tests, help descriptions for
parent/under where needed, README
routing reference, and `skills/tasks/SKILL.md`. No new parser surface requires
an ops vocabulary change; do not hand-edit `tools/cli.toml`.

## Errors and adjacent work

Preserve failures from malformed local config, stale local prefixes, obstructing
Git markers, detached-record pairing and unreadable registries. A full ID is not
permission to bypass those errors. Preserve a matching local/worktree checkout
even when the registry holds an older copy.

tasks-e147b9 is still an open investigation into full-ID attachment routing
beneath an empty ancestor `.git`; it has no reproduced finding to adopt yet.
This proposal shares the existing resolver without changing the NoProject-only
fallback or marker policy. Recheck that task before implementation; a later
finding that changes these assumptions requires an explicit spec revision.
Do not reproduce or fix its incident as part of this task.

## Verification and acceptance

Implementation begins only after user approval of this spec and a subsequent
implementation plan. Execution Setup runs the full `just test` baseline before
code changes and records test counts. Use `just test-one` for focused RED/GREEN
cases, then `just test-fast` and the repository's normal checks before commits.

Extend the existing integration fixtures to cover:

- Routing from outside every project and from another project for both filters
  on list/search/ready, plus next's under; list's parked mode follows the same
  project selection.
- Same-prefix combined filters, canonical aliases, trailing punctuation, and
  cross-prefix conflicts inside and outside a project; missing targets and unknown
  prefixes. Explicit wide scope with both conflicting targets returns no matches.
- Foreign filter routing, same-prefix worktree-only records, `-C`,
  linked records, explicit project overriding a worktree, and group/all-projects
  scopes that include or exclude filter targets.
- Bare suffix failure outside a project and local suffix meaning when explicit
  scope selects a different project.
- Literal source values, including ID-shaped values and punctuation, with
  explicit scope and with an ID filter supplying scope; source alone outside
  still gives `no_project` and names both explicit scope alternatives in its detail.
- Malformed local config and broken Git metadata retaining their typed errors;
  existing positional routing controls continue passing.
- Shared filter/tree completion for a typed foreign prefix inside and outside a
  project, matching local/worktree candidates, no outside suffix candidates,
  no registry-wide enumeration on an empty outside fragment, and unchanged
  explicit completion boundaries.

Use subprocess environments built key by key, temporary HOME/config/state and
isolated Git configuration. Never inherit credentials or Git selectors into
fixtures. Git-writing fixtures verify both their top level and absolute Git
directory before their first write. No host registry or launcher changes are
needed for these tests. Existing output assertions protect JSON contracts.

Success is consistent positional-ID and ID-filter routing without implicit source
interpretation, explicit-scope widening, storage writes, or discovery-error suppression.
