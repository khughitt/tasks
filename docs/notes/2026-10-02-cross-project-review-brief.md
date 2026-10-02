# Cross-project idea review

## Problem

Reviewing related ideas across projects currently requires manual file searches and
separate scope passes. tasks-af97da asks for title/body search; tasks-c591da asks
for registry-wide discovery and clustering; tasks-4e8cef reports the missing
mixed-project handoff contract. Together they form goal tasks-ffdbaa.

## Current behaviour and evidence

- `src/scope.rs` already opens every reachable registered project and reports missing
  projects. `src/commands/list.rs::list` scans full tasks before producing summaries;
  `src/filter.rs::TaskFilter` supplies shared metadata selection. Filter reuse landed
  in `464e684`. There is no task text-search command. tasks-af97da's latest note
  explicitly requests list-equivalent filters, case-insensitive text, and JSON.
- `list --parked` can resolve a task absent from the scanned checkout through its
  recorded worktree. `src/commands/parked.rs::rows_preferring` establishes copy
  precedence; `open_recorded` supplies the full task. Search must match the same
  copy that supplies the displayed row, including worktree-only bodies.
- `skills/scope/SKILL.md` fixes one root and requires a corrected invocation for
  foreign IDs in an unscoped batch. This deliberately protects worktree ownership;
  it also prevents the mixed-project pass requested by the two scope ideas.
  The skill landed in `ad12540`; its validation note records fixed-root, parent,
  stale-member, and rerun acceptance checks.
- tasks-c591da retains its original mindful source, but
  `mindful --json show c21c822e191c2e6c5939f9caa1f05a39` returned no matching thought.
  Its captured body is available; missing source content remains unknown.

## Constraints

Preserve selection semantics, status pools, warnings, output shapes, and parked copy
precedence for search. Shared repeat filters widen except tags, which remain all-of.
No search index or relevance ranking is needed for a literal query over parsed tasks.

A future mixed-project scope pass must fix each participating checkout, revalidate
each member, preserve existing parents/sources, report partial writes, and reuse
handoffs on rerun. Foreign parents are invalid: use supported cross-project
dependencies and notes for hub coordination. The current pass remains entirely in
this checkout. No existing mixed-project handoff, attached draft spec, or overlapping
open research was found here.

tasks-ece1e2 already owns project groups and goal focus under the work-selection
brief. That is related context, not a prerequisite or work to duplicate. Registry-wide
CLI reads already exist, including completed tasks-6d33e6.

## Alternatives

1. **Incremental search and an explicit scope extension.** Reuse list selection and
   rendering for text search; design bounded multi-project discovery and safe writes
   in the existing skill. This is the lean: it meets both needs without a service.
2. **Keep per-project scope and use search for discovery only.** This improves duplicate
   checks but leaves a connected cluster split into separate handoffs.
3. **Build a semantic discovery service.** This could supply topical ranking, but adds
   indexing and operational work unsupported by these reports. Evaluate ordinary
   sources, tags, parents, and agent judgment first.

## Unanswered questions

- Where does a mixed-project brief live, and how are portable references and rerun
  discovery represented? tasks-91c622 will recommend a deterministic contract for
  user review, using the existing hub/dependency model.
- How should registry-wide selection recognize conceptual connections without shared
  metadata while retaining a bounded batch and deterministic tie-breaks? The same
  design task will specify selection and acceptance examples.
- How are partial writes and changed/claimed members reflected in a cross-project
  handoff? The design must extend the existing per-member skip rules explicitly.

## Proposed decomposition

- tasks-af97da — **scoped**, P2/m/mid/direct: literal title/body search with existing
  list filters, scopes, modes, ordering, warnings, JSON, and pretty rendering.
- tasks-c591da — **briefed**, remains an idea: registry-wide scope discovery.
- tasks-4e8cef — **briefed**, remains an idea: mixed-project root and handoff safety.
- tasks-91c622 — P2/m/high/planned: one shared scope design and implementation plan,
  both reviewed before changing the skill. Completion records the approved finding
  on both waiting ideas and updates this brief in the same commit.

All four are children of tasks-ffdbaa. Search and scope design can progress
independently; existing CLI reads suffice for the design. No separate research task
is needed for the contract choices identified here.
