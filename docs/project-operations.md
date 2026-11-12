# Project operations and recovery

Type: guide; document types 1.
This guide covers project adoption, routing, record commits and lifecycle recovery.
See [ownership](ownership.md) for claims and [documents](documents.md) for link resolution.

Use the CLI for ordinary changes. Migration and recovery span records, checkout,
registry and host-local state; preserve every verified copy before deleting one.
Pause other hosts' writers when changing shared identity/storage. Local locks
cannot prevent unsynced remote writes. These procedures cover process interruption,
not power loss: rename/move/migrate do not promise fsync durability.

## Layout and adoption

| Kind | Registered root | Checkout | Records |
| --- | --- | --- | --- |
| in-tree | code checkout | root | root |
| linked | overlay project | foreign code checkout | root |
| detached | code checkout | root | separate records project |

`tasks init --prefix <p>` creates/registers in-tree records. From an overlay use
`init --prefix <p> --checkout <code>` for linked records. From the code main
checkout use `init --prefix <p> --records <existing-records-root>` for detached
records at `<records-root>/<p>`. --checkout and --records conflict. Detached init
writes prefix, checkout path/kind and git.commit=true in records, creates spec/
plan roots and pairs the host registry; nothing is written into code. Existing
config/comments are retained. Commit configuration changes separately.
After pairing, init (including `--force` and `--records`) refreshes the identity
snapshot from the registered root's committed `identity.toml`, without committing
configuration. No committed identity is silent; any refresh failure keeps the
previous snapshot and warns with the error and `tasks identity refresh <prefix>`.
Pairing still succeeds and init exits 0.

For existing records, declare custom spec/plan roots before init if needed;
configured lists replace defaults. Install the shipped tasks skill, adopt its
process/workspace policy in the project's agent guide, require prime at session
start and check before completion. Add checks only with a pinned binary source;
do not silently skip a missing tool. [Configuration](configuration.md) lists keys.

`where` reports project root/checkout/records/kind and holds_records. Within git
it stays in that repository; outside git it walks configs. holds_records also
recognizes tracked config in HEAD even when deleted locally. `root <full-id>`
prints the registered root. `projects --paths` displays detached record locations.
`projects` JSON rows add `name` and `purpose` from the snapshot when present and
reachable. Its pretty table stays unchanged; discovered-only entries appear in
`resolve --all`, not `projects`.
`resolve` is read-only: path classification precedes full ids and prefixes, and
id resolution does not prove a task exists. Each batch item can be resolved,
unregistered, ambiguous or invalid without failing other rows; registry errors
still fail the command. Use show to verify a task. Paths report via/location and
optional worktree/relpath; retired prefix aliases resolve while registered.

`resolve --all` lists every catalogued prefix in prefix order, including entries
discovered under a records store that this host has not registered. It conflicts
with positional inputs and `--stdin`. Registered projects retain their existing
resolution statuses and fields; discovered-only prefixes stay `unregistered`.
Enumeration keeps literal catalogue keys; explicit inputs still follow rename aliases.
Each readable records entry adds `entry`: config, records and declared checkout
paths, layout kind, shorthand and optional profile, color, derived `color_256`, retirement and feedback
scope. A snapshot adds name, public aliases, purpose and source/refreshed metadata
(plus commit/blob for a HEAD snapshot). Unset optional fields are omitted.
An unreadable or malformed entry adds `entry_error` and a warning while other rows
continue. Path rows within a known records store add its root as `records_store`,
including the store root itself, missing suffixes and symlinked ancestors. An
in-tree project's own repository is not a records store.

`resolve --freshness` compares the registered root's committed identity (the
overlay root for a linked project). It adds `entry.freshness`: `current` when the
identity blob matches (or a seeded snapshot still has no committed identity), `behind` when
the committed identity differs, and `unknown` without a snapshot or accessible
root, or when git fails. Git failures also warn. Resolve reads configs and
registry metadata only: it never reads task records, writes or takes mutation
locks. Without `--freshness`, it runs no git except the existing fallback for an
unregistered path.

## Missing record files

`show` and `doc` read a local task first, then the checkout named by that
exact task's held claim or park. An unavailable recorded checkout stays an error;
discovery does not bypass it. With no recorded location, `task_not_found` may
name physical record files observed in other Git worktrees of the same records
repository. Each hint offers explicit canonical `tasks -C <project> show <id>`
inspection; the failed command never returns a sibling task or reads a document.
The retry performs ordinary history, attachment and document validation afresh.

In-tree searches preserve the project's nested offset. Linked projects search
overlay worktrees, not code worktrees. Detached projects already share records
and never resurrect historical code-tree copies. Multiple matches remain
ambiguous, including identical files. Missing listed roots, malformed/unreadable
files, observed changes and exhausted limits make the search incomplete, never
proof that the task was deleted. No dependency or readiness authority changes.

Discovery inspects at most 64 other locations, reads at most 64 KiB per config,
1 MiB per physical record and 8 MiB total (including rechecks), and displays eight
matches and eight unresolved locations with omitted counts. Non-UTF-8 paths
require manual inspection. Symlinks/special files in the config or record
position are unresolved. This is bounded file observation, not an atomic
snapshot or a wall-clock bound on blocked filesystem I/O.

## Identity snapshots

`tasks identity refresh [<prefix>...]` refreshes records-entry snapshots from each
registered root's committed `identity.toml`. With no prefix it selects the current
project; rename aliases select their live prefix. `--all` selects every registered
project, reporting missing roots as `unreachable`. The registered root supplies the
identity even when invoked from a worktree or linked code checkout; linked projects
use the overlay's identity, relative to its directory within the repository.

The source is pinned to one captured HEAD commit and blob, never working files.
Malformed identities produce `identity_invalid`, a declared different prefix produces
`identity_prefix_mismatch`, and detached root HEAD produces `root_detached`; none
replaces the snapshot. No committed identity gives `no_identity`, preserving any
snapshot. `--from <file>` seeds one project only, with `source = "seeded"`, and refuses
`identity_committed` if the root has a committed identity, even a malformed one.

Only `[identity]` is replaced, preserving other config text. An unchanged identity
keeps its bytes and refresh timestamp; a HEAD move with the same identity blob also
keeps the original snapshot provenance. Refresh first holds the canonical project
mutation locks in prefix order, reloads/revalidates registration and refuses pending
lifecycle operations. Changed registration or alias routing while waiting gives an
explicit retry error. Each records repository is then locked across its batch and
receives one commit for changed configs opting into `[git] commit`, with
subject `chore(tasks): refresh identity snapshot for <prefixes>`. Configs without
that switch remain uncommitted; commit failures/deferred commits are warnings.

JSON returns `results` and `warnings`. Each row has `prefix` and `outcome`
(`refreshed`, `unchanged`, `seeded`, `no_identity`, `unreachable` or `error`), plus
snapshot metadata, changed fields and error detail when applicable. Every row that
read HEAD carries `branch` and `commit`, including later failures; `seeded`,
`unreachable` and failures before HEAD omit them. All requested projects are attempted,
and any `error` row makes the command exit 1.

`tasks check` compares its snapshot with the registered root's committed identity
(the overlay for linked records). A stale snapshot adds an `identity_behind`
warning naming `tasks identity refresh <prefix>`. Freshness never adds an error
or refuses a commit, and check does not refresh or write the snapshot.

## Detached pairing and relocation

Remove in-tree tasks/config from working tree and index before detached init;
otherwise `records_in_tree` refuses. A staged removal with HEAD still tracking
it is permitted so the removal commit can pass checks. Code main roots cannot
overlap other projects or share their common git directory. Init refuses a
linked worktree, nonexistent records root and duplicate checkout declarations.

Run detached init on each host after records sync. A missing pairing yields
`records_unpaired`; an old branch still keeping in-tree records yields
`records_moved` and is read-only history until merged/rebased. Plain init in a
paired checkout cannot undo detachment. Present invalid configuration never
falls back to another layout.

Before moving code, refresh `init --prefix <p> --force` on other hosts while old
root/storage exist (add --records for detached projects). Move the checkout,
then run the same detached init with --force at the new root on each host.
The first host rewrites a gone old checkout's config path and warns; commit that
config. A still-existing recorded checkout refuses relocation. Other hosts
repoint only their registries after synced config names the new root. Config is
restored if registry save fails. Until repaired, registry-wide reads name the
required command rather than guessing.

Location history keeps absolute current storage and former root/storage/until
tuples per host. Init/rename/adopt refresh it; unregister removes it. Preserve
all history when manually backfilling a pre-history move. Old binaries can drop
locations/records tables, so upgrade every registry writer first. A dangling
unchanged-root link retains recorded storage; a changed root clears current
storage when it cannot be found. Removed worktree paths can still resolve through
recorded former storage, without making that checkout writable.

## Record commits

`[git] commit = true` opts record writes into scoped commits, once per git worktree,
serialized per repository under `$XDG_STATE_HOME/tasks/claims/.commit-<common_hex>.lock`.
Default false is manual. Commits include command record/attachment paths plus
anything a hook stages; concurrent git staging can also land. Subjects are
`chore(tasks): <verb> <ids>` (add/feedback include title, capped at 72 characters),
without attribution trailers. Migrate uses its source/successor/count subject
and ids in the body. Configuration edits from init/rename/retire/unretire are
not auto-committed.

Read-only status and diff calls run with `GIT_OPTIONAL_LOCKS=0` so they never
create `.git/index.lock`. Index-mutating Git calls retry briefly on transient
`index.lock` contention without unlinking `.git/index.lock`. Before staging,
each commit records its own paths and staged blobs in a per-worktree ledger
(`.commit-<git_dir_hex>.staged`); if a commit is interrupted or fails to
unstage on cleanup, the next commit in that worktree recovers its own staged
entries while still deferring on foreign staging.

`commit_unavailable`, `commit_deferred`, `commit_failed` and cleanup warnings do
not undo the record write or change its success exit status. Inspect paths/index,
clear the cause, then run `tasks commit [--project <p>] [<id|path>...] [-m <msg>]`
to commit specific task IDs (`tasks/<id>.md` plus `tasks/files/<id>/`),
records-relative paths (such as `docs/specs/...` or `docs/plans/...`, which
require `-m <msg>`), or with no targets every uncommitted file under `tasks/`;
this explicit command ignores the switch and fails if it cannot commit.
Retired projects allow explicit config (`tasks/.config.toml`) and working-document
path commits; task IDs, task directories and ordinary no-target sweeps remain refused.
Git repository selection ignores inherited hook GIT_DIR/COMMON_DIR/WORK_TREE/
INDEX_FILE, except the migration guard deliberately checks the committing index.

## Move tasks and resume

`tasks move <id> --to <prefix> --explain` diagnoses without locks/writes. Fresh
move requires an open, unparented, childless, unclaimed task in its registered
checkout and a live destination. Destination docs/needs/dependencies validate.
Apply without --explain preserves fields, notes and live attachments, carries
this host's park/escalation, and leaves a read-only moved tombstone. Archived
note segments remain at both identities. Existing dependents follow moved_to;
new writes/refs to the tombstone refuse and name the replacement command.

MoveOut reports source/new id, fresh/resumed/complete recovery, attachments,
park, inbound dependents and per-root paths. Commit listed paths when auto-commit
is off. Inbound scanning cannot inventory unreachable projects. A successful
rerun writes nothing; interrupted --explain says resume. Rerun the exact command
to finish forward from the host journal. Pending journals freeze both ids and
project lifecycle operations with move_pending; reads continue. Check separates
reachable absent/mismatched pairs (move_incomplete) from an unreachable/unparsable
counterpart (move_counterpart_unreachable). The latter proves nothing about
completion. Other hosts' parks stay keyed to the old id.

### Move manual undo

Prerequisite: surviving journal and verified source/attachment recovery bytes.
Pause writers. There is no undo command. Restore first, delete last:

1. Restore source_text; verify SHA256 equals source_digest.
2. Restore every journal attachment in the source directory with its journal
   digest, copying back from destination if necessary. If neither copy matches,
   preserve both and journal and stop; restore from verified git/other-host backup.
3. Restore source_park and source_escalation verbatim under the source id and
   confirm each reads equal.
4. Only after verification remove destination store entries, attachment directory
   and record. Never remove the sole matching attachment copy beforehand.
5. Remove journal last. Any failure leaves it and both projects frozen.

A forward rerun can finish instead only while every attachment has a matching
copy. Neither direction can reconstruct bytes from a digest alone.

## Prefix rename and adoption

`rename <old> <new> --explain` diagnoses without writes/locks/authorization checks.
Apply requires clean tasks/, no live claims and at most one git worktree. It
renames files/ids/local edges/config/registry and attachment directories, and
moves References paths at or under `tasks/files/<old>-<hex>` to the renamed
directory; body prose and notes keep their old spellings. Aliases
keep old spellings resolving while registered. A dependency's retired/live
spellings mean one edge; remove/add cannot rewrite it in one call.

Retained history adds a Git-backed source/lineage check: all operations must be
completed or proved reconciled and tasks/.config.toml must match HEAD. The edge
anchors the source commit/ref and old/new records subdirectories; historical
notes and attachment contents remain unchanged. No general move/migrate exception
is granted. See [retention](retention.md#project-rename-and-historical-locators).

Pending inventory freezes writes; rerun the same rename to finish forward.
Unexpected destination attachment directories (R9), missing on both sides (R10)
or source attachments appearing after inventory (R11) refuse recovery. Inventory
from pre-attachment binaries cannot load and blocks mutations host-wide: finish
with that binary, then explicitly move each task's attachment directory.

After another host's synced rename, `rename <old> <new> --adopt` updates only this
host's registry and stores. Live old/new claims block it. Use the refusal's
release/adopting remedy for unverifiable old claims only after the holder stops.
Detached rename moves the whole records project directory and updates pairing;
commit the directory rename with `tasks commit --project <new>` and no targets.
A completed rename leaves a host-local receipt in `tasks/rename-commit/` under
XDG_STATE_HOME (or `~/.local/state`). This commit route verifies the frozen config
and task digests, includes source deletions and destination additions under the
repository lock, and removes the receipt on success. A changed or missing task,
changed config or remaining source file refuses the commit and keeps the receipt;
a failed Git commit also keeps it. The default subject is
`chore(tasks): rename <old> -> <new>`; `-m` overrides it. In-tree renames use the
same receipt route for `tasks/`. Rehearse with isolated XDG_CONFIG_HOME and
XDG_STATE_HOME, never against live host stores.

### Reclaiming a retired predecessor's prefix

A retired project's records can be renamed and adopted without unretiring it.
The retirement date, reason and successor stay intact, and task edits remain
blocked. Move its checkout separately, using `init --force` with the existing
records pairing to record the new location.

For one retired predecessor and one fresh successor, reclaim the predecessor's
old prefix in two committed steps. For example, with `forge` retired in favor of
`forge2`:

1. Run `tasks rename forge forge1`, then `tasks commit --project forge1`.
   Sync the committed records before proceeding.
2. Run `tasks rename forge2 forge --explain`, then `tasks rename forge2 forge`
   and `tasks commit --project forge`.

Step 2 requires a reachable, retired `forge1` whose successor names `forge2`,
with no predecessor of its own, unfinished operation, claim, park or escalation.
Its completed Step 1 receipt must be cleared, its tasks must be committed, and
its former detached records path must have no uncommitted deletions or additions.
The two generations' six-hex suffixes must be disjoint; a collision refuses with
both IDs before any files change.

The predecessor must name the successor's immediate current prefix. If it names
an older alias of a successor renamed before handoff, the operation refuses with
an unsupported prior-successor-alias-history diagnostic. That alias still
identifies the successor on this host, but the single `successor_alias` field
cannot preserve additional earlier aliases and carried pointers on a fresh host.

Step 2 freezes the predecessor prefix `forge1`, successor alias `forge2`, and
predecessor suffixes in the successor's `[predecessor]` config and the host
registry's `[predecessors.forge]`, installs `forge2 -> forge`, and updates the retired config's
successor to `forge`. The rename inventory pins both configs' old/new digests;
rerunning the same rename completes interrupted stages. The receipt verifies the
predecessor config as well as the renamed task files. In a shared records Git
repository, the receipt commit includes that config. With separate in-tree
repositories it commits the successor and warns to run
`tasks commit --project forge1 tasks/.config.toml -m "chore(tasks): update successor"`
for the predecessor separately.

A host registered before cutover adopts `forge -> forge1` before
`forge2 -> forge`, even if both directory changes arrive in one sync. Run the
first adoption from the archived predecessor checkout and the second from the
successor checkout. After a checkout move, repeat
`tasks init --prefix forge --records <records-root> --force` there. A reversed
adoption order refuses with the missing first step.

Literal `forge-<old-suffix>` IDs route to `forge1`; `forge2-<new-suffix>` routes
through the host's alias to the current `forge`. `forge2-<old-suffix>` and a bare
old suffix in the new project remain not found. New task IDs reserve the frozen
predecessor suffixes, even when `forge1` is unregistered. A fresh host imports the
predecessor table and its required `successor_alias` when pairing the successor.
Register the final projects in either order with ordinary `tasks init --prefix
<p> --records <records-root>` from their checkouts (omit `--records` for in-tree
records). Reads of old IDs name the missing predecessor until it is registered;
after both registrations, carried migration links and `tasks check` resolve
through the imported alias. No manual alias creation is needed.

Init, rename and adoption accept an identical alias but refuse a name owned by
another alias, live project, group or predecessor before changing configs or
registry. If the shared metadata and host alias disagree, normal opens refuse
and `tasks check` reports `predecessor_registry_mismatch`. Re-run the appropriate
`init --force` command above to restore a free or identically owned alias;
conflicting ownership must be resolved first.

Current limit: later renames of either generation participating in a handoff also refuse before
mutation, because this single-predecessor format cannot preserve every historical
literal prefix after another rename.
A retired project's declared successor must remain registered and reachable to
check that bound against shared configuration. Register and pair the successor
before attempting an initial retirement rename; registering only the predecessor
or unregistering the successor does not permit later participant renames.
`unretire` also refuses for a handoff participant: clearing retirement would erase
shared successor evidence and permit a later rename to break historical routing.
For an ordinary retirement, `unretire` remains available once its declared
successor is registered and reachable; on an already active project it remains a
no-op.

### Prefix rollback

Prerequisite: surviving inventory and recoverable pre-rename bytes. There is no
single-command undo; git checkout alone leaves new untracked files and host state.
Scope every action to inventory entries; never blanket git clean tasks/.

For a retention rename, preserve the inventory's frozen config_payload together
with its fingerprints. Source commits and retained refs must remain available on
resume. Do not regenerate an edge from a later HEAD. Rollback restores the exact
source config/lineage with the owned task paths; never replace unrelated registry
or records changes. Do not delete protected attachments to get past a refusal.

1. Restore each missing source and verify its `from` digest before deleting its
   destination, and delete only when destination digest equals `to`. On failure
   keep destination and stop. Outside git, removed source bytes may be unavailable;
   forward recovery is then the only supported route.
2. Restore old prefix config if that stage landed. For detached directory moves,
   restore the old directory location and verified config before repairing pairing.
3. Restore the old registry project key, remove new key and old→new alias, and
   repoint every alias whose pre-rename target was old. Restore detached records
   pairing and groups/location history from a verified pre-change registry copy.
   Inventory is not a full registry snapshot; if the prior state is unknown,
   preserve both states and stop rather than reconstructing it by assumption.
4. Restore old claim-store parks and escalations from parks_store with ids
   re-prefixed, verify them, then remove the new store.
5. Remove the old-prefix rename inventory last. Keep it on every failure.

## Successor migration

`migrate --to <successor> --draft <file>` drafts one decision per open task in tree
order. Decide carry, drop or close; drop/close requires reason. Carry can settle
parent (carried id, successor task, none), rm_depends, links=drop and replacement
body. Drop/close release lists acknowledge every open outside dependent;
close includes carried dependents under their source ids, while carried tasks
remove edges to drops themselves. Explain reports all refusals, successor tree,
reasons/releases, wakes, inbound edges, removed edges/links and body diffs.

Review the exact `migrate <file> --explain --pretty` output with the user; commit
the decided list in source records before apply. Pause every other host's writers
of both projects through completion and sync. Apply, rerun that exact list after
interruptions, commit reported paths, then `retire --successor <successor>`.
The local journal freezes both projects except exact repair commands the refusal
names. It runs forward; there is no rollback command. Migration notes bind
`list: sha256:<digest>`. Resume revalidates but cannot detect unsynced remote writes.

### Journal-less successor recovery

If the journal is lost after record writes, the refusal classifies unfinished
items. Keep the list unchanged and reconcile item by item:

1. For remaining carried attachments remove a source file only if the tombstone
   ledger says `detached: <name>: moved to <new id>` and destination SHA256 equals
   source. Never remove a note segment named by source note_archives; restore a
   missing/corrupt segment from a descriptor-matching copy or history.
2. For each carried park/escalation, compute expected destination by re-keying
   source to new id; change only park worktree to destination resume root. If
   destination absent, write and verify expected before removing source. If equal,
   remove source. If different, keep both unchanged and stop for human reconciliation;
   a newer destination park is normally retained, but never assumed equivalent.
3. For dropped/closed store entries, verify dropped/done record with this list's
   digest in its migration note before removing the source entry.
4. Commit uncommitted paths by pathspec in each repository, then rerun the same
   migrate command. Expected recovery is complete, not a newly invented migration.

Journal loss after only attachment copies can classify fresh; check reports prior
unledgered copies as attachment_orphan. Preserve/verify them before explicit
detach/removal. Journal-present manual undo follows move undo per task, verifying
every restoration before any destination removal; journal last. Other hosts'
parks remain source-keyed. Power-loss recovery is not promised.

## Retirement and backfills

`retire --reason "<why>" [--successor <prefix>]` runs in registered checkout and
refuses open work in any checkout, live claims or unreadable linked worktrees.
It writes retired date/reason/canonical successor. Writes from old worktrees then
refuse retired_project. Explicit reads warn; group/all-project reads omit it,
projects keeps the row and old ids still resolve. Explicit config and document
commits, prefix renames and rename adoption remain available. `init --force` may repoint a retired project while
preserving its exact retired date, reason and successor. If its old in-tree
checkout was moved away, init adopts the config at the destination; a reachable
retired authority cannot be replaced with an active or differently retired config.
Unretire removes the table for a project outside a handoff, after checking its
declared successor is reachable; commit config separately. A handoff participant
keeps its retirement and shared successor evidence. Unregister removes host registration/aliases/history,
not shared task history. Broader project lifecycle coordination belongs to ops.

`tools/review-backfill` and `tools/wait-backfill` are one-time migration helpers,
not ordinary review commands. Use --tasks to pin the binary, --root for the
project and a persistent --state directory. --freeze-only writes a per-prefix
snapshot without applying. Review backfill freezes unreviewed ideas whose latest
scope/curate note establishes prior review. Wait backfill freezes stranded ideas
with unchanged list/show stamps; it follows moved references and auto-decides only
one resolved open target. Judge entries need leave, awaiting_user or dep decisions.

Before --writers-paused apply, pause every record writer on every host. The flag
confirms operator coordination; a local live-claim check is additional protection,
not distributed exclusion. Every snapshot is validated before applying; each
unchanged eligible entry gets a checkpointed outcome, changed entries are reported,
operational failures remain retryable. Keep snapshots for reruns and inspect all
failed/uncommitted/remaining/undecided results: exit 0 alone does not mean all
decisions were applied. Recover record commits with tasks commit before writers
resume. Do not weaken stamp checks, invent evidence or delete the helpers as part
of documentation migration.
