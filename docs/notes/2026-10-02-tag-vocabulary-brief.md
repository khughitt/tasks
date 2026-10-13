# Tag vocabulary and filter semantics brief

## Problem

Tag meanings should be reusable across projects and, optionally, mindful while
tasks remains useful on its own. Tag filters should also say exactly which tasks
they select. This pass covers tasks-eb2b4e and tasks-ead38f under tasks-cea445;
shared definitions need design, while the reviewed all-of filter decision can
be recorded now.

The 2026-10-08 pass adds tasks-b49ef8: recognizable tag colors across list, show,
prime and tags, with style ownership settled by the same vocabulary design.

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
- Tag rendering groundwork landed with tasks-606552 in `3b58cd7`.
  `src/present.rs::tags` still returns a single Chrome span for the joined names;
  task rows in `src/output.rs` and the overview in `src/show_text.rs` use it.
  `Output::Tags` prints each `row.tag` directly, so changing the shared helper alone
  would miss that view. Project dictionary values remain plain meaning strings in
  `src/repo.rs`; no per-tag style schema is implemented. The original screenshot
  source has no attachment on tasks-b49ef8; this finding is based on current code,
  not a visual judgement.

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

Tag styles must preserve standalone use, plain output and JSON. Keep the existing
rendering and vocabulary dependencies on tasks-b49ef8. A color design must account
for every requested view, tags without assigned styles, malformed style inputs and
terminal contrast; the user judges a rendered sample before visual acceptance.

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

For colors, the captured options are a stable hash-to-palette assignment, optional
styles in the local/shared vocabulary, or styles imported from mindful. Lean toward
settling optional vocabulary styles in tasks-6ac7fd before choosing a palette or
provider. An automatic palette remains an alternative to evaluate, not an approved
fallback for absent or malformed configuration. Avoid a second independent style store.

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
- Should styles belong to the shared definition, and what wins when the same tag
  has different styles across projects? tasks-6ac7fd proposes the contract and
  behavior for unstyled tags; user review settles the intended visual behavior.

## Proposed decomposition

- tasks-cea445 owns this cluster and the eventual delivery.
- tasks-ead38f is scoped as priority 2, small, mid complexity, direct: record the
  all-of exception at the CLI authority and publish it with conformance checks.
- tasks-eb2b4e stays an idea, priority 2, medium, high complexity, planned, pending
  tasks-6ac7fd. Preserve its full captured proposal and questions.
- tasks-6ac7fd produces a user-reviewed design and implementation plan,
  then files implementation children. On completion it updates this brief and
  adds a finding note to both tasks-eb2b4e and tasks-b49ef8 before the result's branch merges so
  scoping can resume. Its existing optional-style question is now explicit in its body.
- tasks-b49ef8 remains an idea, priority 3, briefed against that design. Its rendering
  prerequisite is done, but style-source decisions remain. Preserve its full original
  alternatives and source; no new research or design task is needed.
