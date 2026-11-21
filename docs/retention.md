# Retained lifecycle

Type: protocol, data model; document types 1.
This is retention-1: current record, manifest, request and operation formats are
version 1, with explicitly supported unversioned item records accepted historically.
It defines observation and custody alongside the [task model](task-model.md) and
[attachment integrity](attachments.md).

## Participants and boundary

The open owning task/session declares custody, the current host observes bytes,
and tasks publishes pinned records and operation phases in Git. A receiving task
accepts a transfer; a coordinator finishes or reconciles an operation. Lore owns
the operating policy for what to keep, handoff agreement, session integration
windows and disposal. This document defines the shipped command/data contract;
structural acceptance alone establishes no agreement or safe deletion verdict.

Use one coordinated writer host for every participating task until completion/
reconciliation commits and syncs. Start and hold the tasks first; mutations need
Git records with git.commit=true, validated input bases and ordinary ownership.
New native checkpoint/transfer admission supports detached and linked records;
any in-tree participant refuses before intent or artifact publication. Existing
published in-tree operations retain their retry/reconciliation routes and historical
reads. A staged phase alone grants no admission exception. The manual route remains
available where supported, and cannot bypass a pending operation or failed publication.
Existing retained history needs separately supported recovery before records migration;
changing layout or using an external manifest does not preserve its old locators.
Local locks/heartbeats do not exclude remote writers or create atomic directory snapshots.
No payload copying, relocation, deletion or automatic closure is implemented here.

## Record validation reads

For a project with retention rename lineage, `tasks check` uses one invocation-owned
read session for its retention validation pass. It captures each records repository's
HEAD and local branches on first access, including repositories reached through peer
proofs, and reuses immutable Git reads and parsed logical task history within a
bounded cache. Concurrent ref changes alone do not introduce a stale-basis failure;
the next invocation captures fresh refs. Live storage and protected attachment byte
checks still run, and malformed or missing history still fails validation.

The cache is discarded on return, including errors. The lineage check admits up to
4,096 entries and 32 MiB of accounted payload and keys without eviction. It reserves
the last quarter of each budget for path history and ancestry queries. Reads beyond
their admission limits run uncached. The standalone retained commands keep their
2,048-entry and 32 MiB limits.

## Retention records and manifests

Notes beginning `retained: ` contain strict JSON. Every non-migration write path
(`tasks note`, `tasks note --stdin`, `tasks edit`, and lifecycle status transitions)
validates newly appended `retained:` notes before saving, leaving already-persisted
notes and new-file `move`/`migrate` copies untouched. The `retained:` prefix is
reserved for Retention-1 JSON item records with integer `version: 1`: missing the
space after `retained:` or supplying non-JSON prose fails with `retained_input` and
directs ordinary prose to a non-reserved prefix such as `Custody note: ...`;
unversioned `retained:` JSON fails because newly written records require integer
`version: 1`. `tasks note <id> --stdin` validates every normalized line before
writing or committing any note in the batch and reports 1-based `stdin line <k>: ...`
on failure. The history reader continues to support valid historical unversioned
records without rewriting them. Duplicate keys, unknown fields/versions and wrong
types fail; unsupported history is never silently interpreted as a current subset.
Identity within a task is exactly host/kind/path. Path spelling remains stable;
tokens are data, not shell commands.

| Record field | Contract |
| --- | --- |
| version | 1 for current writers; absent only for supported historical objects. |
| host, kind, path | Host string; kind worktree/scratch/evidence. Worktrees use .worktrees/... or absolute path; data paths use absolute or literal $WORK_ROOT/... . |
| state, verification | kept/released; verified/unknown. |
| purpose, next_owner, cleanup_trigger | Nonempty purpose; kept owner task:<id>, session:<non-whitespace-id>, or user; concrete non-none cleanup trigger. |
| sha256, manifest | Conditional current digest and manifest location; absent differs from null. |
| handoff | Required nonempty text only for user owner; does not prove the user's agreement. |
| verification_action | Required concrete text for unknown; absent after verified successor. |

Verified worktrees omit content digest/manifest and carry Git/ignored-file
observations. Verified data files have exact SHA256; directories need complete
manifest and its digest (required structurally for scratch, checked semantically
for evidence directories). Under `verification: "verified"`, an absent or null
`scratch` manifest fails as `missing manifest`, while any non-absent manifest
locator not starting with `/` or `$WORK_ROOT/` (such as `files:<name>` or a
relative path) fails as `manifest locator must start with / or $WORK_ROOT/`.
Unknown data uses null current sha256 and a concrete action; unknown directories
name the last located manifest or null, and any non-absent non-null manifest
locator not starting with `/` or `$WORK_ROOT/` fails as
`unknown manifest locator must be null or start with / or $WORK_ROOT/`. Unknown
worktrees still omit digest/manifest. Released records use verified, owner/trigger
none and omit digest/manifest/handoff/action; prior evidence remains in history.
Open-owner and real handoff/disposition rules require policy checks beyond JSON.

Manifest 1 is `{version:1,entries:[...]}`; members have path and type file
(sha256), directory, or symlink (literal target). Preflight strictly requires
integer `version: 1` on existing record manifests and refuses pre-`Manifest 1` or
unversioned historical formats (`unsupported historical manifest format; preserve
and migrate it explicitly`); historical formats never acquire `Manifest 1`
authority by inference. Paths are normalized relative
components: no absolute/drive prefix, backslash, empty, dot or dot-dot components.
Include hidden files, empty directories and literal links without following them.
Sort by path and hash exact saved UTF-8 bytes. Duplicate paths, special/unreadable/
unstable members or incomplete enumeration fail semantic verification. A manifest
does not retain symlink target bytes or reconstruct missing member versions.

Keep each manifest outside its inventoried container, with retrievable exact old
members. Generated task-attachment manifests keep the canonical locator
`<records>/tasks/files/<id>/retained-manifest-<sha256>.json` in
`RetentionRecord.manifest` whether stored as a single file within `[attachments]
max_bytes` or split into `.part-NNNN` and `.catalog.json` members through
`[attachments] multipart_validators`. Independent child identities retain their
own owners/dispositions even when included in an enclosing manifest. Do not
exclude metadata to fit a cap. Attachment/layout refusal keeps raw evidence and an
incomplete accounting action; it does not authorize a guessed storage root or loss
of the only copy.

## Bases, sources and artifact locators

All operation formats reject unknown fields and duplicate keys. The following
objects are shared by state, requests and receipts; digests are 64 lowercase hex.

| Object | Exact fields and values |
| --- | --- |
| Identity | host, kind, path. |
| Basis | project (prefix), layout (in-tree/detached/linked), records_subdir (`.` or normalized member path), commit (full 40 lowercase hex), task_path (normalized member path), retained_ref (reachable local refs/heads/... branch). |
| Source | basis, ordinal (positive logical note ordinal, including archived notes). |
| Input | task (canonical id), basis. Inputs have unique tasks. |
| OperationKey | coordinator (task id), operation_id (32 lowercase hex). |
| Git locator | type git, basis, path (normalized member path), sha256. |
| Attachment locator | type attachment, task, name, sha256, optional basis. |
| Directory locator | type directory, host, path, manifest (Git/attachment locator, never nested directory). |
| Declaration | kind, path, purpose, cleanup_trigger. |
| Preservation | identity, artifact (locator for previous bytes). |
| Resolution | bad (Source), declarations (complete correction, `[]` only when 0 identities exist in host/task scope), reason. |
| ResolutionNote | version=1, bad (Source), successors (distinct positive ordinals `> bad.ordinal`, `[]` allowed for 0-item host/task diagnostics), reason. |
| Publication | task, phase, basis. |
| Mapping | sender Identity, receiver Identity. |
| Counts | kept_verified, kept_unknown, released (nonnegative integer counts). |

Attachment basis may be omitted only inside its own task's committed note: it
pins to that containing publication. Logical `retained-manifest-<sha256>.json`
attachment locators resolve either a single-blob `attached:` entry or a live
`multipart:` declaration whose `.part-NNNN` and `.catalog.json` members are all
attached at the pinned basis, reconstructing bytes through `ReadContext`. Foreign
locators and preservation artifacts in requests require explicit bases, including
directory manifests. This prevents self-referential commit hashes without weakening
immutable retrieval. A basis must match actual task/project/layout and reachable
retained branch, not merely pass field-shape checks. Keep named refs and required
Git objects available.

## Participant freshness

A new request's basis B may precede locked HEAD H when its project, records pairing,
layout/rename epoch, task path and publishing branch still match, B is an ancestor
of H, and the participant remained byte-identical throughout. Protected committed
paths include the task record, its entire attachment subtree and required note
archives, including Git modes and additions/deletions. Intervening history is
walked without path-limited simplification. A commit that descends from B is a
lineage commit (also when a reused proof starts the walk at a later proven head):
each of its edges to B or another lineage commit must leave the participant
unchanged, so byte-identity holds throughout the lineage. Every edge of a commit
that does not descend from B (side history) is checked the same way, and so is
every first-parent edge. A lineage merge's later-parent edge to a commit outside
the lineage (pre-basis or unrelated imported history, such as an archive-import
merge whose second parent never held the participant) is not compared: the lineage
edge proves the merge kept the participant, and the side's own edges catch any
edit made there. Change-then-revert (ABA), side-branch edits and merge resolutions
that change the participant relative to a lineage parent refuse, even with equal
endpoints. A refusal names the offending `<parent>..<commit>` edge, or the pinned
basis and current commit when the endpoints differ.
Unavailable Git evidence fails explicitly.

The submitted B stays in the immutable request and phase input_bases. At later
publication boundaries, freshness starts at the last committed and successfully
read-back phase basis P. A restart recovers P from exact durable request/phase
publications, never from its initial HEAD snapshot or an uncommitted phase. Only
successful task/artifact commit readback advances P. A fresh reconciliation starts
from its directive's explicitly pinned inputs; subsequent reconciliation publications
become P while original operation B stays unchanged. Existing retry/preimage checks
still apply; a completed replay remains historical, and a committed sender release
permits its missing receipts to finish despite later receiver work.

For a pending operation, ordinary appended notes and their accompanying owner,
complexity and update metadata changes from park/start may commute. Each intervening edge
must fit those retry rules, keep attachment/archive trees and task mode unchanged,
and match the task writer's exact serialized output. Reserved retention/evidence
notes (`retained:`, `retention operation:`, `retention checkpoint:`,
`retention resolution:`, `attached:`, `detached:`, and `multipart:`), byte-only
edits and their reverts still refuse, and none of those prefixes may appear
between a complete machine checkpoint's `basis` and its publication. This
exception never changes P or weakens new admission.

New admission also compares live task/archive files and the complete attachment
tree against committed bytes and modes, including index changes, ignored/untracked
additions and unexpected directories or symlinks. Any pre-existing unledgered file
under `tasks/files/<id>/` fails `working_preimage` (`retained_stale_basis`) naming
`tasks/files/<id>/<name>` with instructions to preserve and verify a copy outside
`tasks/files/<id>` if needed, remove the unledgered file to restore the committed
preimage, and retry or reconcile. These checks do not replace full
payload verification or authorize recursive proof pruning. No new history-proof
walk is added to admission.

## Query state

`retained state <id> [--at <revision>] [--host <host>] [--all-projects]` reads one
immutable commit for `<id>` and returns `version` 1, `contract` `retention-1`, `task`,
`basis`, `host_filter`, `complete`, `items`, `checkpoints`, `operations`,
`diagnostics`, `incoming` and `warnings`. `--all-projects` conflicts with `--at` at
parse time. Default `HEAD` refuses dirty required task/archive inputs on `<id>`;
explicit `--at` uses only its pinned blobs and sets `incoming` to `{ "scope": "none",
"complete": null, "items": [], "diagnostics": [] }`. Before a default query returns,
it runs a joint bounded `(symbolic-ref, HEAD)` convergence loop (`0..4` iterations)
across `<id>`, `<id>`'s `moved_from`/`moved_to` hops, `moved`-holder hops, verified
candidate holders, newly observed `B_P..HEAD` lineage commit edges, staged `:0:` index
entries, unmerged index stages and working-tree task/archive files. Unrelated records
commits can advance `HEAD` without invalidating the query: output keeps its original
`B` and logical source ordinals. Ordinary non-archive attachment/payload files under
`tasks/files/<tid>/` are not live query inputs; historical proof uses pinned Git
blobs. Explicit `--at` skips live checks and incoming discovery. No implicit
fetch/push/provider occurs, and historical interpretation never hashes live retained
payload bytes on disk.

Each top-level item has `identity`, nullable effective `record`/`source`,
`observed_at`, nullable `checkpoint` `Source` and `coverage`
`recorded`/`changed`/`unknown`/`blocked`. Checkpoint entries have `source` and
nullable `note`/`text`; operation entries `source`/`note`. Top-level diagnostics have
`code`, `scope` (`identity`/`host`/`task`), nullable `host`/`identity`/`source`,
`detail`, `action` and optional `resolved_by` `Source`. A malformed later note
quarantines the identity, host or whole task it could supersede; effective `record`
becomes `null`, `complete` `false`. A host filter never hides task-wide uncertainty or
falls back to older valid state. Interpretable incomplete state can exit 0;
unreadable/corrupt required bases on `<id>` fail. Resolve the exact bad source, not an
assumed latest equivalent.

`incoming` reports `next_owner: "task:<id>"` custody held on tasks outside `<id>` and
`<id>`'s own `moved_from`/`moved_to` chain without altering top-level `items`,
`complete` or `diagnostics`:

- `scope` is `"project"` on bare `retained state <id>`, `"all_projects"` with
  `--all-projects` (scanning all registered projects, including retired projects), or
  `"none"` with `--at`. Clearing task closure (`done`, `drop` or curation closeout)
  requires a complete `--all-projects` result (`complete: true`, `incoming.scope ==
  "all_projects"`, `incoming.complete: true`); bare `scope: "project"` is for local
  inspection and basis lookup only and never clears cross-project custody (`moved`
  local holders that migrated to another project emit `holder_moved_out_of_scope`
  under `scope: "project"`).
- Candidate discovery is bound to each Git-backed project's pinned `B_P` tree
  (`ls-tree` + `cat-file --batch`) and verifies required `.notes` archives through
  `history::load_logical` (descriptor, SHA-256 digest, UTF-8/note syntax, `note_count`
  and live `attached:` ledger; mismatches emit `code: "parse"`). Non-Git projects stay
  silent when clean and emit per-holder `retained_publication_unsupported` when
  mentioning `<id>`'s suffix; corrupt `.git` metadata fails closed. Matching resolves
  reciprocal `moved_from`/`moved_to` chains, retired registry aliases, `[predecessor]`
  displaced suffixes and unadopted `[retention_lineage]` edges.
- Each `IncomingItem` carries `holder` (canonical holder task ID), `identity`,
  effective `kept` `record`, `source`, `observed_at`, nullable `checkpoint` and
  `coverage`, sorted by `(holder, identity)`. Each `IncomingDiagnostic` carries
  `code`, `project`, nullable `holder`/`host`/`identity`/`source`, `detail` and
  `action`; `incoming.complete` is `Some(incoming.diagnostics.is_empty())` on live
  queries (`None` under `--at`). Changes to `<id>` or non-convergence on `<id>`'s
  project return top-level `Err(retained_stale_basis)`; stale, malformed or blocked
  external holders drop that holder's `IncomingItem`s and emit `IncomingDiagnostic`
  (`incoming.complete: false`).

## Requests and checkpoints

Requests are UTF-8 JSON files, not stdin. Keep the mutable request outside its
inventoried directory and copy current basis from state. Choose an operation id
once; whitespace-only changes are harmless but changed parsed terms/bases need a
new operation. Run `tasks retained checkpoint <id> --request <path>`.

| Request | Required fields | Optional defaults |
| --- | --- | --- |
| Checkpoint | version=1, operation_id, host (current hostname), inputs (exactly one) | declarations=[], preserved_previous=[], resolutions=[] |
| Transfer | version=1, operation_id, host, inputs (sender then receiver), nonempty items, receiving purpose, cleanup_trigger | none |
| Reconcile | version=1, operation (original key), reconciliation_id (32 lowercase hex), fresh inputs (all participants, coordinator first), reason | none |

Checkpoint inspects every known kept identity on this host, including omitted
declarations. It cannot discover unrecorded items or inspect other hosts. It fully
hashes unchanged data too, reusing exact item notes/manifests when possible;
historical objects get explicit v1 successors. Worktrees get Git/ignored-file
observations without a content hash. Dirty worktrees are observed state, not
automatically unretained data.

Intended changes require preserved_previous with the exact predecessor's bytes.
Directory proof needs old manifest and every old member; additional immutable
members can coexist. Missing/unstable/unexpected bytes stay kept/unknown with null
hash and concrete action, not released. Unsupported historical manifests or a
manifest inside its container refuse. An unknown successor does not erase the
prior-version requirement; later checkpoints recheck referenced preservation.
Use an explicitly verified replacement locator to repair unavailable prior storage.

On fresh `checkpoint` (against the participant project) and fresh `transfer`
(against the receiver project), existing directory containers run a metadata-only
precheck before `publish::intent` writes a reservation or hashes file contents:
the CLI computes exact `Manifest 1` UTF-8 byte length, exact newline `cut` part
count, and `.catalog.json` byte size against `[attachments] max_bytes`. Missing
or unmanifestable directories skip pre-`intent` refusal so post-`intent`
observation records `kept`/`unknown`. Manifests within `[attachments] max_bytes`
(default 2 MiB) publish as single-file `retained-manifest-<sha256>.json`
attachments. When a manifest exceeds `max_bytes` and `[attachments]
multipart_validators` is declared in `<prefix>/tasks/.config.toml` (for example
`[]` or validator entries) and the manifest, part count, and `.catalog.json` fit
`multipart_max_bytes` (default 16 MiB, ceiling 256 MiB), `multipart_max_parts`
(default 16, ceiling 256), and `max_bytes`, `checkpoint` and `transfer` publish
and verify `Manifest 1` natively as `.part-NNNN` and `.catalog.json` members
under `tasks/files/<id>/`, keeping `RetentionRecord.manifest` and
`ArtifactLocator::Attachment` at `retained-manifest-<sha256>.json` and reusing
unchanged live multipart manifests without duplicate notes. When
`multipart_validators` is undeclared (`multipart_validators_undeclared`) or a
multipart limit is exceeded (`attachment_too_large`), fresh `checkpoint`/`transfer`
refuses cleanly before `publish::intent` with zero notes or files written and
payloads untouched; declare `multipart_validators` (and raise `multipart_max_bytes`
or `multipart_max_parts` within the 256 MiB / 256-part ceiling) and rerun, or
retain an off-container `Manifest 1` file under `$WORK_ROOT/<project>/<task-id>/`
via the manual checkpoint route.

Resolutions in `CheckpointRequest` bind exact bad basis/ordinal with complete
declarations and reason, covering its blocked identity or all known identities in
host/task scope; `declarations: []` is accepted when zero identities exist in the
blocked `host` or `task` scope (for example 0-item accidental `retained:` prose),
publishing `items_published` with `resolutions` before completing with outcome
`no_local_items` and `checkpoint: null`. A scope spanning other hosts needs
explicit history recovery (or standalone `retention resolution:` below). Native
checkpoints publish changed observations/artifacts, verify that commit, recheck
history, then publish checkpoint plus completed receipt. No kept local items
yields `no_local_items` without an empty checkpoint. The command neither parks nor
closes the task.

## Transfers and reconciliation

`retained transfer <from> <to> --request <path>` requires both open tasks claimed
by the actor, fresh bases and selected sender identities on the current host.
An existing receiver identity conflicts even with equal hashes. Containers do
not transfer independent children. Use separate operations for different terms.
Cross-project worktrees receive verified absolute path spelling.

Publish both intents, verify current/prior bytes, commit receiver kept records,
revalidate histories/attachments, then recheck payloads before a separate sender
release commit, even within one repository. Release purpose is exactly
`Transferred to <receiver-task-id>; custody transferred and bytes remain in place`.
Its timestamp follows all receiving kept notes and prior sender update, including
future-dated notes. Failure before release leaves sender custody; retry original
request. A committed release permits only missing receipts to finish, even when
bytes later vanish. Counts cover both participants' recorded local identities;
fresh inspection covers selected items only, never a whole-host checkpoint.

`retained reconcile <coordinator> --request <path>` ends irrecoverable pending
intent without completing the abandoned transfer/checkpoint. Run on its original
retained branch, keep original request immutable and retry the same directive.
Publish observation changes and each ready receipt, then coordinator terminal
binding exact ready bases. Ready alone leaves reservation active. A committed
sender release refuses reconciliation: finish original receipts instead.

Reconciliation keeps current custody and independent exceptions. Unknown/missing
payloads remain kept/unknown, malformed history stays blocked. When a directory's
generated manifest exceeds `[attachments] max_bytes` during `reconcile_ready` and
is not already a live attachment on the task, reconciliation degrades that
observation to `kept`/`unknown` (naming the generated byte count vs `max_bytes`
and instructing the operator to configure `[attachments] multipart_validators`
and run a fresh `tasks retained checkpoint`, or use the manual off-container
manifest route) instead of failing with `attachment_too_large`. Reconciliation
observes effective history rather than applying abandoned replacements/corrections;
use a fresh checkpoint or `retention resolution:` for repair. Reconciled means
neither transferred nor host-checkpointed. Counts describe interpretable
ready-basis history; actions preserve blocked scopes. Terminal replay checks
immutable directives/publications without rehashing current payloads; a new
observation needs fresh bases.

## Source-bound history repair and historical manifest adoption

Historical malformed `retained:` item notes can be repaired without rewriting
history or running whole-host filesystem enumeration by appending a standalone
`retention resolution: <json>` note via `tasks note`:

```json
{"version":1,"bad":{"basis":{...},"ordinal":12},"successors":[14],"reason":"Repair legacy manifest locator"}
```

`retention resolution:` targets only `retained:` item notes (never
`retention operation:` or machine `retention checkpoint:` envelopes). Write-time
admission (`publish::check_write`), `basis::read`, and `proof::validate_history`
verify `bad.basis` against pinned Git history (`..bad.ordinal` exact note-prefix
equality across ordinary, renamed `lineage`, and `retention_recovery` histories);
a forged committed `bad.basis` fails both `tasks retained state` and lifecycle
mutations with `retained_history_blocked`. Replay evaluates all hosts unfiltered
before applying `--host`, and enforces scope coverage on `successors` (each must
be a valid `version: 1` `retained:` note ordinal with `bad.ordinal < s <
resolution_ordinal`):

- **Closed (`done`) tasks and existing attachments:** `tasks note` works directly
  on `done` tasks without reopening implementation (`tasks edit --status todo`)
  or `tasks start`. When a malformed note cited a relative or `files:<name>`
  attachment locator whose manifest already lives in `tasks/files/<id>/<name>`,
  keep that attached file untouched, append a valid `version: 1` `retained:`
  successor citing `<records>/tasks/files/<id>/<name>`, and append
  `retention resolution:` citing the successor ordinal.
- **`released` items and other-host items:** Append a valid `version: 1`
  `retained:` successor (`state: "released"` or the affected host's record) and
  bind its ordinal in `successors`; `tasks retained state --host <local>` sees
  and clears other-host diagnostics cleanly.
- **0-item accidental `retained:` prose:** When an accidental `retained:` note
  quarantined a `host` or `task` scope that has zero prior valid identities,
  append `retention resolution:` with `"successors": []` (no successor item note
  needed), then record any intended prose under a non-reserved prefix such as
  `Custody note: ...`.
- **Historical spaceless `retained:` notes (`retained:{...}` without a space after
  `retained:`):** `state::replay` leaves unresolved spaceless `retained:` notes out of
  item replay so lifecycle write admission (`retained transfer`, `checkpoint`,
  `reconcile`) is unchanged, while `incoming` custody discovery diagnoses any
  unresolved spaceless `retained:` note mentioning `<id>`'s suffix or falling within a
  matching identity's blast radius (`code: "malformed_record"`, `detail: "missing space
  after 'retained:'"`, quarantining earlier provisional `IncomingItem`s in that blast
  radius). Append a valid `version: 1` `retained: <json>` successor note if needed
  (for example `state: "released"` or a re-homed `next_owner`) followed by a
  standalone `retention resolution: <json>` note via `tasks note` binding `bad.basis`
  and `bad.ordinal` (`CheckpointRequest.resolutions` targets spaced `retained: ` notes
  only); once verified, `incoming` clears the spaceless diagnostic and replays the
  spaced successor normally across unfiltered and `--host` queries.

Because `verify::preflight` strictly requires integer `version: 1` and refuses
pre-`Manifest 1` historical manifests (`unsupported historical manifest format;
preserve and migrate it explicitly`), adopt a pre-`Manifest 1` directory manifest
explicitly before running `checkpoint` or `transfer`:

1. Keep the historical manifest file untouched, generate a canonical `Manifest 1`
   JSON file (`{"version":1,"entries":[...]}`) outside the inventoried container,
   compute its SHA-256 `<sha>`, and attach it to the owning task (`tasks attach
   <id> <tmp-manifest> --name retained-manifest-<sha>.json`, adding `--multipart`
   when over `[attachments] max_bytes` with `multipart_validators` configured; or
   place it under `$WORK_ROOT/<project>/<task-id>/` on manual-route projects).
2. Append a valid `version: 1` `retained:` successor record naming `"sha256":
   "<sha>"` and `"manifest":
   "<records>/tasks/files/<id>/retained-manifest-<sha>.json"` (or the
   `$WORK_ROOT/...` path); if the prior `retained:` note had a diagnostic in
   `tasks retained state <id>`, append `retention resolution:` binding `"bad"` to
   `"successors": [<successor-ordinal>]` (or supply `resolutions` in
   `CheckpointRequest`), and confirm `tasks retained state <id>` reports
   `complete: true`.
3. Run `tasks retained checkpoint <id> --request <path>` or `tasks retained
   transfer <from> <to> --request <path>` using the refreshed `basis` from
   `tasks retained state`.

## Phase and checkpoint envelopes

Only lifecycle commands publish `retention operation: ` and JSON
`retention checkpoint: ` notes; generic notes cannot forge machine receipts.
Operation fields are version=1, key, phase, request (locator), input_bases, host,
at (UTC), observer (nullable), outputs and diagnostics. Phase-specific outputs:

| Phase | Exact outputs |
| --- | --- |
| intent | {} |
| items_published | items (positive distinct ordinals), artifacts, resolutions ({bad,successors,reason}) |
| receiver_published | items, artifacts, mapping |
| sender_released | items, receiver_basis, mapping |
| completed | outcome, publications, checkpoint (positive ordinal or null) |
| incomplete | attempt (positive), previous_phase, checkpoint (positive or null), action |
| reconcile_intent | directive (locator), reconciliation_id |
| reconcile_ready | directive, reconciliation_id, items, unresolved (actions) |
| reconciled | directive, reconciliation_id, ready_bases (Publications), unresolved |

Local phases start intent; checkpoint proceeds items_published to completed;
transfer receiver/sender publish on their respective tasks then complete. An
incomplete receipt retains retry state; reconciliation goes reconcile_intent →
reconcile_ready → reconciled. Tasks validate role, exact prior phase, same bound
request/inputs/host, referenced preceding v1 item ordinals and mapping state;
it is not legal to manufacture a successful sequence by appending plausible JSON.

Checkpoint fields are version=1, operation, observed_at, host, observer (nullable),
basis (nullable), scope, counts (nullable), changed, actions and complete. Scope
is `{mode:"all_known_kept_on_host",host,other_hosts:"outside_observation"}`.
Complete requires basis/counts; unknown items can coexist with complete accounting.
Incomplete has actions and no authoritative complete counts. A checkpoint covers
its exact item basis, not later filesystem edits or independent foreign ownership.

## Failure, retry and result interpretation

Mutation success has version, operation, outcome, publications, counts, actions,
metrics and warnings. Outcomes are verified, accounted_unknown, no_local_items,
transferred or reconciled. Only verified establishes current observed local kept
bytes; accounted_unknown records complete accounting with unresolved observations.
Incomplete publication is nonzero and retains reservation plus durable phase.

Retry reestablishes ownership and validates immutable intent/prior publications,
adopting only byte-identical interrupted attachments (including interrupted
multipart `.part-NNNN` and `.catalog.json` manifest members) before hooks.
Artifact batches validate all names, caps, and multipart validators before writing
any file, and roll back newly created files in reverse order on pre-save failure
while preserving pre-existing/reused evidence; if rollback unlink or directory
removal fails, `; cleanup needs attention: could not remove <path>: <cleanup>` is
appended without claiming a restored preimage. Exact committed
completion replays original bases/counts with a warning that current payloads
were not inspected. An uncommitted completion is preserved before a fresh inspection.
Conflicting bytes/branches/stale bases or missing history refuse; journals lost
locally do not authorize reconstructing success from current files.

Prefix and common-directory lock waits report bounded stderr diagnostics after
2 seconds and then at most every 30 seconds, with an acquired line after a shown
notice. Holder PID observations are advisory kernel snapshots; unavailable
observations and diagnostic failures never enter publication warnings or prevent
a target-only incomplete receipt. Locks retain their kernel blocking semantics.

Prefix locks span the operation, so same-project writers still wait during payload
inspection. Common-Git-directory commit locks cover only initial preflight and
individual publication windows, through exact committed readback. Payload inspection
and the immutable range precheck run outside those commit locks. Each window locks
the deduplicated repositories of its mutable participants: both transfer peers until
proved sender release, then only the missing receipt's target; incomplete receipts
and reconciliation intent/ready check only their target, while reconciliation's
terminal checks every participant. Mixed recovery batches take the union of their
phases. Fresh context, ownership, range delta and required working bytes are checked
under the guard; foreign staging, Git operations and dirty required evidence refuse.
Only unrelated appended ordinary notes can commute. Pending intent permits queries,
unrelated notes, normal claim refresh/start and park;
closure, archival and competing task/evidence changes wait for finish/reconcile.
Move/migrate still refuse tasks carrying `retention operation:`,
`retention resolution:`, or JSON `retention checkpoint:` envelopes, as well as
protected retained-op requests, retained-manifests (including `multipart:`
declarations and `.catalog.json` / `.part-NNNN` multipart members) and
retained-checkpoints. Project rename has the verified lineage route
below. Protected evidence (logical and multipart member names alike) cannot be
detached by `tasks detach` or generic `detached:` notes. No force bypass is
provided.

Metrics wall_ns measures the operation's elapsed interval, including prefix-lock
waits and payload inspection; it is not the sum of commit-lock hold intervals.
Metrics hash_ns/bytes_hashed count cumulative
thread SHA256 work, including repeated verification/archive/recovery; history_bytes_read
counts repeated task/archive/metadata reads. Git private I/O is excluded and
channels can overlap. Count CLI calls and actual UTF-8 stdout separately. No
performance or distributed exactly-once guarantee follows. Missing/corrupt saved
objects after reboot fail visibly; sync and ordinary storage failure limits remain.

## Project rename and historical locators

A project with retained history may be renamed after every operation has completed
or its reconciliation has a verified terminal receipt. Rename requires a clean,
Git-backed source, including tasks/.config.toml, and verifies immutable requests,
checkpoints, manifests, archived notes and preserved logical prefixes before intent.
It freezes a [retention lineage](configuration.md) edge and target config bytes
in the existing rename inventory. It preserves old envelopes, ordinals, task
suffixes, hashes, retained refs and original Git paths byte for byte.

An unresolved diagnostic on a `retention operation:`, machine
`retention checkpoint:`, or `retention resolution:` envelope refuses rename, and
fails post-rename `tasks check` for the renamed source history; an operation
citing a malformed `retained:` ordinal is such a diagnostic, and so is an item
note of an unsupported version. A malformed plain `retained:` item note does not
block: rename carries the note unchanged, so `tasks retained state` reports the
same diagnostic, still unresolved, under the new ID.

Current state queries return the current task ID/basis. An explicit historical
`--at` returns the ID and exact basis from that revision, even when requested
through the current ID. Nested historical envelopes remain unchanged. Readers
verify every intervening edge and original blob (or multipart manifest member set
through `ReadContext`) at its declared path/ref; missing proof is a recovery
error. Publication lookup remains in the original epoch.

New operations require current canonical task IDs and participant-fresh input bases.
Custody matching accepts an old next_owner only through verified lineage. Rename
alone does not rewrite item records or move payloads. A recorded generated manifest
(single-file or multipart `retained-manifest-<sha256>.json`) inside a renamed task
attachment directory resolves to its moved attachment only with the original
observation, ledger/multipart declaration, lineage and exact digest verified. External
manual manifest paths retain their literal resolution; arbitrary host-root
relocations are not inferred. A detached rename that would relocate the records
Git root itself refuses before intent and needs an explicit relocation route. A subsequent observation publishes any changed
manifest locator through the ordinary checkpoint/transfer procedure.

Replaying an unchanged completed pre-rename request with current CLI participants
returns its original publication references/counts and the existing warning that
current bytes were not inspected. It still needs current owned claims and locks;
a changed request, absent terminal or unproved participant refuses. No new intent,
observation or payload read is authorized by that replay.

`tasks check` verifies historical sources at their anchored commits and compares
the destination with live config/history, so pre-commit checks work before the
rename commit exists. After committing, normal retained reads verify the committed
current history as well. Adoption verifies synced lineage before host state changes.
Upgrade all participating readers before a writer creates lineage config; keep
one coordinated writer through rename, commit and secondary-host adoption. Preserve
all named refs and cutover commits while their retention evidence is needed.

## Migrated in-tree history

An explicit [recovery declaration](configuration.md#project-settings) can make an
already imported in-tree history readable through its current detached project.
The owning project stays paired to destination records. Exact historical Bases
select an independent source read context by declared project/task/layout/ref;
equal files or object IDs in another repository do not select it. Task/archive,
request/artifact and publication reads use that context without rewriting nested
Bases or output schemas.

Source publication lookup runs on the original first-parent history through the
mapped frozen endpoint, preserving the original publication commit. Disposable
original branch names may be absent; `source_ref` must still retain the cutover and
its endpoints. Keep all required source history after activation. A declaration
or matching import trees does not replace complete immutable phase proofs.

Within one OperationKey, each participant's input, publication and reconciliation
operands stay in one repository epoch. A mixed operation refuses before its Git
comparison. No ancestry bridge across migration is inferred. Source-era
nonterminal operations cannot resume at the destination. New checkpoints and
transfers use current detached inputs/publications and verify recovered
predecessors as separate source operations. Freshness and cache identity include
the complete declaration; changing it invalidates an in-flight operation.

`state --at` resolves its revision only in destination records. Rewritten revisions
before the declared import refuse. To inspect an original source cutover directly,
use the isolated source-snapshot recovery procedure; do not repoint live records.
Historical readability grants no custody, payload verification or disposal.

A manifest binding names one exact source-cutover retained note ordinal. It proves
the old attachment string, ledger before observation, original blob/digest and
import equality; generated manifests additionally require their original
observation binding. A manually named attached manifest needs its prior attachment
provenance, not an invented generation phase. Before returning a current path,
tasks checks current ledger, regular-file storage and exact bytes. Missing or
changed files refuse. External manual manifests retain literal paths, even with
equal digests. No suffix-only search, equal-content fallback or historical-note
rewrite is performed. A new source-style manifest reference after cutover is
uncovered and refuses; a legitimate successor observation can publish a current
manifest path. Payloads stay in place and prior-version preservation still applies.

Routine `tasks check` validates the static import/config/tree boundary and exact
source prefixes once per project invocation, loading each source prefix at most
once and reusing check's live logical tasks. Attachment equality compares tree
objects. This path adds zero full history, terminal-proof or publication-discovery
walks; ordinary attachment/storage checks remain. Errors use the existing finding
shape with kind `retention_recovery`. State and lifecycle commands retain full
proof verification. Check success is partial metadata validation, not a proof
replay or current payload audit.

Before live activation, freeze complete proposed config files and preimages,
rehearse those exact bytes in a throwaway records clone with actual sources
read-only, and compare original publications/ordinals/diagnostics. Measure check
cost on paired identical isolated inputs with baseline and candidate binaries;
that acceptance evidence belongs to the rollout, not to routine check output.
Activation and installation need separate explicit authority. If the live config
preimage changes, preserve/reconcile it and rehearse replacement bytes. Authorized
rollback restores preserved config bytes and never deletes source history.
Older readers ignore the table and stay blocked at the existing layout boundary;
use the updated writer and preserve anchors throughout rollout.
