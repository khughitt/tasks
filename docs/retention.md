# Retained lifecycle

Type: protocol, data model; document types 1.

## Participants and boundary

The open owning task/session declares custody, the current host observes bytes,
and tasks publishes pinned records and operation phases in Git. A receiving task
accepts a transfer; a coordinator finishes or reconciles an operation. Lore owns
the operating policy for what to keep, handoff agreement, session integration
windows and disposal. This document defines the shipped command/data contract;
structural acceptance alone establishes no agreement or safe deletion verdict.

Use one coordinated writer host for every participating task until completion/
reconciliation commits and syncs. Start and hold the tasks first; mutations need
Git records with git.commit=true, exact input bases and ordinary ownership. Local
locks/heartbeats do not exclude remote writers or create atomic directory snapshots.
No payload copying, relocation, deletion or automatic closure is implemented here.

## Retention records and manifests

Notes beginning `retained: ` contain strict JSON. New records use integer version
1; the history reader explicitly supports unversioned predecessors without rewriting
them. Duplicate keys, unknown fields/versions and wrong types fail; unsupported
history is never silently interpreted as a current subset. Identity within a task
is exactly host/kind/path. Path spelling remains stable; tokens are data, not shell
commands.

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
for evidence directories). Unknown data uses null current sha256 and a concrete
action; unknown directories name the last located manifest or null. Unknown
worktrees still omit digest/manifest. Released records use verified, owner/trigger
none and omit digest/manifest/handoff/action; prior evidence remains in history.
Open-owner and real handoff/disposition rules require policy checks beyond JSON.

Manifest 1 is `{version:1,entries:[...]}`; members have path and type file
(sha256), directory, or symlink (literal target). Paths are normalized relative
components: no absolute/drive prefix, backslash, empty, dot or dot-dot components.
Include hidden files, empty directories and literal links without following them.
Sort by path and hash exact saved UTF-8 bytes. Duplicate paths, special/unreadable/
unstable members or incomplete enumeration fail semantic verification. A manifest
does not retain symlink target bytes or reconstruct missing member versions.

Keep each manifest outside its inventoried container, with retrievable exact old
members. Independent child identities retain their own owners/dispositions even
when included in an enclosing manifest. Do not exclude metadata to fit a cap.
Attachment/layout refusal keeps raw evidence and an incomplete accounting action;
it does not authorize a guessed storage root or loss of the only copy.

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
| Resolution | bad (Source), declarations (complete correction), reason. |
| Publication | task, phase, basis. |
| Mapping | sender Identity, receiver Identity. |
| Counts | kept_verified, kept_unknown, released (nonnegative integer counts). |

Attachment basis may be omitted only inside its own task's committed note: it
pins to that containing publication. Foreign locators and preservation artifacts
in requests require explicit bases, including directory manifests. This prevents
self-referential commit hashes without weakening immutable retrieval. A basis
must match actual task/project/layout and reachable retained branch, not merely
pass field-shape checks. Keep named refs and required Git objects available.

## Query state

`retained state <id> [--at <revision>] [--host <host>]` reads one immutable commit
and returns version 1, contract retention-1, task, basis, host_filter, complete,
items, checkpoints, operations, diagnostics and warnings. Default HEAD refuses
dirty required task/archive inputs; explicit --at uses only its pinned blobs.
No implicit fetch/push/provider occurs. Historical interpretation is not fresh
filesystem verification.

Each item has identity, nullable effective record/source, observed_at, nullable
checkpoint Source and coverage recorded/changed/unknown/blocked. Checkpoint entries
have source and nullable note/text; operation entries source/note. Diagnostics
have code, scope (identity/host/task), nullable host/identity/source, detail, action
and optional resolved_by Source. A malformed later note quarantines the identity,
host or whole task it could supersede; effective record becomes null, complete
false. A host filter never hides task-wide uncertainty or falls back to older
valid state. Interpretable incomplete state can exit 0; unreadable/corrupt required
bases fail. Resolve the exact bad source, not an assumed latest equivalent.

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

Resolutions bind exact bad basis/ordinal with complete declarations and reason,
covering its blocked identity or all known identities in host/task scope. A scope
spanning other hosts needs explicit history recovery. Native checkpoints publish
changed observations/artifacts, verify that commit, recheck history, then publish
checkpoint plus completed receipt. No kept local items yields no_local_items
without an empty checkpoint. The command neither parks nor closes the task.

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
payloads remain kept/unknown, malformed history stays blocked. It observes effective
history rather than applying abandoned replacements/corrections; use a fresh
checkpoint for repair. Reconciled means neither transferred nor host-checkpointed.
Counts describe interpretable ready-basis history; actions preserve blocked scopes.
Terminal replay checks immutable directives/publications without rehashing current
payloads; a new observation needs fresh bases.

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
adopting only byte-identical interrupted attachments before hooks. Exact committed
completion replays original bases/counts with a warning that current payloads
were not inspected. An uncommitted completion is preserved before a fresh inspection.
Conflicting bytes/branches/stale bases or missing history refuse; journals lost
locally do not authorize reconstructing success from current files.

Prefix locks and common-Git-directory commit locks stay held through observation
and immediate publication; foreign staging, Git operations and dirty required
evidence refuse. Only unrelated appended ordinary notes can commute. Pending
intent permits queries, unrelated notes, normal claim refresh/start and park;
closure, archival and competing task/evidence changes wait for finish/reconcile.
Move/migrate/rename refuse operation history and protected retained-op requests,
retained-manifests/checkpoints until locator migration exists. Protected evidence
cannot be detached. No force bypass is provided.

Metrics wall_ns measures held-lock duration; hash_ns/bytes_hashed count cumulative
thread SHA256 work, including repeated verification/archive/recovery; history_bytes_read
counts repeated task/archive/metadata reads. Git private I/O is excluded and
channels can overlap. Count CLI calls and actual UTF-8 stdout separately. No
performance or distributed exactly-once guarantee follows. Missing/corrupt saved
objects after reboot fail visibly; sync and ordinary storage failure limits remain.
