# Task model

Type: data model; document types 1.
The persisted task format and collection query model are currently unversioned;
note-archive descriptors accept version 1. Related contracts are the
[user guide](user-guide.md), [configuration](configuration.md),
[ownership](ownership.md), [project operations](project-operations.md),
[documents](documents.md), [artifact integrity](attachments.md) and
[retained custody](retention.md).

This reference defines both the persisted and logical single-record contract
(`tasks/<id>.md`) and the scoped collection model over checkouts, relations,
time and host-local claims. Records are Markdown with strict frontmatter, body,
references and append-only notes. Because the task file format has no explicit
version field, new keys require reader rollout before they sync: older parsers
reject unknown keys. Versioned note-archive descriptors, multipart catalogs and
retained records have their own contracts.

## Record and collection levels

One logical record (`tasks/<id>.md` plus its verified `tasks/files/<id>/notes-*.notes`
archive segments) answers what was authored on that item: its canonical `id`,
`title`, whether it is a live work item (`idea`, `todo`, `doing`, `blocked`,
`shelved`, `done`, `dropped`) or an immutable redirect (`moved` with
cross-project `moved_to`), its priority and optional assessments, `lane` and
`parallel` flags, idea review state, declared `needs` names, `every`/`defer`
settings, advisory `owner`, UTC stamps, stored outgoing task IDs (`parent`,
`depends`, `pieces`, `wake_on`, `follows`, `related`), `tags`, `source`,
`agent`/`model`, `origin`, `spec`/`plan`/`step`, `moved_from`, `body`, `refs`
handles in reading order, and ordered logical `notes`. Explicit `lane: true` or
non-empty `pieces` proves from one record alone that it is a goal. Absence of
both does not prove a record is a leaf: adding a first same-project child turns
an unmarked record into a goal without modifying the parent's file, so an
out-of-scope unmarked target in `blockers` omits `goal` as unknown.

Questions about containment, readiness, membership, inverse links, blockers or
attention are observations over a collection, parameterized by four inputs:

- **Authority:** which checkout supplies each project's scan. A command's focal
  project reads the invoking checkout (or a claimed/parked task's recorded
  worktree on `show`/`prime` elsewhere fallback); foreign projects read their
  registered `records` home. Two checkouts of one project can hold different
  valid versions of the same `TaskId` (for example `todo` on `main` and `done`
  in a feature worktree), unlike an unreachable foreign project where no scan
  is available.
- **Coverage:** which projects were requested and successfully scanned for
  inverse discovery (`dependents`, `followed_by`, `blockers`) versus forward
  lookups (`depends`, `pieces`, `wake_on`, `show` outgoing rows) that resolve
  declared references through the registry outside the scanned scope. An
  unregistered or unreachable project warns or marks coverage incomplete; it
  never masquerades as a scanned empty project.
- **Resolution:** `Registry::canonical_id` normalizes any literal ID once:
  displaced suffixes in `[predecessors.<live>]` take priority, otherwise
  `[aliases]` maps a retired prefix to its live prefix in one hop without
  re-canonicalizing the result. Spelling normalization applies across every
  relation. Move chains (`moved_to` / `moved_from`) are separate, directed
  cross-project relationships interpreted per query (see [Relations](#relations)).
- **Time and host:** evaluation instant `now` decides `defer` dueness
  (`now.date() >= defer`, `00:00:00Z`), recurring `every` dueness (`done` with
  `last_done` absent or `last_done + every <= now`), and 3-day `stalled`
  attention on `goals`. Host-local `ClaimSnapshot` state supplies active or
  `unverifiable` session claims, cross-project exclusive `needs` holds,
  busy-lane serialization, `park` continuations and capability `complexity`
  escalations without mutating `tasks/<id>.md`.

Collection queries keep **value**, **coverage** and **failure** distinct.
Dependency evaluation yields three values: `Satisfied` (`done` — including a due
recurring `done` until `start` reopens it — or `dropped` at the terminal live
record), `Open` (`idea`, `todo`, `doing`, `blocked`, `shelved`), or `Unresolved`
(missing target, unregistered/unreachable project, or `moved_to` cycle). A
`moved` redirect never satisfies on its own, and `Unresolved` never releases
work. Coverage qualifies inverse sets (`dependents` and `followed_by` omit
absent keys when none match in the readable scan; `blockers` reports
`coverage.complete`, `unreachable_projects`, `boundary_targets` and
`unfollowed_targets`) and partial progress (`?N` / `unresolved`). Failure
(`parse`, `config`, IO, or missing archive segments) exits 1 with a typed error
rather than degrading to `Satisfied`, `Unresolved` or an empty result.

Widening an inverse scan from scope $U$ to $V \supseteq U$ (`show` to
`show --all-projects`) preserves every matched record from $U$ and its fields
when overlapping projects supply the same checkout scans, the same `Registry`
alias/predecessor mapping and move chain, and the same focal key; row positions
can shift as newly discovered matches interleave under `ready_order`. Aggregate
impact counts and ranks (`blockers`), `now`-dependent dueness and host-claim
filtering depend on the wider graph, clock or `ClaimSnapshot` and lie outside
set monotonicity.

Rollups and orders follow the structure of each relation:

- **Forest sum versus DAG union:** valid same-project `parent` links form a
  forest with unique paths, so `HierarchyIndex` folds `open_descendant_count`
  bottom-up by addition. Cross-project `pieces` make goal membership a DAG in
  which multiple paths can reach the same record; `Membership` takes the
  canonical-ID set union (`BTreeSet<TaskId>`) before counting `pieces` so a
  shared member counts once, while idempotent rollups (`max` for
  `last_activity`) are path-insensitive.
- **Three distinct orderings:** feasibility and containment preorders (`depends`
  and `parent`, whose cycles writers reject and `check` reports); lexicographic
  total ranking orders (`ready_order`: `priority`, assessed `size` ahead of
  `None`, `created`, canonical `id`; `list_order`: `priority`, `updated`
  descending, `id`; `blockers`: `transitive_work_count` descending,
  `direct_work_count` descending, `priority`, `created`, `id`); and coarse
  whole-second UTC timestamps versus append-only note list order.

## Worked example

Consider projects `app` (with retired registry alias `legacy -> app`), `lib` and
`ops` evaluated at `now = 2026-11-18T12:00:00Z`:

- `app-000001` (`G`): `todo`, `origin: user`, `priority: 1`,
  `depends: [legacy-000003, lib-000010]`, `pieces: [lib-000010]`, and two inline
  notes stamped `2026-11-01T10:00:00Z` (`created`, then `scoped across app and ops`).
- `app-000002` (`C`): `todo`, `priority: 1`, `parent: app-000001`,
  `depends: [ops-000020]`, `pieces: [ops-000020]`.
- `app-000004` (`L`): `todo`, `priority: 1`, `size: s`, `process: direct`,
  `parent: app-000002`, `depends: []`.
- `app-000003` (`Q`): `done`, `priority: 1`, `size: m`, `process: direct`,
  `every: 7d`, `last_done: 2026-10-20T09:00:00Z`, `closed: 2026-10-20T09:00:00Z`.
- `lib-000010` (`P_old`): `moved`, `moved_to: ops-000020`, `depends: []`.
- `ops-000020` (`P`): `todo`, `priority: 2`, `size: m`, `process: direct`,
  `moved_from: lib-000010`, `depends: []`.
- `app-000005` (`F`): `done`, `priority: 3`, `size: xs`, `process: direct`,
  `follows: [lib-000010]`, `closed: 2026-11-04T12:00:00Z`.

| Observation or condition | Result and rule |
| --- | --- |
| Single record `app-000001` (`G`) | Known goal (`pieces` non-empty); two same-second notes retain file sequence `[created, scoped ...]`; `depends` spellings `legacy-000003` and `lib-000010` are uninterpreted without a registry and target reads. |
| Single record `app-000002` (`C`) if `pieces` were `[]` | Unmarked on disk (`lane: false`, `pieces: []`); only scanning `app` reveals `app-000004`'s `parent: app-000002` (`child_count: 1`, implicit goal). |
| Local containment vs `M(G)` (`goals`, `tree --pretty`) | `list` in `app` sums the local `parent` forest (`open_descendant_count: 2` for `C` and `L`). `Membership` follows `lib-000010 -> ops-000020` and unions reachable canonical IDs `{app-000002, app-000004, ops-000020}`: `C` shows `0/2` and `G` shows `0/3` (`pieces: {done: 0, total: 3}`), counting shared `ops-000020` once rather than `1 + 2 + 1 = 4`. |
| Containment vs prerequisite `Q` (`goals`) | `legacy-000003` normalizes via `[aliases]` to `app-000003` (`Q`), which lies outside `M(G)` and has `status: done` (`Verdict::Satisfied`): `goals` reports `dependencies: {open: 0, total: 1}` (`1/1` in pretty `deps`). |
| Due recurrence + picker ranking (`ready`, `ready --all-projects`) | At `now`, `Q` is due (`last_done + 7d <= now`) so it enters `ready` with stored `status: done` while still satisfying `G`'s dependency. Goals `G` and `C` and closed `F` are excluded. `ready_order` ranks `[app-000004, app-000003]` locally (`P1` size `s` before `P1` size `m`), and `[app-000004, app-000003, ops-000020]` under `--all-projects`. |
| Host-local foreign claim on `L` (`ClaimSnapshot`) | A live foreign session claim on `app-000004` hides `app-000004` from `ready` (returning `[app-000003]` with an omission warning) while `tasks/app-000004.md` remains `status: todo`. |
| Outgoing vs inverse move handling (`show`, `check`) | `show app-000001` lists single-hop `depends_on` rows `legacy-000003` (`done`) and `lib-000010` (`moved`), and `check` in `app` warns `retired_prefix` and `moved_dep`. `show app-000003` lists `dependents: [app-000001]` via alias normalization. Literal `dependents` under `--all-projects` gives `[app-000001]` on `lib-000010` (`[]` without `--all-projects`) and `[app-000002]` on `ops-000020`, whereas reciprocal-chain `followed_by` under `--all-projects` returns `[app-000005]` on both `lib-000010` and `ops-000020`. |
| Divergent worktree vs offline `ops` vs missing `moved_to` | Closing `L` in a feature worktree of `app` gives `open_descendant_count: 1` and `tree` `work` `1/3` there while `main` still reads `2` and `0/3`. If `ops` is unreachable instead, `lib-000010` and `ops-000020` both evaluate to `Unresolved`: `goals` warns that `ops` is unreachable and reports `pieces: {done: 0, total: 2}` and `dependencies: {open: 0, total: 3, unresolved: 2}` (keyed by stored canonical IDs since `ops-000020` cannot be read to merge them), `prime` suppresses closeout with an `unresolved goal members` warning once local children close, and ordinary `done app-000001` exits 1 (`open_dependencies`). If `tasks/lib-000010.md` is hand-edited to `status: moved` without `moved_to`, both `ready` and `done` exit 1 with `kind: "parse"` naming `tasks/lib-000010.md`, and `tasks -C lib check` reports a `parse` error. |

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

Validation splits across two boundaries so malformed framing never masquerades
as valid state while hand-edited semantic drift remains diagnosable:

- **Read/parse boundary (`parse_physical` and `Project::read_task`):** rejects
  missing/malformed delimiters, unknown or duplicate keys, wrong value types or
  grammars, `priority > 4`, `step` without `plan`, `last_done` without `every`
  or overflowing `last_done + every`, self-`depends`, reserved delimiters in
  `body`, duplicate `refs` handles, malformed note or `provenance:` lines,
  invalid/missing/corrupt `note_archives` segments, `id` disagreeing with
  `<id>.md` or the project `prefix`, local `spec`/`plan` paths outside
  configured roots, same-project `moved_from` suffixes not reserved in
  `[predecessor]`, and invalid redirect pointers (`status: moved` without
  cross-project `moved_to`, or `moved_to` on any non-`moved` status). Every
  normal read exits 1 with `kind: "parse"`; `tasks check` collects these as
  `parse` error findings.
- **Write and `tasks check` boundary:** status-indexed field combinations, goal
  role restrictions, vocabulary checks, reciprocal move counterparts, document
  existence/step drift on open work, and `parent`/`depends` cycles are accepted
  by `parse_physical` so `$EDITOR` saves can repair them and `tasks check` can
  report typed findings, while CLI writers refuse to create them.

## Lifecycle

Open states are `idea`, `todo`, `doing`, `blocked` and `shelved`. `done` and
`dropped` are closed and satisfy dependencies. A `moved` record is an immutable
redirect to its successor, possibly through a chain; unresolved or cyclic chains
are not completion.

| Status | Kind | Variant-specific fields and writer/`check` invariants |
| --- | --- | --- |
| `idea` | Open work item | Only status allowing `reviewed`, `awaiting_user` and `reviewed_at` (`review_status`); `awaiting_user` and `reviewed_at` require `reviewed: true`. Optional `defer` is a picker/review snooze; `every` allowed without `defer`. Never offered by `ready` (only `next` on an agent-waiting park, to resume scoping). |
| `todo`, `blocked` | Open work item | Optional `defer` snooze on non-goals (`deferred_goal`), mutually exclusive with `every` (`defer_periodic`). Unassessed leaves (`process` absent, not a goal) surface as `needs_review: unassessed`. |
| `doing` | Open work item | Active or parked work (`started` set on first entry and kept across resume/reopen); forbids `defer` (`defer_status`), `reviewed` (`review_status`) and `wake_on` (`wake_status`); `check` warns `process_missing` when `process` is absent. |
| `shelved` | Open work item | Entered only via `shelve`; `unshelve` returns to unreviewed `idea`. Only status allowing `wake_on` (`wake_status`) and the only status where `every` and `defer` may coexist (`defer_periodic`); here `defer` is a required review checkpoint, including on goals. Retained by ordinary `list`/`search` while omitted by pickers; remains open and holds dependents (`shelved_dep` warning). |
| `done` | Closed work item | Sets `completed` and `closed` (`closed_status` elsewhere; `completed_stamp_on_open_task` warning if left on open work). With `every`, sets `last_done` and stays `done` on disk; once due (`last_done` absent or `last_done + every <= now`), `ready`/`next` offer the same ID again while `depends` stays `Satisfied` until `start` reopens it to `doing`. |
| `dropped` | Closed work item | Sets `closed` (`completed` stays unset); satisfies `depends`; never becomes due under `every`. Requires closed local descendants at every depth, with no `--force` override. |
| `moved` | Redirect tombstone | Entered only via `move`; immutable (`Moved` refusal naming successor). Requires cross-project `moved_to` at parse time; forbids `closed` (`closed_status`), `pieces` (`piece_status`), `follows` (`follows_status`) and `related` (`related_status`). Retained `depends` and `plan` links are historical only. |

Across open and closed work items, structural role and vocabulary add four
cross-field rules: a **goal** (`child_count > 0`, `lane: true`, or non-empty
`pieces`) is never offered by `ready`/`next`, cannot carry `every`
(`periodic_goal`), and cannot carry `defer` outside `shelved` (`deferred_goal`);
a **lane** (`lane: true`) cannot nest inside an ancestor lane (`nested_lane`)
and cannot carry `pieces` (`lane_pieces`); every entry in `pieces` must be
foreign (`local_piece`) and also present in `depends` (`piece_not_dependency`);
and every `needs` entry must be declared in `[needs]` (`unknown_need`). `every`
accepts positive canonical `Nd`/`Nw` intervals through 36,500 days; `defer` is
strict `YYYY-MM-DD`, due from `00:00:00Z` on that day, and setting it requires a
future UTC date.

Open states transition to other open states or close subject to command guards.
Closed nonrecurring tasks reopen only to `todo`. `start` can take a `done`
recurrence directly to `doing` for a new occurrence, including an early run.
`done` checks direct dependencies, required local/attachment `output` refs
(`ref_output_missing`, not waivable by `--force`), and, for goals,
[effective membership](#goal-completion). Status transitions clear `defer`;
`shelve` then writes its required checkpoint and optional `wake_on` targets, and
targets leaving `wake_on` (on renewal, `unshelve`, `done` or `drop`) move into
`related`. Pickers and derived `needs_review` states (`unreviewed`, `woken`,
`stranded`, `unassessed`) are detailed in the [user guide](user-guide.md).

Task timestamps are whole-second UTC stamps, not unique event ordinals. Writers
keep `updated` monotonic across saved versions. Do not infer event order from a
strict timestamp comparison alone (`review` wake checks compare
`closed >= reviewed_at`, while `defer` wake compares `defer > reviewed_at.date()`)
or equate task age with blocking duration.

## Relations

Every task-to-task field normalizes spellings once with `Registry::canonical_id`
before comparison or lookup, but direction, graph shape, move traversal and
scope coverage differ by relation:

| Relation | Stored on and arity | Shape | Move policy | Forward and inverse coverage | Validation and hold semantics |
| --- | --- | --- | --- | --- | --- |
| `parent` / `children` | Child stores `0..1` same-project `parent` | Local forest per project | Never crosses projects; `move` refuses or detaches per move rules | Scanned within the single canonical project (`HierarchyIndex`); `children` ordered by `ready_order` | Writers and `check` reject `foreign_parent`, `dangling_parent` and `parent_cycle`; `open_child_of_closed_parent` warns on non-recurring open children. |
| `depends` / `dependents` | Consumer stores `0..*` targets (local or foreign) | Cross-project DAG | Forward `depends` follows `moved_to` chains (`tombstone::follow`); `show` `depends_on` inspects the stored single hop; inverse `dependents` matches canonical spellings literally without following moves | Forward resolves across registered projects; `blockers` stops at out-of-scope boundary targets (`coverage.complete: false` if they have `depends`); `show` scans local by default or all reachable registered projects with `--all-projects` (excluding `moved` tombstones, `ready_order` deduped) | Self-link fails parse; writers/`check` reject `dangling_dep` and `cycle` (`tombstone::edges` uses `[moved_to]` on tombstones); `Open` and `Unresolved` hold `ready` and ordinary `done`. |
| `pieces` | Goal stores `0..*` foreign targets, each also in `depends` | Cross-project membership DAG (`M(g)`) | `Membership` follows `moved_to` chains (`piece_now_local` warns if a piece moved into the goal's own project) | `Membership` opens reached registered projects regardless of `--project`/`--group` narrowing so goal counts stay invariant | Writers/`check` error on `local_piece`, `piece_not_dependency`, `lane_pieces`, `piece_status`, and warn `self_piece`, `duplicate_piece`; holds `done` and `prime` closeout via [goal completion](#goal-completion). |
| `wake_on` | `shelved` task stores `0..*` targets | Arbitrary cross-project edges (independent of `depends`) | Review evaluation (`review::Deps`) follows `moved_to` chains; `show` `wake_targets` inspects the stored single hop | Forward lookup via scan + registry; no inverse view | Allowed only on `shelved` (`wake_status`); any `Satisfied` target or due `defer` wakes review (`woken`); `Unresolved` warns and marks `stranded` unless already woken. |
| `follows` / `followed_by` | Continuation stores `0..*` earlier task IDs | Arbitrary cross-project history links (never gates execution/review) | Outgoing `show` `follows_tasks` inspects the stored single hop (preserving order/duplicates); inverse `followed_by` matches against the focal task's verified reciprocal `moved_from`/`moved_to` chain | Outgoing resolves via scan + registry; `followed_by` uses `show` (`--all-projects` widens) or `next` scope, excluding `moved` followers and focal-chain self-matches, `ready_order` deduped | Writers reject new self, duplicate, missing or `moved` targets; `check` errors on `follows_status` and warns `self_follows`, `duplicate_follows`, `dangling_follows`. |
| `related` | Citing task stores `0..*` informational IDs | Arbitrary cross-project citations | Auto-linking on write follows `moved_to` to live destinations; stored `show` `related_to` inspects the single hop | Outgoing single-hop lookup via scan + registry; no inverse view | Forbidden on `moved` (`related_status`); `check` warns `dangling_related` locally; never gates readiness, review or closure. |
| `source` (task ID) | Task stores `0..1` opaque `source` string | Optional single citation when `source` parses as a registered-prefix `TaskId` | Single-hop display only; never rewritten or followed across moves | `show`/`next` `source_task` resolves via scan + registry; `list --source` compares literal strings | Unresolved task-id `source` warns on `show`/`next`; never gates execution or `check`. |
| `moved_to` / `moved_from` | Tombstone stores `moved_to`; successor stores `moved_from` | Cross-project redirect chains (1-to-1 per hop) | Defines the redirect chain (`tombstone::follow` forward; `follows::identity_set` reciprocal) | Inspected across registered projects | `parse_physical` enforces `moved` iff cross-project `moved_to`; `check` errors `move_incomplete` on missing/non-reciprocal counterparts and warns `move_counterpart_unreachable`. |

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
prefixes and write only the active suffix. Sequence position across the archived
segments and inline suffix — not whole-second `at` timestamps — defines note
order. [Attachments](attachments.md#note-archives) covers preview, apply, retry
and coordinated reader rollout.

Lifecycle commands append human-readable notes such as started/resumed, parked,
done, dropped, and recurring completion/next-due receipts. Notes have no separate
event-kind discriminator: a user's identical text is not distinguishable by text
alone. A creation note is identified structurally at the creation timestamp;
provenance proves a reported session source, not that a note is a lifecycle event.

Writers link newly cited tasks into `related` automatically on write. Text read:
the body (`add`, `edit --body`, editor saves), plus writer-appended notes (`note`,
`park` next step, `shelve` wake condition, and closure messages for `done`,
`drop`, `block`). Tool-generated notes (`move`, `migrate`, `rename`, receipts,
start/resume, takeover) and `feedback` never link citations. A citation is a
maximal token matching `[a-z][a-z0-9]{1,7}-[0-9a-f]{6}` whose prefix the registry
knows. Only new citations count: every id in an appended note counts, and only
ids absent from the old body count. Each new citation resolves to its live
destination across move chains (`tombstone::follow`). Unresolvable citations warn
`<task> cites <id>, which resolves to no task here; not linked`. Citations are
skipped without warning if the cited id or its destination matches the record
itself, a child, an id already in `related`, or an id held in `depends`, `parent`,
`wake_on`, `follows`, `moved_to`, `moved_from`, a task-id `source`, or removed in
the same write. Explicit related changes (`--rm-related`, `--no-related`, editor
changes to `related`) take precedence over automatic linking. While a retained
operation is pending on the record, automatic linking is deferred with a warning
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
registered project; next uses its existing requested local/group/all-project scope.
A missing field means none found in that readable scope, not none elsewhere.

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

`add --piece REF` and `edit --piece REF` (repeatable) mark REF as a piece and add it to
`depends` when absent, in one save; a piece already held as a dependency, under any
spelling, is marked silently, and one already marked warns and changes nothing. The
target must resolve and must not be a tombstone; done and dropped targets are accepted.
`edit --rm-piece REF` unmarks a piece and keeps the dependency, `--no-pieces` unmarks
every piece, and naming one id in both `--piece` and `--rm-piece` is refused. A lane
cannot take `--piece`. An editor save that changes the list is validated by the same
rules and resolves newly added pieces. When a piece moves to a still-foreign successor,
`check`'s `moved_dep` names `tasks edit <goal> --piece <new>` then
`tasks dep <goal> --rm <old>`.

Because a piece stays a dependency, readiness, `done`'s `open_dependencies` refusal and
blocker traversal treat it as before. A record that declares pieces is never offered by
`ready` or `next` and is not unassessed review work; it reaches closeout once it has no
open effective members, all membership paths resolve and every direct dependency is
satisfied. See [goal completion](#goal-completion) for closeout diagnostics. `dep --rm`
of a piece also unmarks it; `edit --no-depends` that would strand a piece and
`edit --lane` on a record with pieces are refused.

A goal's members are every record reachable from it by repeatedly following `parent`
children (same project) and declared pieces (resolved through moves), excluding the goal
itself, deduplicated by canonical id so a foreign piece reached along multiple paths or
nested subgoals counts once. `goals` counts progress (`done` over
`done | todo | doing | blocked`, with `idea` counted separately and `shelved`,
`dropped` and `moved` excluded), activity and attention over members, and counts as
`dependencies` only distinct targets of the goal and its counted members outside the
membership (unresolved targets cannot match a live member and are keyed by stored
canonical id). A piece no record answers, including one in an unreachable or
unregistered project, is not a member; as a dependency it is counted as unresolved,
never done or open. Members are read through the registry whatever the requested scope.

`check` reports `piece_not_dependency`, `local_piece`, `lane_pieces` and `piece_status`
(pieces on a tombstone) as errors, and `self_piece` and `duplicate_piece` as warnings.
On an open record, a piece that moved into the record's own project reports
`piece_now_local` in place of `moved_dep`, with the remedy `tasks dep <goal> --rm <old>`
then `tasks edit <new> --parent <goal>`. `show` labels dependency rows that are pieces
`piece`; its JSON is unchanged.
Moving a goal carries its pieces with `depends` and clears them from the tombstone; a
move or migration into a piece's project is refused, since that piece would become
same-project work.

## Goal completion

For a fresh transition to Done, follow same-project children and declared pieces
through Done subgoals. Stop each path at a Dropped subgoal. An item reached by
another active path still counts. A shared member counts once. Every open state
holds completion, including ideas, shelves and open child goals. A recurring
member satisfies the guard between runs, even when due. Restarting it affects
the next completion check and preserves previously recorded closures.

`prime` applies this rule to otherwise eligible closeout candidates. It omits
open work silently, warns with the declaring member and target for a missing
piece or move hop (`G has unresolved goal members: H -> B`), and warns with
the directed path for a cycle (`G has a membership cycle: X -> Y -> X`).
A closeout row's `open_descendant_count` still counts all open local descendants.
It can be positive when they lie entirely beneath dropped subgoals. An otherwise
finished goal held only by a direct prerequisite retains its prerequisite
warning. A failed read or parse fails the command. Each writer (`done`,
`edit --status done` and an editor save to Done) checks fresh evidence, write
authority and required outputs before saving. Foreign projects can change
between reads.

A reachable membership cycle returns `goal_membership_cycle` with its directed
path. After fatal checks, `open_dependencies` takes precedence for a direct
hold. Next, `unresolved_goal_members` lists each declaring member and
unresolved piece as `source -> target`, followed by any known open members. An
open-only hold returns `open_descendants` when every holding member has an
effective local path, or `open_goal_members` when a holding member needs a
foreign path.

Explicit force can waive direct, unresolved and open obligations for this
transition. A successful forced goal completion names each applicable family
in that order in `warnings`. Cycles, parse or read failures, write authority
and required outputs still prevent completion. Editor saves use ordinary
completion. A previous force grants no waiver on a later transition.

## JSON and errors

Task JSON exposes complete logical `body`, `refs` and `notes`, not physical
archive descriptors. Unset optional fields and empty Task lists are omitted;
false booleans, zero numbers and empty body remain. Response containers such as
`tasks` and `warnings` remain arrays, and an empty `next` result is explicit null.
Summary/park/goal rows are projections, not interchangeable full Task objects.
Resolved relationship entries on `show` and `next` (`parent`, `children`,
`depends_on`, `dependents`, `related_to`, `follows_tasks`, `followed_by`,
`wake_targets` and `source_task`) carry `id`, `title`, `status` and `updated`
(plus `resolved` on `depends_on`, `related_to`, `follows_tasks`, `wake_targets`
and `source_task`, where unresolved entries omit `title`, `status` and `updated`). `dependents`
lists every task whose `depends` names this one under any spelling of its id (a
retired prefix counts), open and closed, in ready order, deduplicated by
canonical id and excluding moved tombstones. Matching is literal: it follows no
move chain. Plain `show` reads its own project's scan; `show --all-projects`
adds every other reachable registered project (its own records stay the copy
already read) and carries unreachable-project warnings; `next` uses its
requested scope. The same widened scan feeds `followed_by`; no other field
changes. A missing field means none found in that readable scope, not none
elsewhere. Additive output fields are allowed; clients should ignore unknown
output fields.

JSON success goes to stdout; a clean `tasks check` is silent with exit 0.
Document/export bodies and version/help/completion output have their own formats.
Application failures exit 1 with a typed error; CLI usage failures exit 2.
Warnings on success must not be discarded. Parse errors, missing archive history
and invalid configuration never become empty successful results. Pretty output
is presentation and may change.

Compatibility and drift checks separate three change axes:

- **Persisted format (`tasks/<id>.md`, `note_archives`, `tasks/.config.toml`):**
  update parser/serializer/validation and round-trip fixtures, output
  projections, CLI/completion surface (`tools/cli.toml`), shipped skills, this
  reference and migration/read compatibility in the same commit. Install the
  updated binary on every host that reads synced records before records carrying
  new keys sync; verify one host end to end, and the other hosts updated by the
  same install path are covered.
- **Public JSON and error codes (`src/output/contract.rs`, `src/error.rs`):**
  never change existing JSON shapes or error `kind` values without a task;
  update CLI JSON fixtures, shipped skills and this reference in the same commit.
- **Query and guard semantics (`dependency`, `membership`, `hierarchy`, `query`,
  `review`, `follows`, `blockers`, `mutate`, `check`):** behavioral changes alter
  answers even when persisted files and JSON schemas are unchanged. Update this
  reference and the corresponding unit/integration scenario fixtures in the
  behavior-changing commit; `just check-hygiene` checks document-type shape and
  live corpus invariants, CI runs every test target without skips, and periodic
  `curate-docs` passes audit commits touching governed modules after this file's
  last revision.
