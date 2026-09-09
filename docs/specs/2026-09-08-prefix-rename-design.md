# Renaming a project prefix

Status: implemented (2026-09-08)
Tasks: tasks-8c9398

## 1. Problem

A prefix is a mutable human label embedded in an immutable identifier. Renaming the label
therefore rewrites identity everywhere: the registry key, `prefix` in the project's
`tasks/.config.toml`, every task filename and `id:` field, and every inbound reference in
every *other* registered project.

Done by hand on 2026-09-06 for `aut` → `autonomy` (empty, trivial) and `dot` → `dots`
(2 task files, 4 inbound refs across ops and prism, 3 doc mentions). The manual procedure
worked but was unguided, and left `dot.lock` and `dot.toml` in the claim store.

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
project used to have. Aliases last for as long as the project stays registered and never
become a second live name — new tasks always receive the live prefix.

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
| P6 | Write `claims/<new>.toml` from the inventory's recorded park entries when it holds any, verify it, remove `claims/<old>.toml`, then the inventory. |

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

source      = "dot"
target      = "dots"
root        = "/home/…/dotfiles"
config_from = "<digest>"
config_to   = "<digest>"
entries     = [ { hex = "a00088", from = "<digest>", to = "<digest>" }, … ]
```

`from` is the digest of the source file as preflight read it; `to` is the digest of the
transformation preflight computed for it. Together they let recovery say, of every task the
rename covers, whether it is untouched, correctly finished, conflicting, or **missing** —
the last of which no on-disk comparison could establish once the source is gone.
`config_from` and `config_to` do the same for `tasks/.config.toml`, without which an
unrelated edit to that file would be indistinguishable from the rename's own.

**The inventory is the authority, not git.** Every recovery judgement is a digest
comparison against it: which sources still stand, which destinations are finished, whether
the config has moved, and whether any task file exists that the rename never knew about.
Git status is not consulted at all during classification. That is what makes recovery
behave identically inside and outside a repository, and it is why the previous draft's
`Dirt` value has left the classifier — a project outside git could not previously recover
its file pass at all.

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
- A fresh rename refuses a target store that holds park entries; recovery does not repeat this check.
- The repository has **more than one git worktree**. Other checkouts keep the retired
  prefix in their config and filenames, which would recreate the routing hazard of §4
  after an otherwise successful rename.

**The claim and worktree checks are not `Fresh`-only.** They are authorization, and they
gate *every* mutating path — `ResumeFiles`, `ResumeRegistry`, and `ResumeCleanup` alike.
Between an interruption and its recovery the locks are gone (§5.7), so a claim can be taken
or a worktree added in the interval; P6 deleting a claim store that went live after the
crash is exactly the failure this prevents. Only `--explain` (§5.6) skips them, because it
writes nothing.
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

Entry    = { hex, source: Absent | Present(digest),
                  dest:   Absent | Present(digest) }

Snapshot = { registry:  Registry,
             config:    Absent | Present { prefix: name, digest },
             inventory: Absent | Present(Inventory),
             entries:   [Entry],        // empty when the inventory is absent
             named:     { source: int, target: int },  // task files by prefix, scanned
             strays:    [path] }        // task files in no inventory entry

Recovery = Fresh | ResumeFiles | ResumeRegistry | ResumeCleanup
         | Complete | Refuse(reason)
```

`Dirt` is gone. Git status is not an input: every judgement below is a digest comparison
against the inventory, so recovery reads the same inside and outside a repository.

**Two regimes, and the vocabulary says which is which.** With an inventory present, the
baseline exists and every judgement is a digest comparison against it. With the inventory
absent there is no baseline to compare to — `config_from` and `config_to` do not exist —
so the only honest inputs are the config's **parsed prefix** and a **scan of task
filenames**. `config` therefore carries both a name and a digest, and `named` counts task
files by prefix. Baseline comparisons are reserved for recovery; the inventory-free rows
use the name and the counts. An earlier draft defined `config_old` and `config_new` purely
as digest comparisons and then used them on the two rows where the inventory is absent,
which could not be evaluated at all.

### Refusals, evaluated before the table

A too-broad row swallows a bad state that a trailing "anything else" can then never see, so
every refusal is decided **first**. Any of these ends classification with `Refuse`, naming
what it saw:

**R1–R6 apply only when the inventory is present**; they are baseline comparisons and have
nothing to compare against otherwise. **R7–R8 always apply**: they read the registry alone.

| | Scope | |
|---|---|---|
| R1 | inventory | the inventory's `source`, `target`, or `root` disagrees with this invocation |
| R2 | inventory | a source file is present whose digest is not its entry's `from` — edited mid-rename |
| R3 | inventory | a destination is present whose digest is not its entry's `to` — a conflicting file |
| R4 | inventory | an entry has `source: Absent` **and** `dest: Absent` — a **missing task**, visible only against the baseline |
| R5 | inventory | `strays` is non-empty — a task file the rename never knew about, created after P2 |
| R6 | inventory | the config digest is neither `config_from` nor `config_to` — an unrelated config edit |
| R7 | always | `old_key` and `new_key` are both present and name different roots |
| R8 | always | `new_key` is present and does not name this project's root |

R2, R5, and R6 are what an earlier draft's `rename_only` predicate was reaching for and
could not express: it compared against git's idea of dirt, which accepted *any* modified
`.config.toml` and could not see a stray file at all outside a repository.

### The table

With refusals discharged, each row names the file, config, and registry states **exactly**.
Nothing is left to the trailing row that an earlier row could already match.

Files, from the entries: `untouched` (every source `Present(from)`, every dest `Absent`),
`done` (every source `Absent`, every dest `Present(to)`), `partial` (anything else that
survived R2–R4). An empty project is `untouched` and `done` simultaneously; the rows below
resolve it by config and registry, never by file state alone.

Config, in the two regimes: with an inventory, `config_old` means the digest is
`config_from` and `config_new` means it is `config_to`; without one, `config_named(p)`
means the parsed prefix is `p`.

Registry: `registry_old` is `old_key = Root(root)`, `new_key: Absent`, `alias: Absent`;
`registry_new` is `new_key = Root(root)`, `old_key: Absent`, `alias: Present(target)`.

| Inventory | Files | Config | Registry | Recovery |
|---|---|---|---|---|
| absent | `named.target == 0` | `config_named(old)` | `registry_old` | `Fresh` |
| present | any | `config_old` | `registry_old` | `ResumeFiles` — P3 onward |
| present | `done` | `config_new` | `registry_old` | `ResumeRegistry` — P5 onward |
| present | `done` | `config_new` | `registry_new` | `ResumeCleanup` — P6 |
| absent | `named.source == 0` | `config_named(new)` | `registry_new` | `Complete` |
| — | — | — | — | `Refuse` |

The second row is deliberately broad in its *file* dimension and exact in the other two.
That is what closes the three holes the previous table left: immediately after P2 nothing
has moved yet (`untouched`), after a partial file pass some have, and after a complete one
all have while the config has not yet been written — and an empty project sits in the same
row rather than falling through. All four are the same situation, "the file pass is not
finished being followed by the config write", and all four resume identically because P3 is
idempotent per file.

`ResumeCleanup` exists because an interruption between P5 and P6 leaves a fully renamed
project with a stale claim store and a stale inventory. Reporting that as `Complete` and
exiting would strand both; an earlier draft did exactly that.

`registry` distinguishes `old_key` from `new_key` separately, rather than as one three-way
value, because both can be present at once — and can name *different* roots, which R7
refuses rather than resuming.

**The set is closed.** The final row is not a formality: any observation that does not match
an enumerated state is a refusal that says what it saw, never a best-effort guess. This is
the difference between "we thought of these cases" and "unlisted cases fail loudly", and it
is the property the enumeration in §8 checks.

**Classification is separate from authorization.** `classify` decides *what happened*; it
is pure, needs no locks, and can be run to explain a situation. Acting on its verdict is a
second decision, taken only while holding both prefix locks and after post-lock
revalidation (§6). A `Recovery` value is never itself permission to write.

**Git's role is now only a courtesy.** A dirty `tasks/` still refuses a `Fresh` rename
(§5.2), because starting a large mechanical rewrite on top of unreviewed edits is a bad
idea and `git status` is the cheapest way to say so. But it is a usability guard, not a
correctness one: no recovery verdict depends on it, and outside a repository its absence
costs nothing.

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

**Diagnosis is a separate, read-only mode.** `tasks rename <old> <new> --explain` observes,
classifies, and prints the verdict with its reasoning — including which refusal fired and
on which path — while writing nothing, taking no locks, and running no authorization
checks. Re-running `rename` is itself a mutating recovery command and cannot double as
"look before you touch"; the previous draft claimed it could.

The supported recovery is **forward**: re-run `tasks rename <old> <new>`, which classifies
and resumes. That covers every interruption for which the inventory survives.

A genuine rollback is manual, and is documented rather than automated because it spans both
stores. Every step is scoped by the inventory — nothing is removed that the inventory does
not name — and no step deletes a file before its replacement is back and verified.

Restoring a source means `git checkout tasks/<old>-<hex>.md`. **Outside a repository there
is nothing to restore from**, so once P3 has removed a source, rollback is unavailable and
forward recovery is the only route; §9 records this.

1. **Restore before deleting, per entry.** For each entry whose source is absent, restore
   `tasks/<old>-<hex>.md` and verify its digest equals `from`; only once that succeeds,
   delete `tasks/<new>-<hex>.md` if its digest equals `to`. If the source cannot be
   restored and verified, **keep the destination and stop** — a digest identifies bytes but
   cannot reconstruct them, so deleting the only surviving copy first would destroy the
   task outright, which is what the previous ordering did whenever `git checkout` failed.
   A blanket `git clean -f tasks/` is wrong for a second reason: it would delete unrelated
   task files added after the interruption, which are untracked exactly as the rename's own
   destinations are.
2. If P4 landed, restore `prefix` in `tasks/.config.toml` — `git checkout` covers it, since
   the config is tracked.
3. If P5 landed, repair the registry by hand, and note that restoring the `<old>` key is not
   sufficient on its own. Remove the `<new>` key, restore `<old>` pointing at the root,
   remove the `old -> new` alias, and **re-point every alias P5 moved**: any alias that
   targeted `<old>` before the rename now targets `<new>` and must be pointed back. After a
   second rename that is more than one entry, which is why the inventory records `source`
   explicitly.
4. If the store step landed, restore `claims/<old>.toml` from the inventory's `parks_store`
   with ids re-prefixed back, verify it, then remove `claims/<new>.toml`; the entries carry
   no other state.
5. Remove `~/.local/state/tasks/rename/<old>.toml` **last**. Until it is gone the project
   stays frozen (§5.7), so removing it first would unfreeze a half-rolled-back project.

### 5.7 A pending rename freezes the project

Locks die with the process that held them. After an interruption the inventory remains but
nothing is locked, and the world an ordinary command sees is a completely valid project
under its old name: config `old`, files `old-<hex>.md`, registry key `old`. Nothing stops
it adding a task, taking a claim, or unregistering the project — and then recovery would
delete a live claim store, or R5 would refuse forever on a task file created in good faith.

So a pending inventory freezes the project it names. Every **mutating** command — including
`add`, which §6 already brings under the lock — refuses with a typed error naming the
pending rename and the two ways out: finish it by re-running `rename`, or roll it back per
§5.6.

**Discovery reads the inventories, not the registry.** The pending directory
`~/.local/state/tasks/rename/` is scanned and each inventory matched on its recorded
`source`, `target`, and `root`; a command is frozen if its project's root matches, or if
its prefix equals either recorded name. Looking up `rename/<prefix>.toml` by the caller's
current prefix does not work, and the window where it fails is an ordinary one: between P4
and P5 the config already says `new` while the registry still holds `old` and no alias
exists, so a caller resolving from the registry looks for `rename/<new>.toml` and never
finds `rename/<old>.toml`. An `add` would sail straight through the freeze into a project
mid-rename. The recorded names are stable across every phase; the resolvable ones are not.

**The target name is reserved while a rename is pending.** Until P5 the target is not in
the registry at all, so `init --prefix <target>` in an unrelated directory would otherwise
succeed and take the name out from under the recovery. `init` and `rename` therefore check
the pending inventories for the name they are about to claim, alongside the live prefixes
and aliases of §2.

Discovery runs **under the relevant locks** — the prefix lock a mutating command already
holds, and the registry lock for registry mutations — so a scan cannot race a rename that
is in the middle of writing or removing an inventory.

Reads are unaffected. They report the project as it currently stands, which is the truth.

Registry mutations are covered too: `init --force` re-pointing either name, and
`unregister` of either name, refuse while a rename is pending for that project. Both would
otherwise invalidate a baseline that recovery still depends on.

The freeze is what makes the interval after process death safe, and it is why R5 can treat
a stray task file as a refusal rather than something to reconcile: under the freeze, a
stray can only arrive from a binary predating this design or from a hand-edit, and both
deserve to be told rather than absorbed.

## 6. Concurrency

**Both prefix locks, in sorted order.** Holding only `old` would let a writer acquire
`new` the moment P5 lands. `rename` holds the `old` and `new` mutation locks for the whole
operation, acquired in a deterministic order so two concurrent renames cannot deadlock.

**A registry lock.** `Registry::load` → mutate → `save` is an unserialized
read-modify-write; atomic replacement prevents a torn file but not a lost update. A lock
file beside `projects.toml` is taken by `init`, `unregister`, and `rename` for the whole
read-modify-write. This closes a race that predates this design.

**Every writer participates.** `add` acquires the mutation lock for the project it is
about to write, on both the local and `--project` arms. Exclusive file creation alone
would prevent an id collision but allow a fresh `old-<hex>.md` after P3 had emptied the
source files. Feedback creation/recurrence and editor reacquisition use the same shared
lock and routing revalidation, including the pending-rename freeze.

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
tasks rename <old> <new> [--explain]
    Renames a project's prefix. Rewrites the project's own task ids, its config, and the
    registry key; records <old> as an alias for as long as the project stays registered,
    so existing references elsewhere keep resolving. Refuses on a dirty tasks/, a live
    claim, more than one worktree, or a name already taken. Re-run to resume an interrupted rename; --explain classifies and reports
    the verdict without writing, taking locks, or checking authorization.

rename -> { prefix, previous, root, tasks: int, aliases: [string], recovery, warnings }
           prefix is the new live prefix, previous the retired one, tasks the number of
           task files rewritten by this run, aliases every alias now targeting this
           project, and recovery the §5.3 verdict this run acted on ("fresh",
           "resume_files", "resume_registry", "resume_cleanup", "complete") so a resume
           is observable rather than inferred from counts. Explain reports "refuse" with
           the classifier reason in warnings when recovery is blocked; it always reports
           zero task writes. Mutating refusals remain typed errors.

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
`config` over its prefix name × 3 and its digest × 3 (matching `config_from`, matching
`config_to`, matching neither), `inventory` × 2, `named` over zero and non-zero in each
component, `strays` × 2, and an entry list over `source` × 3 and `dest` × 3 for file counts
of zero, one, and two. The enumeration deliberately includes inventory-absent snapshots
carrying entries and inventory-present ones carrying none, so that a predicate reaching for
a baseline that is not there is caught rather than assumed away. That is a bounded cover, not the whole state space — an earlier draft
overclaimed. What the bound buys is that no *shape* of
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
- **every phase boundary resumes, including the ones an earlier draft refused**: immediately
  after the inventory write with nothing moved, after a partial file pass, after a complete
  file pass with the config not yet written, and on a project with **zero tasks** — each
  reaching `ResumeFiles` rather than `Refuse`
- the same set outside a git repository, reaching the identical verdicts
- each refusal R1–R8 fires on its own: a source edited mid-rename, a conflicting
  destination, a task deleted mid-rename, a stray task file, an unrelated `.config.toml`
  edit, both registry keys naming different roots, and `new_key` naming a foreign root
- the freeze: `add`, `start`, `unregister`, and `init --force` all refuse while an inventory
  is pending, and reads still succeed
- an authorization check on a *resume* path: a claim taken after the interruption blocks
  `ResumeCleanup` rather than having its store deleted
- `--explain` writes nothing, takes no lock, and reports the same verdict the mutating run
  would act on
- `Fresh` and `Complete` classify with **no inventory on disk**, from the config's parsed
  prefix and the filename scan alone
- the freeze holds in the P4–P5 window, where the config says `new` and the registry still
  says `old`: an `add` there is refused, having found the inventory by its recorded names
  rather than by resolving the caller's prefix
- `init --prefix <target>` in an unrelated directory is refused while a rename to that name
  is pending
- rollback with an unrestorable source keeps the destination and stops, rather than leaving
  neither copy
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
- **Outside a git repository, a rename past P3 cannot be rolled back**, only completed.
  Rollback restores sources from git, and nothing else holds the original bytes — the
  inventory records digests, which identify bytes without reproducing them. Forward
  recovery still works, since it needs only the destinations and the baseline.
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
