# Former roots and project resolution

Status: draft 2026-10-06, awaiting review
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
- `former` lists roots this project lived at on this host, oldest first. Each entry has:
  - `root`: absolute and canonical when it was recorded;
  - `storage`: optional, absent when it was unknown at recording time;
  - `until`: the UTC date the root was displaced.

Load validation fails early with `config`, as `[groups]` does:

- every key must be a live prefix;
- every `root` and `storage` must be absolute;
- every `until` must be `YYYY-MM-DD`;
- no former root may equal the project's current root;
- no project may list the same former root twice.

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
| `rename old new --adopt` | when the checkout's root differs from the registered one | `[locations.old]` moves to `new`; old root appended to `former`; `storage` record set for the new root |
| `unregister P` | — | `[locations.P]` removed with the project, as aliases are |

- **Appending a former root.** The entry's `storage` comes from the live realpath of
  `<old root>/.worktrees` when that is a symlink that still resolves. Otherwise it comes
  from the `storage` record. When neither is available it is left out, and the command
  adds a warning that names the root.
- **Refreshing the `storage` record.**
  - If the root's `.worktrees` is a symlink, the record is set to its realpath.
  - If `.worktrees` is a directory inside the root, or does not exist, the record is
    removed. An in-tree `.worktrees` is already under the root.
- **Moving back to a former root.** That entry is removed from `former`, so a root is
  never both current and former.

The `rename` and `init --force` outputs add the warning described above. No JSON shape
changes.

### 2.2 Why record storage before a move

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

Each input is classified by its spelling alone:

- **path**: contains `/`, or starts with `.` or `~`. A relative path resolves against
  the working directory, or against `-C` when it is given.
- **id**: `<prefix>-<6 hex>`, by the `TaskId` grammar. The bare-hex shorthand is not
  accepted, because a resolver's answer must not depend on the caller's cwd. It comes
  back as `invalid`.
- **prefix**: matches the prefix grammar with no hex part, e.g. `ai`.
- **invalid**: anything else.

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
- `worktree`: present only under a storage location, and only when a component follows
  `.worktrees`.
- `relpath`: relative to `worktree` when there is one, otherwise to `location`. It is
  `""` when the input is the location itself.

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
5. **On a miss, for a path that exists:** ask git for the main checkout of the path
   (`git rev-parse --git-common-dir`) and match that against the current roots
   (`via: "git"`). This covers a live worktree outside `.worktrees` storage. It spawns
   a process only on a miss, so matched paths cost nothing extra.
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
- the move-back rule.

**CLI tests** in `tests/cli.rs`, with scratch `XDG_CONFIG_HOME` and `XDG_STATE_HOME`:

1. `rename`, a directory move (storage symlink included), and `init --force`. Then
   `resolve` the old-prefix id, the alias prefix, a path under the old root, and a path
   under the old storage for a worktree that no longer exists. All four resolve to the
   new prefix.
2. A second registry that adopts the same move with different storage paths gives the
   same ids and prefixes, with its own storage.
3. Adoption without a prior storage record keeps the former root without storage and
   warns.
4. Unregistered, invalid, and ambiguous items in one `--stdin` batch.
5. A malformed `[locations]` table fails with `config` on stderr and exit 1.
