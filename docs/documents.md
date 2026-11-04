# Documents and references

Type: guide, data model; document types 1.

This reference combines using documents with their small reference and provider
formats: resolution rules are needed to interpret the stored values correctly.

## Working documents and resolution

`tasks docs` lists Markdown files directly under configured spec/plan roots and
`docs/notes` in the project's records directory. Repeated `--kind` widens selection;
kinds are spec, plan, brief, handoff and note (directory first, then brief/handoff
filename suffix). `--search` matches title/body. Each row contains project, kind,
stored path, absolute file, first H1 title (else stem), and tasks linking that exact
path through spec, plan or ref, including closed tasks. A non-UTF8 file is listed
under its stem with a warning and never matches search; broken `.md` symlinks warn,
and unreadable files fail. Detached code reference documents are not listed here.

`--spec` and `--plan` accept local paths within configured roots, bare names
searched within those roots, or external references. A configured root list
replaces defaults. `--step` requires a plan and an exact heading in it. Drift
checks link plan headings and task steps; moved-only historical links do not
cover current local plan headings. Missing plans on closed tasks can be historical
warnings; open work still requires its linked local plan.

Resolution checks records first, then a records main checkout when applicable,
then (for detached projects) the invoking code worktree and code main checkout.
The first existing candidate wins. A detached code reference is an intentional
public-code location and does not produce `ref_in_main_checkout`; an ordinary
main-checkout fallback is reported. Present unreadable files fail instead of
falling through. Root strings such as `docs/specs` remain valid project working
document locations even when a particular code repository keeps none there.

## Task references

`add --ref` and `edit --ref` accept entries stored as bullets in the managed
References section before Notes:

```text
- <handle> (<relation>[, required][, kind=<kind>]): <reference>[ — <description>]
```

Omit `- ` in the CLI argument. Handles match `[a-z][a-z0-9-]{0,31}` and are unique.
Relations are context, input and output. Kind is a descriptive
`[a-z][a-z0-9-]*` label. Attribute order is fixed as above. Descriptions use
space/em-dash/space. Decoded targets must also be unique within the task.

Targets are normalized repo-relative files/directories (no trailing slash),
`files:<name>` attached items, or `<scheme>:<remainder>`. Cross-project `@`
references are unsupported. Whitespace, quote or backslash requires double
quoting with only `\"` and `\\` escapes. Changed inputs/context must exist locally
or be attached; outputs may be pending. External shape is validated without
fetching for ordinary ref-entry validation.

`edit --ref` replaces a handle in place or appends it. `--rm-ref` removes named
handles; `--no-refs` conflicts with both. Body citations use `[ref:<handle>]`;
unknown/malformed citations fail saves and open-task checks. Citations inside
CommonMark code spans/blocks are literal; notes are not audited. Remove body
citations together with entries. `list --ref`, `search --ref` and `--plan`
filters compare the stored reference exactly without fetching or alias expansion.

## Read and completion gates

`tasks doc <id> <handle>` or `--spec` or `--plan` accepts exactly one target.
It prints exact UTF-8 body bytes, without newline/envelope/color/header even
with --json/--pretty. Directories/binary files fail `ref_not_text`; missing or
pending targets fail `doc_not_found`. Attachment reads require live audited
storage; logical multipart reads reconstruct verified text and individual
members cannot be used to bypass the logical declaration.

`show.inventory` orders spec, plan, explicit refs, source, then unreferenced
attachments by name. Rows contain origin/location/status and entry metadata.
Refs resolving to the same spec/plan merge into that row; attachment refs replace
standalone rows. Spec and plan are always required, then refs marked required.
Missing outputs are pending; inventory never fetches external refs. Full Task
JSON has nonempty refs; summary rows omit them.

Every fresh done transition (including editor/edit and recurring/reopened
completion) requires local output files and attachments to exist. Special files
are refused. `--force` cannot bypass `ref_output_missing`. Produce the outputs
or explicitly remove their refs; drop and external outputs are not checked.
Repeating done on an already closed task still releases its claim even when an
output has since vanished.

## External providers

Schemes match `[a-z][a-z0-9-]*`; the nonempty opaque remainder can include
spaces/quotes/colons/slashes but no CR, LF or NUL. A colon in the first path
segment reserves external syntax; `docs/plans/x:y.md` remains local. Working
document roots cannot contain a first-segment colon. Host configuration declares
trusted scheme commands; `files` is reserved for attachments.

Tasks invokes `sh -c '<command> "$1"' -- <remainder>` with stdin closed: the
remainder is one literal argument. The provider returns one UTF-8 JSON object
with nonempty single-line, NUL-free string `id` and string `body`; extra fields
are allowed. Successful writes store canonical `<scheme>:<id>`. Each strict
fetch has 30 seconds, 2 MiB stdout and a diagnostic prefix of at most 200 stderr
bytes. Remaining process-group members are terminated on shell exit or limits.
Providers must not daemonize or escape that group; escaped jobs remain their
responsibility, although pipe reads stay deadline-bounded.

Writes fetch changed spec/plan external links and verify a step only when its
plan/step changes. Alias changes count; unrelated edits work without a provider.
Document export and moving a task with an external step require successful
reads; a move without a step preserves external references without fetching.
Results are reused within a resolver/read, not an advertised durable cache.

`check` spends one five-second budget with up to four concurrent fetches,
prioritizing open stepped plans, other open plans, open specs, then closed-only
plans; references sort lexically within tiers and shared references fetch once.
Missing schemes/attempted failures warn `doc_unreachable` per open task;
unfinished references produce one priority-ordered `doc_unchecked` summary.
Local checks continue, and available plans still fail missing steps. Malformed
host settings are fatal.

`show`/`next` spend at most five seconds fetching a stepped external plan.
Unavailable open plans warn and omit step_found; closed plans warn historically
and use false. Without step, these commands do not fetch; specs never trigger
their optional fetch. External paths display stored references, local paths
absolute filenames. `--no-fetch` runs no provider for show/next/doc: show/next
omit external step_found without availability warnings; doc fails external
reads with `doc_not_found`. Local reads continue normally.

## Failures and rollout

Missing local plans for open work, unsafe attachments and unreadable present
files remain errors. A closed missing plan warns with step_found false when a
step exists; a readable plan still checks its heading. A skipped fetch establishes
neither availability nor absence. Exact stored-string filters do not discover
equivalent provider aliases.

Upgrade host binaries before adding provider settings, and every synced-record
reader before creating external links. Remove settings before downgrading the
host and remove external links before downgrading all readers. Workflow activation
requires the separately owned ops provider pilot: three clean full-backlog checks
per host with its real wrapper/web app, under five seconds with no doc_unchecked.
This reference neither activates providers nor claims that pilot has passed.
