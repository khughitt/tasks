# Where task records and working documents live — design

Task: tasks-dc16eb. Status: draft for user review (round 1).

Supersedes, where they differ: the 2026-10-02 ruling on hq-d7b5b2 (task records and
specs stay in the repository); hq's `2026-10-10-agent-docs-collection-design.md` §1
item 4, §6 and §7 (the collection in mindful and the `design_docs: mindful` profile
value; its §3–§5, the external document reference, stand); ops's
`2026-10-01-linked-checkouts-design.md` decision 2 (a profile without an overlay cannot
link) and §7 (solo repositories keep records in-tree).

## 1. Decision and scope

Every personal project's task records and working documents leave its code repository.
Three homes replace the one tree:

1. **The records root.** One private, per-user git repository, `<records>/`, holds
   every project's records at `<records>/<prefix>/tasks/` and its working documents at
   `<records>/<prefix>/docs/`. The user names the directory and the private remote.
2. **The code repository** keeps only durable reference material: `README.md`,
   `AGENTS.md`, guides, and reference documents that describe current behaviour and are
   updated by the change that alters it.
3. **mindful** is not a store for either. It may later index the records root as
   agent-owned raw thoughts, with the dependency pointing from mindful to the records
   root and never the other way (§9, deferred).

The project's registered root stays the code checkout. A project whose records live
outside its root is a **detached-records project** (`kind: detached`), a third shape
beside in-tree (`kind: in-tree`) and the existing linked checkout (`kind: linked`,
whose root is an overlay and whose code checkout is someone else's repository). The
three share one model (§3.1) and one code path in the CLI.

Scope: the personal-profile projects (the 21 registered on 2026-10-22, the verifiably
family excepted, which keeps its conventions unless it opts in). Work projects stay as
they are: the two linked ones keep their overlay, the two in-tree ones may detach under
the same procedure when their owner chooses. The external profile is unchanged.

Not decided here, and listed in §11: rewriting the history of public repositories that
already carry records; the artifact policy for large `tasks/files/` (ops-bf9329); the
per-project consolidation of superseded design specs into reference documents.

### 1.1 Why

- **Clutter and leaks.** 5,051 records sit in 21 repositories; 2,712 of them are in
  the nine public ones (tasks, hq, relay, fam, prism, material, wali, tui, dots). The
  layout check exempts `tasks/*.md`, so records carry the hostname (72 in ops, 38 in
  dots, 24 in hq, 18 in tasks), home paths and sync paths today. The move takes the
  records out of every tree at once instead of scrubbing each.
- **Records that branch with code fork.** Five reports filed in the week of 2026-10-20
  (tasks-1b6606, tasks-81410d, tasks-41b34d, tasks-06afbe, tasks-de3e19) are one
  defect: the scope queue on `main` sees a record that a worktree branch has already
  advanced. One copy per record on one branch removes the class; `stale_copy` keeps
  its meaning for the records repository's own worktrees, of which there are none.
- **Publication.** ops-741c5e publishes a kit whose tree holds no records or working
  documents; ops-487484 implements this design.

### 1.2 Alternatives rejected

- **Gitignore `tasks/` in place.** Records lose history, worktrees receive no copy,
  `git clean -fdx` deletes them, and backup rests on the sync alone.
- **Status quo with pruning.** Leaves 2,712 records public and the fork class open.
- **mindful as the records store.** Records are a high-churn, machine-owned dataset
  (every note, park, claim and done is a write); mindful has one corpus, no
  namespaces, a writer lock the web app holds, and ten readers (§4) parse record files
  or `tasks` JSON today. The graph benefit is a read concern and is served by indexing
  (§9).
- **mindful as the working-documents store** (hq-5a3e12, accepted 2026-10-10). Of its
  seven pieces only the tasks reference (tasks-91f63b) has landed. Its hardest part,
  the execution snapshot (§6.4, four review rounds), exists only because a plan held
  in mindful is not a file; beside the records the plan is a file again, every
  existing `docs/plans/...` link keeps resolving unchanged, and no skill depends on a
  running mindful. The user's 2026-10-19 note widened the move to specs by default,
  which that design excluded. The external reference form stays (§7.3) for documents
  that do live elsewhere.
- **Linked checkouts as implemented, root = overlay.** Ten consumers read `root` as
  the code checkout (`vendored`, `ops-docs`, `harness-links`, crew, tui, `ops-check`'s
  layout values, the sessionstart and profile hooks, `tt-report`,
  `assemble-instructions`) and would point into the records directory. For a work
  repository the overlay is the project; for a personal one the code is. Keeping
  `root` as the code checkout and naming the records location separately leaves every
  consumer that wants the code correct and changes only those that open record files
  by path (§4).

## 2. Evidence from the tree (2026-10-22)

| fact | value |
|---|---|
| records in personal projects | 5,051 in 21 repositories, 296 attachment files |
| records in public repositories | 2,712 in 9 repositories |
| open records linking a spec or plan | 80 (66 spec, 43 plan) |
| done records linking a spec or plan | 2,847, over 655 distinct paths, none missing |
| record bodies naming a docs path in prose | 1,224 |
| design specs / plans / briefs / handoffs | 316 / 364 / 190 / 6 |
| largest `tasks/files/` | material 17 MB, prism 12 MB |
| projects opting in to `[git] commit` | all but transponder |
| flow-trial-1 projects | `tack` (hq) and `obs`; readout 2027-02-16 |
| linked projects today | two, both work, in one overlay repository |
| external (`mindful:`) spec or plan links stored | none |

The linked mechanism already runs many prefixes in one repository: the registry maps
prefix to root with roots at any offset, `worktree_records` joins the offset under
each worktree, and record commits strip to the repository top and make one commit per
working tree (`src/repo.rs`, `src/record_commit.rs`).

## 3. The model

### 3.1 Three locations per project

Every project has three paths. `tasks projects` rows and `tasks where` carry all three
and a `kind`.

| field | meaning | in-tree | detached | linked |
|---|---|---|---|---|
| `root` | the project's home: `AGENTS.md`, `identity.toml`, `tools/`, `.worktrees/`, hooks | the checkout | the checkout | `<overlay>/<prefix>` |
| `checkout` | the code checkout the sessions run in | = root | = root | the foreign repository |
| `records` | where `tasks/` and `docs/` live | = root | `<records>/<prefix>` | = root |

`root` is what the registry's `[projects]` table holds today and what `tasks root`
prints; neither changes meaning. `checkout` already exists on linked rows (linked
design §8.2) and is now present on every row. `records` is new on every row.

Rules that follow:

- Record reads and writes, `tasks/.config.toml`, `tasks/files/`, and the spec and plan
  directories open under `records`.
- Claims, parks, `resume_root` and the owner fallback use the code worktree, as for
  linked projects today.
- `sibling_task_copies` enumerates the worktrees of the repository holding `records`.
  The records repository keeps no worktrees (policy, §3.4), so `stale_copy` never
  fires for a detached project; if someone adds one, the check returns across every
  prefix in that repository, which is the correct behaviour.
- Record commits under `[git] commit` land in the repository holding `records`; one
  commit per working tree per command, so a command that writes two prefixes in the
  records root makes one commit, as the record-commit design §3.7 already says.
- Git checks that today run against the root at the project's offset (prime's branch
  line, `start`, `rename`) run against `records` at its offset.
- Path references (`--ref`, `--spec`, `--plan`, `--source` paths) resolve against
  `records` first, then `root`, so a migrated spec and a reference document kept in
  the code tree both link. Skills write new documents under `records` (§7).

### 3.2 Declaration and discovery

The synced declaration lives in the records config, as it does for linked projects:
`<records>/<prefix>/tasks/.config.toml` carries `[checkout] path = "../../<dir>"`,
relative to the project's records directory, and `[checkout] kind = "detached"`. The
kind is in the file so that the config is self-describing: a byte-identical config
without the key is a linked overlay, and `Project::open` on either reports its kind
without consulting the registry. Both hosts keep the checkouts and the records root
under one synced tree with the same relative layout, so the relative path holds on
both.

The host-local registry pairs the two: `[projects] <prefix> = <checkout>` as today,
plus a new table `[records] <prefix> = <records dir>`. A prefix absent from
`[records]` is in-tree or linked, exactly as now. A registered prefix whose root holds
no `tasks/.config.toml` and has no `[records]` row (the second host between the sync
and its own `init --records`) is a typed error, `records_unpaired`, naming the
command; it is never opened as in-tree, linked or null.

`Project::locate` from a cwd:

1. Walk up for `tasks/.config.toml`. A hit inside `<records>/<prefix>/` opens that
   project with `records` = that directory and `root` from the registry; a hit in a
   code tree is in-tree, except as §3.3 says.
2. On a miss inside git, take the main checkout through `--git-common-dir` (as today).
   If the registry's `[projects]` maps a prefix to that checkout and `[records]` names
   its records, open the detached project. No config scan is needed.
3. Otherwise the linked scan runs as today.

`locate` reads the registry on every walk hit (a read, never the linked scan), since
§3.3's refusal cannot be decided from the config alone; a missing registry is empty and
a malformed one is an error, as now.

`tasks where` reports `kind`, `records` and `checkout` beside `root`, keeps
`holds_records` (true when the tree itself holds records: in-tree, an old branch under
§3.3, and the records repository itself, so the hook never guards it as foreign), and
keeps its three outcomes. Running `tasks` inside the records repository above any
project directory is `project: null`, as it is above an overlay.

`tasks resolve` matches a path under `<records>/<prefix>/` to the project with a new
`via` value, `records`, ranked after `root` and `checkout` (the records root lies
inside no registered root, so the two never compete), so that `where` and `resolve`
agree for every path, as the linked design's §4.1a requires.

### 3.3 A tree that still holds the moved records

A worktree on a branch that predates the move still has `tasks/.config.toml` in its
tree. `locate` finds it first (step 1) and must not open it: the walk's hit is checked
against the registry, and a config whose prefix is in `[records]` with a different
directory is refused with a typed error, `records_moved`, naming the records
directory and the remedy (rebase or merge the branch onto the main branch; the
in-tree copy is read-only history). `check` run in the code checkout reports the same
as an error while HEAD still tracks `tasks/.config.toml`.

### 3.4 The records repository

- Layout: `<records>/<prefix>/tasks/` and `<records>/<prefix>/docs/{specs,plans,notes}`
  per project, a `README.md` at the top describing the layout, and nothing else of a
  project's: `identity.toml`, `tools/` and `AGENTS.md` stay in the code root. This
  differs from the work overlay, which holds identity and a private guide because the
  code repository cannot. The directory name equals the prefix: this is a contract,
  not a convention. `init --records` takes the records root and derives the directory
  (§5.1), `rename` moves it (§5.4), and the hooks and `ops-check` rely on it.
- Private: a private GitHub repository under the user's account, named by the user,
  created only on the user's go-ahead. The working tree sits beside the project
  checkouts in the synced tree, so the two hosts share it the way they share every
  repository today; `.worktrees/` is linked to host storage like the others, and the
  policy is that the repository has no worktrees and one branch.
- Hooks: `.githooks/pre-commit` runs `tasks check` for every prefix whose files are
  staged, and `ops-check`'s machine-layout rule stays off for it (the repository is
  private by construction; the `^tasks/[^/]+\.md$` exemption gains the
  `<prefix>/tasks/` form for completeness). No CI.
- `[git] commit = true` in every `<records>/<prefix>/tasks/.config.toml`; the record
  commit lock is keyed by the repository's common dir, so writes from different
  projects serialize, which is acceptable at today's write rates (the lock is held for
  one `git add` and one `git commit`).
- Size: all records and attachments today total under 80 MB, dominated by material and
  prism attachments; ops-bf9329 decides whether generated artifacts belong there.

## 4. Consumers

Readers that open record files or treat `root` as the records directory change; readers
that take `root` as the code checkout do not. Each row is an implementation piece or a
no-op; §10 files them.

| reader | today | change |
|---|---|---|
| ops `claude-pretooluse` check 5 (`foreign_leak.py`) | exempts a tree with `HEAD:tasks/.config.toml`, guards every other checkout and refuses task ids, `tasks/` paths, vendored tools and `identity.toml` | keyed on `kind`: `linked` keeps every rule; `detached` refuses only writes under `tasks/` in the code tree (`records_in_tree`); task ids, vendored tools and identity are the repository's own |
| ops `claude-pretooluse` check 4 (design docs) | `docs/superpowers/` only, keyed on `design_docs` | under `design_docs: records`, refuses a new file under `docs/specs/`, `docs/plans/`, `docs/superpowers/`, `docs/designs/`, `docs/notes/*-brief.md`, `docs/notes/*-handoff.md` in a detached checkout; tracked files revise in place |
| ops `ops-projects link` | refuses a profile without an overlay | `link` gains the detached form, or `tasks init --records` replaces it (§5.1); the overlay stays for work |
| ops `ops-check` layout rule | exempts `^tasks/[^/]+\.md$` | also exempts `^[a-z0-9-]+/tasks/[^/]+\.md$` in the records repository; `tasks/files/` stays covered |
| ops `obs-index` `task_journeys` | globs `root/tasks/<prefix>-*.md` | globs under `records` from `tasks projects` |
| ops `tt-latency` | reads `root/tasks/<id>.md`, understands the overlay offset | reads under `records` |
| ops `ops-docs` | git top level as project root; family publish writes `tools/family.toml` into root | unchanged for detached (root is the code); ops-ba81cf stays a linked-only gap |
| obs `stores._record_bytes` | `<checkout>/tasks/<id>.md` | `<records>/tasks/<id>.md`; copies collapse to one |
| obs `authorship.EXCLUDED` | excludes `tasks/`, `docs/specs/`, `docs/plans/` always | excludes nothing when `kind` is `detached` or `linked`, matching `flow-state` |
| obs `context_events` | `git log` in the resolved root | unchanged (root is the code) |
| hq `session-episodes` | `git worktree list` of root, reads `<wt>/tasks/<id>.md`, requires `root/tasks` | reads the one copy under `records`; trial measurement, so it lands kind-aware before any trial project detaches (§8) |
| hq `rename-cutover` | in-tree layout | takes `records` for the record and `tasks/files/` paths |
| crew `run.ts` | `git -C root hash-object tasks/<id>.md` | hashes under `records` |
| flows `flow-state` `excluded()` | in-tree: excludes the three paths; linked: nothing | `detached`: nothing, through `tasks where`'s `kind`; the `tasks/.config.toml` existence test goes |
| flows `trial-arm local_prefix` | parses the in-tree config | falls back to `--project`; no change |
| flows skill | "the record lands with the code" | §6 |
| lookout, tui, `claim-guard`, `project-colors`, `retros`, `ops-profile`, `vendored`, `harness-links`, sessionstart, lore hooks | `root` as the code checkout, or CLI only | nothing |

`flow-state`'s fingerprint for a detached project covers the whole code tree: there is
nothing of the record's in it, which is the rule the boundary review asked for
(revision 4, change 2). `reference: true` is not needed: no retained content is exempt.

## 5. The tasks CLI

### 5.1 `tasks init --records <path>`

Run in the code checkout (or with `-C`):

    tasks init --prefix <p> --records <records-root>

- Takes the records root and derives the project's records directory
  `<records-root>/<p>`. Registers `<p>` with `root` = this checkout and `[records] <p>`
  = that directory, creates `<p>/tasks/.config.toml` there with `prefix`, `[checkout]
  path` relative to it, `[checkout] kind = "detached"` and `[git] commit = true`, and
  creates `<p>/docs/specs` and `<p>/docs/plans`.
- For a migration, the directory already holds the moved records and config (§8); the
  command then keeps the config, adds the `[checkout]` table and verifies the prefix
  matches; a prefix mismatch is a typed error.
- Refuses with `records_in_tree` while the checkout's HEAD tracks `tasks/.config.toml`
  or the directory exists on disk, naming the removal commit as the remedy; with
  `checkout_conflict` when another prefix declares this checkout; and with the existing
  validations of `init --checkout` otherwise (a main checkout, not a worktree, not
  inside another registered root).
- The repoint of an already-registered prefix needs `--force` as today and preserves
  aliases, groups and location history (`unregister` is never part of the procedure).
- A second host runs the same command after the sync delivers both directories; the
  records config already exists, so the command only writes the registry.

`init --records` and `init --checkout` are mutually exclusive. The CLI change starts on
ops's `cli.toml` as the rules require.

### 5.2 Output contract

- `tasks projects` rows gain `records` (always) and `kind` (`in-tree` | `detached` |
  `linked`); `checkout` becomes always present. JSON keys are added, none change
  meaning.
- `tasks where` gains the same three; `linked` stays as a boolean for existing callers
  and equals `kind == "linked"`.
- `tasks resolve` rows gain `records`, and `via` gains the value `records` (§3.2).
- Errors: `records_moved` (§3.3), `records_unpaired` (§3.2), `records_in_tree` (§5.1).

### 5.3 Reads and writes inside the CLI

One place decides each path. `Project` carries `root`, `checkout`, `records` and
`kind`; every `tasks/` and docs path derives from `records`, every resume path from
`checkout`, every git check from the repository holding the path it checks. The test
`no command writes into a linked checkout` gains the detached case: no command writes
under a detached project's root.

### 5.4 `tasks rename` for a detached project

`rename <old> <new>` keeps working: it rewrites the records under `records` as today,
moves `<records>/<old>` to `<records>/<new>` in the records repository (`git mv`, one
stage of the existing inventory, with the directory move recorded so recovery can
finish or undo it), rewrites `[checkout] path` for the new depth if it changed (it does
not: the directory stays one level below the root), and moves the `[records]` key with
the prefix. The existing refusals hold (clean `tasks/`, no live claims, at most one
worktree of the repository holding the records). hq's `rename-cutover` reads the new
location through `tasks projects`.

### 5.5 `tasks docs`

Moved documents stay findable from the CLI, as the 2026-10-19 note asks:

    tasks docs [--project <p> | --all-projects] [--kind spec|plan|brief|handoff|note] [--search <phrase>]

lists documents under each project's `records` docs directories with the tasks that
link them (`spec`, `plan`, `--ref` entries), kind from the directory and filename
(`docs/specs`, `docs/plans`, `*-brief.md`, `*-handoff.md`, else `note`), and matches a
phrase against title and body the way `search` does. `tasks doc <id> ...` already reads
one. Creating and editing stay the editor's job; a document is a file in a repository
the user owns.

## 6. flows: where the record lives

The flow skill's rule becomes: the record has one copy, under the project's `records`,
whatever branch the code is on. The closing sequence for a detached project is the
linked sequence without the id rule:

- `flow-state` and every fingerprint run in the code worktree; the fingerprint covers
  the whole tree.
- Step 5 and 6: `tasks done` commits the record in the records repository itself
  (`[git] commit`), so "stage the `tasks/` change" and "one commit" become "the code
  commit, then `tasks done`", in that order, so that the done note's provenance follows
  the landed code. Task ids may appear in a detached project's commits and branches;
  they are the user's own repository.
- The in-tree paragraph stays for in-tree projects (the verifiably family, and the two
  trial projects until §8).

flows-2ea069 (planned gate accepts a mindful reference) is re-scoped to this wording
change and the `excluded()` change in §4.

## 7. Working documents and reference documents

### 7.1 New documents

Under the personal profile, `design_docs` takes a third value, `records`: design specs,
plans, briefs and handoffs are written under `<records>/<prefix>/docs/` at the same
relative paths the skills use today (`docs/specs/`, `docs/plans/`, `docs/notes/`), and
committed there by the record commit or by hand. The session line reads "design docs
under the records root"; `ops-profile audit` lists in-tree documents of those kinds.
`work` and `external` keep `exclude`.

The skills change only their wording: brainstorming's and writing-plans' "where it goes"
becomes the records docs directory, `--spec docs/specs/<name>.md` and `--plan` stay as
they are (they resolve under `records`), and the scope skill's brief lands at
`<records>/<prefix>/docs/notes/`.

### 7.2 Reference documents

A code repository's `docs/` holds reference documents: a description of current
behaviour, named for its subject rather than dated, updated by the change that alters
the behaviour. A design spec is a decision record and is not a reference document; one
becomes a reference document by being rewritten as one, not by being left in place.
lore's design-docs fragment and the personal profile say so; the writing is lore's
piece (§10). Which documents each repository promotes is per-project work, filed by the
project when it next touches the subsystem, and is not part of the migration.

### 7.3 Existing documents and the external reference

- Migrated documents keep their relative paths under `records`, so every stored
  `spec`/`plan` link (2,847 done, 80 open) resolves without a record edit, and prose
  paths in 1,224 bodies read correctly relative to the project's records directory.
- A document the code repository's own tracked files cite (a skill's relative link to a
  spec section, a README anchor, an `AGENTS.md` pointer) stays in the code repository
  until that project rewrites it as a reference document; the migration computes this
  set by searching tracked non-docs files for `docs/` paths and leaves those files out
  of the move. The task that links such a file still resolves it through the
  `records`-then-`root` order, trying the worktree the command runs in before the main
  checkout, as the resolver does today.
- The external reference form (`<scheme>:<rest>`, `doc_fetch.rs`, `[docs.schemes]`,
  `tasks doc`) stays. It costs nothing, no stored reference uses it, and it is the way
  a document that genuinely lives elsewhere (a thought, a wiki page) is linked. This is
  the explicit ruling revision 4, change 6 asked for: keep, not retire.

### 7.4 hq-5a3e12 and its pieces

| piece | ruling |
|---|---|
| hq-5a3e12 (design) | §3–§5 stand; §1 item 4, §6 and §7 superseded by this design; close with that note once the pieces below are re-scoped |
| tasks-91f63b | done; stands |
| ops-721511 (`mindful-doc`, `plan-snapshot`, `design_docs: mindful`, hooks) | superseded; its profile and hook parts re-filed under §4 and §7.1; the mindful parts become the §9 idea |
| tasks-00723a (skill wording) | re-scoped to the `records` wording |
| lore-ef7e5b (profile wording) | re-scoped to `design_docs: records` and §7.2 |
| flows-2ea069 | re-scoped per §6 |
| obs-caab0c (external_doc placement) | deferred: no external reference is stored; reopen when one is |
| mind6-646ca1 (`edit --body-file`) | independent of this design; stands |

## 8. Migration

Per project, in this order, by a script in ops (`records-migrate <prefix>`) that stops
at the first failed check and is rerunnable:

1. **Preconditions.** No live claim for the prefix (`tasks claims`); every worktree of
   the code checkout is merged or its record changes are merged into the main branch
   (a worktree with a record diff against main stops the migration); `tasks check`
   clean; the second host has synced (the user confirms).
2. **Export history.** In a temporary clone of the code repository,
   `git filter-repo --path tasks/ --path docs/specs/ --path docs/plans/ --path
   docs/notes/ --path docs/superpowers/ --path docs/designs/ --path-glob
   '<kept files>' --invert-paths ... --to-subdirectory-filter <prefix>`, with the kept
   set from §7.3 excluded, then fetch and merge it into the records repository with
   `--allow-unrelated-histories`. The source repository is never rewritten; each moved
   file keeps its full log under `<records>/<prefix>/`.
3. **Remove from the code tree.** One commit on the main branch deleting the moved
   paths; the pre-commit hook's docs-only fast path no longer matches, which is
   harmless.
4. **Register.** `tasks init --prefix <prefix> --records <records>/<prefix> --force`
   in the code checkout (§5.1); then on the other host once synced.
5. **Verify.** `tasks where` in the checkout says `kind: detached`; `tasks check`
   clean in the checkout and in the records directory; `tasks show` of a record with a
   spec and a plan resolves both; `tasks list --plan <path> --status done` finds the
   migrated steps; `tasks projects` shows `kind: detached` with `records` set. The
   location history is unchanged: `root` did not move, so a path under the old in-tree
   `tasks/` still resolves to the prefix through `root`.

Order of projects:

- **Pilot:** two small projects with no trial units and few consumers, `wali` and `tui`,
  to exercise every step end to end including the second host.
- **Then** every personal project except hq and obs, smallest first, once the §4 pieces
  that touch each have landed (the CLI first; `obs-index`, `tt-latency` and the hooks
  before the first non-pilot project; crew before `crew`'s own move).
- **hq and obs** after the flow-trial-1 readout on 2027-02-16: their records are the
  trial's measured environment, and `session-episodes` reads them. The kind-aware
  reader changes land earlier as no-ops for in-tree projects.
- **ops** keeps its prefix and detaches like the others; the public kit is a fresh
  project created with `tasks init --records` from the start (ops-741c5e).

Each host's `tasks` binary is updated before the first migration; an older binary
cannot read the `[records]` table and would treat a detached project as unregistered.

## 9. mindful, deferred

mindful may index `<records>/` as raw agent-owned thoughts (owner `agent:<harness>`,
`derivedFrom` the file path and commit), giving the graph operations over tasks and
documents the user described without a write dependency in either direction. Filed as
a mind6 idea; nothing in this design depends on it.

## 10. Pieces

Filed after review, each as a dependency of ops-487484 (Phase B implementation), with
the ordering noted:

- **tasks** (planned): the three-location model, `[records]`, `init --records`, `rename`,
  `records_moved` and `records_in_tree`, the output additions, the resolution order,
  the no-write test; `tasks docs` (direct, after it). CLI rows land on ops first.
- **ops** (direct, after tasks): `records-migrate`; check 5 and check 4 by `kind`;
  `design_docs: records`; `ops-check` exemption; `obs-index` and `tt-latency` reads;
  `ops-projects link` or its retirement for the detached form; the records repository
  skeleton and hooks (the GitHub repository on the user's go-ahead).
- **flows** (direct): skill wording §6; `excluded()` by `kind`.
- **obs** (direct): `_record_bytes` and `authorship.EXCLUDED` by `kind`.
- **hq** (direct): `session-episodes` and `rename-cutover` read `records`; close
  hq-5a3e12 with the §7.4 ruling.
- **crew** (direct): `run.ts` hashes under `records`.
- **lore** (direct): design-docs fragment, personal profile, reference-document rule
  (§7.2), feedback fragment's "one with `tasks/.config.toml`" wording.
- **mind6** (idea): §9.
- Re-scopes per §7.4 and the pilot and bulk migration tasks in ops, one per project.

## 11. Out of scope

- Rewriting public repositories' history to remove records already published; after
  the move the trees are clean and the user decides per repository.
- The artifact policy for generated files under `tasks/files/` (ops-bf9329).
- Consolidating chains of superseded specs into reference documents (per project, §7.2).
- Catalogue freshness (revision 4, change 8): the records root syncs the way every
  repository syncs today, so this design changes nothing there.
- The verifiably family and the work projects' own moves.
- The overlay's linked-only gaps (tasks-6360ac, ops-ba81cf, ops-c3a7e0, obs-29a062,
  obs-ba704d) stay as filed; none blocks a detached project.

## 12. Acceptance

1. In a scratch registry, a repository initialised with `init --records` reports
   `kind: detached` from its root, from a worktree of it, and from inside its records
   directory; `tasks add`, `note`, `start`, `park`, `done`, `attach` write only under
   `records` and commit there.
2. A worktree on a branch that still tracks `tasks/.config.toml` gets `records_moved`
   from `tasks list` and from `tasks note`, naming the records directory.
3. `init --records` on a checkout whose HEAD tracks `tasks/.config.toml` refuses with
   `records_in_tree`; after the removal commit it succeeds; `--force` on the registered
   prefix keeps its aliases and location history.
4. A migrated project's done record with `plan: docs/plans/x.md` and `step` resolves
   (`step_found: true`); a spec kept in the code tree per §7.3 resolves through `root`.
5. `stale_copy` does not fire across two code worktrees writing the same record; it
   does fire when the records repository itself has a second worktree.
6. `flow-state fingerprint` in a detached checkout excludes nothing and equals the
   `--head` form after a closing commit.
7. `tasks docs --project <p> --kind plan --search <phrase>` lists the migrated plan
   with its linking task ids.
8. The pilot migration of `wali` and `tui` passes §8 step 5 on both hosts, and
   `obs-index`, `tt-latency`, `session-episodes` (on a non-trial project) and
   `claude-pretooluse` check 5 behave as §4 says in the migrated checkout: a commit
   message naming a task id is allowed, a write under `tasks/` is refused.
