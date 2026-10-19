# Task references: one explicit, ordered list of what a task cites

Status: approved by the user 2026-10-19, with spec review round 2's fixes applied; implementation plan next
Task: tasks-3a445e
Coordinates with: tasks-dc16eb (document roots), tasks-a73dce, tasks-82bd08 and
tasks-6360ac (branch-only and linked-checkout resolution)

## 1. Goal

A task's documents are spread across four mechanisms: `spec` and `plan` (typed, resolved,
validated), `source` (an opaque origin), attachments (bytes under `tasks/files/<id>/`) and
prose. Prose carries most of them. On 2026-10-19 the records of all registered projects
cited `docs/plans/…` about 2,800 times, `docs/specs/…` about 1,700 times and
`docs/notes/…` about 1,700 times, and 156 records had a hand-written `Where:` section.
Those citations cannot be queried, checked, or rewritten when a document moves. An agent
picking up a task also has no single answer to "what must I read first".

This design adds **task references**: explicit entries for the documents and artifacts a
task cites, each with a short handle that prose cites. `show` reports one computed
**inventory** of everything the task points at:

- the governing spec and plan;
- the explicit references;
- the source;
- attachments.

Each row says where it came from. The **reading list** is the inventory's required rows,
in order.

Success means the following:

- A task like crew-5aa683 states its reading in the record. `tasks show` reports one
  inventory with each row's status, in JSON and in pretty output, and nothing has to be
  merged from other fields to find what the task points at.
- `check` catches a cited document that disappears from open work, and a task cannot
  close while an output it declared is missing.
- A later move of documents (tasks-dc16eb) can rewrite references by handle, not by
  searching prose.

## 2. Inputs settled before this design

These come from the 2026-10-19 discussion and a codex review of the proposal (recorded
on the task):

- `spec`, `plan`, `step` and `source` keep their fields and roles. A spec cited as a
  reference is context unless it is also the linked spec (§5.2).
- The inventory is not the reading list. Being required is a marker of its own,
  separate from the relationship, and entry order is reading order. Sources and
  attachments appear in the inventory but are never required.
- Relationship, kind and location are separate dimensions. Kind is descriptive only.
- Prose cites a reference with a reserved form, not a bare bracket, and the sentence
  explains why the reference matters. The tool does not warn about raw paths in prose.
- Locating and reading are separate. `Resolver::read` reads UTF-8 bodies only.
- Being an output (produced by this task) is separate from existing yet. An output may
  already exist, as when the task revises a runbook, or may not exist yet.
- Code locations are out of scope (§11).

## 3. Record format

### 3.1 Where references live: a managed body section

References are stored in a `## References` section that the binary manages. It sits
after the free-form body and before `## Notes`, and is written only when the task has
at least one reference:

```
<body>

## References

- brief (input, required, kind=brief): docs/notes/2026-10-15-trigger-dispatch-brief.md — admission rules, §7
- r1 (input, required, kind=report): docs/notes/2026-10-15-trigger-sweep-r1.md — sweep findings R1, §6-8
- alloc (context, kind=brief): docs/notes/2026-10-15-allocation-and-resolution-brief.md
- runbook (output, required, kind=runbook): "docs/runbooks/trigger sweep.md" — revised by this task; read the current one first
- after (output, kind=image): files:after.png — screenshot of the result

## Notes
…
```

Why a body section and not frontmatter (my call; the review left storage open):

- **Older binaries.** An older `tasks` binary rejects every unknown frontmatter key
  (`src/format.rs`, `unknown key`; tasks-476fd7 records how badly that reads). A record
  with a new key would stop parsing on any host whose binary is behind. An older binary
  reads a body section as ordinary body text and writes it back unchanged.
- **Structure.** The frontmatter subset holds only scalars and flat lists of strings. A
  list of structured entries would need either a grammar packed into list items or a
  new nested syntax for every reader.
- **Precedent.** The section follows `## Notes`: a reserved heading, one entry per
  line, strict parsing, and `edit --body` refusing the reserved heading. No record in
  any registered project used a `## References` heading on 2026-10-19.

The cost is a second reserved heading. `validate_body` refuses `## References` the way
it refuses `## Notes`. The parser also errors on a second `## References`, on one placed
after `## Notes`, and on a malformed entry line.

### 3.2 Entry grammar

```
- <handle> (<relation>[, required][, kind=<kind>]): <reference>[ — <description>]
```

- **handle**: `[a-z][a-z0-9-]{0,31}`, unique within the task.
- **Unique references.** The decoded reference (after unquoting) is also unique among a
  task's entries: one document or artifact has one entry, so one inventory row carries
  one set of metadata. A write that would hold two entries with equal references is
  refused as `ref_duplicate`, naming both handles. That covers `add`, `edit --ref` and
  saves from the flagless editor. `check` reports `ref_duplicate` for a pair that
  arrives by hand edit.
- **relation**: `context` (consult when needed), `input` (the work uses it) or `output`
  (the work produces or revises it).
- **required**: puts the entry on the reading list (§5.3). It is allowed on every
  relation. On an output that already exists it means "read the current version before
  starting". On an output that does not exist yet, the row is `pending` and there is
  nothing to read.
- **kind=**: optional, `[a-z][a-z0-9-]*`. It is descriptive and not checked against any
  list (§3.4).
- **reference**: written in one of two forms.
  - *Bare*: no whitespace, not starting with `"`, and no `"` or `\` anywhere.
  - *Quoted*: `"…"` with `\"` and `\\` as the only escapes. This is the frontmatter
    subset's existing quoting (`frontmatter::take_quoted` and `quote`), reused rather
    than reinvented. The writer quotes exactly when the bare form is not allowed.

  A reference ends at its first whitespace when bare, and at its closing quote when
  quoted. Either way, the description separator cannot occur inside a reference: a
  literal ` — ` inside a path or external reference is inside the quotes.
- **description**: optional, one line, after ` — ` (space, em dash, space) following the
  reference. It says what the referenced thing is. Why it matters belongs in the prose
  that cites it (§4). Whatever follows the reference must be empty or start with ` — `.

Attributes follow a fixed order: the relation, then `required`, then `key=value` pairs.
That leaves room for later attributes, such as a pinned revision (§11), without a new
grammar. An attribute the binary does not know is a parse error, as an unknown
frontmatter key is today.

### 3.3 Reference forms

| Form | Example | Located by |
|---|---|---|
| Local path | `docs/notes/x.md`, `results/run-12` | `Resolver::locate_entry` (§9) |
| Attachment | `files:after.png` | the task's attachment ledger and `tasks/files/<id>/` |
| External | `mindful:thought:<hex>`, `https://…` | `[docs.schemes]` when read; never fetched for the inventory |

- **Local path.**
  - A normalized repo-relative path under the rule `validate_doc_shape` applies: no
    empty, `.` or `..` segments.
  - Any file type is allowed, not only `.md`.
  - The path may name a regular file or a directory, after following symlinks. A
    directory is spelled without a trailing slash (`results/run-12`), and a trailing
    slash is refused with that spelling as the hint. Whether the path is a file or a
    directory is observed, not declared. Any other object at the path (a FIFO, socket or
    device) is refused as `ref_special_file`, wherever the path is resolved (writes,
    `check`, `doc`, the completion gate, `show`), so nothing ever opens one.
  - Spaces are allowed, written in the quoted form. The first segment contains no colon.
- **Attachment.** `files:<name>`, where `<name>` passes `attachments::validate_name`.
  - A `context` or `input` attachment must be held as attached by the ledger when
    written.
  - An `output` attachment need not exist yet. It is `pending` until `tasks attach`
    stores a file under that name.
  - `files` becomes a reserved scheme: a `[docs.schemes]` entry named `files` is a
    configuration error.
- **External.** The `<scheme>:<rest>` grammar of the external document references spec
  (`docs/specs/2026-10-11-external-document-references-design.md` §3.1). Its opaque
  remainder may contain spaces and quotes, written in the quoted form. An `https:` URL
  is external. Without a configured fetcher it can be listed but not read.
- **Reserved.** A reference starting with `@` is refused with "cross-project references
  are not supported yet". The character is kept for `@<prefix>/<path>` (§11).

### 3.4 Kinds

Kind is a free label so readers can tell entries apart. Nothing behaves differently by
kind, and there is no inference from the file name and no per-project kind dictionary,
until some feature needs one. The tasks skill suggests a vocabulary drawn from how
documents are named across projects on 2026-10-19: `brief` (153 of the notes), `spec`,
`plan`, `report` (findings, probes, spikes, diagnoses, audits, runs), `review`,
`ledger`, `handoff`, `retro`, `runbook`, `guide`, `decision`, and for artifacts
`dataset`, `image`, `log`, `trace`, `transcript`.

## 4. Citations in prose

The body cites a reference as `[ref:<handle>]`:

> Follow the admission rules in [ref:brief] §7. The earlier sweep [ref:r1] establishes
> which refusals can be decided without running project code.

Recognition:

- A citation is `[ref:` followed by text up to the next `]`.
- Text inside an inline code span or a fenced code block is never a citation. That is
  also the escape: to write a citation literally, as this spec does, put it in
  backticks or a fence.
- Only the body is read. Notes are append-only history, so a note's citation text is
  not validated and does not pin an entry (my call).

Validation:

- The text between `[ref:` and `]` must be a valid handle that names an existing entry.
  A malformed handle (`ref_citation_malformed`) and an unknown one
  (`ref_citation_dangling`) are both refused.
- `add` and `edit` refuse a write that would leave such a citation, whether from a new
  body, `--rm-ref` or `--no-refs`, naming the handle. `check` reports either finding for
  one that arrives by hand edit.
- An entry need not be cited.
- Several citations may point at different sections of one entry. The section is prose
  ("§7"), not part of the entry.

The tool never scans prose for raw paths. Promoting cited paths to entries is curate's
work (§10).

## 5. The inventory and the reading list

### 5.1 Rows and their order

The inventory is computed by `show`, never stored. Its rows come in this order:

1. the governing `spec`, when one is linked;
2. the `plan`, when one is linked;
3. every explicit entry, in entry order, except one merged into row 1 or 2 (§5.2);
4. the `source`, when one is set;
5. every attachment the ledger holds as attached that no explicit entry names, in name
   order (the ledger keeps each name's latest entry). A row whose file is gone has status
   `missing`. A file in storage that the ledger does not hold is not a row; the existing
   attachment audit warnings still report it.

Each row records where it came from (`origin`: `spec`, `plan`, `ref`, `source` or
`attachment`) and whether it is required.

### 5.2 Merging, so nothing appears twice

- **Entry equal to the spec or plan.** An explicit entry is a merge candidate when its
  decoded reference equals the linked spec or plan, byte for byte. It merges into that
  row only when both resolve to the same target. The merged row keeps the governing
  origin and position and takes the entry's handle, relation, kind and description. It
  stays required. Citations of the handle keep working. This lets a task declare
  `sweep-spec (output, kind=spec): docs/specs/…` before writing the spec, then link it
  with `edit --spec` without the inventory listing it twice (my call; refusing the
  overlap would block that flow).

  The governing row resolves through the file-only `locate`, and the entry through
  `locate_entry` (§9). The two agree when:

  - **local, both found**: they resolve to the same absolute path;
  - **local, both missing**: neither finds anything. The merged row takes the governing
    row's status (`missing`), because an unresolvable governing link is the error
    `check` already reports;
  - **external**: equal external strings always agree. The merged row takes the
    governing row's status (§7.3).

  Otherwise they disagree. Examples: a directory at the path in the worktree and a file
  at that path in the main checkout; or the entry finding a directory while the
  governing `locate` finds nothing. Then the entry keeps its own row at its entry
  position, with origin `ref` and its own `path` and `status`. `show` adds a warning
  naming the handle and both resolved targets (or `missing`), so the difference is
  visible rather than hidden in one row. `doc <id> --spec` and `doc <id> <handle>` then
  read what their rows say.
- **Spec and plan linking the same document.** Nothing forbids this today, and both
  governing rows stay. A matching entry merges into the spec row only, under the rule
  above. The plan row keeps its own fields.
- **Entry naming an attachment.** An explicit entry naming `files:<name>` replaces that
  attachment's row from step 5. One row, with origin `ref`.
- **Source.** The source is never merged. It is provenance and stays opaque.

### 5.3 The reading list

The reading list is the inventory's required rows, in inventory order: spec, plan, then
required entries. It is not a separate structure, so it cannot drift from the
inventory. Including spec and plan is my call: the workflow already requires reading
them, and the list should be complete without a reader having to merge it with them.
Sources and unreferenced attachments are never required.

## 6. CLI

Every option below starts as rows in ops's `cli.toml` and is adopted into
`tools/cli.toml` together with the parser change (AGENTS.md, Rules).

### 6.1 Writing entries

- **`add --ref '<entry>'`** (repeatable) and **`edit --ref '<entry>'`** (repeatable).
  The argument uses the entry grammar of §3.2 without the leading `- `, so the CLI and
  the file share one grammar, and filing a task with its references takes one call and
  one commit. On `add`, a handle repeated within the call is refused. On `edit`, an
  existing handle is replaced in place, keeping its position, and a new handle is
  appended.
- **`edit --rm-ref <handle>`** (repeatable) and **`edit --no-refs`**, which conflicts
  with `--ref` and `--rm-ref`. Both refuse while the body still cites the handle,
  unless the same call replaces the body.
- **Reordering** is done in the flagless editor path, where the section appears as
  text. No reorder flag in this increment.

Write-time validation for `--ref`:

- the grammar of §3.2 and the forms of §3.3;
- for `context` and `input` entries, a local path or attachment must exist (§9), or the
  write fails as `ref_not_found`;
- an `output` entry is not checked for existence;
- an external reference is checked for shape only and never fetched.

### 6.2 Reading and querying

- **`list --ref <reference>`**: keeps tasks holding an entry whose reference equals the
  value byte for byte, like `list --source` and `list --plan`. It answers "which tasks
  cite this document" before a document is moved or rewritten.
- **`doc <id> <handle>`**: the `doc_kind` group becomes exactly one of `--plan`,
  `--spec` or a handle. A local file or attachment prints when it is UTF-8 text. A
  directory or binary file fails as `ref_not_text`, naming its absolute path. A pending
  output fails as `doc_not_found`. An external reference goes through `[docs.schemes]`
  as an external plan does, and `--no-fetch` applies.

### 6.3 The completion gate

A **fresh transition to done** refuses while an `output` entry's local path or attachment
is missing (`ref_output_missing`), naming each handle. The remedy is to produce the
output, or to remove the entry with `edit --rm-ref`. An expected output thus either
becomes a produced one or is dropped explicitly.

- **Where it lives.** The gate is in the shared `transition()` (`src/commands/mod.rs`),
  under the same `completing` condition (`to == Done && status != Done`) that stamps
  `completed`. That reaches every entry point: `done`, `edit --status done` and the
  flagless editor path.
- **When it runs.** It checks the record as it will be written, including entries the
  same `edit` call adds or removes, and runs before anything is written.
- **Not overridable.** `--force` does not override it, unlike open dependencies and
  descendants. The remedy is explicit and fits in one call
  (`edit <id> --status done --rm-ref <handle>`), and a closed record is never checked
  again, so an overridden gate would leave a record claiming an output that never
  existed (my call).
- **Already done.** `tasks done` on a record that is already done (the claim and park
  cleanup path, including a recurring task's release) is not a fresh transition, so the
  gate does not run and cleanup still works when an output has since disappeared.
- **Recurring tasks and reopened tasks.** Each completed occurrence of a recurring task
  is a fresh transition and is checked, and so is a reopened task closing again.
- **What it checks.** External outputs are not checked. `drop` does not check outputs.

### 6.4 Interactions with existing commands

- **`detach`** refuses an attachment that any entry names (`ref_cites_attachment`),
  pointing at `edit --rm-ref`.
- **`move`** preflight validates `context` and `input` local paths against the
  destination, as `validate_destination_docs` does for spec and plan, and refuses with
  `edit --rm-ref` as the remedy. Attachment entries move with the attachments.
- **`search`** does not match entries; it searches the body. `list --ref` is the query
  for entries.

## 7. Output

The JSON contract changes only by addition and follows the existing omission
convention: an empty list and an absent optional value are omitted, not written as `[]`
or `null`.

### 7.1 `task.refs`

`task.refs` is on the full task object (`show`, `next`) and omitted when empty. Each
element is `{handle, relation, required, reference}`, plus `kind` and `description` when
present. Summary rows (`list`, `ready`, `prime`, `tree`) do not gain it.

### 7.2 `ShowFields.inventory`

The inventory of §5, omitted when it has no rows. Each row has:

- **`origin`**: `spec`, `plan`, `ref`, `source` or `attachment`.
- **`required`**: true for spec, plan and entries marked required.
- **`reference`**: the stored value.
- **`location`**: `path`, `attachment`, `external`, or `opaque` (the source, which is
  never interpreted).
- **`status`** (§7.3).
- When present: **`handle`**, **`relation`**, **`kind`** and **`description`**, from an
  explicit entry, merged or not.
- **`path`**: absolute, for local paths and attachments.
- On the plan row: **`step`** when one is linked, and **`step_found`** with exactly the
  meaning of the existing top-level field (omitted when it cannot be determined).
- **`cause`**: with status `unavailable`.

The existing `spec_path`, `plan_path` and `step_found` fields keep their meaning. The
reading list is the rows with `required: true`, so it gets no field of its own.

### 7.3 Status

| Status | Meaning |
|---|---|
| `present` | A local path or attachment found here, or only in the main checkout. In the second case the resolver's main-checkout warning is also emitted. |
| `missing` | A local path or attachment that is absent, on a row that is not an output. |
| `pending` | An absent local path or attachment on an output row. |
| `available` | An external document `show` fetched and read. |
| `unavailable` | An external document `show` tried to fetch and could not; `cause` carries `UnavailableCause::detail()`. |
| `unchecked` | Not looked at: every external entry, an external spec, an external plan without a step, the source, and anything external under `--no-fetch`. |

Governing documents keep `show`'s current fetch behaviour, with nothing new fetched:

- a local spec or plan is `present` or `missing`;
- an external plan with a step is fetched today to compute `step_found`, and its row
  reports `available` or `unavailable` from that same read;
- a budget-exhausted read is `unavailable` with its cause;
- `--no-fetch` makes it `unchecked`.

### 7.4 Pretty `show`

A `references` section follows `related`. It opens with a "read first" line naming the
required rows in order, then one row per inventory row: origin or handle, relation
(marked when required), kind, reference, status when not `present`, and description.
The plan settles the layout within the show layout spec's conventions
(`docs/specs/2026-10-03-show-layout-design.md`).

## 8. Check

These findings apply to open tasks only. A closed record's references are history, as
its spec and plan links are (check's "Spec §7" rule).

| Finding | Severity | When |
|---|---|---|
| `ref_missing` | error | A `context` or `input` local path or attachment is absent, matching `doc_missing` for spec and plan. |
| `ref_in_main_checkout` | warning | Found only in the main checkout, matching `doc_in_main_checkout`. |
| `ref_citation_dangling` | error | The body cites a handle with no entry (§4). |
| `ref_citation_malformed` | error | The body has a `[ref:…]` citation that is not a valid handle (§4). |
| `ref_duplicate` | error | Two entries hold the same decoded reference (§3.2). |

- Outputs are not checked while the task is open, whether or not they exist yet. The
  completion gate covers them (§6.3).
- `check` does not fetch external references. Unlike a linked external spec, which
  `check` fetches, references can be many and are often plain URLs (my call).
- A malformed section is a parse error, as a malformed note line is.

## 9. Resolution contract

A local path in an entry resolves as a linked spec or plan does:

- relative to the project root the resolver uses;
- the current file, not a pinned revision;
- this checkout first, then the main checkout.

The resolver gains `locate_entry`, because `Resolver::locate` recognises only files and
spec and plan must stay file-only:

- **Precedence.** If anything exists at the path in this checkout (file or directory),
  it is the entry's target, and the main checkout is not consulted. The main checkout is
  consulted only when nothing exists at the path here. A directory in the worktree
  therefore never yields to a file at the same path in the main checkout, and the
  reverse holds too.
- **Result.** The located path plus whether it is a file or a directory. Symlinks are
  followed, as `is_file` follows them today.
- **`locate` is unchanged.** It stays file-only, so spec and plan links keep requiring a
  file.

Whatever tasks-dc16eb decides about document roots, and tasks-6360ac about overlay
document roots, applies to references through the resolver, with no change to the
record. If documents move to another root, `list --ref` finds the affected records and
`edit --ref` rewrites each entry in place, while prose citations stay valid because they
name handles. Branch-only documents (tasks-a73dce, tasks-82bd08) are resolved for
references when they are resolved for spec and plan, and not before.

This design does not wait for tasks-dc16eb's relocation rollout. It depends only on
references going through the resolver, never building paths of their own.

## 10. Skills

- **`skills/tasks/SKILL.md`**:
  - When filing or scoping, documents the work cites go in `--ref` entries and the
    prose cites them as `[ref:<handle>]`, saying why each matters.
  - Mark `required` only what must be read first, in the order to read it.
  - Declare outputs the task will produce.
  - On picking up a task, read the inventory's required rows from `tasks show` before
    starting.
  - Includes the kind vocabulary of §3.4.
- **`skills/scope/SKILL.md`**: a scope pass that writes a brief adds it as a reference
  on the tasks it scopes.
- **`skills/curate/SKILL.md`**: promoting a prose path to an entry, with the prose
  rewritten to cite it, becomes one of curate's bounded edits.
- Skills owned by other projects (flow in flows, quick-add in ops, lore's instruction
  corpus) are follow-ups filed on those projects when this lands. They are not part of
  this design.

## 11. Non-goals and deferred work

- **Code locations** (path plus symbol, drift): their own design, separate from
  references.
- **Cross-project references** (`@<prefix>/<path>` through the registry): deferred, with
  `@` reserved (§3.3). Until then, cite another project's document in prose or through
  an external scheme.
- **Pinned revisions** for evidence and reviews (`rev=<commit>`): deferred, and the
  attribute grammar leaves room for them. A reference names the current document. For a
  tracked local file, git history can recover an earlier version, but nothing in the
  record says which version was meant. External documents and untracked local artifacts
  have no such recovery. A task that needs evidence fixed in time attaches a copy until
  pinning exists.
- **Locators in entries** (a structured section or heading): deferred. Sections live in
  the citing prose and the description.
- **References to tasks**: dependencies and parents already relate tasks, and prose
  cites ids.
- **A "pickup packet"** that prints every required document's body in one call, and
  reading-list output from `start` or `prime`: deferred until agents' use of the
  inventory shows a need.
- **Migrating existing prose**: none. Records without the section parse unchanged, and
  curate promotes paths over time.
- **Kind-specific behaviour, kind inference, per-project kind dictionaries.**

## 12. Compatibility and rollout

- Records without a `## References` section are unchanged.
- A host running an older binary reads the section as body text and writes it back
  unchanged on any write that keeps the body. An older binary's `edit --body` would
  replace it along with the rest of the body. Install the new binary on every host
  (`cargo install --path .` here, and each host's usual route) before skills tell
  agents to write references.
- Order: ops `cli.toml` rows, then the parser, format, resolver, show, check, doc,
  list, transition, detach and move changes, with the vendored table adopted in the
  same commit. The skills update last, once the binary is installed.

## 13. Calls made in this design

These are my calls, settled without the user and open to change in review:

1. Storage as a managed body section, not frontmatter (§3.1).
2. One entry grammar shared by the file and `--ref`, with the frontmatter subset's
   quoting for references that need it (§3.2, §6.1).
3. `required` allowed on every relation, including outputs that already exist (§3.2).
4. Spec and plan lead the inventory and are always required (§5).
5. An entry equal to the spec or plan merges into the governing row, not refused, when
   both resolve to the same target. Otherwise it keeps its own row, with a warning
   (§5.2).
6. The reading list is a filter of the inventory, with no field of its own (§5.3, §7.2).
7. Citations validated in the body only, not in notes; code spans and fences escape
   (§4).
8. The completion gate is not overridable by `--force` (§6.3).
9. `check` does not fetch external references (§8).
10. Directories and any file type allowed as local references, with worktree-first
    precedence (§3.3, §9).
11. `list --ref` included in this increment (§6.2).
12. Cross-project references, pinned revisions and entry locators deferred, with room
    kept for them (§11).
13. Entry references unique within a task (§3.2).

## Revision history

- Round 1 (codex/gpt-6-astra, revise; P2 6, minor 3):
  - one inventory with origins, and the reading list derived from it (§5, §7.2);
  - outputs separated from existence, with `required` allowed on outputs and pending
    attachment outputs (§3.2, §3.3);
  - the completion gate on every fresh transition to done, with already-done cleanup
    preserved (§6.3);
  - quoted references (§3.2);
  - directory spelling and `locate_entry` precedence (§3.3, §9);
  - plan rows carry `step` and `step_found`, with a status for every fetch outcome
    (§7.2, §7.3);
  - citation escaping via code spans (§4);
  - the JSON omission convention (§7);
  - the git-history claim corrected (§11).
- Round 2 (codex/gpt-6-astra, revise; P2 2):
  - entry references unique within a task (§3.2, §8);
  - merging only when the resolved targets agree, with rules for missing and external
    targets and for spec and plan sharing a document (§5.2).
- Plan review round 1:
  - special filesystem objects refused as `ref_special_file` (§3.3);
  - attachment rows built from the ledger, in name order, with `missing` for a recorded
    attachment whose file is gone (§5.1).
