# Task references: one explicit, ordered list of what a task cites

Status: draft, awaiting the user's review
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

This design adds **task references**: an ordered list of the documents and artifacts a
task cites, each with a short handle that prose cites. A computed **reading list** says
what to read before starting: the governing spec and plan, then the references marked
required.

Success means the following:

- A task like crew-5aa683 states its reading in the record. `tasks show` reports that
  list, in order and with each entry's status, in JSON and in pretty output.
- `check` catches a cited document that disappears from open work.
- A later move of documents (tasks-dc16eb) can rewrite references by handle, not by
  searching prose.

## 2. Inputs settled before this design

These come from the 2026-10-19 discussion and a codex review of the proposal (recorded
on the task):

- `spec`, `plan`, `step` and `source` keep their fields and roles. A spec cited as a
  reference is context, not the task's governing spec.
- The inventory is not the reading list. Being required is a marker of its own,
  separate from the relationship, and entry order is reading order. Sources and
  attachments appear in the inventory but are never added to the reading list
  automatically.
- Relationship, kind and location are separate dimensions. Kind is descriptive only.
- Prose cites a reference with a reserved form, not a bare bracket, and the sentence
  explains why the reference matters. The tool does not warn about raw paths in prose.
- Locating and reading are separate. `Resolver::read` reads UTF-8 bodies only.
- Expected outputs and produced outputs differ: an output not yet written must not fail
  the existence check that an input does.
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
- sweep-spec (output, kind=spec): docs/specs/2026-10-20-trigger-sweep-design.md — the spec this task writes

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
- **relation**: `context` (consult when needed), `input` (the work uses it) or `output`
  (the work produces it).
- **required**: puts the entry on the reading list (§5). It is allowed on `context` and
  `input` and refused on `output`, because a document the task has not yet produced
  cannot be read first.
- **kind=**: optional, `[a-z][a-z0-9-]*`. It is descriptive and not checked against any
  list (§3.4).
- **reference**: one whitespace-free token in one of the forms of §3.3.
- **description**: optional, one line, after ` — ` (space, em dash, space). It says what
  the referenced thing is. Why it matters belongs in the prose that cites it (§4).

Attributes follow a fixed order: the relation, then `required`, then `key=value` pairs.
That leaves room for later attributes, such as a pinned revision (§11), without a new
grammar. An attribute the binary does not know is a parse error, as an unknown
frontmatter key is today.

### 3.3 Reference forms

| Form | Example | Located by |
|---|---|---|
| Local path | `docs/notes/x.md`, `results/run-12/` | `Resolver::locate`: this checkout, then the main checkout |
| Attachment | `files:screenshot-1.png` | the task's attachment ledger |
| External | `mindful:thought:<hex>`, `https://…` | `[docs.schemes]` when read; never fetched to list |

- **Local path.** A normalized repo-relative path, as `validate_doc_shape` requires,
  except that any file type is allowed, not only `.md`, and so is a directory, for
  datasets and result folders. Its first segment contains no colon.
- **Attachment.** `files:<name>`, where `<name>` passes `attachments::validate_name` and
  the ledger holds it as attached. `files` becomes a reserved scheme: a
  `[docs.schemes]` entry named `files` is a configuration error.
- **External.** The `<scheme>:<rest>` grammar of the external document references spec
  (`docs/specs/2026-10-11-external-document-references-design.md` §3.1). An `https:`
  URL is external. Without a configured fetcher it can be listed but not read.
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

- A citation must name an existing entry. `add` and `edit` refuse a write that would
  leave a citation without its entry, whether from a new body, `--rm-ref` or
  `--no-refs`, naming the handle. `check` reports `ref_citation_dangling` for one that
  arrives by hand edit.
- An entry need not be cited.
- Only the body is checked. Notes are append-only history, so a note's citation text is
  not validated and does not pin an entry (my call).
- Several citations may point at different sections of one entry. The section is prose
  ("§7"), not part of the entry.

The tool never scans prose for raw paths. Promoting cited paths to entries is curate's
work (§10).

## 5. The reading list

The reading list is computed, never stored. In order, it holds:

1. the governing `spec`, when one is linked;
2. the `plan`, when one is linked, with its `step`;
3. every entry marked `required`, in entry order.

It is what an agent or the user reads before working on the task. Including spec and
plan is my call: the workflow already requires reading them, and the list should be
complete without a reader having to merge it with them. Sources, attachments and
entries not marked required are never on the list.

## 6. CLI

Every option below starts as rows in ops's `cli.toml` and is adopted into
`tools/cli.toml` together with the parser change (AGENTS.md, Rules).

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
- **`list --ref <reference>`**: keeps tasks holding an entry whose reference equals the
  value byte for byte, like `list --source` and `list --plan`. It answers "which tasks
  cite this document" before a document is moved or rewritten.
- **`doc <id> <handle>`**: the `doc_kind` group becomes exactly one of `--plan`,
  `--spec` or a handle. A local file or attachment prints when it is UTF-8 text. A
  directory or binary file fails as `ref_not_text`, naming its absolute path. An
  external reference goes through `[docs.schemes]` as an external plan does, and
  `--no-fetch` applies.

Write-time validation for `--ref`:

- the grammar of §3.2 and the forms of §3.3;
- for `context` and `input` entries, a local path or attachment must exist, found the
  way `Resolver::locate` finds a spec (this checkout, then the main checkout, with the
  same warning), or the write fails as `ref_not_found`;
- an `output` entry is not checked for existence;
- an external reference is checked for shape only and never fetched.

Interactions with existing commands:

- **`detach`** refuses an attachment that an entry names (`ref_cites_attachment`),
  pointing at `edit --rm-ref`.
- **`done`** refuses while an `output` entry's local path or attachment is missing
  (`ref_output_missing`), pointing at producing it or at `edit --rm-ref`. That gate is
  where an expected output either becomes a produced one or is dropped explicitly (my
  call). `drop` does not check outputs.
- **`move`** preflight validates `context` and `input` local paths against the
  destination, as `validate_destination_docs` does for spec and plan, and refuses with
  `edit --rm-ref` as the remedy. Attachment entries move with the attachments.
- **`search`** does not match entries; it searches the body. `list --ref` is the query
  for entries.

## 7. Output

The JSON contract changes only by addition.

- **`task.refs`**: on the full task object (`show`, `next`), always present, `[]` when
  empty. Each element is `{handle, relation, required, kind, reference, description}`,
  with `kind` and `description` null when absent. Summary rows (`list`, `ready`,
  `prime`, `tree`) do not gain it.
- **`ShowFields.references`**: omitted when empty, like the other optional show fields.
  One element per entry, in entry order: `{handle, location, path, status}`.
  - `location` is `path`, `attachment` or `external`.
  - `path` is absolute for local paths and attachments and null for external references.
  - `status` is one of:
    - `present`: found here or in the main checkout, with the resolver's main-checkout
      warning in the latter case;
    - `missing`: an absent `context` or `input` entry;
    - `pending`: an absent `output` entry;
    - `unchecked`: external, because `show` never fetches.
- **`ShowFields.reading`**: omitted when empty. It holds the list of §5 in order, each
  element `{source, handle, reference, path, status}`. `source` is `spec`, `plan` or
  `ref`, and `handle` is null for spec and plan. The spec and plan elements repeat what
  `spec_path`, `plan_path` and `step_found` already report, so the list stands alone.
  Those fields keep their meaning.
- **Pretty `show`**: a `references` section after `related`, opening with a "read
  first" line of the reading list, then one row per entry: handle, relation (marked
  when required), kind, reference, status when not `present`, and description. The
  plan settles the layout within the show layout spec's conventions
  (`docs/specs/2026-10-03-show-layout-design.md`).

## 8. Check

These findings apply to open tasks only. A closed record's references are history, as
its spec and plan links are (check's "Spec §7" rule).

| Finding | Severity | When |
|---|---|---|
| `ref_missing` | error | A `context` or `input` local path or attachment is absent, matching `doc_missing` for spec and plan. |
| `ref_in_main_checkout` | warning | Found only in the main checkout, matching `doc_in_main_checkout`. |
| `ref_citation_dangling` | error | The body cites a handle with no entry. |

- Outputs are not checked while the task is open. `done` gates them (§6).
- `check` does not fetch external references. Unlike a linked external spec, which
  `check` fetches, references can be many and are often plain URLs (my call).
- A malformed section is a parse error, as a malformed note line is.

## 9. Resolution contract

A local reference resolves exactly as a linked spec or plan does:

- relative to the project root `Resolver::locate` uses;
- the current file, not a pinned revision;
- this checkout first, then the main checkout.

Whatever tasks-dc16eb decides about document roots, and tasks-6360ac about overlay
document roots, applies to references through that one resolver, with no change to the
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
  - On picking up a task, read `reading` from `tasks show` before starting.
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
- **Pinned revisions** for evidence and reviews (`rev=<commit>`): deferred. The
  attribute grammar leaves room for them. No consumer needs a pinned read yet, and git
  history records what the file held when the entry was written.
- **Locators in entries** (a structured section or heading): deferred. Sections live in
  the citing prose and the description.
- **References to tasks**: dependencies and parents already relate tasks, and prose
  cites ids.
- **A "pickup packet"** that prints every required document's body in one call, and
  reading-list output from `start` or `prime`: deferred until agents' use of `reading`
  shows a need.
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
- Order: ops `cli.toml` rows, then the parser, format, show, check, doc, list, done,
  detach and move changes, with the vendored table adopted in the same commit. The
  skills update last, once the binary is installed.

## 13. Calls made in this design

These are my calls, settled without the user and open to change in review:

1. Storage as a managed body section, not frontmatter (§3.1).
2. One entry grammar shared by the file and `--ref` (§3.2, §6).
3. `required` refused on outputs (§3.2).
4. Spec and plan lead the reading list (§5).
5. Citations validated in the body only, not in notes (§4).
6. `done` gates missing outputs; `check` ignores outputs (§6, §8).
7. `check` does not fetch external references (§8).
8. Directories and any file type allowed as local references (§3.3).
9. `list --ref` included in this increment (§6).
10. Cross-project references, pinned revisions and entry locators deferred, with room
    kept for them (§11).
