# Tag vocabulary and filter semantics brief

## Problem

Tag meanings should be reusable across projects and, optionally, mindful while
tasks remains useful on its own. Tag filters should also say exactly which tasks
they select. This pass covers tasks-eb2b4e and tasks-ead38f under tasks-cea445;
shared definitions need design, while the reviewed all-of filter decision can
be recorded now.

## Current behaviour and evidence

- `src/repo.rs::tag_dictionary` reads an optional project `[tags]` table and
  rejects empty or multiline entries. `src/commands/check.rs` warns
  `undefined_tag` only for open work in a project with a dictionary; feedback
  records in an opted-in project receive explicit generated-tag exemptions.
- `src/commands/tags.rs::meaning_of` uses the local dictionary, or the first
  project defining a tag in the selected all-project scope. It displays one
  meaning, even when projects define the same spelling differently. `tags`
  lists tags used by selected tasks, rather than every dictionary entry.
  Commit `50cc66a` delivered dictionaries; the integration test
  `tags_carry_the_dictionary_meaning_and_check_holds_open_work_to_it` records
  these rules. This checkout has no `[tags]` table.
- `src/filter.rs::TaskFilter::matches` requires every requested tag.
  `src/cli.rs::FilterArgs`, its unit coverage, and the implemented
  `docs/specs/2026-09-30-task-filters-design.md` agree. Commit `464e684`
  introduced the shared filter. Repeats of other selection fields widen.
  The list and ready tag rows in `tools/cli.toml` have no exception text.
- tasks-eb2b4e describes a missing tag meaning incident and mindful tag thoughts.
  These are captured reports: the foreign incident records and mindful
  implementation were not inspected here. Its source remains
  `user:2026-09-30 thought-sweep session`.

## Constraints

The implemented dictionary spec preserves projects without dictionaries and
defers shared dictionaries, mentioning rendering shared entries into project
tables. Host configuration in `src/config.rs` currently concerns relay identity;
no tag-provider contract exists there. Tag meanings and validation must agree
without introducing silent failure. Preserve JSON shapes unless the design
explicitly authorizes a change, and account for feedback-generated tags.

The CLI inventory is vendored from ops. Its header requires changes at the
authority followed by publication. tasks-964c95 records the prior publication
workflow; the current authoritative workflow must be inspected before editing.
Shared definitions do not require changing tag selection.

## Alternatives

1. **Render common entries into existing project dictionaries.** Reuses current
   validation and runtime behavior. It leaves ownership of the shared source and
   regeneration to tooling; an optional mindful export could feed that source.
2. **Resolve a shared local dictionary plus project overrides.** Offers immediate
   reuse across projects. It needs explicit precedence, opt-in/enforcement rules,
   and a policy for conflicting meanings in cross-project output. An optional
   import can keep runtime independent of mindful.
3. **Query an external provider at runtime.** Matches the captured provider idea
   but adds process execution, schema, caching, and failure decisions; two-way
   synchronization also needs rename and conflict semantics.

Lean: compare the first two before choosing a live provider; existing tables or
a local snapshot may cover the need. Keep all-of tag filtering and annotate its
vocabulary exception, rather than changing established callers to any-of.

## Unanswered questions

- Who owns a common meaning, and can a project override, narrow, or rename it?
  The design author should propose precedence for user review.
- Should shared entries enable validation when no project table exists, and how
  should cross-project output expose conflicting meanings? The design author
  should provide examples and acceptance checks.
- What can mindful export or import today, and is a one-way snapshot sufficient?
  The design author must inspect its actual interface; user review settles the
  desired direction and alias/archive semantics.
- What happens when an explicitly configured source is missing or malformed?
  Design must specify errors or visible warnings, offline behavior, and snapshot
  freshness while preserving standalone use.

## Proposed decomposition

- tasks-cea445 owns this cluster and the eventual delivery.
- tasks-ead38f is scoped as priority 2, small, mid complexity, direct: record the
  all-of exception at the CLI authority and publish it with conformance checks.
- tasks-eb2b4e stays an idea, priority 2, medium, high complexity, planned, pending
  tasks-6ac7fd. Preserve its full captured proposal and questions.
- tasks-6ac7fd produces a user-reviewed design and implementation plan,
  then files implementation children. On completion it updates this brief and
  adds a finding note to tasks-eb2b4e in the same commit so scoping can resume.

