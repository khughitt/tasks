# Adopting a project rename on another host

Status: proposed for review
Task: tasks-7f1596

## Goal

A synced checkout has already changed from `old` to `new` on another host. On this
host, the checkout's `tasks/.config.toml` and task files say `new`, but the local
registry still has `old`, possibly at a path that no longer exists. One command
converges the local registry and state without writing the synced checkout.

Success means `new` is the sole live registry key at the checkout's current root,
`old` and any earlier aliases resolve directly to `new`, and local parks and
escalations follow the task IDs. A second invocation succeeds without changing
anything. Ordinary `tasks rename old new` keeps its existing behavior.

## Command

Run `tasks rename old new --adopt` from inside the renamed checkout. The command
uses the local project root, not the old path in the registry. It requires the
local config to name `new`; it never rewrites `tasks/.config.toml` or task files.
`--explain` with `--adopt` reports the observed adoption stage or a refusal
without writing. JSON keeps the existing rename shape: `tasks` is zero, and
`recovery` is `fresh`, `resume_registry`, `resume_cleanup`, or `complete`.

`init --prefix new --force` only moves a live key's root and leaves the `old`
key behind. Replaying ordinary `rename` expects old-prefix files. The explicit
adoption flag distinguishes this host's state from an unfinished local rename.

## Preconditions

Under the `old` and `new` mutation locks and registry lock, reload all inputs
before a write:

- Both prefixes are valid and distinct. The checkout config names `new`, and
  every task filename and its `id` name `new`; no `old` task file remains.
- The registry either has live `old` and no taken `new`, or has live `new` at
  this root with `old -> new` for a retry. An earlier alias to `old` is allowed.
  Any other live key at this root, or a live `old` root that still exists at a
  different filesystem location, is a conflict.
- No unfinished ordinary rename inventory names this root or either prefix.
  No live claim exists in either prefix's state store. An existing target store
  may be empty or exactly match the state this adoption would write; other
  target parks or escalations are a conflict.

The command refuses with a typed error naming the conflict. A stale claim is
discarded as ordinary `rename` does; live claims block adoption. Parks and
escalations are rekeyed from `old-<hex>` to `new-<hex>` using the existing
`ClaimStore::carried_renamed_text` rule. The `old` store must contain only
`old` IDs. The matching synced task must exist for each parked or escalated ID;
otherwise adoption refuses rather than hiding orphaned state.

## Mutation and recovery

Preflight computes the carried target store before any write. Then, while all
three locks remain held:

1. Write and verify `claims/new.toml` if there are parks or escalations to
   carry. An identical target file is accepted on retry.
2. Change the registry in memory with `Registry::rename(old, new)`, repoint
   `new` to the current root, and save once. This also flattens earlier aliases.
3. Remove `claims/old.toml`, retaining its `.lock` inode. Do not touch the
   synced checkout.

After step 1, retry recomputes the same target bytes from the old store and
finishes. After step 2, retry verifies the settled registry and target store,
then removes the old store. After step 3, retry reports `complete`. A changed
source or conflicting target on retry causes refusal. This covers process
interruption; it does not promise durability across power failure, matching
ordinary rename's existing limit.

## Checks

End-to-end tests use one checkout copy and two isolated pairs of
`XDG_CONFIG_HOME` and `XDG_STATE_HOME`. Host A renames the copy, moves its root,
and uses `init --prefix new --force`; host B retains its old registration and
a parked task before adopting. Assert old IDs resolve, the park and escalation
survive under new IDs, earlier aliases flatten, no checkout file changes, and
a retry is a no-op. Exercise interruption after each mutation boundary, plus
conflicting target state, a live claim, mismatched checkout files, a foreign
live old root, and an unfinished rename inventory. Run `just gate` before
completion.

## Alternatives

- Extending `init --force` would make a command that currently only repoints
  a root rewrite prefix identity and state. Keep its established meaning.
- Synthesizing old-prefix task files to replay ordinary `rename` would write
  the synced checkout and introduce a second file migration. Adoption only
  needs local registry and state changes.
