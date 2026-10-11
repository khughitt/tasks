# External plan and spec references

Status: draft for user review
Task: `tasks-91f63b`
Source: hq `docs/specs/2026-10-10-agent-docs-collection-design.md`, §3–§4
(accepted in `hq-5a3e12`, spec round 4; its draft header is stale).

## 1. Goal and scope

A task can link a plan or spec held outside its repository, and the tracker can
read it through a host-configured command. The motivating reference is
`mindful:thought:<32 lowercase hex>`. Tasks knows only the scheme and opaque
remainder; the fetcher owns mindful's IDs, aliases, JSON and storage layout.

Success means that writes store canonical references, heading checks use the
fetched body, and records remain parseable on a host without the scheme. Execution
helpers get the body through `tasks doc` and find completed steps through
`tasks list --plan <reference> --status done` across parents.

This implements the tasks portion of the accepted source design. The mindful
fetcher, host configuration rollout, plan snapshots, profile changes and the
broader agent workflow belong to the other filed pieces. Existing repository
documents stay where they are. This feature does not migrate them, change
`--source`, introduce approval state or write external documents.

## 2. Approach and existing behavior

Extend `src/resolve.rs` with one document reader. This keeps local lookup,
worktree/main-checkout resolution and external fetching behind the resolver that
callers already use. Rejected alternatives are independent fetch paths in each
command, which duplicate validation and failure handling, and direct mindful
file reads, which couple this CLI to another tool's storage format.

The present boundaries that need to change are:

- `src/format.rs`: document shape and configured-root validation, reached by
  record parsing and `Project::validate_docs`.
- `src/config.rs`: host config currently contains only the identity table.
- `src/resolve.rs`: bare-name/path resolution, `step_exists`, display paths and
  warnings for documents read from the main checkout.
- `src/commands/mod.rs` and `edit.rs`: flag writes and editor saves;
  `moving/preflight.rs`: destination document validation.
- `src/commands/check.rs`: document existence, step headings and uncovered plan
  headings; `show.rs::describe`: heading reads shared by `show` and `next`.
- `src/cli.rs`, command dispatch and a new `commands/doc.rs`: document export
  and the exact plan filter; `output.rs` and `main.rs`: raw export and step rows.
- `tools/cli.toml`: adopted shared command inventory; README and the tasks skill:
  public reference/configuration/output contracts and rollout requirements.

Keep `Located` as a local-filesystem concept. An external reference never becomes
a synthetic file path or a new `Located` variant.

## 3. Reference and configuration contracts

### 3.1 Shape

Inspect the first path segment, before any `/`. If it contains `:`, the value
must be `<scheme>:<rest>`, with scheme `[a-z][a-z0-9-]*` and a nonempty remainder.
Split at the first colon in the full value: further colons, slashes, spaces and
quotes belong to the opaque remainder. Newlines, carriage returns and NUL are
invalid. Do not trim or decode the remainder.

Examples: `mindful:thought:abc`, `x:y`, `x:folder/item` and `x:/item` are external;
`:y`, `x:`, `Docs:plan.md` and `9x:y` are invalid. `docs/plans/x:y.md` is a
repository path because the colon is outside its first segment. Without a colon
in that segment, retain current normalized `.md` path rules and bare-name lookup.

`validate_doc_shape` accepts a valid external reference without loading host
config. `validate_doc_path` skips configured roots only for that form. Use one
shared classifier at these boundaries and in the resolver; a malformed
colon-bearing first segment cannot fall through to bare-name lookup.

### 3.2 Host configuration

```toml
[docs.schemes]
mindful = "mindful-doc"
```

Extend `HostConfig` with the scheme-to-command table, empty by default. Retain
`deny_unknown_fields` on the root and nested configuration tables. Validate
scheme keys against the reference grammar; reject blank commands and commands
containing NUL. Keep identity settings and their defaults unchanged.

The location remains `$XDG_CONFIG_HOME/tasks/config.toml`, or
`$HOME/.config/tasks/config.toml`. Project configuration does not accept commands.
Record parsing and commands that only scan records never load this table merely
because a link is external. The resolver loads it lazily when a fetch is needed,
once per resolver; malformed/unreadable host config remains a fatal typed error.

## 4. Reader and fetch boundary

### 4.1 Reader

```rust
struct Doc {
    reference: String,
    body: String,
}

Resolver::read(kind: DocKind, value: &str) -> Result<Doc>
```

For a stored repository path, use existing lookup followed by `read_to_string`;
`reference` is the stored relative path. Preserve worktree-first lookup, the main
checkout fallback, and existing warnings and I/O errors. A non-file path,
invalid UTF-8 or metadata/read error must not become an unavailable historical
document. Keep the existing distinction between path resolution on writes
(`DocNotFound`) and a failed body read (`Io`).

For an external reference, run its configured fetcher and return the original
scheme plus the canonical remainder returned by it, with the fetched body.
`resolve_doc` preserves local path/name resolution; for an external input it
uses the fetch result's `reference`. `step_exists` checks `read(Plan, value).body`.
`abs` returns an external reference verbatim without fetching or adding a
main-checkout warning.

Reuse a successful external fetch within a resolver, including the canonical
reference after resolving an alias, so canonicalization and heading validation
use the same body. No cross-command or disk cache: the next invocation fetches
again. `check` also retains failed document outcomes for its invocation, so a
shared unreachable plan is not retried for each task.

### 4.2 Fetch protocol

Run the trusted host command as `sh -c '<cmd> "$1"' -- <rest>`, with null stdin
and piped stdout/stderr. Only the host's command is shell syntax; the reference
remainder is one positional argument and is never interpolated into shell code.
Use the invoking CLI's working directory and environment, as the editor does.

Success requires exit 0 and exactly one UTF-8 JSON object:

```json
{"id":"thought:0123456789abcdef0123456789abcdef","body":"# Plan\n"}
```

Both fields are required strings. The ID is a nonempty, single-line, NUL-free
opaque remainder; validate the assembled external reference before returning it.
An empty body is valid. Accept additional object fields but reject malformed
JSON, trailing non-whitespace output, missing/wrongly typed fields and invalid
IDs. Reads do not rewrite task records when a canonical ID changes.

Use a fixed 2 MiB stdout cap, the existing clipboard default, independent of
project attachment settings. Retain only the first 200 bytes of stderr while
draining the rest; render that prefix with lossy UTF-8 decoding. Read stdout and
stderr concurrently, with bounded retention. Reuse the capped-read pattern but
map overflow to `DocNotFound`, without attachment-specific diagnostics.

A 30-second monotonic deadline covers child execution and both pipe reads.
Putting the deadline after a blocking read is insufficient. Overflow, timeout,
spawn/read failure and nonzero exit return `DocNotFound` identifying the
reference and cause; include the bounded stderr prefix when available.

On Unix, start the shell in its own process group. On failure terminate the
group, wait for the direct child and finish both readers. On success terminate
any remaining descendants before returning as well. A shell that exits
while a descendant holds a pipe is still subject to the deadline. Finish cleanup
before returning; the fetch protocol permits no surviving background jobs.
Implement this locally with the standard library and existing `libc` dependency,
without adding a generic subprocess framework or touching the clipboard runner.

A missing configured scheme is `Error::Config` naming `[docs.schemes]`, the
scheme and the host config location. Fetch failures use `Error::DocNotFound`.
The consuming commands distinguish those expected availability failures from
malformed host configuration and local I/O errors without parsing error strings.

## 5. Command behavior

### 5.1 Writes and movement

Flag-based `add`/`edit` and editor saves fetch external links and store the
canonical reference before writing. Editor validation currently discards
`resolve_doc`'s return value; assign it back to `edited.spec`/`edited.plan`.
Validate any step against the same fetched body. A failed fetch or missing
heading leaves the record unchanged; editor errors retain the edited file as
today. Existing source-idempotent adds retain their reuse behavior.

For `move`, an external link is project-independent: destination root validation
accepts it and destination document preflight does not fetch or canonicalize it.
Preserve the reference verbatim. A linked step still requires a successful plan
fetch and heading check before movement. Thus movement without a step works on
a host without the scheme, while movement with a step refuses if it cannot
verify that heading. This is the specific exception in source §4.4 to its more
general write-time summary.

### 5.2 Check and historical display

`check` reads each distinct linked plan once per invocation and uses that body
for both `step_missing` and `unlinked_step`. Share external reads across kinds
when a spec and plan have the same reference. Keep findings per affected open
record, even when the document read is shared.

| Condition | Result |
|---|---|
| Open task, missing local path | Existing `doc_missing` error |
| Local document read or host configuration error | Existing fatal typed error |
| Open task, external scheme missing or fetch failed | `doc_unreachable` warning, naming reference and cause |
| Available plan lacks an open task's step heading | `step_missing` error |
| Available plan heading has no linking task | Existing `unlinked_step` warning |
| Closed task alone links an unavailable external plan | No drift finding |

After an unavailable document, skip dependent heading checks; do not pretend it
is an empty body. Closed records still count when matching plan headings to
steps, as today. Available closed-only plans still receive the existing
uncovered-heading check; an unavailable closed-only external plan is skipped.
Warnings alone keep `check` at exit 0. Preserve `doc_in_main_checkout` findings.

`show`/shared `describe` keeps the external reference in `spec_path`/`plan_path`.
A closed task's unavailable external plan gives the existing historical-plan
warning and `step_found: false` when it has a step; otherwise omit that field.
An available plan reports its actual heading result. Ordinary record scans
remain possible without the fetcher; an open task's requested step validation
retains strict failure behavior, as for local plans. Malformed host config is
fatal rather than being labelled an unavailable historical plan.

### 5.3 Document export

```text
tasks doc <id> --plan
tasks doc <id> --spec
```

Exactly one selector is required; neither or both is a clap usage error (exit 2).
Use existing ID routing, shorthand and registered-project handling, including
the recorded-checkout lookup used by `show` when this checkout lacks a task.
Read the task without calling `describe`: a spec export must not fetch its plan.
An unset selected link is `DocNotFound`; a missing task retains `TaskNotFound`.
There is no special historical tolerance for this explicit body request.

On success stdout is the selected body's exact UTF-8 bytes, with no added newline,
JSON envelope, color or terminal layout. This is a deliberate export exception
to JSON-default output, including under `--json`, `--pretty` or `TASKS_FORMAT`.
Warnings go to stderr in every mode; errors use the existing structured error
and exit 1. Fully read/validate the body before emitting it, so fetch failures
cannot leave a partial snapshot. Preserve existing broken-pipe behavior.

### 5.4 Exact plan filter and rows

Add a single `--plan <value>` to `ListArgs`, shared by `list` and `search`, not
the filters shared with readiness commands. Match `task.plan` byte for byte,
with no fetch, canonicalization, path resolution or parent restriction. A value
held by no task yields the usual empty list. Combine with existing filters and
status pools; `--status done` is how snapshot helpers include completed steps.
Apply it in parked mode as well, using the resolved record's plan; an unresolved
park cannot match.

Add optional `step` to `TaskSummary` and its resolved `ParkedRow` conversion,
omitting it when unset. Shared summary consumers gain that same additive field;
no pretty-table columns change. Do not add a plan field to rows or introduce
a separate list schema. Task objects already carry `plan` and `step`.

## 6. Documentation and shared inventory

Document the reference grammar, fetch JSON/config, availability warning,
`tasks doc` export exception, exact plan filter and additive step field in the
README. Update the tasks skill's link/read guidance in step with implementation.
The separate `tasks-00723a` owns the wider agent-document workflow changes.
Add ordinary task-ID completion to `doc`; do not execute fetchers during TAB.

The command surface test compares the parser with vendored `tools/cli.toml`.
Changes must land in its ops authority before `vendored adopt cli.toml` brings
them here, per `tools/README.md`. Register `doc`, its selectors, the list/search
plan filters and its documented output/verb exception there. This is an
implementation prerequisite, not permission to hand-edit the vendored copy.
No shared vocabulary, host config or launcher is changed during this design phase.

## 7. Verification and rollout

Use the existing isolated integration environment with scratch XDG config and
state, and local stub fetchers; do not require mindful or the live desktop.

- Shape/config units cover valid and invalid schemes, slash/colon boundaries,
  opaque arguments, no-config parsing, config defaults and unknown keys.
- Writes cover canonicalization for add, flag edit and editor save; heading
  presence/absence; atomic failure; and unchanged source-idempotent reuse.
- Fetch tests cover exact argument delivery with shell metacharacters, valid
  empty bodies, malformed JSON/fields/IDs, nonzero exit, bounded stdout/stderr,
  a hung command, and a shell descendant holding a pipe after the shell exits.
  Assert child cleanup, not merely the timeout error. A private deadline
  parameter lets focused unit tests exercise this promptly; production stays
  fixed at 30 seconds, with no public timeout option. Keep one real CLI deadline
  case to verify production wiring, ignored in the fast set and included in the
  full suite so each ordinary code commit does not add a 30-second wait.
- Check tests cover both heading findings, warning-only exit 0, missing schemes,
  failed fetches, one fetch for many linking records and failed-read reuse,
  closed-only plans, and existing main-checkout/path I/O behavior.
- Show/next tests cover reference display, historical unavailable plans with and
  without a step, available changed headings and strict open-step reads.
- Export tests assert byte-exact stdout (with and without a final newline),
  stderr warnings in all formats, unset link, selector usage, foreign/shorthand
  IDs and local/main/overlay/recorded-checkout reads.
- Filter tests use open and completed steps under different parents, exact
  misses, conjunctions, parked/search modes and omitted/present step fields.
- Move tests prove no-step external links need no host scheme, step-bearing
  moves validate fetched headings, and refusals leave source/destination intact.

At execution setup run the deferred worktree baseline. Use `just test-one`
for affected tests, `just test-fast` before code commits and `just check` for
parser-inventory, hygiene, formatting, lint and record consistency. Keep full
suite execution with the configured hook policy. Install to a disposable Cargo
root and invoke that binary explicitly for the initial installation smoke.

An old binary rejects these references at parse time. Before the first external
link is written into synced records, install the reviewed new binary on both
hosts and verify version/help plus a no-scheme record read/check on the second
host. Record each host's evidence; an unavailable host leaves rollout pending.
Live installation is a separately approved host action, with no worktree pointer
repointing. ops supplies `mindful-doc` and its host config in its rollout task.

No persistent fetch cache, provider SDK, new dependency, direct mindful storage
access or migration of existing documents is included.
