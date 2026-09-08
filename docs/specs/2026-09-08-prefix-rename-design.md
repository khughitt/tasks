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

**A stale local checkout is checked explicitly, because no existing guard sees it.**
`open_registered`'s "registry maps X to a root whose prefix is Y" check validates the
*registered destination*, which after a rename is entirely consistent — it never looks at
the checkout the caller is standing in. And `Project::locate` reads the local
`tasks/.config.toml` without consulting the registry at all, so a checkout of a branch
predating the rename presents prefix `dot` and every command believes it: `add` would mint
a fresh `dot-<hex>.md` with nothing objecting. Every command therefore checks its **local**
project prefix against the registry: a local prefix that is a retired alias of a registered
project is a typed refusal naming both names and telling the caller to update the checkout.
Reads refuse too — a `list` that silently reports a project under a name the registry
retired is the same lie, more quietly told.

## 5. The `rename` command

    tasks rename <old> <new>

`<old>` is explicit so the command works from anywhere, symmetric with
`unregister <prefix>`.

### 5.1 Phases

Mutation happens only after a complete preflight. Repo precedes registry because
`tasks/.config.toml` is the authoritative name and the registry is an index over it: an
interruption after P4 leaves a project genuinely renamed with a stale index, rather than
an index asserting something untrue.

| Phase | Action |
|---|---|
| P1 | Preflight. No writes. |
| P2 | Write the **inventory** (§5.1.1). First mutation; purely additive. |
| P3 | File pass, per file: write `new-<hex>.md`, then remove `old-<hex>.md`. |
| P4 | `tasks/.config.toml` prefix (atomic). |
| P5 | Registry key and alias records (atomic, under the registry lock). |
| P6 | Remove `claims/<old>.toml`, then the inventory. |

P6 removes the claim store's `.toml` only. **`<old>.lock` is never unlinked**, in this phase or any
other: it is an flock inode another process may hold or be waiting on, and removing it
would silently break mutual exclusion for that waiter. The orphaned lock file is inert and
is accepted as litter.

The inventory is removed last, after the claim store, so from the moment P2 completes
until P6 finishes its presence means "a rename is unfinished", and means nothing else.

#### 5.1.1 The inventory

P3 deletes its sources, so from that moment recovery has nothing to compare a destination
against and no way to notice a task that vanished entirely. Recovery therefore needs a
baseline captured before any file moves:

```
~/.local/state/tasks/rename/<old>.toml

source = "dot"
target = "dots"
root   = "/home/…/dotfiles"
entries = [ { hex = "a00088", from = "<digest>", to = "<digest>" }, … ]
```

`from` is the digest of the source file as preflight read it; `to` is the digest of the
transformation preflight computed for it. Together they let recovery say, of every task the
rename covers, whether it is untouched, correctly finished, conflicting, or **missing** —
the last of which no on-disk comparison could establish once the source is gone.

It lives in the state directory, beside claims and locks, rather than in `tasks/`: it is
machine state, it must not appear in `git status` as an untracked file, and it must survive
`git checkout .`.

This does not contradict §9. The `fsync` argument rules out durability across power loss;
it says nothing against an ordinary file under the process-death scope this design chose,
where a written file is perfectly adequate evidence. §5.3's rejection of a journal was
therefore too broad, and is corrected there: what is rejected is a journal *of intent* used
in place of observation, not a *baseline* that makes observation meaningful.

### 5.2 Preflight

**Classification comes first.** The command observes and classifies (§5.3) *before* any
fresh-operation check. This ordering is load-bearing: after P5, `<old>` canonicalizes to
`<new>`, so a re-run of the very same command would trip both the `new == old` and the
taken-name refusals below and reject the idempotent re-run this design promises. Those two
checks therefore apply **only** on the `Fresh` branch; every other verdict routes straight
to its resumption.

On the `Fresh` branch, preflight refuses before touching anything, naming the specific
obstacle:

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
`open_registered`, and accepts a config prefix of either `old` or `new`. Between P4 and P5
the registry names `old` while the config already says `new`, and `open_registered`'s
"registry maps X to a root whose prefix is Y" guard fires on exactly that state — correctly
for every other command, which should fail loudly in that window, but fatally for the one
command whose job is to close it. This is the only mismatch `rename` tolerates, and only
between those two specific values.

### 5.3 Resume

Recovery is derived from the world, not from a record of intent: what a previous process
*meant* to do is never taken as evidence of what happened. The inventory (§5.1.1) is not a
counter-example — it carries no intent to resume and no progress marker, only the baseline
against which the world is read.

The derivation is a **pure function**, `classify(snapshot) -> Recovery`, borrowed in shape
from atoms' A3 executable recovery model
(`python/src/atoms/core/recovery/classifier.py`). It touches no filesystem: the caller
observes once, and the classifier decides. That is what makes the recovery model testable
as a unit over its whole state space rather than only at the boundaries an integration
test happens to interrupt at (§8).

```
Registry = { old_key: Absent | Root(path),
             new_key: Absent | Root(path),
             alias:   Absent | Present(target) }

Entry    = { hex, source: Present(digest) | Absent,
                  dest:   Absent | Expected | Conflicting(digest) }

Dirt     = Clean | RenameOnly | Foreign([path]) | Unknown

Snapshot = { registry: Registry,
             config_prefix: Old | New | Other(name),
             inventory: Absent | Present(Inventory),
             entries: [Entry],
             dirt: Dirt }

Recovery = Fresh | ResumeFiles | ResumeRegistry | ResumeCleanup
         | Complete | Refuse(reason)
```

Three predicates, defined once so the table can be read literally:

- **`files_done`** — the inventory is present and every one of its entries has
  `dest: Expected` and `source: Absent`. Any entry with `dest: Absent` and `source: Absent`
  is a **missing task**, detectable only against the inventory, and is a `Refuse`.
- **`registry_done`** — `new_key` is `Root(root)` for this project, `old_key` is `Absent`,
  and `alias` is `Present(new)`.
- **`rename_only`** — every dirty path is an entry's source (deleted), an entry's
  destination (added, digest equal to its `to`), or `tasks/.config.toml`. `Foreign` names
  the paths that are none of these. Outside a git repository dirt is `Unknown`, and
  `Unknown` is treated as `Clean` for the `Fresh` branch only, with the §5.2 warning.

| Observed | Recovery |
|---|---|
| no inventory, config `old`, no entry has `dest: Expected`, dirt `Clean` | `Fresh` |
| inventory present, not `files_done`, dirt `RenameOnly` | `ResumeFiles` — finish P3, then P4 |
| inventory present, `files_done`, config `new`, not `registry_done` | `ResumeRegistry` (reached only by opening the recorded root directly, §5.2) |
| inventory present, `files_done`, `registry_done` | `ResumeCleanup` — finish P6 |
| no inventory, `registry_done`, config `new` | `Complete` — report and exit 0 |
| anything else | `Refuse`, naming what was observed |

`ResumeCleanup` exists because an interruption between P5 and P6 leaves a fully renamed
project with a stale claim store and a stale inventory. Reporting that as `Complete` and
exiting would strand both; the earlier draft did exactly that.

`registry` distinguishes `old_key` from `new_key` separately, rather than as one three-way
value, because both can be present at once — and can point at *different* roots, which is
neither a resumable state nor a fresh one but a `Refuse` naming both paths.

**The set is closed.** The final row is not a formality: any observation that does not
match an enumerated state is a refusal that says what it saw, never a best-effort guess.
This is the difference between "we thought of these cases" and "unlisted cases fail
loudly", and it is the property the exhaustive unit tests in §8 actually check.

**Classification is separate from authorization.** `classify` decides *what happened*; it
is pure, needs no locks, and can be run to explain a situation. Acting on its verdict is a
second decision, taken only while holding both prefix locks and after post-lock
revalidation (§6). A `Recovery` value is never itself permission to write.

The dirty check is **verified on resume, not skipped**. An alias alone is not evidence of
an interrupted rename — it equally describes a completed one — so skipping the check
whenever an alias exists would wave unrelated edits through. Instead every dirty path must
be an expected member of the rename set: a deleted `old-<hex>.md`, an added `new-<hex>.md`
whose digest equals its inventory `to`, or the modified `.config.toml` — the `rename_only`
predicate above. Anything else classifies as `Refuse` and names the file. Content measured
against the baseline, not a progress marker, is what separates an interrupted rename from a
conflicting edit.

At a file boundary a crash can leave both source and destination present: that is the
`dest: Expected` case, and the source is removed only if the destination equals the
expected transformation of it. `dest: Conflicting` is a `Refuse` naming both paths.

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

Each file is rewritten by parsing its frontmatter, adjusting `id` and any local `depends` /
`parent` naming the source prefix, re-serializing *only* the frontmatter, and appending
everything after the closing delimiter verbatim.

**Not `frontmatter::parse` directly.** `RESERVED` includes `:`, so the strict subset parser
rejects any bare scalar containing one — which every `created:` and `updated:` value does.
`parse_task` already handles this by pre-quoting exactly those two lines before parsing
(`src/format.rs:24`), and `serialize_task` emits them back as `Value::Raw` so they stay
unquoted (`src/format.rs:295`). Calling the subset parser directly would reject ordinary
task files; re-serializing without the `Raw` half would quote every timestamp and change
bytes the rename must not touch. That pre-quote/raw-emit pair is factored out of
`parse_task` and `serialize_task` into a shared helper this pass uses, rather than
duplicated or reinvented.

`serialize_task` is not used. It reconstructs the whole file from the model — rebuilding
frontmatter pairs and re-emitting notes — so a file carrying accepted but non-canonical
whitespace would come back normalized, turning a rename into an unreviewable diff. Tests
therefore feed hand-written files, not serializer output.

References to *other* projects inside the renamed repository are untouched.

### 5.6 Undo and manual recovery

**There is no single-command undo, and `git checkout .` is not one.** The new filenames are
untracked, so a checkout leaves them in place, and the registry lives outside the
repository entirely. Two stores are involved and only one of them is under version control.

The supported recovery is **forward**: re-run `tasks rename <old> <new>`, which classifies
and resumes. That covers every interruption for which the inventory survives.

A genuine rollback is manual, and is documented rather than automated because it spans both
stores:

1. `git checkout tasks/ && git clean -f tasks/` — restores the source files and removes the
   untracked destinations.
2. If P4 landed, restore `prefix` in `tasks/.config.toml` (step 1 does this when the config
   is tracked, which it is).
3. If P5 landed, repair the registry by hand: restore the `<old>` key and remove the
   `[aliases]` entry.
4. Remove `~/.local/state/tasks/rename/<old>.toml`.

The classifier makes step 3 diagnosable rather than guesswork: running `rename` again
reports which phase the world stopped at before anything is written.

## 6. Concurrency

**Both prefix locks, in sorted order.** Holding only `old` would let a writer acquire
`new` the moment P5 lands. `rename` holds the `old` and `new` mutation locks for the whole
operation, acquired in a deterministic order so two concurrent renames cannot deadlock.

**A registry lock.** `Registry::load` → mutate → `save` is an unserialized
read-modify-write; atomic replacement prevents a torn file but not a lost update. A lock
file beside `projects.toml` is taken by `init`, `unregister`, and `rename` for the whole
read-modify-write. This closes a race that predates this design.

**Every writer participates.** `add` currently takes no mutation lock at all — `open_ctx`
and the `--project` arm both construct a `Ctx` with `lock: None`, relying on
`create_task`'s exclusive create, which guards against a colliding id and against nothing
else. After P3 an unlocked `add` would happily create a fresh `old-<hex>.md` in a directory
the rename has already emptied, and no later phase would notice. `add` therefore acquires
the mutation lock for the project it is about to write, on both arms.

**Revalidation after acquiring a lock.** A writer that was already waiting on a prefix
lock resolved its routing *before* the rename landed. Write commands therefore re-resolve
the registry and the project's config after acquiring their lock, not before, and act on
the re-resolved context.

**Reacquisition when identity changes.** Re-resolving is not enough on its own: a waiter
that entered holding `old` and re-resolves to `new` is holding the wrong lock, and `rename`
released `old` on the way out. Such a writer releases `old`, acquires `new`, and
re-resolves again under it — repeating at most a small bounded number of times before
failing with a typed error rather than looping. The loop terminates in practice because
each round observes a strictly later registry state.

Claims are checked under the lock, not before it.

**Authorization is the second decision.** §5.3's `classify` says what happened; it is pure,
holds nothing, and may be run at any time — including to explain a situation to a human. It
never confers permission. Authorization is separate and requires all three: both prefix
locks held, the registry and config re-resolved under them, and the snapshot re-observed
after the locks were taken, since the snapshot that produced the verdict may predate them.
A `Recovery` value computed before the locks is a diagnosis, not a warrant.

## 7. Command surface and JSON contract

```
tasks rename <old> <new>
    Renames a project's prefix. Rewrites the project's own task ids, its config, and the
    registry key; records <old> as a permanent alias so existing references elsewhere keep
    resolving. Refuses on a dirty tasks/, a live claim, more than one worktree, or a name
    already taken. Re-run to resume an interrupted rename.

rename -> { prefix, previous, root, tasks: int, aliases: [string], recovery, warnings }
           prefix is the new live prefix, previous the retired one, tasks the number of
           task files rewritten by this run, aliases every alias now targeting this
           project, and recovery the §5.3 verdict this run acted on ("fresh",
           "resume_files", "resume_registry", "resume_cleanup", "complete") so a resume
           is observable rather than inferred from counts.

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

**The recovery classifier is unit-tested by bounded enumeration.** Because `classify`
(§5.3) is pure, its input is enumerable up to a bound: `registry` over its three fields,
`config_prefix` × 3, `inventory` × 2, `dirt` × 4, and an entry list over `source` × 2 and
`dest` × 3 for file counts of zero, one, and two. That is a bounded cover, not the whole
state space — the earlier draft overclaimed. What the bound buys is that no *shape* of
disagreement goes unrepresented, since every predicate in §5.3 is decided per entry and two
entries suffice to make any pair of per-entry verdicts disagree. Tests assert two
properties over that space:

- every enumerated state maps to the `Recovery` the §5.3 table names
- **every state outside that table maps to `Refuse`** — asserted by construction over the
  generated space, not by listing the cases someone thought of

This is the property the closed set exists for, and it is why the classifier is pure: the
end-to-end interruption tests below can only sample the boundaries an integration test
happens to stop at, whereas this covers the space. The two are complementary — the
enumeration proves the classifier total, the interruption tests prove the observer feeds it
the truth.

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
- a re-run of the completed command reports `Complete` and exits 0, rather than tripping the
  `new == old` or taken-name refusals
- a task deleted mid-rename is detected as missing against the inventory and refuses
- `add` blocks on the mutation lock during a rename and cannot mint an old-prefix file
- a worktree checked out from a branch predating the rename refuses with the stale-local
  message, on both a read and a write, rather than routing silently
- `unregister` drops the project's aliases and names them; `unregister <alias>` is refused;
  `init --prefix <alias>` is refused
- **interruption after every mutation boundary**: after the inventory write, mid file pass,
  between the file pass and the config write, between the config write and the registry
  write, and between the registry write and the cleanup — each resumed by re-running the
  command.
  These check both that the *observer* reports the world faithfully and that **executing**
  the resulting recovery reaches the correct final state: every file renamed, config and
  registry consistent, the alias recorded, and the claim store and inventory both gone.
  Asserting only that observation was accurate would leave the execution half untested
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
- **The inventory is machine state, not a repository artefact.** Losing the state directory
  between P3 and P6 — a different machine, a wiped `~/.local/state` — leaves a half-renamed
  repository with no baseline, which classifies as `Refuse` rather than resuming. That is
  the correct outcome, and recovery is then the manual procedure of §5.6.
- **A branch predating the rename** produces a checkout whose config and filenames use the
  retired prefix while the registry names the new one. No pre-existing guard catches this —
  `open_registered` validates the registered destination and `Project::locate` never
  consults the registry — so §4 adds an explicit stale-local-config check to close it.
  Renaming refuses while a second worktree exists, but cannot prevent a later checkout of
  an old branch, which is why the check is a permanent part of the design rather than a
  migration aid.
- **Unregister-then-reuse remains ambiguous.** Unregister `dots`, later `init --prefix
  dot`, and old prose naming `dot-a00088` resolves into the new project if that hex exists
  there. Six hex characters make this vanishingly unlikely, and the hazard already exists
  for any unregistered-then-reused prefix.
- **Prose is never validated.** `check` sees structured references only, so the 124 prose
  mentions are neither warned about nor rewritten. They keep resolving through the alias,
  which is the entire point.
