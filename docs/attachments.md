# Attachments and artifact integrity

Type: protocol, data model; document types 1.
The ordinary ledger/protocol is currently unversioned; accepted multipart catalogs,
note-archive descriptors and eligibility responses are version 1. This defines
artifact storage, publication and recovery alongside the [task model](task-model.md)
and [retained custody](retention.md).

## Participants and ordinary storage

The task owns evidence through its logical note ledger; the CLI validates storage
and publishes records/files together; the operator checks publication before
relying on portability. Storage is `tasks/files/<id>/<name>` under project records.
The parent and per-task directories must be real directories, never symlinks.
Names are 1–128 ASCII bytes of letters, digits, dot, underscore or hyphen, with no
leading dot. Ordinary files default to a 2 MiB cap; [configuration](configuration.md)
defines overrides and multipart admission limits.

`tasks attach <id> <path>...` stores each file separately, with a ledger note
`attached: <name> (<n> bytes)` and optional caption. All names, per-file sizes,
duplicate basenames, collisions and unsafe storage are checked before any write.
One batch saves/commits once. Caption applies to every source. --name, stdin `-`
and --clipboard require a single source; clipboard uses the host clipboard tool.
JSON always has `files: [{name,path,bytes}, ...]`; pretty prints paths in input order.
There is no singular file object contract.

`detach <id> <name> "<why>"` records `detached: <name>: <why>` and removes the file.
The latest matching task ledger event establishes ownership. Show lists attached
files; feedback does not carry attachments. Write/save failure rolls back files
created by that ordinary attach, preserving pre-existing files. After a process
interruption an unnoted file can be adopted by the same-byte attach with --name,
or explicitly detached. Retry interrupted ordinary detach to finish it.

## Eligibility and audit

`tasks -C <project> attachments <path>...` is a read-only protocol for 1–128 unique
literal paths `tasks/files/<id>/<name>`. It consults only that directory's config,
requested owner records and files: no registry search, other checkout or provider.
Closed tasks and retired projects can own eligible evidence. No claims are written.

Version-1 JSON reports canonical records directory, prefix, configuration SHA256
and ordered rows `{path,eligible,problems}`. Eligible rows also have record_sha256
and file_sha256 with empty problems. Ineligible rows omit both hashes and report
audit kinds/severities. Attached ledger, valid name, safe regular storage and cap
are required; below the project directory symlinks are not followed. Exit 0 means
a complete answer including negative rows. Malformed/unreadable config/records,
I/O failures or changes during reading fail with no usable eligibility result.
Payload bytes and task text are never included.

Check errors are attachment_unsafe, attachment_orphan, attachment_unnoted and
attachment_missing; attachment_invalid and attachment_too_large are warnings.
A negative audit does not authorize deleting the only evidence copy.

## Multipart text and catalog version 1

`attach <id> <path> --multipart` accepts one UTF-8 document; stdin requires --name.
Clipboard and batches are refused. JSON logical names require valid JSON, with
domain checks supplied by configured validators. No binary/compression route is
provided. Cuts prefer newlines, otherwise UTF-8 boundaries, preserving exact bytes.
Logical names use ordinary grammar with a 115-byte limit.

Each part `<name>.part-0001`, etc., and `<name>.catalog.json` obeys the per-file
cap. The deterministic catalog has exactly `version: 1`, `name`, `bytes`, `sha256`
and ordered `parts`; each part is `{name,bytes,sha256}` with consecutive generated
names. Digests are 64 lowercase SHA256 hex characters. Parsing rejects unknown
fields, unsupported version, invalid names/hashes and invalid part count; rebuild
also validates sizes, ordering, member and whole hashes. A catalog alone cannot
recover absent parts.

The record owns every member through attached notes and declares the whole:

```text
multipart: <name> (<bytes> bytes, <n> parts, sha256 <whole-hex>, catalog sha256 <catalog-hex>)[: <caption>]
```

Show/next expose files with member_of plus logical multipart entries. `files:<name>`
references the whole, which has no whole-file storage path. Individual members
cannot be detached or used as document refs. Check adds multipart_broken and
multipart_leftover errors, and multipart_removal_unpublished warning.

Default admission limits are 16 MiB and 16 parts. Zero multipart_max_bytes disables
new admissions. Absent multipart_validators refuses admission; [] explicitly
declares no extra gates. Fixed recovery ceilings are 256 MiB and 256 parts;
configuration above them fails. Lower admission limits do not destroy recoverability.

## Publication, reservations and removal

Multipart attach returns multipart, validation and publication. Published with
commit means exact committed declaration/catalog/parts rebuilt and passed current
validation. Local with cause/next means bytes landed but portability is unproven.
Both exit 0. Causes include commit_disabled, commit_deferred, commit_failed,
commit_unavailable, verify_failed, validation_failed and validation_unavailable.
Resolve the cause, tasks commit if needed, then verify the named commit explicitly.

Detach by logical name removes the set, recording member detach notes before the
logical one. Retry the same command after interruption. Names remain reserved
until removal is committed and local old-owned members are gone. A verified
published removal may still say reuse blocked when a hook recreated local members;
follow publication.next for cleanup. Later legitimate ordinary files using released
names are preserved, not treated as old leftovers automatically.

## Export and current validation

```sh
tasks export <id> <name> -o <destination>
tasks export <id> <name> --at <commit> -o -
tasks export <id> <name> --at <commit> --verify
```

Ordinary and multipart export read working files by default, or one resolved Git
commit for record, archives, catalog and parts. Integrity checks finish before
writing. File output is atomic; stdout is exact bytes despite format/color flags.
No reconstructed whole file is retained. Recovery requires byte integrity, not
current admission limits/validators. A clean clone can recover committed bytes
without original scratch.

Verify writes no file and reports integrity plus current validation. Validation
has basis current_runtime, passed, config_sha256 and validator runs. Each run has
match, command, optional program_sha256 (inside records Git top level) or
program external, exit (possibly null), outcome, cleaned/survivors PIDs and stderr.
Inspect passed and outcomes: exit 0 alone is not a validation verdict and current
checks do not establish historical schema validity.

## Multipart validators

Each config entry has match (fnmatch-style glob) and nonempty command argv.
Matching validators receive whole bytes on stdin; {name} is the only argument
substitution. They run at the records Git top level (else records directory),
with PATH, temporary HOME, LANG and TASKS_ATTACHMENT_NAME only, a 120-second
timeout and bounded captured output (64 KiB). The selected executable is resolved,
identified and executed by the same path. A broken earlier PATH candidate makes
validation unavailable; it does not fall through to a different executable.

Validators must be self-contained. Processes launched through service managers,
D-Bus or other daemons lie outside containment. Uncertain cleanup makes validation
unavailable and stops subsequent validators. Upgrade every reader before committing
multipart config and attaching; install the merged CLI before activating matching
shared skills/settings. Ordinary evidence publication does not grant host actions.

## Note archives

`archive-notes <id>` previews source_sha256, archive_name/bytes, note_count,
record_bytes_before/after, eligible and reasons without writing. Empty suffix,
no saving, unsafe/conflicting storage or over-cap segment is ineligible; corrupt
history fails. Shared apply needs all readers/consumers upgraded and one coordinated
writer host. That rollout is tracked separately; normal guidance remains preview
only until its activation gate is satisfied.

Technical apply is `archive-notes <id> --expect <source_sha256>` from the exact
preview. No force bypass exists. Foreign held claims refuse; closed/shelved status
is preserved. The source recheck is local, not distributed compare-and-swap.
The exact Notes-heading-through-EOF bytes, including CRLF/whitespace/provenance,
become notes-<sha>.notes with ordered v1 descriptors described in
[task model](task-model.md#references-and-notes). Apply adds attached and
`archived-notes: v1 source=<hash> segment=<hash> count=<n>` receipts.

Same-source retry returns already_archived without duplicate receipts, even after
later notes/archives. Apply reports actual sizes/hash/name/action/warnings. Before
record switch, original is authoritative; only a byte-identical orphan is reusable.
Uncertain save preserves bytes. After switch commit warnings leave landed state;
retry can finish publication. Ordinary saves/editors use the compact record while
logical show still exposes all notes.

Apply proves its own result and reports it as `verified`: `invariants` names each
check that held, in order, and `written` gives the absolute record and segment
paths. A failed check refuses with `archive_unverified`. The checks are:

- `notes_appended`: the new logical notes are the old notes, unchanged and in
  order, plus exactly the `attached:` note and the `archived-notes:` receipt by
  one author (so the attachment ledger gains only the segment).
- `fields_unchanged`: every other field is equal except `updated` and the one
  added descriptor.
- `files_unchanged`: just before the switch, the task's attachment directory
  holds the same entries as before (regular files by content hash, symlinks by
  target, other entries by type) plus the segment. Apply holds the project
  mutation lock throughout, so this catches writers that bypass it.
- `record_reloaded`: after the switch, the record read back through the normal
  loader equals the prepared record and binds its receipt.

The first two run before anything is written. A `files_unchanged` failure keeps
the published segment for an exact-bytes retry, and the record is not switched.
`written` names the segment even when a retry reused a byte-identical one. A
`record_reloaded` failure leaves the record switched and uncommitted; a retry would
report `already_archived`, so inspect the record against the source preview first. `already_archived` reports
`receipt_bound` (the receipt sits at its descriptor's exact boundary after its
attach note) and `segments_verified` (each segment matched its hash and note count
on load), and `written` is empty.

## Failure and recovery limits

Missing/unsafe/hash-mismatched archives fail reads/saves: restore descriptor-exact
bytes from verified copy or pinned Git revision. Never skip, sanitize or regenerate
history. Historical bases require their segments, not today's. Downgrade requires
a verified pre-archive record with later history preserved separately; no automatic
undo exists. Moves retain segment copies at both ids, rename preserves bytes, and
required archives/protected retained-operation artifacts cannot be detached.

Archival does not reduce total evidence/full-show cost. Per-file cap/layout checks
still apply, with no multipart/compression/off-tree bypass for note segments.
Body-dominant records may still warn; preserve exact body bytes before editing.
Multipart published/local and removal reuse verdicts must be read independently.
Local atomic writes and rollback do not establish cross-host transactions.
