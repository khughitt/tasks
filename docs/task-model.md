# Task records

Type: data model; document types 1.

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
| `tags` | Grouping labels; optional vocabulary documents their meanings. |
| `source` | Opaque single-line origin reference; never interpreted for routing. |
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
guards. Closed tasks reopen only to todo. Moved records are immutable. `done`
requires closed dependencies and descendants unless forced; `drop` requires
closed descendants with no force override. Shelved entry requires `shelve`,
and move entry requires `move`; generic editing cannot manufacture them.

The write paths validate parent/dependency cycles, same-project parents,
non-nested lanes, needs declarations, recurrence and review invariants.
`every` accepts positive canonical `Nd`/`Nw` intervals through 36,500 days.
Recurring goals and `every` with `defer` are refused. `defer` is strict
`YYYY-MM-DD`, due from that day's start in UTC; setting it requires a future
date. Status transitions clear it. Shelf wake edges are independent from depends.
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

## Origin and provenance

`origin` is whose ask a record carries, not who typed it: user, agent or both.
The filer supplies evidence-based origin; the tool does not infer it from stamps,
source, tags, parent or session presence. Frontmatter is the current judgment.
Occurrence notes (creation and feedback recurrence) preserve their own origin and
optional `origin_session` forever.

When frontmatter origin is present it supersedes all occurrence labels. When
absent, origin is unknown; an occurrence labelled user/mixed is partial evidence
of user words. Recurrence with different known values combines them to mixed;
an unknown current field stays unknown and warns when a known occurrence arrives.
`edit --origin` corrects the current field; `--no-origin` withdraws it without
rewriting notes. Add a stamped evidence note for a correction.

The optional native pair is `harness_session` plus `harness_session_source`:
`claude-code:<id>` with `CLAUDE_CODE_SESSION_ID`, or `codex:<id>` with
`CODEX_THREAD_ID`/`CODEX_SESSION_ID`. Native ids must be nonempty and control-free.
Codex thread takes precedence over session; conflicting Claude/Codex variables
or a contradictory native-prefixed TASKS_SESSION refuse provenance. Arbitrary
claim identities do not create native provenance. Occurrence `origin_session`
accepts these same two key prefixes. Optional `root_session` is a codex key only
with a CODEX_THREAD_ID harness pair; a differing CODEX_SESSION_ID supplies it.
Relay claim support does not add other provenance source strings to this format.

## JSON and errors

Task JSON exposes complete logical `body`, `refs` and `notes`, not physical
archive descriptors. Unset optional fields and empty Task lists are omitted;
false booleans, zero numbers and empty body remain. Response containers such as
`tasks` and `warnings` remain arrays, and an empty `next` result is explicit null.
Summary/park/goal rows are projections, not interchangeable full Task objects.
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
