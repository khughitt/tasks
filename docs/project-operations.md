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

For existing records, declare custom spec/plan roots before init if needed;
configured lists replace defaults. Install the shipped tasks skill, adopt its
process/workspace policy in the project's agent guide, require prime at session
start and check before completion. Add checks only with a pinned binary source;
do not silently skip a missing tool. [Configuration](configuration.md) lists keys.

`where` reports project root/checkout/records/kind and holds_records. Within git
it stays in that repository; outside git it walks configs. holds_records also
recognizes tracked config in HEAD even when deleted locally. `root <full-id>`
prints the registered root. `projects --paths` displays detached record locations.
`resolve` is read-only: path classification precedes full ids and prefixes, and
id resolution does not prove a task exists. Each batch item can be resolved,
unregistered, ambiguous or invalid without failing other rows; registry errors
still fail the command. Use show to verify a task. Paths report via/location and
optional worktree/relpath; retired prefix aliases resolve while registered.

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

`[git] commit = true` opts record writes into scoped commits, once per git worktree.
Default false is manual. Commits include command record/attachment paths plus
anything a hook stages; concurrent git staging can also land. Subjects are
`chore(tasks): <verb> <ids>` (add/feedback include title, capped at 72 characters),
without attribution trailers. Migrate uses its source/successor/count subject
and ids in the body. Configuration edits from init/rename/retire/unretire are
not auto-committed.

`commit_unavailable`, `commit_deferred`, `commit_failed` and cleanup warnings do
not undo the record write or change its success exit status. Inspect paths/index,
clear the cause, then `tasks commit [--project <p>]` to sweep uncommitted tasks/
paths; this explicit command ignores the switch and fails if it cannot commit.
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
renames files/ids/local edges/config/registry and attachment directories; aliases
keep old spellings resolving while registered. A dependency's retired/live
spellings mean one edge; remove/add cannot rewrite it in one call.

Pending inventory freezes writes; rerun the same rename to finish forward.
Unexpected destination attachment directories (R9), missing on both sides (R10)
or source attachments appearing after inventory (R11) refuse recovery. Inventory
from pre-attachment binaries cannot load and blocks mutations host-wide: finish
with that binary, then explicitly move each task's attachment directory.

After another host's synced rename, `rename <old> <new> --adopt` updates only this
host's registry and stores. Live old/new claims block it. Use the refusal's
release/adopting remedy for unverifiable old claims only after the holder stops.
Detached rename moves the whole records project directory and updates pairing;
commit the directory rename in records. Rehearse with isolated XDG_CONFIG_HOME
and XDG_STATE_HOME, never against live host stores.

### Prefix rollback

Prerequisite: surviving inventory and recoverable pre-rename bytes. There is no
single-command undo; git checkout alone leaves new untracked files and host state.
Scope every action to inventory entries; never blanket git clean tasks/.

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
projects keeps the row and old ids still resolve. Unretire removes the table;
commit config separately. Unregister removes host registration/aliases/history,
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
