# Former roots and project resolution

Status: approved 2026-10-06 (spec review rounds 1–2); the dangling-link rule in §2.1 and §2.2 was amended in plan review round 1
Task: tasks-7580d2 (lane tasks-a7460c; blocks tack-8b7a28; used by obs-ff4e76, flows-44890e)

## 1. Goal

Today a prefix rename leaves aliases behind (`[aliases] ai = "tack"`), but once a
project's directory moves, nothing on the host records where it used to live: the rename
inventory is deleted on completion, and `init --force` and `rename --adopt` throw the
displaced root away. Each consumer that maps an id or a path to a project reads
`[projects]` by hand, ignores `[aliases]`, and knows only current roots: obs's indexer
and join, flows' trial tools, tack's `session-episodes`, and several ops hooks. A rename
therefore cuts their history at the move (tack `docs/specs/2026-10-06-rename-to-hq-design.md`
§3.5, §3.6).

This design adds two things:

1. **A per-host location history in the registry.** When a command changes a project's
   root or prefix, the registry records the former root and that root's worktree storage
   on this host.
2. **One read-only resolver, `tasks resolve`.** It takes any id form (alias prefixes
   included), a bare prefix, or a path (current root, former roots, and the worktree
   storage of each) and returns stable JSON. A hook calls it once per event; an indexer
   calls it once per batch.

Success:

- After `tasks rename tack hq`, a directory move, and `tasks init --prefix hq --force`,
  every id written `tack-…` or `ai-…`, and every path under the old root or its old
  storage, resolves to `hq`. That includes paths that no longer exist.
- A second host that adopts the rename (`rename --adopt`) ends with the same answers,
  using its own storage paths.
- The existing `ai` history is backfilled on this host and documented as a hand edit
  for the other host.

## 2. Registry: the `[locations]` table

`~/.config/tasks/projects.toml` gains one optional table per live prefix. `[projects]`
keeps its shape exactly, because eight hand readers parse it today.

```toml
[locations.tack]
storage = "<WORK_ROOT>/tack/.worktrees"
former = [
  { root = "<Dropbox>/ai", storage = "<WORK_ROOT>/ai/.worktrees", until = "2026-09-27" },
]
```

- `storage` records where the current root's `.worktrees` resolved the last time a
  command wrote this project's registry entry. It is a record, not a live answer: the
  resolver reads the live link first (§4.2). The record exists so that a root which has
  already moved away can still name its storage when it is displaced.
- `former` lists the locations this project has left on this host, ordered by `until`
  and then by `root`. Each entry has:
  - `root`: absolute and canonical when it was recorded;
  - `storage`: optional, absent when it was unknown at recording time;
  - `until`: the UTC date the location was displaced.

  An entry's identity is the pair (`root`, `storage`). A root can appear more than
  once, with different storage each time. It can also equal the current root after a
  move back (§2.1). Either way, no storage path the project ever used is dropped.

Load validation fails early with `config`, as `[groups]` does:

- every key must be a live prefix;
- every `root` and `storage` must be absolute;
- every `until` must be `YYYY-MM-DD`;
- no project may list the same (`root`, `storage`) pair twice.

The table is not written when it is empty, so a registry without history keeps its
shape. It is host-local and unsynced, like the rest of the registry.

**Old binaries.** An older `tasks` reads this registry fine, because serde ignores the
unknown table. But any registry write by that binary (an `init`, a `group set`) drops
`[locations]`. Every host must therefore run a current binary before history is
recorded. The registry is per host, so this concerns only that host's own install.

### 2.1 When history is written

All writes happen under the registry lock the command already takes.

| Command | Root changes? | What is recorded |
| --- | --- | --- |
| `init` (new registration) | — | `storage` record for the new root |
| `init --prefix P --force` at the same root | no | `storage` record refreshed |
| `init --prefix P --force` at a new root | yes | old root appended to `former`; `storage` record set for the new root |
| `rename old new` | no (prefix only) | `[locations.old]` moves to `new`; `storage` record refreshed |
| `rename old new --adopt` | when the checkout's root differs from the registered one | `[locations.old]` merged into `[locations.new]` (§2.2); old root appended to `former`; `storage` record set for the new root |
| `unregister P` | — | `[locations.P]` removed with the project, as aliases are |

- **Appending a former root.** The entry's `storage` comes from the live realpath of
  `<old root>/.worktrees` when that is a symlink that still resolves. Otherwise it comes
  from the `storage` record. When neither is available it is left out, and the command
  adds a warning that names the root. If an entry with the same (`root`, `storage`)
  pair already exists, only its `until` is updated.
- **Refreshing the `storage` record.**
  - If the root's `.worktrees` is a symlink that resolves, the record is set to its
    realpath.
  - If `.worktrees` is a directory inside the root, or does not exist, the record is
    removed. An in-tree `.worktrees` is already under the root.
  - If the symlink dangles, the command warns. Then:
    - **Unchanged root:** the record is kept. It still describes this root, for
      example while its storage is briefly missing.
    - **Root changed in this command:** the record is removed. The record never
      describes a root other than the current one. A displaced root's storage lives
      only in its `former` entry.

    The second host adopts in this state on purpose: it runs `--adopt` before its own
    storage move, so the new checkout's link dangles.
- **Moving back to a former root.** Every `former` entry stays. Take a project that
  moves `A` (storage `S1`) → `B` → `A` (storage `S2`). Paths under `S1` keep resolving
  through the entry (`A`, `S1`). Paths under `A` itself match the current root, which
  wins over the former one at equal depth (§4.3).

The `rename` and `init --force` outputs add the warning described above. No JSON shape
changes.

### 2.2 Merging history on adoption

A plain `rename` requires a free target, so `[locations.old]` simply becomes
`[locations.new]`. Adoption is different: it also supports a target that is already
registered at the checkout's root (`Registry::adopt`, the partial-init branch). That
target can have a table of its own, recorded by the `init` that registered it. Adoption
therefore merges the two tables and never replaces one with the other:

1. **`former`** becomes the union of both lists. Entries with the same (`root`,
   `storage`) pair collapse into one that keeps the later `until`. The result is
   re-sorted by `until`, then by `root`.
2. **The old project's displaced root** is then appended as in §2.1, with the old
   table's `storage` record as its fallback storage. It is appended only when that root
   differs from the checkout's root.
3. **The `storage` record** starts empty and is then refreshed from the checkout's live
   `.worktrees` by the rules in §2.1. Neither table's record is copied into it. With a
   dangling link, it stays empty.

The merge is a pure function of the two tables plus the displaced root, so it is
idempotent. A second `--adopt` run finds `old` already flattened and changes nothing.

### 2.3 Why record storage before a move

The worktree storage of a root that has already moved can't be read. The cutover moves
the checkout and its storage together (tack spec §1), and on the second host Dropbox
deletes the old directory before anyone runs `--adopt`. So storage is recorded whenever
tasks still sees the old root:

- On the renaming host, `tasks rename` runs at the old root before the move, so its
  record is fresh when `init --force` displaces that root.
- On every other host, the cutover runbook gains one step before the move:
  `tasks -C <root> init --prefix <p> --force`. This is idempotent, and it refreshes that
  host's `storage` record. Without the step, `--adopt` still records the former root,
  just without storage, and warns.

Rejected:

- **Inferring old storage from the work-link layout.** tasks would have to encode the
  dotfiles' anchor rule (`<scan root>/../.dropbox-work/<rel>`), and a wrong guess would
  read as a fact.
- **A new recording verb.** `init --force` at the current root already means "register
  this root as P on this host".

## 3. `tasks resolve`

```
tasks resolve <INPUT>...
tasks resolve --stdin        # one input per line
```

The command is read-only: it takes no locks and makes no writes. It needs no current
project, so it works from anywhere. The command needs a row in ops `cli.toml` before it
lands ("The project, canonical id, or live prefix for an id, prefix, or path, through
renames and moves"), followed by `vendored adopt`.

### 3.1 Input classification

Each input is classified by its spelling alone. The first rule that matches wins:

1. **path**: contains `/`, or starts with `.` or `~`. A relative path resolves against
   the working directory, or against `-C` when it is given.
2. **id**: `<prefix>-<6 hex>`, by the `TaskId` grammar.
3. **prefix**: matches the prefix grammar `[a-z][a-z0-9]{1,7}`, e.g. `ai`.
4. **invalid**: anything else.

`resolve` never expands the bare-hex task shorthand, because its answer must not depend
on the caller's cwd. This creates an overlap: a six-hex string that starts with a
letter, such as `abcdef` or `beef00`, is also a valid prefix. Such a string is
classified as a prefix and resolves (or is `unregistered`) as one. A six-hex string
that starts with a digit, such as `4f2a9c`, matches neither grammar and is `invalid`.

### 3.2 Output

The command prints one object. `results` follows the input order:

```json
{"results": [
  {"input": "ai-1a2b3c", "kind": "id", "status": "resolved",
   "id": "hq-1a2b3c", "prefix": "hq", "alias": "ai", "root": "<Dropbox>/hq"},
  {"input": "ai", "kind": "prefix", "status": "resolved",
   "prefix": "hq", "alias": "ai", "root": "<Dropbox>/hq"},
  {"input": "<WORK_ROOT>/tack/.worktrees/x/src/a.rs", "kind": "path", "status": "resolved",
   "prefix": "hq", "root": "<Dropbox>/hq",
   "via": "former_storage", "location": "<WORK_ROOT>/tack/.worktrees",
   "worktree": "<WORK_ROOT>/tack/.worktrees/x", "relpath": "src/a.rs"},
  {"input": "zz-123456", "kind": "id", "status": "unregistered", "prefix": "zz"},
  {"input": "/tmp/elsewhere", "kind": "path", "status": "unregistered"},
  {"input": "/x/y", "kind": "path", "status": "ambiguous",
   "candidates": [{"prefix": "a", "via": "former_root", "location": "/x"},
                  {"prefix": "b", "via": "former_root", "location": "/x"}]},
  {"input": "beef00", "kind": "prefix", "status": "unregistered", "prefix": "beef00"},
  {"input": "4f2a9c", "kind": "invalid", "status": "invalid", "detail": "…"}
], "warnings": []}
```

**Per-item outcomes.** Every item gets a `status`, and none of them fails the command,
so a batch survives one odd input. The statuses are `resolved`, `unregistered`,
`ambiguous`, and `invalid`.

**Errors.** Only registry problems are errors: an unreadable registry, a malformed one,
or one that fails load validation. These use the usual envelope: stderr, exit 1.

**Fields.** Optional fields are omitted when they don't apply:

- `alias`: the retired prefix the input was spelled with.
- `worktree`: the git worktree the input lies in. It is present in two cases: a
  storage match where a component follows `.worktrees`, and a `git` match, where it is
  the worktree top `T` (§4.3, step 5).
- `relpath`: relative to `worktree` when there is one, otherwise to `location`. So
  joining `worktree`, or else `location`, with `relpath` always gives back the
  normalized input. It is `""` when the input is that directory itself.

`root` is always the project's current registered root.

**What resolution checks.** Id resolution is a rule, the same one `canonical_id` already
applies. It does not check that the task file exists. A consumer that needs the record
calls `tasks show` with the canonical id.

**Pretty output.** One line per input:

```
<input>  <status>  <prefix|id>  <via>
```

### 3.3 Id and prefix resolution

1. A live prefix resolves to itself.
2. An alias resolves to its live target in one hop, and the result carries `alias`.
3. An unknown prefix gives `unregistered`.

Retired projects stay registered, so they resolve normally.

## 4. Path resolution

### 4.1 Normalization

1. Make the path absolute.
2. Canonicalize its longest existing ancestor.
3. Append the remaining components lexically. A `..` among the remaining components
   gives `invalid`.

This is how `~/d/ai/x` and `<Dropbox>/ai/x` reach the same answer even though
`ai` no longer exists: `~/d` still resolves. Every match is component-wise, so
`/a/tack` never matches `/a/tackle`.

### 4.2 Locations

Every registered project contributes these locations:

| via | location |
| --- | --- |
| `root` | the registered root |
| `storage` | the realpath of `<root>/.worktrees` when it is a live symlink, otherwise the `storage` record |
| `former_root` | each `former[].root` |
| `former_storage` | each `former[].storage` |
| `checkout` | the linked code checkout, once tasks-9b0a2e lands (§5) |

### 4.3 Matching

1. Collect every location that is the path or one of its ancestors.
2. The deepest location wins, so a nested project beats its parent's root.
3. Among locations of equal depth, current ones (`root`, `storage`, `checkout`) beat
   former ones (`former_root`, `former_storage`). A directory that a new project
   registered after an old one moved away belongs to the new project.
4. When two different projects remain tied at the same depth and currency, the result
   is `ambiguous`, listing the candidates. A tie within one project resolves normally.
5. **On a miss, for a path that exists:** map the path into its main checkout, keeping
   its offset, and match again. This covers a live worktree outside `.worktrees`
   storage.
   1. Ask git for the path's worktree top `T` and its common directory
      (`git rev-parse --show-toplevel --git-common-dir`).
   2. The main checkout `M` is the common directory's parent when the common directory
      is a `.git` directory. With no main checkout (a bare repository, or a separate git
      dir), stop at step 6.
   3. If `T` equals `M`, the path is already in the main checkout, so stop at step 6.
   4. Otherwise build the mapped path `M/<path relative to T>`, then apply steps 1–4 to
      it.

   For example, a path `T/sub/nested/x` maps to `M/sub/nested/x`. If a project is
   registered at `M/sub/nested`, the depth rule picks it over a parent project at `M`.

   A match through this step reports `via: "git"`. Its `location` is the matched
   location under `M`, its `worktree` is `T`, and its `relpath` is the input
   relative to `T` (`sub/nested/x` in the example), following the field rule in §3.2. Git is spawned only on a miss, so matched paths cost nothing
   extra.
6. Anything else is `unregistered`.

A storage match splits off the first component after the location as `worktree`.

### 4.4 Cost

A call loads the registry and does one `readlink` per project. It spawns git only on
the miss case in §4.3, step 5. That fits a hook's budget. The `--stdin` form lets an
indexer resolve all of a run's distinct paths in one process.

## 5. Boundary with `tasks where` (tasks-9b0a2e)

The two commands answer different questions, and they share one location index.

**`where [PATH]`** (ops linked-checkouts design §4, §8) answers which project a
command run at PATH would open. It is live and filesystem-bound:

- it walks up for `tasks/.config.toml`;
- it never crosses a git boundary;
- it falls back to the linked `[checkout]`;
- it reports `holds_records`.

A nested foreign clone is "no project" there.

**`resolve`** answers which project's history an id or a path belongs to. It is
registry-bound and does not depend on the path existing. It knows former roots and
storage, and it does not inspect repository boundaries for paths that are gone.

**What they share** is a new module, `src/locations.rs`. It builds each registered
project's location set from the registry (§4.2) and does the normalization in §4.1.

- `resolve` uses the module for matching.
- `where`'s link step, when it lands, takes the registered checkouts from the same
  module and adds `checkout` as a location kind there.
- Neither command reads the registry file another way.
- `where` does not consult former roots: a command never opens a project at a root it
  has left.

No amendment to the linked-checkouts spec is needed: `resolve` is additive, and `where`
keeps its three outcomes. A note on tasks-9b0a2e records the shared module.

## 6. Backfilling `ai`

On the host that renamed it, the history is recorded by hand, before `tasks rename tack hq`:

```toml
[locations.tack]
former = [
  { root = "<Dropbox>/ai", storage = "<WORK_ROOT>/ai/.worktrees", until = "2026-09-27" },
]
```

Then `tasks init --prefix tack --force` from the tack checkout refreshes the `storage`
record. The source for these values is tack `docs/specs/2026-09-27-rename-to-tack-design.md`
§3: the root was `<Dropbox>/ai`, the storage was `.dropbox-work/ai`, and the rename was
done here on 2026-09-27.

The other host adopted on 2026-09-28. Its Dropbox root and storage anchor are its own,
and nothing on this host records them. The README documents the same hand edit with
that host's paths. Until it is applied there, that host's `ai` paths are `unregistered`,
which is a stated limit rather than a guess. `ai-…` ids resolve on both hosts already,
through the alias.

## 7. Non-goals

- **Rewriting recorded `worktree` paths in claims and parks.** A move still leaves
  them naming the old checkout. A reader that wants the project behind one can pass it
  to `resolve`.
- **Time-aware disambiguation**, such as "which project held `/x` on a given date".
  `until` is recorded and makes this possible later; nothing reads it yet.
- **Syncing history between hosts.** Each host records its own paths.
- **Migrating the consumers.** obs-ff4e76, flows-44890e and tack's `session-episodes`
  switch to `resolve` under their own tasks, keeping their identity contract and
  end-to-end checks. This task notes the contract on each.

## 8. Testing

**Unit tests** in `src/locations.rs`:

- normalization through a symlinked ancestor and through a missing tail;
- component-wise matching;
- the depth rule, current over former, and the `ambiguous` tie;
- storage splitting.

**Registry unit tests:**

- `[locations]` round trip;
- validation failures;
- carry on rename, adopt and unregister;
- the adoption merge (§2.2): union of both `former` lists, collapsing duplicate
  (`root`, `storage`) pairs to the later `until`, plus idempotence on a second run;
- the move back `A(S1) → B → A(S2)` keeps (`A`, `S1`), and a path under `S1` still
  resolves.

**CLI tests** in `tests/cli.rs`, with scratch `XDG_CONFIG_HOME` and `XDG_STATE_HOME`:

1. `rename`, a directory move (storage symlink included), and `init --force`. Then
   `resolve` the old-prefix id, the alias prefix, a path under the old root, and a path
   under the old storage for a worktree that no longer exists. All four resolve to the
   new prefix.
2. A second registry that adopts the same move with different storage paths gives the
   same ids and prefixes, with its own storage. It is run twice: once with the target
   unregistered, and once with the target already registered at the checkout, carrying
   its own `[locations]` history. Neither table's entries are lost.
3. Adoption without a prior storage record keeps the former root without storage and
   warns.
4. Unregistered, invalid, and ambiguous items in one `--stdin` batch, including the
   prefix/hex overlap: `abcdef` is a prefix, both when registered and when
   `unregistered`, and `4f2a9c` is `invalid`.
5. A git worktree created outside `.worktrees` storage, holding a nested project
   registered at `M/sub/nested`, plus a parent project registered at `M`. A path at
   `T/sub/nested/x` resolves to the nested project with `via: "git"`, `worktree: T`,
   `location: M/sub/nested`, and `relpath: "sub/nested/x"`. Without the parent registered, it still resolves to the nested
   project.
6. A malformed `[locations]` table fails with `config` on stderr and exit 1.
