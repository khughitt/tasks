# External Document References Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (the recommended inline method for this plan) or superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

Status: accepted by Keith after plan round 2 (2026-10-11)

**Goal:** Resolve external plan/spec links through host fetch commands while ordinary
task operations remain usable without the provider.

**Architecture:** Extend the existing resolver with typed document outcomes and a
small supervised fetch module. Writes require available changed documents; descriptive
reads tolerate unavailability; check schedules up to four fetches in a shared budget.
The CLI exposes exact body export and an exact plan filter without provider knowledge.

**Tech Stack:** Rust 2024, standard library processes/threads/channels, existing `libc`,
`serde_json`, `toml`, `clap`, `assert_cmd` and `tempfile`. No new dependency.

**Spec:** `docs/specs/2026-10-11-external-document-references-design.md`
(accepted by Keith after spec round 3).

**Workspace:** `.worktrees/tasks-91f63b-external-docs`, branch
`feat/tasks-91f63b-external-docs`. Run commands there; file paths below are relative
to that checkout. User-facing links include the `.worktrees/` prefix.

**Execution method:** Inline (accepted by Keith in plan round 1). The six
code/document steps share resolver, command and output interfaces, so sequential implementation avoids repeated context
and overlapping edits. Use a fresh whole-branch reviewer after implementation.
Spec approval does not approve this plan or live installation.

## Global Constraints

- Reference scheme: `[a-z][a-z0-9-]*`; remainder nonempty and opaque. Reject newline,
  carriage return and NUL. Reserve `:` in the first segment of configured doc roots.
- Host config only: `[docs.schemes]`; retain `deny_unknown_fields`. Malformed or
  unreadable config is fatal, unlike a missing scheme.
- Fetch: `sh -c '<cmd> "$1"' -- <rest>`, null stdin, one positional remainder.
  Response: one JSON object with string `id` and `body`; empty body is valid.
- Limits: stdout 2 MiB; stderr retained prefix 200 bytes, remainder drained;
  strict per-fetch deadline 30 seconds. No public timeout or budget setting.
- Own a process group for every fetch. Observe shell exit with
  `waitid(P_PID, pid, WEXITED | WNOHANG | WNOWAIT)`, retaining the unreaped child
  until the group kill. Never call `try_wait` or reap before that kill, and never
  signal the old group after reaping. Reap the direct child and join readers on
  every return before waiting indefinitely on pipe EOF.
- `check`: at most four concurrent fetches; one five-second fetch budget. Priority:
  open stepped plans, other open plans, open specs, closed-only plans; lexical per tier.
  Each shared reference belongs to its highest tier.
- Budget incompleteness: one `doc_unchecked` finding, `id` unset, `file: tasks/`,
  detail listing references in priority order. Missing schemes/failed fetches use
  `doc_unreachable` for affected open records. Neither outcome invents an empty body.
- `show`/`next`: five seconds per description; unavailable open-step results omit
  `step_found`. No external fetch without a step. Local behavior stays unchanged.
- Unchanged external links/steps do not fetch on edits. Changed links canonicalize;
  changed steps require a body. Preserve atomic writes and edited-temp retention.
- `move`: external links without steps need no fetch; linked steps must validate.
- `doc`: exactly one selector; exact body bytes on stdout in every format, no added
  newline. Warnings always on stderr. Existing structured errors/broken-pipe behavior.
- `list`/`search --plan`: single exact string filter; no fetch or parent restriction.
  Optional `step` on shared summaries/resolved parks; no new pretty columns.
- Preserve worktree/main/overlay lookup and local I/O failures. `Located` stays local.
- No persistent cache, provider SDK, approval field, migrated document, new installer
  or shared launcher pointing into a worktree. `--source` remains opaque.
- Native project test front door only: focused `just test-one`, `just test-fast`
  before code commits, `just check`; full suite under the installed hook policy.
- Conventional commits, no AI attribution. `tasks check` before every commit.
- Live rollout: new binary before host scheme config; both new binaries before the
  first synced external link. Real-wrapper whole-backlog pilot gates activation.

## Review Focus

- A fetcher floods stderr before producing stdout: bounded concurrent draining must
  finish without deadlock or retaining the whole stderr stream. Task 1.
- An alias returns its canonical ID while its body changes on a second fetch:
  canonicalization and heading validation must use the same fetched body. Task 2.
- Clear-and-restore flags yield unchanged external links: compare with the original
  record before clearing, so a no-scheme host still saves the unrelated edit. Task 2.
- Malformed host config plus a pending queue: fail before spawning any worker;
  never disguise the error as budget exhaustion. Task 3.
- Raw body export under pretty mode with invalid terminal width, and parked filtering
  through a recorded checkout: bypass layout and match the record actually shown.
  Task 5.

## Execution setup and gates

The worktree already exists and is locked. Its justfile has no setup recipe.
Do not create another worktree or guess an installer. Before the first code change:

```sh
git status --short
git config --get core.hooksPath
just test-fast
```

Read the accepted spec and this plan, and follow the worktree skill's deferred
baseline step. A failing baseline is investigated before feature edits. Use the
executing-plans ledger, including each final `Ruling:` and `minor (deferred)` item.

After user plan acceptance, file one direct child per numbered task with this plan
and its exact heading, set size/complexity explicitly, then link the parent's plan.
Tasks 1 and 3: `m/high`; Tasks 2, 4 and 5: `m/mid`; Task 6: `s/low`; Task 7: `s/mid`.
Record sequential dependencies: each child after Task 1 depends on the preceding
child. The steps share files and are not marked parallel. The separate ops
inventory task must not depend on this parent; its change is needed before
Task 5 can complete.
Start each child and run `trial-arm` as required. No children are filed while the
plan is unreviewed. Record the plan-review verdict before acting on it.

The shared CLI inventory is a prerequisite for Task 5, not for Tasks 1–4.
During execution setup, before Task 1, prepare its small update under its own
scoped direct ops task/worktree, observing ops's profile and agent guide. Use the
exact rows and validation commands in Task 5 step 1 to make the change concrete.
Once validated, immediately request approval to integrate that shared global
surface; do not wait until Task 4 completes to ask. Use an asynchronous approval
request when available, so Tasks 1–4 can proceed while the user considers it.
Approval is required only for integration, not for those independent code steps.
Record the owning ops task, commit/check evidence and pending approval on this
task. Integrate only after explicit approval, then verify it landed in ops main.
Task 5 requires that observed landed result; elapsed time is not approval.
Do not edit `tools/cli.toml` here or waive conformance.

The execution ledger also records the intentionally temporary intermediate
behavior: Tasks 1–2 introduce external link parsing/writes before Tasks 3–4
teach check/show their availability rules. Those intermediate commits are not
feature-complete: check may report such links as `doc_missing`, and show may
fail hard. All fixture records are isolated; no real record acquires an external
link and no intermediate binary is installed. Tasks 3–4 resolve that gap before
the whole-branch review and rollout.

---

### Task 1: Typed document reader and supervised external fetching

**Files:** Create `src/doc_fetch.rs`, `tests/common/docs.rs`, `tests/external_docs.rs`.
Modify `src/main.rs` (module declaration), `src/format.rs`, `src/repo.rs`,
`src/config.rs`, `src/resolve.rs`, `tests/common/mod.rs`.

**Interfaces:**

```rust
// format.rs: classifier also rejects malformed colon-bearing first segments.
pub fn external_doc(value: &str) -> Result<Option<(&str, &str)>>;

// config.rs: existing relay_identity remains; add docs_schemes.
pub struct HostConfig {
    pub relay_identity: bool,
    pub docs_schemes: std::collections::BTreeMap<String, String>,
}

// resolve.rs: these outcomes and documents can be cloned for the command cache.
pub struct Doc { pub reference: String, pub body: String }
pub enum UnavailableCause {
    MissingScheme { scheme: String, config: std::path::PathBuf },
    FetchFailed(String),
    BudgetExhausted,
}
pub enum DocRead {
    Available(Doc),
    Unavailable { reference: String, cause: UnavailableCause },
}
impl DocRead { pub fn required(self) -> Result<Doc>; }
impl UnavailableCause { pub fn detail(&self) -> String; }
impl Resolver<'_> { pub fn read(&self, kind: DocKind, value: &str) -> Result<DocRead>; }

// doc_fetch.rs: owned request passed to a worker, no Resolver/RefCell sharing.
pub(crate) struct FetchRequest {
    pub reference: String, pub command: String, pub rest: String,
}
pub(crate) const MAX_STDOUT: u64 = 2 * 1024 * 1024;
pub(crate) const STRICT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
pub(crate) const OPTIONAL_BUDGET: std::time::Duration = std::time::Duration::from_secs(5);
pub(crate) fn fetch(request: FetchRequest, deadline: std::time::Instant) -> DocRead;
```

- [x] **1. Write reference/config RED cases and the integration fixture.** Add unit
  cases for external forms, malformed first segments, later-segment colons, NUL,
  root reservation, empty scheme table, valid identity+docs config and unknown keys.
  Keep classifier/config tests next to their production modules. Representative:

```rust
#[test]
fn external_doc_shape_is_host_independent() {
    for value in ["x:y", "mindful:thought:abc", "x:/item", "x:folder/item"] {
        assert!(validate_doc_shape("plan", value).is_ok(), "{value}");
    }
    for value in [":y", "x:", "Docs:plan.md", "9x:y", "x:a\0b"] {
        assert!(validate_doc_shape("plan", value).is_err(), "{value:?}");
    }
    assert!(external_doc("docs/plans/x:y.md").unwrap().is_none());
}
```

  The shared integration fixture is deliberately small. Put it in
  `tests/common/docs.rs`, and add `pub mod docs;` to common:

```rust
use super::TestEnv;
use std::{fs, path::{Path, PathBuf}};

pub struct DocFixture {
    config: PathBuf, response: PathBuf, calls: PathBuf, pub script: PathBuf,
}
impl DocFixture {
    pub fn new(env: &TestEnv, body: &str) -> Self {
        let root = env.home.path().join("doc-fixture");
        fs::create_dir_all(&root).unwrap();
        let fixture = Self {
            config: env.home.path().join(".config/tasks/config.toml"),
            response: root.join("body.json"), calls: root.join("calls"),
            script: root.join("fetch.sh"),
        };
        let quote = |path: &Path| format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"));
        let calls = quote(&fixture.calls);
        let response = quote(&fixture.response);
        // shortcut: fixture IDs are simple names; use a preencoded reply for arbitrary-ID cases.
        fs::write(&fixture.script, format!(r#"#!/bin/sh
printf '%s\n' "$1" >> {calls}
case "$1" in
    alias) id=canonical ;;
    *) id="$1" ;;
esac
printf '{{"id":"%s","body":%s}}' "$id" "$(cat {response})"
"#)).unwrap();
        let quoted = quote(&fixture.script);
        let config = serde_json::json!({"docs":{"schemes":{"stub":format!("sh {quoted}")}}});
        fs::create_dir_all(fixture.config.parent().unwrap()).unwrap();
        fs::write(&fixture.config, toml::to_string(&config).unwrap()).unwrap();
        fixture.set_body(body);
        fixture
    }
    pub fn set_body(&self, body: &str) {
        fs::write(&self.response, serde_json::to_vec(body).unwrap()).unwrap();
    }
    pub fn calls(&self) -> Vec<String> {
        match fs::read_to_string(&self.calls) {
            Ok(text) => text.lines().map(str::to_owned).collect(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => vec![],
            Err(error) => panic!("fixture log: {error}"),
        }
    }
    pub fn disconnect(&self) { fs::remove_file(&self.config).unwrap(); }
}
```

  Use existing `TestEnv`; no live registry or mindful. Add `mod common;` and
  `use common::{TestEnv, docs::DocFixture};` to `tests/external_docs.rs`.
  Tests needing a failed/hung/changing provider overwrite `fixture.script`;
  no additional configuration API is needed. The body is a Rust-encoded JSON
  string, inserted with `printf`, not interpreted as shell syntax. For arbitrary
  argument/ID tests, log the exact argument with quoted `printf '%s' "$1"` and
  `cat` a complete Rust-serialized reply instead of using this simple-ID template.
  Unit/integration fetch fixtures require only `sh` and existing basic utilities,
  not Python. The one-off installation smoke can still use Python.

- [x] **2. Run RED through the front door.**

```sh
just test-one external_doc_shape_is_host_independent
just test-one config::tests
```

  Confirm missing behavior, not a fixture/import error; then implement shape,
  root and host-table parsing. The docs tables have their own unknown-field checks.

- [x] **3. Write fetch/reader RED tests before the implementation.** Unit tests use
  `fetch` with a short absolute deadline and controlled shell fixtures.
  Cover valid/empty body, exact single-argument delivery of spaces/quotes/`$()` and
  semicolons, invalid JSON/UTF-8/ID/field types, trailing output, nonzero exit,
  stdout `MAX_STDOUT + 1`, stderr well beyond 200 bytes before valid stdout, a hung
  shell, and complete JSON plus an exit-0 shell's sleeping pipe-holder. Assert the
  sleeping PID no longer lives after both timeout and successful cleanup. Test
  the `DocRead` strict mappings and that malformed config is `Err`, not unavailable.
  Add `shell_remains_unreaped_until_group_kill`: a test-only supervisor observer
  checks `waitid(... WNOWAIT)` still reports the exited owned shell immediately
  before and after the group-kill operation; only then may the reap event occur.
  Assert the event order and exactly one reap, and that the group guard performs
  no later kill on scope exit. Exercise the real OS wait state, not only a mocked
  action log; PID exhaustion/reuse simulation is unnecessary.

  A private test-only observer reports `BeforeGroupKill(pid)`, `AfterGroupKill(pid)`
  and `Reaped(pid)` as `CleanupEvent`. `fetch_observed(request, deadline,
  observer: &mut dyn FnMut(CleanupEvent)) -> DocRead` uses the same supervisor
  with that observer; callbacks also cover guard cleanup. The ordering test is:

```rust
#[test]
fn shell_remains_unreaped_until_group_kill() {
    let mut order = Vec::new();
    let mut pinned = Vec::new();
    let mut observe = |event| match event {
        CleanupEvent::BeforeGroupKill(pid) => {
            order.push("before kill");
            pinned.push(matches!(peek_exit(pid), Ok(true)));
        }
        CleanupEvent::AfterGroupKill(pid) => {
            order.push("after kill");
            pinned.push(matches!(peek_exit(pid), Ok(true)));
        }
        CleanupEvent::Reaped(_) => order.push("reaped"),
    };
    let request = FetchRequest {
        reference: "stub:test".into(), command: "printf '%s'".into(),
        rest: r#"{"id":"test","body":""}"#.into(),
    };
    let doc = fetch_observed(request, Instant::now() + Duration::from_secs(1),
                             &mut observe).required().unwrap();
    assert_eq!(doc.body, "");
    assert_eq!(order, ["before kill", "after kill", "reaped"]);
    assert_eq!(pinned, [true, true]);
}
```

```rust
let request = FetchRequest {
    reference: "stub:test".into(), command: "printf '%s'".into(),
    rest: r#"{"id":"test","body":""}"#.into(),
};
let doc = fetch(request, Instant::now() + Duration::from_secs(1))
    .required().unwrap();
assert_eq!(doc.reference, "stub:test");
assert_eq!(doc.body, "");
```

```sh
just test-one doc_fetch::tests
just test-one resolve::tests
```

- [x] **4. Implement the fetch boundary and wire the reader.** Start `sh` with
  `CommandExt::process_group(0)`. Two readers send bounded stdout/stderr results
  over a channel; stderr drains in chunks, retaining only its prefix. The controller
  alternates a non-reaping `waitid` peek with
  `recv_timeout(min(remaining, 10 ms))`; it never blocks on stdout or a reader
  join while the child/group can still run. Initialize `siginfo_t` to zero before
  each peek; a zero `si_pid` means no exited child yet. Retry `EINTR` within the
  deadline; other observation failures become a fetch error.

  On observed exit, leave the child unreaped, kill remaining group members,
  and only then call `Child::wait`. Deadline/overflow/pipe-error cleanup uses
  the same kill-before-reap order. Accept `ESRCH` only while the owned child is
  still unreaped; its pinned PID cannot have become another group's ID. Disarm
  the group guard when reaped, including before subsequent parsing/error paths,
  so Drop never signals a recycled PGID. If child ownership is lost (`ECHILD`),
  fail without signaling that stale PGID; do not recover by calling `kill`.
  Join both readers after cleanup. Other cleanup failures appear in the cause.

```rust
fn peek_exit(pid: libc::pid_t) -> std::io::Result<bool> {
    // Zero initializes the C record and its no-exit sentinel.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    let result = unsafe {
        libc::waitid(libc::P_PID, pid as libc::id_t, &mut info,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT)
    };
    if result == -1 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { info.si_pid() } != 0)
}
```

  The controller handles this private helper's errors as described above.
  Semantics reference:
  [Linux waitid documentation](https://www.man7.org/linux/man-pages/man2/waitpid.2.html).

```rust
let mut command = std::process::Command::new("sh");
command.args(["-c", &format!("{} \"$1\"", request.command), "--", &request.rest])
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped());
use std::os::unix::process::CommandExt;
command.process_group(0);
// Read limits/deadline and cleanup are supervised before parsing this payload.
#[derive(serde::Deserialize)]
struct Payload { id: String, body: String }
```

  Parse the whole stdout as one `Payload`; validate `scheme + ':' + id` with the
  same classifier. Missing host command returns typed `MissingScheme` without
  spawning. Pipe/spawn/protocol errors become `FetchFailed`; bad host config is
  fatal. Use a private child/group owner so every early Rust return performs
  the same cleanup. No clipboard changes or general subprocess library.

  Add a lazy successful `HostConfig` cell and external `DocRead` cache to Resolver;
  keep configuration parse failures fatal and uncached. Cache an available alias
  result under requested and canonical reference. Route external `resolve_doc`
  and strict `step_exists` through `read`; local `read` uses existing `find` and
  `read_to_string`, preserving `Io`. `abs` returns external references directly.

- [x] **5. Verify GREEN and commit this coherent reader change.** Add a fixture
  integration test proving an external record can be listed after disconnecting
  the host scheme. Preserve existing local/main-root tests.

```sh
just test-one doc_fetch::tests
just test-one shell_remains_unreaped_until_group_kill
just test-one external_doc
just test-one --test cli show_missing_historical_plan_keeps_open_and_read_failures
just test-fast
tasks check
git add src/doc_fetch.rs src/main.rs src/format.rs src/repo.rs src/config.rs src/resolve.rs tests/common/mod.rs tests/common/docs.rs tests/external_docs.rs
git commit -m 'feat: resolve external task documents through host fetchers'
```

### Task 2: Changed-link writes and destination step validation

**Files:** Modify `src/commands/mod.rs::apply_fields`, `src/commands/add.rs`, `src/commands/edit.rs`,
`src/moving/preflight.rs`, `tests/external_docs.rs`, `tests/move.rs`.

**Interfaces:** Consume Task 1's classifier, resolver and strict document mapping.
Capture original spec/plan/step once before clearing flags. Pass that baseline
to shared field application as `Option<&Task>`: add uses `None`, flag edit uses
`Some(&original)`. Editor already has `original`. No persistent change-tracking
type or comparison of canonical IDs before a fetch.

```rust
pub fn apply_fields(ctx: &mut Ctx, task: &mut Task, fields: &FieldArgs,
                    original: Option<&Task>) -> Result<()>;
```

- [x] **1. Write RED tests for no-scheme edits and canonicalization.** With the fixture,
  add an external spec/plan and step; disconnect, change title/priority, and assert
  the log is unchanged. Check the changed fields via `list`/raw record, without
  depending on Task 4's tolerant display. Include editor body-only changes,
  explicit identical links/step, clearing links, and clear-then-restore flags.

```rust
#[test]
fn external_doc_unrelated_edit_does_not_fetch() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let docs = DocFixture::new(&env, "### Task 1: one\n");
    let created = env.json(&dir, &["add", "Before", "--plan", "stub:alias",
                                 "--step", "Task 1: one"]);
    let id = created["id"].as_str().unwrap();
    let calls = docs.calls();
    docs.disconnect();
    env.json(&dir, &["edit", id, "--title", "After", "-p", "1"]);
    assert_eq!(docs.calls(), calls);
    let rows = env.json(&dir, &["list"]);
    assert_eq!(rows["tasks"][0]["title"], "After");
    assert_eq!(rows["tasks"][0]["priority"], 1);
}
```

  Count fetches for changed spec only, changed plan only, changed step only,
  alias-to-canonical validation and failure before writes. For an alias whose
  second fetch would omit the heading, the first successful resolve+step check
  must still write successfully with exactly one provider call. Editor tests
  use the existing editor-shim pattern and assert retained edit temp on refusal.

```sh
just test-one --test external_docs external_doc_unrelated_edit_does_not_fetch
```

- [x] **2. Apply only required external reads, preserving local checks.** Add's empty
  baseline means supplied links are new; edits compare the final proposed values
  with the saved original before resolution. The core predicates are:

```rust
let plan_changed = proposed_plan != original_plan;
let spec_changed = proposed_spec != original_spec;
let step_changed = proposed_step != original_step;
let validate_external_step = proposed_step.is_some() && (plan_changed || step_changed);
```

  These names are borrowed `Option<&str>` values derived from fields+clears and the
  saved record, not a new public interface. Resolve changed present external
  links, assign returned references and use the same resolver for the step read.
  Leave unchanged external references untouched. Local resolution and existing
  local step rechecks still run. Reject a remaining step with no plan before save.
  Editor assigns changed-link resolution results, retaining its `keep` error path.

- [x] **3. Prove move semantics RED→GREEN.** Extend `tests/move.rs`: a childless,
  unparented task with external links but no step moves after removing the scheme;
  a stepped move requires its heading, and unavailable/missing-heading refusals
  leave source bytes unchanged and create no destination record.

```rust
for (kind, link) in [(DocKind::Spec, &task.spec), (DocKind::Plan, &task.plan)] {
    if let Some(value) = link && crate::format::external_doc(value)?.is_none() {
        resolver.resolve_doc(kind, value)?;
    }
}
// The existing separate step_exists call remains strict for both kinds of plan.
```

```sh
just test-one --test move external_doc
just test-one --test external_docs external_doc
just test-fast
tasks check
git add src/commands/mod.rs src/commands/add.rs src/commands/edit.rs src/moving/preflight.rs tests/external_docs.rs tests/move.rs
git commit -m 'fix: validate external links only when edits change them'
```

### Task 3: Concurrent drift checks with truthful incomplete outcomes

**Files:** Modify `src/resolve.rs`, `src/doc_fetch.rs`, `src/commands/check.rs`,
`tests/external_docs.rs`; extend module unit tests in those production files.

**Interfaces:**

```rust
// resolve.rs: cache population is confined to the caller/controller thread.
pub fn set_fetch_budget(&mut self, budget: Duration);
pub fn prefetch_external(&self, references: &[String])
    -> Result<Vec<String>>; // unfinished references, in supplied priority order

// check.rs: private tier builder, deduplicates a reference at its minimum tier.
fn external_check_order(tasks: &[Task]) -> Result<Vec<String>>;
```

  `prefetch_external` is a Resolver method, not a free reader. Task 1's lazy config
  is validated before workers; missing schemes cache `MissingScheme` without
  consuming a worker. `read` subsequently consumes the available/unavailable cache.
  `set_fetch_budget` configures one resolver-owned lazy absolute deadline, shared
  by `read` and `prefetch_external`; it is called before any fetch. No second
  scheduler budget or clock is introduced.

- [x] **1. Write RED tier, cache and real-worker budget tests.** The tier builder's
  expected output for interleaved inputs is stepped plan `stub:z`, other plan
  `stub:b`, spec `stub:a`, closed-only plan `stub:c`; a spec also used as a stepped
  plan appears once at the stepped tier. Permute task input order to verify stability.
  Existing Task construction patterns in `query.rs`/`model.rs` avoid new model helpers.

  Resolver tests use a private short budget and shell fixtures that append their
  reference/PID before waiting on a release file. Wait for four arrivals, assert
  no fifth has started, then release one and observe the next queued reference.
  A deadline test leaves them blocked and asserts all started PIDs are dead,
  queued references were not started, and outcomes retain priority order. Record
  stderr and stdout limits in Task 1, not duplicate parser tests here.

```rust
resolver.set_fetch_budget(Duration::from_millis(150));
let unfinished = resolver.prefetch_external(&references)?;
assert_eq!(unfinished, references);
for reference in &references {
    assert!(matches!(resolver.read(DocKind::Plan, reference)?,
        DocRead::Unavailable { cause: UnavailableCause::BudgetExhausted, .. }));
}
```

  In the above test every fixture deliberately waits past the deadline. Add a
  separate reversed-completion fixture where all results succeed, to verify
  finding order independently of worker order. A malformed host config must
  fail before any PID log exists, even when the budget is zero.
  Unit resolver fixtures use `Project::init` with a temp root and
  `Registry::default`, then initialize the private host-config cell directly;
  do not mutate process-global HOME/XDG variables in parallel Rust tests.
  Malformed on-disk config coverage uses child CLI processes with `TestEnv`.

```sh
just test-one external_check_order
just test-one prefetch_external
```

- [x] **2. Implement the priority queue and bounded scheduler.** Build a
  `BTreeMap<String, u8>` from records, taking the minimum tier for shared refs;
  sort its entries by `(tier, reference)`. Skip external specs on closed records.
  Pass the resulting refs into `prefetch_external` once. Scoped worker threads
  each own a Task 1 `FetchRequest`; the controller starts at most four at a time,
  receives `(queue_index, DocRead)` and updates the cache itself. All workers
  receive the same absolute deadline, initialized at the first configured fetch.
  Check calls `resolver.set_fetch_budget(OPTIONAL_BUDGET)` before that prefetch;
  ordinary/strict readers on an unconfigured resolver retain the 30-second limit.
  Launch no new fetch after it; unfinished/not-started refs get `BudgetExhausted`.
  Join active workers after their group cleanup before returning. Do not clone
  Error globally or make Resolver thread-safe to accommodate workers.

- [x] **3. Move check's body consumers behind `read`, and write integration RED cases.**
  Keep local spec existence checks and their existing `Located` warnings; do not
  newly read local spec bodies just to prove existence. Read each distinct local
  plan once into check's own body map, preserving missing/non-file/read failures
  and historical behavior. External body/cache reads drive both heading loops.

```rust
match resolver.read(DocKind::Plan, plan)? {
    DocRead::Available(doc) => {
        if let Some(step) = &task.step && !has_heading(&doc.body, step) {
            errors.push(finding(Some(task), file.clone(), "step_missing",
                format!("heading {step:?} not found in {plan}")));
        }
        plan_bodies.insert(plan.clone(), doc.body);
    }
    DocRead::Unavailable { cause: UnavailableCause::BudgetExhausted, .. } => {
        // Already named in the single invocation-level doc_unchecked finding.
    }
    DocRead::Unavailable { reference, cause } => {
        warnings.push(finding(Some(task), file.clone(), "doc_unreachable",
            format!("{reference}: {}", cause.detail())));
    }
}
```

  The example is the open stepped-plan branch; `plan_bodies` is check's
  `BTreeMap<String, String>` consumed by the uncovered-heading loop. Iterate
  all affected tasks against the one body, not just the first owner. Use existing
  `finding`/`has_heading`/`step_headings`; `doc_unchecked` has no
  task ID and uses `tasks/` as its file. Sort final document findings by their
  stable task/ref order, not channel arrival. Preserve all non-document checks.

  Integration cases: available external step removed gives `step_missing`/exit 1;
  unmatched available heading gives `unlinked_step`; missing scheme and failed
  script give `doc_unreachable`/exit 0; many open/closed records sharing a plan
  produce one fetch; closed-only failures have no drift warning; an exhausted
  budget yields exactly one `doc_unchecked`, with no false per-record unreachable
  warning, and still detects an unrelated local missing plan. Keep one production
  five-second deadline integration case, ignored in the fast suite; use private
  short budgets for the matrix so each commit does not pay the production wait.

```sh
just test-one --test external_docs external_doc_check
just test-one --test cli check_in_a_worktree_reads_git_excluded_docs_from_the_main_checkout
just test-fast
tasks check
git add src/resolve.rs src/doc_fetch.rs src/commands/check.rs tests/external_docs.rs
git commit -m 'feat: check external document drift within a shared fetch budget'
```

### Task 4: Tolerant external plan display in show and next

**Files:** Modify `src/resolve.rs`, `src/commands/show.rs`, `tests/external_docs.rs`; existing
historical/routing regressions remain in `tests/cli.rs` and `tests/move.rs`.

**Interfaces:** Consume `DocRead`/`UnavailableCause::detail` and Task 1's reader.
Use Task 3's `Resolver::set_fetch_budget(&mut self, budget: Duration)`, the internal
command limit initialized on its first configured fetch. `read` stays the single
body reader and honors that deadline; no second provider reader is added.
Strict `step_exists` remains for writes/moves whose resolvers have no shorter limit.

- [x] **1. Write RED show/next tests.** Create a stepped external task, disconnect
  the host config and assert both commands succeed, preserve the plan reference,
  emit an availability warning and omit `step_found`. Repeat with a failing
  configured fetcher. Close the record: unavailable stepped history gives `false`;
  no-step external history has no fetch and no historical warning. An available
  plan with a removed heading gives `false`, unlike an unavailable open plan.

```rust
let shown = env.json(&dir, &["show", &id]);
assert_eq!(shown["plan_path"], "stub:canonical");
assert!(shown.get("step_found").is_none());
assert!(!shown["warnings"].as_array().unwrap().is_empty());
let next = env.json(&dir, &["next"]);
assert_eq!(next["next"]["task"]["id"], id);
assert!(next["next"].get("step_found").is_none());
```

  Use a private budget reader test for fast timeout coverage; one CLI call
  verifies the real five-second display deadline and no surviving child; mark
  that production-wait case ignored in the fast suite as well.
  Malformed config remains a fatal `config` error, not a successful warning.

```sh
just test-one --test external_docs external_doc_show
```

- [x] **2. Handle only external stepped plans opportunistically.** Classify before
  `locate`'s local historical branch. With no external step, do not call the
  reader. For a stepped external plan:

```rust
resolver.set_fetch_budget(OPTIONAL_BUDGET);
match resolver.read(DocKind::Plan, plan)? {
    DocRead::Available(doc) => Some(has_heading(&doc.body, step)),
    DocRead::Unavailable { reference, cause } => {
        warnings.push(format!("{}plan {reference} is unavailable: {}",
            if task.status.is_open() { "" } else { "historical " }, cause.detail()));
        if task.status.is_open() { None } else { Some(false) }
    }
}
```

  Make the local resolver binding mutable for the budget setter. Keep the
  existing local missing-history guard, its metadata checks and
  error propagation intact. `describe` is already shared by `next`, so no
  second display implementation is needed. Pretty display marks only actual
  `Some(false)` as missing; an unknown open result stays unmarked with its warning.

```sh
just test-one --test external_docs external_doc_show
just test-one --test cli show_closed_records_keeps_missing_historical_plan_links
just test-one --test cli show_missing_historical_plan_keeps_open_and_read_failures
just test-one --test cli show_historical_plan_keeps_main_checkout_metadata_failures
just test-one --test move show_moved_record_keeps_missing_historical_plan_and_move_pointers
just test-fast
tasks check
git add src/resolve.rs src/commands/show.rs tests/external_docs.rs
git commit -m 'fix: describe tasks when external plans are unavailable'
```

### Task 5: Document export and exact plan selection

**Files:** Create `src/commands/doc.rs`. Modify `src/cli.rs`,
`src/commands/mod.rs`, `src/commands/show.rs`, `src/commands/list.rs`,
`src/output.rs`, `src/main.rs`, `tests/external_docs.rs`, `tests/cli.rs`;
adopt `tools/cli.toml`. Existing completion machinery is reused through attributes.

**Interfaces:** `Command::Doc { id: String, plan: bool, spec: bool }`,
`Output::Doc(DocOut { body: String, warnings: Vec<String> })`,
`commands::doc::run(ctx: Ctx, id: String, kind: DocKind) -> Result<Output>`;
`ListArgs.plan: Option<String>`; optional `step` on `TaskSummary`/`ParkedRow`.
Promote show's shared record selection to
`pub(super) fn read_record(ctx: &mut Ctx, id: &TaskId, claims: &ClaimSnapshot)
-> Result<(Option<Project>, Task, Vec<Task>)>`; both commands select through it.
This factors existing lookup only, preserving its warnings and failure cases.

- [x] **1. Verify/adopt the inventory prepared during execution setup.** Setup's
  owning ops task/worktree adds this exact command row and one nonrepeatable
  `{ names = ["--plan"], value = "string" }` option to both list and search:

```toml
[[cli.tasks.commands]]
path = ["doc"]
summary = "Print a linked plan or spec body"
protocol = "raw UTF-8 document body on stdout"
exception = "document export is raw even under JSON/pretty flags; noun command names its exported artifact"
args = [{ name = "id", value = "ref", required = true }]
options = [
  { names = ["--plan"], value = "none" },
  { names = ["--spec"], value = "none" },
]
```

  The authority permits nonempty `protocol`/`exception`; do not invent
  `output = "raw"`, which its validator rejects. Ops verifies with
  `just test-one tests.test_cli tests.test_cli_surface`, `just test-fast` and its
  normal checks; its own guide/profile controls the worktree and integration.
  This preparation/validation and the approval request happen before Task 1,
  not at this step. Verify explicit approval and the observed commit on ops main;
  if still pending, stop here and park for that approval without changing the
  parser or vendored copy. Record the owning task ID on `tasks-91f63b`.
  Once landed, from this worktree:

```sh
TASKS_DOC_OPS_ROOT=$(tasks resolve ops | python3 -c 'import json,sys; print(json.load(sys.stdin)["results"][0]["root"])')
python3 "$TASKS_DOC_OPS_ROOT/bin/vendored" adopt cli.toml
git diff --stat -- tools/cli.toml
```

- [x] **2. Write RED byte-export and selector/routing tests.** Use `env.cmd(...).output()`
  because `env.json` requires JSON. Assert exact bytes for empty body, no trailing
  newline and a body with one. Test default/`--json`/`--pretty`/`TASKS_FORMAT=pretty`
  and color/header flags. Neither/both selectors use `env.usage`; absent selected
  link uses `doc_not_found`; no stdout on an unavailable provider. A spec export
  must never fetch its unrelated plan.

```rust
let docs = DocFixture::new(&env, "# body without final newline");
let id = env.json(&dir, &["add", "Doc", "--plan", "stub:alias"])["id"]
    .as_str().unwrap().to_owned();
for flags in [vec![], vec!["--json"], vec!["--pretty"]] {
    let out = env.cmd(&dir).args(&flags).args(["doc", &id, "--plan"])
        .env("COLUMNS", "invalid").output().unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(out.stdout, b"# body without final newline");
}
```

  Add local/main-only/overlay/recorded-checkout export tests to `tests/cli.rs`,
  reusing its `worktree_with_excluded_docs` and existing linked/claim fixture
  patterns. Verify warnings are on stderr in JSON as well as pretty mode.
  Add a completion test for doc task IDs that asserts no fetch log was created.

```sh
just test-one --test external_docs external_doc_export
```

- [x] **3. Implement selectors, routing and raw output.** Use a required,
  nonmultiple clap argument group for `plan` and `spec`, ordinary task-ID
  completion, and the exact summary registered above. Dispatch with
  `open_id_ctx`; parse the ID and load its claim snapshot as show does. Select
  through `read_record`, then read only the chosen link with `required()` and
  combine existing resolver/record-home warnings into `DocOut`.

```rust
let value = match kind { DocKind::Plan => task.plan.as_deref(),
                         DocKind::Spec => task.spec.as_deref() }
    .ok_or_else(|| Error::DocNotFound(format!("{} has no {} link", task.id, kind.name())))?;
let doc = resolver.read(kind, value)?.required()?;
```

  Add Doc to main's read-command classification. In the `Ok(out)` branch, handle
  `Output::Doc` before theme/layout rendering: `write_to(stdout.lock(), &body)`
  without `format!("{body}\n")`, and stderr warnings independent of output format.
  Reuse the same BrokenPipe and structured I/O error handling as normal output.
  Do not let `COLUMNS`/pretty layout reject a raw export. Global syntax/config
  validation otherwise retains current behavior. Add the Doc arm to existing
  Output matches (warnings/render/pretty), and keep raw rendering consistent;
  no serializer is used on the CLI raw path.

- [x] **4. Write RED exact-filter and row-contract cases, then implement.** Create
  completed steps under different parents plus an open step and an unstepped task.
  `list --plan stub:canonical --status done` returns only the linked completed
  rows, carrying their step. Near-match/alias input returns empty without fetching.
  Combine with existing parent/status/priority filters; search adds its phrase.
  For parked mode, filter the `Option<Task>` returned by `rows_with_records`
  before discarding it, so a recorded checkout's copy is authoritative and an
  unresolved park cannot match. Do not add plan to `Fields` or all ready filters.

```rust
let ListArgs {
    statuses, filter, plan, sort, reverse, parked, waiting_on, periodic, deferred, scope: _,
} = args;
let plan_matches = |task: &Task| {
    plan.as_deref().is_none_or(|wanted| task.plan.as_deref() == Some(wanted))
};
```

  Pass `plan: Option<String>` as a new final argument to `list_parked`; when
  it is present, filter its `(row, Option<Task>)` pairs with the same exact
  comparison before their records are discarded. An unresolved pair is false.
  Add `#[serde(skip_serializing_if = "Option::is_none")] step: Option<String>`
  to the two shared row structs and their conversions/literals. Shared JSON
  consumers gain it only when set; no table changes or separate list schema.

```sh
just test-one --test external_docs external_doc_filter
just test-one --test cli doc_
just test-one parser_surface_equals_table
just test-fast
tasks check
git add src/cli.rs src/commands/doc.rs src/commands/mod.rs src/commands/show.rs src/commands/list.rs src/output.rs src/main.rs tests/external_docs.rs tests/cli.rs tools/cli.toml
git commit -m 'feat: export linked documents and filter tasks by plan'
```

### Task 6: Document the contract and verify the complete branch

**Files:** Modify `README.md`, `skills/tasks/SKILL.md`, and
`docs/specs/2026-08-29-tasks-design.md` §5.1 (document the export exception beside
the existing general output contract). Maintain this spec/plan status; task
records are written only through the CLI.

**Interfaces:** No new runtime interface. Consumers receive the documented
`doc_unchecked` JSON kind and optional `step` field; `tasks-00723a` owns wider
agent-document workflow guidance. Real-wrapper evidence is linked on
`ops-721511`, not substituted into the dated spec measurement.

- [x] **1. Update README's opening output contract as well as command/check sections.**
  Include this explicit consumer-facing distinction near the existing JSON contract:

```text
check warnings include doc_unreachable (this host could not resolve a reference)
and doc_unchecked (one summary of external checks unfinished because the shared
fetch budget ended). Warning-only checks exit 0; a warning is not a verified
document. Finding consumers must recognize both kinds. Summary/park rows include
step only when set. tasks doc exports exact UTF-8 body bytes in every format;
its warnings go to stderr, and it does not wrap the body in JSON.
```

  Also cover grammar, host fetch config/JSON, unrelated edit behavior, concurrent
  budget tiers, tolerant display, exact plan filtering and upgrade/config order.
  Update the generic tasks skill's link/read contract without host build commands
  or duplicating the separate workflow task. No tests mirroring documentation.
  Add a short export-exception link in the original design's §5.1 so its
  JSON/stderr rules do not contradict the new body-export command. Clarify its
  summary `step`, external `spec_path`/`plan_path` and unknown `step_found`
  descriptions at the same output boundary without rewriting historical addenda.

- [x] **2. Run the required full-feature evidence.**

```sh
just test-one --test external_docs external_doc
just test-one --test cli doc_
just test-one --test move external_doc
just test-fast
just check
```

  Name the production deadline tests `external_doc_strict_production_deadline`,
  `external_doc_check_production_deadline`, and `external_doc_show_production_deadline`;
  mark these three ignored in the fast suite. Fast private-deadline cases exercise
  the same supervisor/scheduler. Run the real wiring once here:

```sh
just test-one --test external_docs production_deadline -- --ignored
```

  The installed
  pre-push hook carries the complete suite; no direct test runner or hook bypass.

- [x] **3. Smoke a real standalone release install without replacing the live CLI.**

```sh
TASKS_DOC_INSTALL_ROOT=$(mktemp -d)
cargo install --locked --path . --root "$TASKS_DOC_INSTALL_ROOT"
"$TASKS_DOC_INSTALL_ROOT/bin/tasks" --version
"$TASKS_DOC_INSTALL_ROOT/bin/tasks" doc --help
"$TASKS_DOC_INSTALL_ROOT/bin/tasks" list --help
command -v tasks
readlink -f "$(command -v tasks)"
git rev-parse HEAD
sha256sum "$TASKS_DOC_INSTALL_ROOT/bin/tasks"
```

  Retain this verified binary through the live-install gate in Task 7; record
  its absolute scratch path in a task note, not a machine-specific committed doc.
  Run the smallest isolated add/step/check/doc/filter flow with that binary:

```sh
python3 - "$TASKS_DOC_INSTALL_ROOT/bin/tasks" <<'PY'
import json, os, pathlib, shlex, subprocess, sys, tempfile
binary = str(pathlib.Path(sys.argv[1]).resolve())
body = "### Task 1: smoke\n"
with tempfile.TemporaryDirectory() as temporary:
    root = pathlib.Path(temporary)
    repo = root / "repo"
    repo.mkdir()
    config = root / "config" / "tasks"
    config.mkdir(parents=True)
    script = root / "fetch.py"
    script.write_text("import json,sys\nprint(json.dumps({'id':'canonical','body':"
                      + repr(body) + "}))\n")
    config.joinpath("config.toml").write_text(
        "[docs.schemes]\nstub = " + json.dumps("python3 " + shlex.quote(str(script))) + "\n")
    env = dict(os.environ, HOME=str(root), XDG_CONFIG_HOME=str(root / "config"),
               XDG_STATE_HOME=str(root / "state"), TASKS_SESSION="doc-install-smoke",
               TASKS_SESSION_PID=str(os.getpid()))
    for key in ("TASKS_FORMAT", "TASKS_COMPLETE", "TASKS_MAX_COMPLEXITY", "TASKS_WITHOUT",
                "TASKS_COLOR", "TASKS_PALETTE", "TASKS_THEME", "COLUMNS", "TASKS_OWNER"):
        env.pop(key, None)
    def run(*args):
        return subprocess.run([binary, "-C", str(repo), *args], env=env,
                              stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                              stderr=subprocess.PIPE, check=True, timeout=10)
    run("init", "--prefix", "smoke")
    task_id = json.loads(run("add", "Smoke", "--plan", "stub:alias",
                             "--step", "Task 1: smoke", "--process", "direct",
                             "--complexity", "low").stdout)["id"]
    assert run("check").stdout == b""
    assert run("doc", task_id, "--plan").stdout == body.encode()
    rows = json.loads(run("list", "--plan", "stub:canonical").stdout)["tasks"]
    assert len(rows) == 1 and rows[0]["id"] == task_id and rows[0]["step"] == "Task 1: smoke"
    run("start", task_id)
    run("done", task_id, "isolated install smoke passed")
    print("isolated add/step/check/doc/filter smoke passed")
PY
```

  This uses scratch HOME/XDG config+state and no live registry. Confirm exact
  export bytes and new help; no external link enters the real synced corpus.

- [x] **4. Commit docs and obtain the whole-branch review.**

```sh
tasks check
git add README.md skills/tasks/SKILL.md docs/specs/2026-08-29-tasks-design.md
git commit -m 'docs: describe external document availability and output'
```

  Dispatch one fresh whole-branch reviewer with the accepted spec, plan, full
  diff and test evidence. Record its verified identity and verdict before acting.
  In the execution ledger, resolve or rule on Critical/Important findings, with
  corrective fix+scoped re-review rounds up to five; prove fixes RED→GREEN with
  a green fast suite. Record `minor (deferred)` items and final `Ruling:`. Surface
  residual findings if that limit is reached. Do not claim the feature complete
  or begin host installation with unresolved blocking findings.

### Task 7: Approve installation, verify both hosts and close the task

**Files:** No new runtime code. Task/ops notes hold host installation and rollout
evidence; README/spec amendments only if verification reveals a real contract gap.

**Interfaces:** Consume the reviewed binary/commit; hand installation evidence
and the real-wrapper pilot contract to `ops-721511` after this task closes.
This operational task does not imply approval to alter host binaries/configuration,
push, publish, change profile rules or open a live desktop window.

- [ ] **1. Present the concrete live-install action for approval.** Name the verified
  commit/binary/hash, actual executable resolution on each host, destinations and
  how to restore the prior binary. The binary smoke and branch review must already
  be complete. Recheck registered projects' configured doc roots and existing
  first-segment-colon paths before upgrade; get roots from `tasks projects`, not
  a hand-parsed registry. A conflicting root must be resolved, never reinterpreted.
  If the second host is unavailable, park with the named host check
  as the agent's next action; retain the worktree and root task open.

- [ ] **2. Install the approved binary on both hosts, then verify host-config order.**
  Install a standalone binary, never a launcher/symlink pointing into this worktree.
  Use the normal Cargo destination and existing host controls, not a new installer.
  For a compatible second host, use the same reviewed artifact; if it must rebuild,
  use the exact reviewed commit in an isolated checkout there and record the result.
  Run version/doc help and the isolated no-scheme record read/edit/show/next/check
  cases explicitly with each installed binary. Only after that host passes may
  `ops-721511` add its scheme table in its later rollout. Do not add that table in
  this task. Remove it before any old-binary rollback if ops has since installed it.

- [ ] **3. Record the real-wrapper activation handoff on ops.** The wrapper is
  absent at planning time, so no live mindful integration success is claimed here.
  `ops-721511` depends on this task and `mind6-646ca1`; requiring its completed
  pilot here would create a cycle. This step passes the evidence/requirements,
  not an instruction to force-start ops past that dependency. The ops task later
  supplies `mindful-doc`/config and performs the pilot: on each host, use the actual distinct
  open-reference backlog, at least 20 varied refs, with the web app running.
  ops measures per-call reads and three tracker checks at concurrency four. Each must
  finish every open reference within the five-second fetch budget, without
  `doc_unchecked`; failures require the source §4.2 direct-file fallback in ops
  and a repeated pilot. Its executor saves output as an attachment on the owning
  ops task and adds its stable evidence pointer there. Keep this spec's dated
  measurement intact. Here, verify that ops's notes hold the binary/config order,
  concurrency/budget pilot gate and evidence-location requirement before handoff.

  A host configured without a scheme must still edit unrelated fields and run
  show/next/check; a stopped provider must warn instead of blocking descriptions.
  Explicit body reads and changed-link writes remain strict. Test a provider
  outage with the isolated fixture, not by shutting down the user's live service.

- [ ] **4. Record the completion boundary accurately and integrate locally.** Both
  host binary installations are required to close `tasks-91f63b`. If ops's wrapper
  pilot remains pending, note that the binary is ready but external workflow
  activation remains owned by `ops-721511`; no first real external link is written
  until its pilot and both-host binary gate pass. Do not force-close a blocked child.

  Close the implementation/installation children on this branch, then confirm
  root closeout eligibility and close the root with code/test/review and both-host
  evidence. Local merge in this personal repository is the bounded integration
  default; push/PR/global rollout remain separately authorized actions. Preserve
  unresolved activation work on its existing ops task. Before any later worktree
  removal, run `tt-report`, inspect ignored files and plan ledgers, check host
  pointers, then unlock/remove under the repository rules. No force removal.
