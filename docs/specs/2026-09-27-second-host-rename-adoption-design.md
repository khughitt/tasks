# Adopting a project rename on another host

Status: reviewed 2026-09-27; corrections from review incorporated
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

Run `tasks rename old new --adopt` from the renamed project found from the
working directory or global `-C`. The command uses that project's root, not
the old path in the registry. Before the registry switches, `old` must be the
live registry key; passing an earlier alias refuses and names its live key.
After completion, a flattened alias of `new` is accepted as a no-op replay.
The local config must name `new`;
the command never rewrites `tasks/.config.toml` or task files. `--explain`
with `--adopt` classifies without locks or writes, as ordinary rename does;
mutating preflight repeats the observation under locks.

Adoption returns the existing rename JSON fields with `tasks: 0` and an
additional `mode: "adopt"`. Ordinary rename omits `mode`, so consumers can
distinguish the commands without interpreting recovery labels. Adoption's
`recovery` values are defined below; `resume_files` never appears.

`init --prefix new --force` only moves a live key's root and leaves the `old`
key behind. Replaying ordinary `rename` expects old-prefix files. The explicit
adoption flag distinguishes this host's state from an unfinished local rename.

## Preconditions

Under the `old` and `new` mutation locks and registry lock, reload all inputs
before a write:

- Both prefixes are valid and distinct. The checkout config names `new`, and
  every task filename and its `id` name `new`; no `old` task file remains.
- The registry has live `old`. `new` is either untaken, or already live at
  this root because `init --prefix new --force` ran first. On retry, `new` is
  live at this root and `old -> new`. Earlier aliases to `old` are allowed
  and will be flattened. Any other live key at this root is a conflict.
  An `old` root elsewhere conflicts only if it still contains
  `tasks/.config.toml`; an empty directory left by a move does not block.
- No unfinished ordinary rename inventory names this root or either prefix.
  Before the registry switches, no live claim exists in either prefix's
  state store. Before this adoption has written the target, its parks and
  escalations must be a subset of the carried entries, with equal values;
  any other entry conflicts. Local commands can
  already write the `new` store before the registry switches, because they
  use the checkout's prefix. After the switch, cleanup only requires the
  target store to exist and parse when there was carried state; it never
  overwrites that store or bans its live claims.

The command refuses with a typed error naming the conflict. A stale claim is
discarded as ordinary `rename` does. A live old claim names its owner in the
error; the owner must stop using the old checkout and release it, or the
claim must become stale before retry. `tasks claims` can inspect it without
opening the missing checkout. Parks and escalations are rekeyed from
`old-<hex>` to `new-<hex>` using the existing
`ClaimStore::carried_renamed_text` rule. The `old` store must contain only
`old` IDs. An entry whose synced task is absent is still carried, with a
warning naming its ID. `tasks list --parked` already displays an unresolved
park entry, so adoption does not make an orphan disappear.

## Mutation and recovery

Preflight computes the carried target store before any write. Then, while all
three locks remain held:

1. If there is state to carry and the target has none of its parks or
   escalations, write the carried parks and escalations together with the
   target's existing claims, then verify the file. A claim colliding with
   a carried park is a conflict. The same atomic write records
   `adopted_from = "old"` in the target store; ordinary claim-store saves
   retain this field. If that marker is present on retry, skip the write
   and treat the target as authoritative, even when every carried park was
   consumed. Without the marker, a matching nonempty subset also skips the
   write: missing carried entries may already have been consumed by a local
   command. With nothing to carry, skip this step and leave the target
   store unchanged, including stale claims.
2. Move the registry's live `old` key to `new`, repoint `new` to the current
   root, and save once. Reuse `Registry::rename` when `new` is free; when an
   earlier `init --force` already registered `new` here, remove only the
   `old` project entry, flatten its earlier aliases, and add `old -> new`.
   Both paths preserve the registry's invariants in memory before saving.
3. Remove `claims/old.toml`, retaining its `.lock` inode. Do not touch the
   synced checkout.

| Observed state | `recovery` | Next action |
| --- | --- | --- |
| No adoption write | `fresh` | Write carried store, then registry. |
| Target has the adoption marker or a matching nonempty subset, registry still has live `old` | `resume_registry` | Keep target claims and entries; save registry. |
| Registry settled, old store remains | `resume_cleanup` | Verify target file exists and parses if state was carried; remove old store. |
| Registry settled, old store gone | `complete` | No write. |

With nothing to carry, step 1 is skipped and `resume_registry` cannot occur.
`TASKS_RENAME_STOP_AFTER=store`, `registry`, and `claims` inject stops after
the target write, registry write, and old-store removal. Before step 2, retry
compares an unmarked target's remaining park and escalation entries with the
carried set, refusing a foreign or changed entry, and refuses live claims.
A marked target has already received the carried state, so later local
park and escalation changes are authoritative. It does not require consumed
entries to reappear; the live-claim refusal still applies before step 2.
After step 2, ordinary commands
may also replace a carried park with a claim or clear an escalation. The
settled registry and a valid target store are sufficient for cleanup.
Ordinary rename has the same post-registry concurrency window; adoption
leaves the target store authoritative on both sides of the switch. This
covers process interruption, not power failure, matching ordinary rename's
existing limit.

## Checks

End-to-end tests use one checkout copy and two isolated pairs of
`XDG_CONFIG_HOME` and `XDG_STATE_HOME`. Host A renames the copy, moves its root,
and uses `init --prefix new --force`; host B retains its old registration and
a parked task before adopting. Assert old IDs resolve, the park and escalation
survive under new IDs, earlier aliases flatten, no checkout file changes, and
a retry is a no-op. Repeat host B with the partial `init --force` workaround.
Exercise the `store`, `registry`, and `claims` stop points, including a
`start` that consumes a park after `store` and after `registry` before retry.
Assert stale target claims survive the first write. Check orphaned
parks, conflicting target state, a live old claim, mismatched checkout files,
a foreign old root with a task config, an empty old directory, an older alias
as the argument, and an unfinished rename inventory. Run `just gate` before
completion.

## Alternatives

- Extending `init --force` would make a command that currently only repoints
  a root rewrite prefix identity and state. Keep its established meaning.
- Synthesizing old-prefix task files to replay ordinary `rename` would write
  the synced checkout and introduce a second file migration. Adoption only
  needs local registry and state changes.
