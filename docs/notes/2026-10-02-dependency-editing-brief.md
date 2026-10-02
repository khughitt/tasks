# Predictable dependency editing

## Problem

Changing one dependency should preserve the others and make the effect clear. Three
reports concern the same mutation paths: a swap requires two commands, adding an
existing alias-equivalent edge silently succeeds, and `edit --depends` unexpectedly
replaces the whole list. The cluster goal is tasks-671956.

## Current behaviour and evidence

- tasks-45e400: `src/cli.rs::Commands::Dep` makes `--on` conflict with `--rm`;
  `src/commands/dep.rs::run` also chooses one branch. It already builds an in-memory
  candidate and saves once, so combined mutations can reuse that structure.
- tasks-2942cb: `dep::run` deduplicates and removes by canonical identity. Adding
  a live ID and then removing its retired spelling removes the same edge. This
  follows the rename contract and its implementation in `1e7b9bc`, rather than an
  alias-resolution defect. Existing tests cover identity, deduplication, removal,
  and preservation of stored retired spellings.
- tasks-8efda8: `src/commands/mod.rs::apply_fields` assigns
  `task.depends = dependencies`; `edit::run` calls it. `TaskFields` help says
  “Depend on another task” without identifying replacement. Tags append and have
  explicit `--no-tags` replacement. Replacement dates to `0c4ff24`. The report
  says five edges were lost, but has no incident transcript; code confirms the
  replacement mechanism, not that specific incident.

No existing dependency-editing brief, attached draft spec, related open research,
or overlapping open implementation task was found in this checkout.

## Constraints

Preserve typed validation, cross-project cycle checks, mutation locking, and the
existing JSON shape. Alias spellings identify one task, and stored spellings remain
until deliberately changed, as required by section 3 of
`docs/specs/2026-09-08-prefix-rename-design.md`. Successful duplicate additions are
already idempotent. Failed mutation batches must leave the original record intact.
Removal must continue to allow cleanup of a stored unreachable dependency.

## Alternatives

1. **Additive edits with explicit replacement.** Make `edit --depends` follow tags,
   with an explicit clear/replace operation. This is the current lean: adding an
   edge preserves existing work. The design must assess intentional replacement
   use cases and settle clear-all behavior before changing this public contract.
2. **Keep replacement and clarify it.** Preserve `edit --depends`, identify whole-list
   replacement in help and examples, and direct additions to `dep --on`. This is
   smaller but leaves protection dependent on reading documentation.
3. **Add a separate replacement command.** This makes the operation explicit but
   expands the command surface; prefer an existing flag pattern if it covers the need.

The two scoped changes can proceed independently of that choice: accept distinct
add/remove sets in one save, rejecting canonical overlap; and warn on existing
edges while preserving successful idempotence. A new alias-migration command is
unnecessary for the reported confusion.

## Unanswered questions

- Which intentional replacement use cases would additive edits alter? The design
  agent can inspect this checkout and tests; external consumers are unknown.
- Should replacement remain the default or become explicit, and how should clear-all
  work across flags and editor mode? tasks-e9af16 will recommend a contract for user
  review, using the existing tag implementation as the starting point.

## Proposed decomposition

- tasks-45e400 — **scoped**, P2/s/mid/direct: one-save dependency swaps, canonical
  overlap rejection, final-graph validation, and failure atomicity checks.
- tasks-2942cb — **scoped**, P2/s/low/direct: explain duplicate canonical additions
  through existing warnings; retain one edge and its stored spelling.
- tasks-8efda8 — **briefed**, remains an idea: waits on tasks-e9af16.
- tasks-e9af16 — P2/m/high/planned: design safe `edit --depends` semantics; obtain
  spec and plan reviews before implementation. Completion records the approved
  finding on tasks-8efda8 in the same commit so it can be re-scoped.

All four are children of tasks-671956. No separate research task is needed: the
remaining work is choosing a public mutation contract, with local caller inspection
inside that design task.
