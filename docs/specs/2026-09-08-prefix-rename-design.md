# Renaming a project prefix

Status: designed (2026-09-08)
Tasks: tasks-8c9398

## 1. Problem

A prefix is a mutable human label embedded in an immutable identifier. Renaming the label
therefore rewrites identity everywhere: the registry key, `prefix` in the project's
`tasks/.config.toml`, every task filename and `id:` field, and every inbound reference in
every *other* registered project.

Done by hand on 2026-09-06 for `aut` → `autonomy` (empty, trivial) and `dot` → `dots`
(2 task files, 4 inbound refs across ops and prism, 3 doc mentions). The manual procedure
works but is unguided, and it leaves debris: `dot.lock` and `dot.toml` still sit in the
claim store today.

Measured across the 16 registered projects and 787 tasks:

| | |
|---|---|
| cross-project `depends` | 64 |
| foreign parents | 0 — `check` rejects them, so `depends` is the only structured inbound ref |
| prose id mentions in bodies and notes | 124 |
| worst single prefix | `ops` (13 structured, 36 prose); `material` (13 structured, 21 prose, 96 own files) |

A stale inbound reference is not fatal today: `check` reports `unreachable_dep` as a
*warning*. The system degrades rather than breaking. That is the bar to beat, not zero.

**Rejected: opaque ids.** Hex-only ids with the prefix demoted to a display alias make
rename a one-line registry edit, but cross-project references stop resolving without a
global index, collision pressure forces longer ids, and `depends: [5bc782, b32b3d]` tells
a human reading a diff nothing. Reading an id is a daily act; renaming a prefix happens
once per project lifetime. Do not pay the common case to cheapen the rare one.

**Rejected: rewriting inbound references in other repositories.** The original sketch had
`rename` write to every affected project. Section 2 makes that unnecessary: because a
rename preserves the hex, an alias needs to rewrite only the prefix component, so every
one of the 64 structured refs and all 124 prose mentions keep resolving with no writes
outside the renamed repository at all. Prose in particular must never be rewritten — a
note reading "moved to dot-a00088 after registering dotfiles" was true when written, and
mechanically editing it would trade historical fidelity for a resolvability the alias
already provides.

## 2. Model

A **prefix** is a project's live name. An **alias** is a retired prefix: a name the
project used to have. Aliases are permanent and never become a second live name — new
tasks always receive the live prefix.

**Resolution rule.** An id `<a>-<hex>` where `a` is an alias of project P resolves to
`<P.prefix>-<hex>` in P. Only the prefix component is rewritten; the hex is preserved by
the rename, which is what makes this a rule rather than an id index.

**Aliases never chain.** Renaming `dots` → `dotfiles` re-points the existing `dot` alias
as well, so every alias targets a live prefix and resolution is always one hop.

**Where an alias is accepted:** anywhere a project is named — inside an id (`show`,
`tree`, `dep --on`, every write command) and in `--project`. An alias given as `<old>` to
`rename` resolves to the live source prefix, and the command reports which project it is
actually renaming.

**Name reservation is scoped to registration, not to eternity.** A prefix that is any
project's live prefix *or* any project's alias cannot be claimed by `init` or `rename`.
`unregister` releases both (§5.4). Reserving names beyond registration was considered and
rejected: `Registry::unregister` already frees a prefix, a unit test pins that behaviour,
and a tombstone store would have to outlive the thing it describes.

## 3. Registry format

Purely additive. A second top-level table, `#[serde(default)]`, so existing files load
unchanged:

```toml
[projects]
dots = "/home/…/dotfiles"

[aliases]
dot = "dots"
```

`Registry` gains `aliases: BTreeMap<String, String>` mapping alias to live prefix, and:

- `canonical_prefix(&self, prefix: &str) -> &str` — the live prefix, following at most one
  alias hop.
- `canonical_id(&self, id: &TaskId) -> TaskId` — the same rule applied to an id.
- `is_taken(&self, prefix: &str) -> bool` — live prefix or alias of any project.
- `rename(&mut self, old: &str, new: &str)` — moves the key, records `old -> new`, and
  re-points every alias that targeted `old`.
- `unregister` — additionally drops every alias targeting the removed prefix and returns
  them, so the caller can name them in its output.

Two invariants are enforced in `Registry::load_from`, so a hand-broken file fails on the
next command rather than resolving wrongly: every alias target is a live prefix, and no
alias collides with a live prefix. Both are `config` errors naming the offending key.

## 4. Canonicalization

`canonical_id` owns the rule, but several existing callers must apply it. The placement
is deliberate in all three directions.

**Applied to user input**, in one helper wrapping `TaskId::parse`, so that every newly
written reference stores the live prefix: `show`, `tree` (through `open_id_read_ctx`),
`root`, every write command (through `open_id_write_ctx`), `dep --on` and `dep --rm`,
and `--parent` / `--depends` on both `add` and `edit`.

**Applied to resolution**, so that references already stored with a retired prefix still
resolve: `Resolver::resolve_task`, the edge function in `find_cycle`, and parent lookups
in `hierarchy`.

**Applied to comparison**, on both sides: `dep`'s self-dependency check
(`dependency == task.id`), its duplicate check (`task.depends.contains`), and its removal
(`task.depends.retain`) — so `dot-a00088` and `dots-a00088` are one task for dedup,
removal, self-reference, and cycle detection. `dep --rm` accordingly matches either
spelling.

**Deliberately not applied to `task.depends` and `task.parent` in memory.** `save`
rewrites the whole file from the model, so canonicalizing the loaded record would mean
that any unrelated later edit silently rewrites that repository's stored references —
erasing the `retired_prefix` nudge as a side effect of an unrelated command. Comparisons
canonicalize; the bytes stay put until someone deliberately updates them.

**Routing compares canonical against local.** `open_id_write_ctx` and `open_id_read_ctx`
compare the *canonical* id prefix against the *local project's* prefix. Without this, an
alias of the current project fails the equality test and re-opens the registered root —
which from a worktree is the main checkout, silently routing a write out of the checkout
the caller is standing in. That is the failure tasks-76671b was filed for, re-entering
through the alias door.

## 5. The `rename` command

    tasks rename <old> <new>

`<old>` is explicit so the command works from anywhere, symmetric with
`unregister <prefix>`.

### 5.1 Phases

Mutation happens only after a complete preflight. Repo precedes registry because
`tasks/.config.toml` is the authoritative name and the registry is an index over it: an
interruption after P3 leaves a project genuinely renamed with a stale index, rather than
an index asserting something untrue.

| Phase | Action |
|---|---|
| P1 | Preflight. No writes. |
| P2 | File pass, per file: write `new-<hex>.md`, then remove `old-<hex>.md`. |
| P3 | `tasks/.config.toml` prefix (atomic). |
| P4 | Registry key and alias records (atomic, under the registry lock). |
| P5 | Remove `claims/<old>.toml`. |

P5 removes the claim store only. **`<old>.lock` is never unlinked**, in this phase or any
other: it is an flock inode another process may hold or be waiting on, and removing it
would silently break mutual exclusion for that waiter. The orphaned lock file is inert and
is accepted as litter.

### 5.2 Preflight

Refuses before touching anything, naming the specific obstacle:

- `new` fails `is_valid_prefix`, or `new == old` after alias resolution.
- `new` is taken — a live prefix or an alias of any project.
- `old` does not resolve to a registered, reachable project.
- The project's `tasks/` is dirty, and the dirt is not a resumable rename (§5.3). Outside
  a git repository there is no dirty check and no undo; the rename proceeds with a warning.
- Any **live claim** exists on the project.
- The repository has **more than one git worktree**. Other checkouts keep the retired
  prefix in their config and filenames, which would recreate the routing hazard of §4
  after an otherwise successful rename.
- Any task file fails to parse, or disagrees between filename and `id:` field.
- A destination `new-<hex>.md` already exists **and is not the expected transformation of
  its source** — a destination that *is* the expected transformation is a resumable
  boundary (§5.3), not a collision.

Parsing in preflight and in the file pass uses `parse_task`, which carries no
project-prefix check, so a half-finished directory is readable. The filename/frontmatter
agreement and collision checks above are what `read_task_with_raw` would otherwise have
provided, and are performed explicitly rather than lost.

Preflight opens the project by the **root recorded in the registry**, not through
`open_registered`, and accepts a config prefix of either `old` or `new`. Between P3 and P4
the registry names `old` while the config already says `new`, and `open_registered`'s
"registry maps X to a root whose prefix is Y" guard fires on exactly that state — correctly
for every other command, which should fail loudly in that window, but fatally for the one
command whose job is to close it. This is the only mismatch `rename` tolerates, and only
between those two specific values.

### 5.3 Resume

There is no journal. A journal could not be trusted here in any case: `atomic_write` does
no `fsync` (§9), so a marker file is no more durable than the state it describes. The
three interruption states are instead distinguishable from the world itself:

| Observed | Meaning | Action |
|---|---|---|
| config `old`, some files carry `new` | interrupted in P2/P3 | finish the file pass, then P3 |
| config `new`, registry key `old` | interrupted at P4 | finish the registry (reached only by opening the recorded root directly, §5.2) |
| config `new`, registry `new`, alias recorded | already complete | report and exit 0 |

The dirty check is **verified on resume, not skipped**. An alias alone is not evidence of
an interrupted rename — it equally describes a completed one — so skipping the check
whenever an alias exists would wave unrelated edits through. Instead every dirty path must
be an expected member of the rename set: a deleted `old-<hex>.md`, an added `new-<hex>.md`
whose bytes equal the expected transformation of its source, or the modified
`.config.toml`. Anything else refuses and names the file. Content, not a marker, is what
separates an interrupted rename from a conflicting edit.

At a file boundary a crash can leave both source and destination present. On resume the
source is removed only if the destination equals the expected transformation of it;
otherwise the command refuses, naming both paths.

### 5.4 `unregister` and `init`

`unregister <prefix>` removes the project **and every alias targeting it**, naming the
dropped aliases in its output rather than dropping them silently. Leaving them would
dangle the §3 invariant and fail every subsequent command. Consistency argument: once the
project is unregistered its ids cannot resolve at all, alias or not.

`unregister <alias>` is refused — "`dot` is a retired prefix of `dots`; unregister `dots`
to remove the project."

`init --prefix <taken>` is refused when the prefix is a live prefix or any alias.
`init --force` re-pointing a live prefix needs no special handling: aliases target the
prefix name, not the root, so they follow the project to its new location.

### 5.5 Byte preservation

Each file is rewritten by parsing its frontmatter with `frontmatter::parse`, adjusting
`id` and any local `depends` / `parent` naming the source prefix, re-serializing *only*
the frontmatter, and appending everything after the closing delimiter verbatim.

`serialize_task` is not used. It reconstructs the whole file from the model — rebuilding
frontmatter pairs and re-emitting notes — so a file carrying accepted but non-canonical
whitespace would come back normalized, turning a rename into an unreviewable diff. Tests
therefore feed hand-written files, not serializer output.

References to *other* projects inside the renamed repository are untouched.

## 6. Concurrency

**Both prefix locks, in sorted order.** Holding only `old` would let a writer acquire
`new` the moment P4 lands. `rename` holds the `old` and `new` mutation locks for the whole
operation, acquired in a deterministic order so two concurrent renames cannot deadlock.

**A registry lock.** `Registry::load` → mutate → `save` is an unserialized
read-modify-write; atomic replacement prevents a torn file but not a lost update. A lock
file beside `projects.toml` is taken by `init`, `unregister`, and `rename` for the whole
read-modify-write. This closes a race that predates this design.

**Revalidation after acquiring a lock.** A writer that was already waiting on a prefix
lock resolved its routing *before* the rename landed. Write commands therefore re-resolve
the registry and the project's config after acquiring their lock, not before, and act on
the re-resolved context.

Claims are checked under the lock, not before it.

## 7. Command surface and JSON contract

```
tasks rename <old> <new>
    Renames a project's prefix. Rewrites the project's own task ids, its config, and the
    registry key; records <old> as a permanent alias so existing references elsewhere keep
    resolving. Refuses on a dirty tasks/, a live claim, more than one worktree, or a name
    already taken. Re-run to resume an interrupted rename.

rename -> { prefix, previous, root, tasks: int, aliases: [string], warnings }
           prefix is the new live prefix, previous the retired one, tasks the number of
           task files rewritten, aliases every alias now targeting this project.

unregister += aliases: [string]   the aliases dropped with the project

check    += kind retired_prefix (warning): a depends entry naming a retired prefix,
            quoting the current id.
```

No existing JSON shape changes meaning. Refusals reuse existing error kinds: `config` for
a taken or unresolvable name and registry invariant violations, `validation` for a dirty
`tasks/`, multiple worktrees, or a conflicting file at a resumed boundary, and `claimed`
for a live claim.

`--pretty` prints the new prefix, as `init` prints its prefix.

Completion offers live prefixes only. Aliases resolve; they are not suggestions.

## 8. Testing

Unit: `canonical_prefix` and `canonical_id` (alias hit, miss, live prefix unchanged);
`Registry::rename` re-points a chained alias; `unregister` returns the aliases it drops;
`load_from` rejects a dangling alias target and an alias colliding with a live prefix; a
registry with no `[aliases]` table loads unchanged.

End to end (`tests/cli.rs`):

- ids, filenames, and local `depends` / `parent` rewritten; body and notes byte-identical,
  starting from a **hand-written** file with non-canonical whitespace
- an inbound `depends` from another project still resolves after the rename, with that
  project untouched on disk
- `show`, `tree`, `dep --on`, and `--project` all accept a retired prefix
- a retired id of the *current* project routes to the current worktree, not the registered
  root
- `dep --on` with a retired id deduplicates against the stored live one; `dep --rm` matches
  either spelling; a cycle through a retired id is detected; a retired id of the task
  itself is a self-dependency
- an unrelated `edit` does **not** rewrite that repository's stored retired references
- `check` raises `retired_prefix` in the referring repository while the renamed repository
  is clean
- refusals: dirty `tasks/`, live claim, more than one worktree, `new` taken as prefix or
  as alias, `new == old`, filename/frontmatter disagreement, destination collision
- a double rename collapses chains so every alias targets a live prefix
- `unregister` drops the project's aliases and names them; `unregister <alias>` is refused;
  `init --prefix <alias>` is refused
- **interruption after every mutation boundary**: mid file pass, between the file pass and
  the config write, between the config write and the registry write, and between the
  registry write and the claim-store removal — each resumed by re-running the command
- a conflicting duplicate at a resumed file boundary refuses and names both paths
- dirty recovery: an unrelated edit present alongside a half-finished rename refuses
- concurrent writers: a writer waiting on the prefix lock during a rename re-resolves and
  acts on the new identity rather than the stale one

## 9. Known limits

- **"Crash recovery" means process death and interruption, not power loss.**
  `atomic_write` performs no `fsync` on the file or its directory, so nothing in this
  design — or anywhere else in the tool — establishes durability across power failure.
  Changing that is a separate piece of work affecting every write.
- **An orphaned `<old>.lock` remains** after a rename, by design (§5.1).
- **A branch predating the rename** produces a checkout whose config and filenames use the
  retired prefix while the registry names the new one. Commands there fail loudly through
  the existing `open_registered` guard. Renaming refuses while a second worktree exists,
  but cannot prevent a later checkout of an old branch.
- **Unregister-then-reuse remains ambiguous.** Unregister `dots`, later `init --prefix
  dot`, and old prose naming `dot-a00088` resolves into the new project if that hex exists
  there. Six hex characters make this vanishingly unlikely, and the hazard already exists
  for any unregistered-then-reused prefix.
- **Prose is never validated.** `check` sees structured references only, so the 124 prose
  mentions are neither warned about nor rewritten. They keep resolving through the alias,
  which is the entire point.
