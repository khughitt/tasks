# Task records

Type: data model; document types 1.
The task format is currently unversioned; note-archive descriptors accept version 1.
Related contracts are [configuration](configuration.md), [ownership](ownership.md)
and [artifact integrity](attachments.md).

This is the current persisted and logical task contract. Records are Markdown
with strict frontmatter, body, references and append-only notes. The task file
format has no explicit version field. New keys require reader rollout before
they sync: older parsers reject unknown keys. Versioned note-archive descriptors,
multipart catalogs and retained records have their own contracts.

## Identity and physical format

An id is `<prefix>-<hex6>`. Prefixes match `[a-z][a-z0-9]{1,7}`; the suffix is
exactly six lowercase hex digits. The filename is `<id>.md` and must agree with
the record id. Prefix rename and task move use explicit protocols; ordinary
editing cannot change identity. [Project operations](project-operations.md)
defines the in-tree, linked and detached storage layouts.

Frontmatter opens with `---` on the first line and closes with a line `---`.
This parser accepts a strict subset, not general YAML: known lowercase/underscore
keys, colon-space separation, single-line scalars and inline lists. Lists use
comma-space separators without trailing commas. Double-quoted scalars escape
only `\"` and `\\`; reserved punctuation must be quoted. Empty strings use `""`.
RFC 3339 timestamps are supported as written by the serializer. Unknown or
duplicate keys, wrong value types and malformed delimiters fail parsing.

The required keys are `id`, `title`, `status`, `priority`, `created`, `updated`,
`depends` and `tags`; the last two are present even when `[]`. The following
table covers every accepted task frontmatter key. Optional lists default empty,
optional booleans false, and other optional values absent.

| Field | Value and meaning |
| --- | --- |
| `id`, `title` | Immutable canonical id; nonempty single-line title. |
| `status` | `idea`, `todo`, `doing`, `blocked`, `shelved`, `done`, `dropped`, `moved`. |
| `priority` | Integer 0–4, lower is more urgent; creation default 2. |
| `size`, `complexity`, `process` | Optional `xs/s/m/l/xl`, `low/mid/high`, `direct/planned`; absent means unassessed. |
| `parallel`, `lane` | Advisory parallel assessment; goal defining a non-nested lane. |
| `reviewed`, `awaiting_user`, `reviewed_at` | Idea review flag, explicit user wait and UTC review checkpoint. |
| `needs` | Declared project resource names, lowercase letters/digits/hyphens, no leading hyphen. |
| `every`, `defer`, `wake_on` | Recurrence interval, one-shot UTC calendar date, independent shelf wake target ids. |
| `note_archives` | Ordered immutable note-segment descriptors; omitted from logical Task JSON. |
| `owner` | Advisory tracked owner, `[A-Za-z0-9._/@+-]+`; active session ownership is outside git. |
| `created`, `updated` | Immutable creation and monotonic write timestamps, RFC 3339 UTC seconds. |
| `started` | First transition into doing; preserved on resume and reopen. |
| `completed` | Latest completion; cleared when reopened. |
| `closed` | Done/drop closure checkpoint for wake decisions; cleared on reopen, not a moved redirect's closure. |
| `last_done` | Recurring cycle anchor, preserved when reopening. |
| `depends`, `parent` | Execution dependency ids (foreign allowed); optional same-project containment parent. |
| `related` | Non-blocking ids of tasks the record cites; foreign allowed; never an execution, review or containment edge. Written by add/edit --related, edit --rm-related and edit --no-related. |
| `tags` | Grouping labels; optional vocabulary documents their meanings. |
| `follows` | Optional ordered task-id list for additional work continuing earlier work; foreign ids allowed. Explicit add/edit writers append; edit removes or clears, and editor changes validate. No inference or historical conversion. Never gates execution or review. |
| `pieces` | Foreign tasks that are part of this goal; each is also in `depends`. |
| `source` | Opaque single-line origin reference; never interpreted for routing, filtering or rewriting. `show` and `next` display a source that is exactly one task id with a registered prefix as a related task with its live status and `updated` timestamp; that display reads the record and changes nothing. |
| `model`, `agent` | Opaque single-line latest completion model and filing harness/model stamps. |
| `origin` | Current judgment `user`, `agent`, `mixed`; absent means unknown. |
| `spec`, `plan`, `step` | Working-document paths and optional exact plan heading; step requires plan. |
| `moved_to`, `moved_from` | Cross-project successor/predecessor ids maintained by task move. |

The body follows the frontmatter. Exact lines `## References` and `## Notes` are
reserved delimiters and cannot occur in body text. There is at most one of each,
references precede notes, and reference sections contain only their entry bullets.
The serializer's first structural newline and trailing serialization whitespace
are not body content; other leading whitespace is retained.

## Lifecycle

Open states are idea, todo, doing, blocked and shelved. Done and dropped are
closed and satisfy dependencies. A moved record redirects resolution to its
successor, possibly through a chain; unresolved/cyclic chains are not completion.

Open states can transition to other open states or close subject to command
guards. Closed nonrecurring tasks reopen only to todo. Start can take a done recurrence
directly to doing for a new occurrence, including an early run. Moved records are immutable. `done`
requires closed dependencies and descendants unless forced; `drop` requires
closed descendants with no force override. Shelved entry requires `shelve`,
and move entry requires `move`; generic editing cannot manufacture them.

The write paths validate parent/dependency cycles, same-project parents,
non-nested lanes, needs declarations, recurrence and review invariants.
`every` accepts positive canonical `Nd`/`Nw` intervals through 36,500 days.
Recurring goals are refused; `every` and `defer` coexist only on a shelf, where
the date is a required review checkpoint. `defer` is strict
`YYYY-MM-DD`, due from that day's start in UTC; setting it requires a future
date. Status transitions clear it; shelve then writes the new required checkpoint
and wake targets. Shelf wake edges are independent from depends. A wake target leaves
`wake_on` only into `related`. Unshelve returns
to unreviewed idea; ordinary list/search retain shelves while pickers omit them.
Pickers and derived review states are explained in the [user guide](user-guide.md).

Task timestamps are whole-second UTC stamps, not unique event ordinals. Writers
keep updated monotonic across saved versions. Do not infer event order from a
strict timestamp comparison alone or equate task age with blocking duration.

## References and notes

Logical `refs` entries contain `handle`, `relation` (`context`, `input`, `output`),
`required`, optional `kind`, `reference` and optional `description`. Handles are
unique per task. The [document reference grammar](documents.md#task-references)
defines physical entries, resolution, drift and reader failure behavior.

Inline notes are bullets `- <RFC3339> (<by>): <text>`. New text must be one
nonempty line; trailing spaces/tabs are removed, leading and inner whitespace
retained. A following indented `provenance: {JSON}` line may carry the keys below;
unknown keys, an empty object or an incomplete harness pair are invalid.
Existing note text is preserved by ordinary writes.

Logical notes are the ordered archive segments followed by the active inline
suffix. A descriptor `v1:<64 lowercase SHA256 hex>:<positive count>` resolves only
to `tasks/files/<id>/notes-<sha>.notes`. Exact bytes from the earlier Notes heading
through EOF, count, grammar, provenance and live attachment ledger must validate.
Missing/corrupt/unsafe segments fail the read; a partial Task is never returned.
Physical parsing alone performs no filesystem IO. Ordinary saves preserve archived
prefixes and write only the active suffix. [Attachments](attachments.md#note-archives)
covers preview, apply, retry and coordinated reader rollout.

Lifecycle commands append human-readable notes such as started/resumed, parked,
done, dropped, and recurring completion/next-due receipts. Notes have no separate
event-kind discriminator: a user's identical text is not distinguishable by text
alone. A creation note is identified structurally at the creation timestamp;
provenance proves a reported session source, not that a note is a lifecycle event.

Writers link newly cited tasks into `related` automatically on write (decision D1: R1).
Text read: the body (`add`, `edit --body`, editor saves), plus writer-appended notes (`note`,
`park` next step, `shelve` wake condition, and closure messages for `done`, `drop`, `block`).
Tool-generated notes (move, migrate, rename, receipts, start/resume, takeover) and `feedback`
never link citations. A citation is a maximal token matching `[a-z][a-z0-9]{1,7}-[0-9a-f]{6}`
whose prefix the registry knows. Only new citations count: every id in an appended note
counts, and only ids absent from the old body count. Each new citation resolves to its live
destination across move chains (`tombstone::follow`). Unresolvable citations warn
`<task> cites <id>, which resolves to no task here; not linked`. Citations are skipped without
warning if the cited id or its destination matches the record itself, a child, an id already
in `related`, or an id held in `depends`, `parent`, `wake_on`, `follows`, `moved_to`, `moved_from`, a
task-id `source`, or removed in the same write. Explicit related changes (`--rm-related`,
`--no-related`, editor changes to `related`) take precedence over automatic linking. While a
retained operation is pending on the record, automatic linking is deferred with a warning
naming the unlinked ids.

## Origin and provenance

`origin` is whose ask a record carries, not who typed it: user, agent or both.
The filer supplies evidence-based origin; the tool does not infer it from stamps,
source, tags, parent or session presence. A decomposition child of a user-requested
goal is `agent` unless the user asked for that piece; its parent link already leads
to the user's ask. Frontmatter is the current judgment.
Occurrence notes (creation and feedback recurrence) preserve their own origin and
optional `origin_session` forever.

When frontmatter origin is present it supersedes all occurrence labels (labels
written before or after a correction are history; a later recurrence still updates
the field by the combine rule). When
absent, origin is unknown; an occurrence labelled user/mixed is partial evidence
of user words. Recurrence with different known values combines them to mixed;
an unknown current field stays unknown and warns when a known occurrence arrives.
`edit --origin` corrects the current field; `--no-origin` withdraws it so
occurrence labels speak again without rewriting notes. Add a stamped evidence
note for a correction.

The optional native pair is `harness_session` plus `harness_session_source`:
`claude-code:<id>` with `CLAUDE_CODE_SESSION_ID`, `codex:<id>` with
`CODEX_THREAD_ID`/`CODEX_SESSION_ID`, or `agy:<id>` with
`ANTIGRAVITY_CONVERSATION_ID`. Native ids must be nonempty and control-free.
Codex thread takes precedence over session, and `ANTIGRAVITY_CONVERSATION_ID`
is read only when neither Claude Code nor Codex is set; conflicting Claude/Codex
variables or a contradictory native-prefixed TASKS_SESSION refuse provenance.
Arbitrary claim identities do not create native provenance. Occurrence
`origin_session` accepts these same three key prefixes (`claude-code:`, `codex:`,
`agy:`). Optional `root_session` is a codex key only with a CODEX_THREAD_ID
harness pair; a differing CODEX_SESSION_ID supplies it. Relay claim support does
not add other provenance source strings to this format.

## JSON and errors

Task JSON exposes complete logical `body`, `refs` and `notes`, not physical
archive descriptors. Unset optional fields and empty Task lists are omitted;
false booleans, zero numbers and empty body remain. Response containers such as
`tasks` and `warnings` remain arrays, and an empty `next` result is explicit null.
Summary/park/goal rows are projections, not interchangeable full Task objects.
Resolved relationship entries on `show` and `next` (`parent`, `children`,
`depends_on`, `dependents`, `related_to`, `follows_tasks`, `followed_by`, `wake_targets` and
`source_task`) carry `id`,
`title`, `status` and `updated` (plus `resolved` on `depends_on`, `related_to`, `follows_tasks`,
`wake_targets` and `source_task`, where unresolved entries omit `title`,
`status` and `updated`).
`dependents` lists every task whose `depends` names this one under any spelling of
its id (a retired prefix counts), open and closed, in ready order, deduplicated by
canonical id and excluding moved tombstones. Matching is literal: it follows no move
chain. Plain `show` reads its own project's scan; `show --all-projects` adds every
other reachable registered project (its own records stay the copy already read) and
carries unreachable-project warnings; `next` uses its requested scope. The same
widened scan feeds `followed_by`; no other field changes. A missing field means none
found in that readable scope, not none elsewhere.
Additive output fields are allowed; clients should ignore unknown output fields.

JSON success goes to stdout; a clean `tasks check` is silent with exit 0.
Document/export bodies and version/help/completion output have their own formats.
Application failures exit 1 with a typed error;
CLI usage failures exit 2. Warnings on success must not be discarded. Parse
errors, missing archive history and invalid configuration never become empty
successful results. Pretty output is presentation and may change.

When adding a persisted field, update parser/serializer/validation and round-trip
fixtures, all output projections, CLI and completion surface, shipped skills,
current references, migration/read compatibility and the relevant tests together.
Install the updated binary on every host that reads synced records before records
carrying new keys sync; verify one host end to end, and the other hosts updated
by the same install path are covered.

## Follows links

`follows` means this task contains additional work arising from named earlier tasks,
without asserting closure, prerequisites or replacement. Existing `source` (filing
origin) and `related` (informational citation) values remain unchanged. No command
infers or backfills follows. Empty lists are omitted from frontmatter and full Task
JSON; summary rows do not gain the field. Parsing checks id syntax only.

Show and next preserve stored outgoing order and duplicates in `follows_tasks`.
Missing targets remain unresolved with warnings; moved targets display the stored
tombstone. The optional `followed_by` rows (id/title/status/updated) discover direct
follow-ups, including open, done and dropped work, excluding moved citing records.
They deduplicate by canonical citing id and use ready ordering, as children do.
Plain show scans locally; `show --all-projects` widens the scan to every reachable
registered project; next uses its existing requested local/group/all-project scope. A missing field means none found in that readable scope, not none elsewhere.

For an ordinary focal task, inverse matching compares canonical ids without
looking up follows targets. For moved focal work, the view builds its identity
from its own reciprocal move chain, preferring captured records and visiting each
history id at most once. Missing, inconsistent or looping history gives one
incomplete-coverage warning; direct and verified-history matches remain visible.
No task in the focal identity set becomes its own follower. Unrelated broken
follows never produces inverse warnings or per-edge lookups. Queries for old and
new identities agree over the same complete scan/history; local results can differ
after a cross-project move, so use `show --all-projects` there. Source and stored
inbound follows remain unchanged.

Check warns `self_follows`, `duplicate_follows` (canonical aliases included), and
`dangling_follows` for missing local targets. Foreign targets are resolved on show,
not globally scanned by check. Tombstone follows produces the `follows_status`
error. Unchanged lists, including semantic errors, survive unrelated editor saves
and non-editor writes. Explicit flags validate the resulting list, rejecting
canonical duplicates and self-links. Changed editor lists validate their entire
shape and resolve only new targets; unchanged unreachable ids remain preserved.
New missing or moved targets refuse. Add/edit `--follows REF` appends, edit
`--rm-follows REF` removes an existing canonical id, and `--no-follows` clears.
Clear conflicts with add/remove; the same id in add and remove refuses atomically.
Repeated additions deduplicate; stored additions warn unchanged.

Apply follows changes before citation linking: both held and same-write removed
follows ids suppress automatic related links by cited and resolved spelling.
Explicit related overlap remains allowed; existing related values are never pruned.
A later separate citation after removal may add related. Source/title idempotent
add reuse writes nothing, including supplied follows flags.

Prefix rename rewrites local follows ids. Move preserves the list on the successor
and clears the tombstone; inbound stored links remain unchanged. Migration remaps
carried targets and records `follows OLD carried as NEW`; uncarried targets retain
their references. Follows never acts as a migration prerequisite.

## Goal pieces

A goal is a record with children, a lane, or a record that declares `pieces`. `pieces`
lists foreign tasks that are part of the goal: every id is in another project (a
same-project piece is a child), every piece is also one of the record's `depends`, and
there are no self-links or canonical duplicates. A lane cannot carry pieces. Parsing
checks id syntax only; `check` diagnoses the rest. Empty lists are omitted from
frontmatter and full Task JSON. No command infers or backfills pieces.

Because a piece stays a dependency, readiness, `done`'s `open_dependencies` refusal and
blocker traversal treat it as before. A record that declares pieces is never offered by
`ready` or `next` and is not unassessed review work; it reaches closeout once it has no
open children and every dependency is satisfied. A goal still waiting on one of its own
pieces stays out of closeout silently; one held only by other prerequisites is named in
a warning. `dep --rm` of a piece also unmarks it; `edit --no-depends` that would strand
a piece and `edit --lane` on a record with pieces are refused.

A goal's members are every record reachable from it by repeatedly following `parent`
children (same project) and declared pieces (resolved through moves), excluding the goal
itself, so a foreign piece that is itself a goal contributes its own members. `goals`
counts progress, activity and attention over members, and counts as dependencies only
targets outside the membership. A piece no record answers, including one in an
unreachable or unregistered project, is not a member; as a dependency it is counted as
unresolved, never done or open. Members are read through the registry whatever the
requested scope.
