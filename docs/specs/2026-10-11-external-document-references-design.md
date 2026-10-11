# External plan and spec references

Status: accepted by Keith after spec round 3 (2026-10-11)
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
- `src/repo.rs::doc_root`: reserve a colon in the first segment for external
  references, including at configured-root validation.
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

`doc_root` rejects a colon in the first segment of `spec_dirs` or `plan_dirs`
with `Error::Config`; a later segment may contain one. This prevents a configured
root such as `notes:x` from silently turning repository paths into external
references. It tightens an existing permissive config rule. No registered
project currently uses these roots, per the review's corpus check; recheck this
before rollout. A conflicting custom root must be renamed before upgrading,
rather than being reinterpreted. Default roots and stored ordinary paths retain
their meaning.

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

enum UnavailableCause {
    MissingScheme { scheme: String, config: PathBuf },
    FetchFailed(String),
    BudgetExhausted,
}

enum DocRead {
    Available(Doc),
    Unavailable { reference: String, cause: UnavailableCause },
}

Resolver::read(kind: DocKind, value: &str) -> Result<DocRead>
DocRead::required(self) -> Result<Doc>
```

`Ok(Unavailable)` represents an external document this host could not reach;
it is never a substitute for a body. `Err` retains malformed/unreadable host
config, invalid input and local I/O failures. Strict callers use `required`:
`MissingScheme` maps to `Error::Config`, while `FetchFailed` and
`BudgetExhausted` map to `Error::DocNotFound`, with reference and cause.
`check` and descriptive reads consume the outcome directly and emit warnings.
This internal distinction leaves existing public error kinds intact and avoids
matching error text. Local-file reads never return `Unavailable`.

For a stored repository path, use existing lookup followed by `read_to_string`;
`reference` is the stored relative path. Preserve worktree-first lookup, the main
checkout fallback, and existing warnings and I/O errors. A non-file path,
invalid UTF-8 or metadata/read error must not become an unavailable historical
document. Keep the existing distinction between path resolution on writes
(`DocNotFound`) and a failed body read (`Io`).

For an external reference, run its configured fetcher and return the original
scheme plus the canonical remainder returned by it, with the fetched body.
`resolve_doc` preserves local path/name resolution; for a changed external input
it requires the fetch result and uses its `reference`. Strict `step_exists`
checks the required body; `describe` uses `read` directly to handle unavailability.
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
report overflow as `Unavailable(FetchFailed)`, without attachment diagnostics.

A 30-second monotonic per-fetch deadline covers child execution and both pipe reads;
an optional command budget can shorten it (§5.2).
Putting the deadline after a blocking read is insufficient. Overflow, timeout,
spawn/pipe-read failure, invalid fetch JSON and nonzero exit return
`Unavailable(FetchFailed)` identifying the
reference and cause; include the bounded stderr prefix when available.

On Unix, start the shell in its own process group. Once the shell exits, on
success or failure, forcibly terminate remaining members of that group and
finish both readers within the deadline. On exit 0, drain buffered stdout to
EOF after terminating descendants, then validate the complete JSON; do not wait
for a background child's natural exit. A valid complete response can therefore
succeed even if a descendant had kept a pipe open. For overflow, deadline expiry
or pipe failure while the shell is alive, terminate the group and wait for the
direct child before finishing the readers. Finish cleanup
before returning; the fetch protocol permits no surviving background jobs.
Implement this locally with the standard library and existing `libc` dependency,
without adding a generic subprocess framework or touching the clipboard runner.

A missing configured scheme returns `Unavailable(MissingScheme)`, retaining
`[docs.schemes]`, the scheme and the host config location for its strict error
or warning. Malformed/unreadable host config remains `Err`, not an availability
outcome. The typed split in §4.1 applies to cached outcomes as well.

## 5. Command behavior

### 5.1 Writes and movement

`add` fetches supplied external links and stores canonical references before
creating the record. For flag edits, retain the original link/step values before
clearing flags mutate the working record. For both flag edits and editor saves,
compare the proposed final spec, plan and step values against that original,
before canonicalization:

- Fetch/canonicalize an external spec only when its value changes to a present
  value, and an external plan only when its value changes to a present value.
- When an external plan remains linked and a step is present, require a plan
  body and verify the heading only if the plan or step value changed.
- Unchanged links and steps are not fetched, even if explicitly supplied with
  the same stored value. A title, priority, body or other unrelated edit thus
  works without the scheme or fetcher. Removing a link/step needs no fetch;
  keep the existing requirement that a step cannot remain without a plan.

An alias different from the stored ID counts as a changed input and must resolve;
the tracker cannot know they are equivalent before fetching. Changing only the
step does not rewrite the unchanged plan's reference. Changed-link
canonicalization and heading verification share the fetched body. Keep local
path validation and local step rechecks as today; this availability rule applies
to external links. Shape, project and other record validation still run.

The editor compares against `original` and assigns a changed link's
`resolve_doc` return value back to `edited.spec`/`edited.plan`; currently it
discards that result. A failed fetch or missing
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

External fetching in `check` has one fixed five-second monotonic budget per
invocation, beginning with its first configured fetch attempt. Fetch at most four
distinct references concurrently. Dispatch references in this priority order,
lexical within each tier, assigning a shared reference to its highest tier:

1. Plans linked by an open task with a step.
2. Other open tasks' plans.
3. Open tasks' specs.
4. Closed-only plans.

Use the resolver's existing fetch boundary and command-local outcome cache;
workers return bounded outcomes to the resolver rather than sharing its
`RefCell` state across threads. No provider-specific logic or general worker-pool
framework is introduced. Once a worker completes, dispatch the next highest
priority uncached reference only if budget remains. Malformed host config is
validated before starting workers and remains fatal. A fetch's deadline is the
earlier of its 30-second per-fetch limit and the remaining command budget. If the
command budget ends, terminate/clean up all in-flight fetches and classify their
unfinished outcomes as `Unavailable(BudgetExhausted)`. Successful cache entries
are not discarded. Worker completion order must not change finding order.

After exhaustion, uncached configured references are not spawned: their outcome
is `Unavailable(BudgetExhausted)`, with cause `budget exhausted`. Existing
cached bodies remain usable and cached failures remain unchanged. Host config
validation and missing-scheme classification still occur without spawning, so
malformed config never becomes a budget warning. All local, task, dependency
and attachment checks continue normally. The five seconds bounds fetch waiting,
with the small additional time needed for mandatory process cleanup; it is not
a deadline for the whole command. There is no public budget option or setting.

Report all budget-incomplete references with one `doc_unchecked` finding per
invocation: `id` unset, `file` = `tasks/`, and detail naming `budget exhausted`
and listing each reference in priority order. Include unfinished attempted
fetches and those never started, including closed-only plans. Do not also emit
per-task `doc_unreachable` findings for these references. Missing schemes and
attempted fetch failures still produce the existing per-record `doc_unreachable`
warning for open tasks; they are different outcomes from an incomplete check.

Budget exhaustion remains an outage/oversized-backlog safeguard, not accepted
steady behavior: the rollout pilot below must finish the whole open backlog
without `doc_unchecked`. The source spec's direct-file fallback belongs in the
ops fetcher if real-wrapper latency exceeds the budget, preserving its contract.
No rotating queue, persistent cache or randomly unchecked tail is introduced.
Explicit `tasks doc` and changed-link writes retain the full per-fetch deadline.

| Condition | Result |
|---|---|
| Open task, missing local path | Existing `doc_missing` error |
| Local document read or host configuration error | Existing fatal typed error |
| Open task, external scheme missing or attempted fetch failed | `doc_unreachable` warning, naming reference and cause |
| External checks incomplete because the budget ended | One `doc_unchecked` warning listing unfinished/unstarted references |
| Available plan lacks an open task's step heading | `step_missing` error |
| Available plan heading has no linking task | Existing `unlinked_step` warning |
| Closed-only external plan has a missing scheme or failed fetch | No drift finding |

After an unavailable document, skip dependent heading checks; do not pretend it
is an empty body. Closed records still count when matching plan headings to
steps, as today. Available closed-only plans still receive the existing
uncovered-heading check; an unavailable closed-only external plan is skipped.
Warnings alone keep `check` at exit 0. Preserve `doc_in_main_checkout` findings.

`show`/shared `describe` keeps the external reference in `spec_path`/`plan_path`.
For an external plan with a step, read opportunistically with the same fixed
five-second budget, scoped to that description:

- An available body reports the actual heading result, including `false` if
  the heading is absent.
- An open task's unavailable plan emits a warning naming the reference and
  cause and omits `step_found`; unknown is not reported as `false`. `show` and
  `next` still succeed and describe the chosen task.
- A closed task's unavailable plan emits the historical-plan warning (with
  cause) and `step_found: false`, preserving its established historical result.
- Without a step, neither open nor closed external plans are fetched, and no
  historical availability warning is attempted. External specs are display-only
  here as well; `abs` never fetches them.

Local-plan behavior remains unchanged, including strict open-step failures and
the existing missing-historical-path warning without a step. Malformed host
config encountered during a fetch is fatal, never an availability warning.
The budget is per description, not shared across separate commands: three
separate stepped-task descriptions can wait up to 15 seconds plus cleanup.
The current `show` accepts one ID; this design does not add a multi-ID command.

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
unchanged-link edit behavior, optional display reads and five-second check budget,
four concurrent check fetches and the separate `doc_unchecked` summary,
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
  opaque arguments, no-config parsing, config defaults, unknown keys and
  colon-bearing first segments of configured roots (but allowed later colons).
- Writes cover canonicalization for add, flag edit and editor save; heading
  presence/absence; atomic failure; and unchanged source-idempotent reuse.
  On a no-scheme host, prove title/priority and editor body edits of a task with
  an external spec, plan and step succeed without fetching. With a counting
  fetcher, verify identical explicit links/steps and clearing values do not
  fetch, while a changed spec, plan or step fetches only required documents.
- Fetch tests cover exact argument delivery with shell metacharacters, valid
  empty bodies, malformed JSON/fields/IDs, nonzero exit, bounded stdout/stderr,
  a hung command, and an exit-0 shell with complete JSON and a descendant holding
  a pipe. The last case succeeds promptly after descendant cleanup rather than
  timing out. Assert child cleanup, not merely the result. A private deadline
  parameter lets focused unit tests exercise this promptly; production's strict
  per-fetch deadline stays 30 seconds, with no public timeout option. Keep one
  real CLI deadline case to verify production wiring, ignored in the fast set
  and included in the full suite so each ordinary code commit does not add a
  30-second wait.
- Check tests cover both heading findings, warning-only exit 0, missing schemes,
  failed fetches, one fetch for many linking records and failed-read reuse,
  closed-only plans, and existing main-checkout/path I/O behavior. Budget tests
  use multiple references to prove one shared deadline, the four-fetch cap,
  stepped-plan/other-plan/spec/history priority, stable findings despite reversed
  completion order, no post-budget spawning, cleanup of all in-flight children,
  and one `doc_unchecked` summary (never `doc_unreachable` for untried references).
  Also verify cause `budget exhausted`, continued local error detection and
  bounded pre-commit behavior. Malformed config must be
  fatal in strict and tolerant callers, unlike a missing scheme.
- Show/next tests cover reference display, historical unavailable plans with and
  without a step, no fetch without an external step, available changed headings,
  successful open-step descriptions with unknown/omitted `step_found` when no
  scheme or fetcher is available, and bounded optional reads. Preserve local
  strict failures and historical behavior.
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

Latency evidence before planning (2026-10-11, first host): `mindful-doc` is not
installed yet. A pilot of the installed `mindful --json show` path plus an
in-process `id`/`body` projection succeeded in 0.459 seconds. Twenty warm repeats
of the same existing thought took 9.476 seconds serially (median 0.472 seconds)
and 2.889 seconds at concurrency four (median call 0.580 seconds). This confirms
startup cost and that concurrent reads work in that sample; it excludes the
future wrapper's startup, varied documents, cold state and the second host.
The timing and required real-wrapper pilot are noted on `ops-721511`.

Before activating the external-document workflow, time the implemented
`mindful-doc` on each host against the actual distinct open-document backlog,
with at least 20 varied references if the backlog is smaller, and with the
mindful web app running. Verify per-call timing and the tracker's four-worker
check end to end: every open reference completes inside five seconds with no
`doc_unchecked`, repeated across three runs. A failed pilot blocks workflow
activation. ops must then use source §4.2's direct-thought-file read fallback
inside `mindful-doc`, preserving canonical IDs, aliases and the `id`/`body`
JSON contract, and repeat the pilot. Tasks never learns mindful's storage
format. This changes neither external availability handling nor the fixed
budget, and avoids accepting a permanently skipped backlog tail.
If routine checks begin reporting `doc_unchecked` as the backlog grows, treat
that as a fetch-latency incident: repeat the whole-backlog pilot and remedy the
fetcher, rather than accepting the warning on every commit.

Old binaries reject both external references at record parse time and the new
`[docs.schemes]` table at host-config load time. The latter can break ordinary
claim/identity commands even before any external record exists, because
`relay::enabled()` loads `HostConfig` from those paths. Roll out in this order:

1. On each host, install and verify the reviewed new tasks binary while leaving
   its existing host config unchanged.
2. Only after that host is running the new binary, ops installs/configures its
   fetcher and writes `[docs.schemes]` there. Verify ordinary claim/identity
   commands with the new table. Do not downgrade that host's binary while the
   table remains: remove the table first if a rollback is required.
3. Only after both hosts have the new binary, write the first external reference
   into synced records. A host may still lack a scheme or have its fetcher down;
   verify its ordinary edits, `show`, `next` and warning-only `check` continue
   working, while explicit body reads and changed-link validation fail clearly.

Pass this config-order requirement to the ops rollout task `ops-721511`.
Record each host's evidence; an unavailable host leaves rollout pending.
Live installation is a separately approved host action, with no worktree pointer
repointing. ops supplies `mindful-doc` and its host config in its rollout task.

No persistent fetch cache, provider SDK, new dependency, direct mindful storage
access or migration of existing documents is included.
