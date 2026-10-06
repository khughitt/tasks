# Codex Worker Identity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans
> for the recommended inline execution. If the user selects delegation, use
> superpowers:subagent-driven-development. Steps use checkboxes for tracking.

**Status:** completed on 2026-10-06. Approved spec/plan execution, four sequential
steps, deterministic/native/genuine relay acceptance and final independent review
are complete. The shared branch was merged locally into main and reinstalled
from main; executable/version and installed behavior were verified. Private
scratch state, sign-in copy and rollouts were removed. Namespace support remains
separate as tasks-b9cbe3; no push, PR or global hook/configuration change occurred.

**Goal:** Give Codex workers distinct, persistent claim identity and native note
provenance, retaining root association without transferring ownership.

**Architecture:** Keep the existing precedence ladder and ownership guards.
Share only native-value validation; claims, provenance and relay keep their own
selection/error policies. Add optional root metadata to claims, tighten Codex
process-proof continuation, and document release before worker handoff.

**Tech Stack:** Existing Rust crate, serde/TOML/JSON, existing TestEnv and harness
shims, Just test front door. No new dependencies or registry schema.

**Spec:** [Approved design](../specs/2026-10-06-delegated-identity-design.md).
Evidence: [research brief](../notes/2026-09-30-delegated-identity-brief.md).

## Global Constraints

- Preserve explicit override → configured relay → Claude → Codex → Unix precedence.
- Native Codex chooses the thread when present; both native values are validated.
  Malformed selected Codex input is a typed error, not Unix fallback.
- Native claims keep raw keys, tagged park identities, no PID and the four-hour TTL.
- Root association never grants ownership, heartbeat, closure or shared holds.
- Keep the provenance pair's serialized shape and historical notes unchanged.
- Relay stays opt-in and fail-closed, with all existing matching/cardinality checks.
- Require a matching concrete thread for Codex proof-only ownership. Missing-thread
  continuation loss is deliberate, with explicit `TASKS_SESSION` owner recovery.
- Session-only workers remain indistinguishable from their root controller.
- No upstream export repair, relay producer change, new closure capability, hook
  policy, inferred immediate parent, or automatic historical migration.
- Reuse `.worktrees/tasks-5e2472` for this design task. All file paths below are
  repository paths; commands run from the implementation worktree.
- No code work before plan approval. Then file four sequential `process: direct`
  implementation tasks under tasks-2fa8c6, linked to this spec and their exact
  `Task N` headings, and chain their dependencies. Keep tasks-5e2472 as the design
  deliverable, not the implementation goal. All four implementation tasks share
  `.worktrees/delegated-identity` on branch `feat/delegated-identity`, following
  the workspace sequence below. Never create a fresh worktree from main for a
  later step, and never merge or deploy an intermediate implementation step.
- Use `just test-one <filter>` while changing code; `just test-fast`, `tasks check`
  and `git diff --check` before each commit. Hooks supply `just check`; the full
  suite remains in the pre-push hook unless required by repository exceptions.
- Do not deploy partial steps. Live checks use the worktree binary by explicit path.
  Installation (`cargo install --path .`) follows acceptance and integration,
  as required by the repository; never repoint a shared launcher for testing.

## Review Focus

1. Malformed *unselected* root beside a valid thread must not bypass validation
   (Task 1); read-only views still work and count failed identity as owning no holds.
2. A same-owner refresh without root data must retain the recorded association,
   while an actual takeover must replace it (Task 2).
3. An unavailable relay registry and shared process must not let a sibling or a
   thread-less caller continue another actor's claim (Task 3).
4. A controller completion result does not release a worker claim: test reverse
   refusal, release/start handoff, and explicit takeover separately (Task 4).
5. An agent-waiting park is visible to third-party agents, not reserved for the
   named controller. Document and test this limit (Task 4).

## File map and sequence

| Step | Files and responsibility |
| --- | --- |
| 1 | New `src/native.rs`: value validation only. `src/main.rs`: module declaration. `src/claims.rs`, `src/provenance.rs`: native actor selection. `tests/cli.rs`: native command behavior. `README.md`, `skills/tasks/SKILL.md`: identity, source-value change and required handoff. |
| 2 | `src/claims.rs`: optional root fields, persistence validation and continuation. `src/commands/mod.rs`: acquisition/refresh rules. `src/output.rs`: JSON projection. `src/relay/resolve.rs`: result field declaration. `tests/cli.rs`: persistence and recovery. |
| 3 | `src/relay/resolve.rs`, `src/relay/mod.rs`, `src/claims.rs`, `src/commands/mod.rs`: lossless native inputs, relay hint validation, root association and strict ownership proof. Existing relay tests and shims in `tests/cli.rs`. |
| 4 | `tests/cli.rs`: handoff and takeover regression. README, shipped skill and existing specs: final contract. `docs/notes/2026-10-06-delegated-identity-acceptance.md`: sanitized live verdicts. Task records through CLI only. |

Use the existing large test modules and fixtures; do not split them as incidental
cleanup. These steps share types and execute in order, not concurrently. Each
step leaves the crate compiling and its tests passing; full remedy acceptance is
the final step. No new schema is added to `HarnessProvenance` or relay snapshots.

### One implementation workspace

After plan approval, integrate the approved design/plan and the four linked task
records into main as a document/task-only change. Note the common worktree path
and branch on all four records. Start Task 1 in main, run its trial arm, and
commit its updated record before creating the implementation worktree:

```bash
work-link --ensure .worktrees
git worktree add .worktrees/delegated-identity -b feat/delegated-identity
git worktree lock --reason "on WORK_ROOT storage (host: $(uname -n))" .worktrees/delegated-identity
```

Run `just setup` there immediately if the justfile then defines it (the reviewed
baseline does not). The first task command there is `tasks start <Task 1 id>`,
followed by its trial arm. Execute and commit all four steps on this same branch.
Each step's `tasks done` goes in its own code commit in this shared worktree.
Before the next step, run `tasks start <next task id>` there, then its trial arm;
its predecessor's code and closed task record are already in the branch. No
cherry-pick, per-step branch, or intermediate main merge is needed.

If interrupted, resume the current task in this same worktree and branch; read
its notes and `git log` to find the last completed step. Do not recreate the
worktree or reset it to main. Keep it through all steps and final review. Task 4
reviews the entire implementation diff since the approved document/task baseline
and performs the sole implementation merge to main after all acceptance gates
pass. Intermediate step completion means committed on this branch, not deployed.

### Task 1: Resolve native Codex claims and provenance by thread

**Interfaces:** add only
`native::value(get: &impl Fn(&str) -> Option<OsString>, name: &str) -> Result<Option<String>, String>`.
Keep `claims::identity_from` and `provenance::resolve_from` externally unchanged.
If the claims warnings argument becomes unused, rename it `_warnings` until a
separate reason justifies changing all callers.

- [x] **Write the regression and run it red.** Replace the old
  `disagreeing_codex_variables_warn_and_skip_the_level` expectation in the claims
  tests with the following case, using existing `env_of` and `relay_off`:

```rust
#[test]
fn delegated_native_identity_is_the_thread() {
    let mut warnings = Vec::new();
    let id = identity_from(
        env_of(&[("CODEX_SESSION_ID", "root"), ("CODEX_THREAD_ID", "worker")]),
        Some(11), relay_off(), &mut warnings,
    ).unwrap();
    assert_eq!(id.session, "worker");
    assert_eq!(id.tagged, "codex:worker");
    assert_eq!(id.pid, None);
    assert!(id.proof.is_none());
    assert!(warnings.is_empty());
}
```

  Run `just test-one delegated_native_identity_is_the_thread`. The baseline
  fails with `sid:11` rather than `worker`, not a compilation error.

- [x] **Move the existing provenance validator into the small shared module.**
  Declare `mod native;` in `src/main.rs`. Use this implementation:

```rust
use std::ffi::OsString;

pub fn value(
    get: &impl Fn(&str) -> Option<OsString>,
    name: &str,
) -> Result<Option<String>, String> {
    let Some(value) = get(name) else { return Ok(None) };
    let value = value.into_string().map_err(|_| format!("{name} is not Unicode"))?;
    if value.chars().any(char::is_control) {
        return Err(format!("{name} contains control characters"));
    }
    Ok((!value.is_empty()).then_some(value))
}
```

  In claims, invoke it only after explicit/relay/Claude precedence is exhausted:

```rust
let invalid_native = |detail| Error::Config(format!(
    "{detail}; correct the native variables or set TASKS_SESSION to this \
     session's explicit identity (use the exact held session from tasks show \
     when recovering an existing claim)"
));
let session = crate::native::value(&get, "CODEX_SESSION_ID").map_err(invalid_native)?;
let thread = crate::native::value(&get, "CODEX_THREAD_ID").map_err(invalid_native)?;
if let Some(session) = thread.or(session) {
    return Ok(Identity {
        tagged: format!("codex:{session}"), session, pid: None, proof: None,
    });
}
```

  Keep this recovery hint at the claim boundary, not in the shared validator;
  relay and provenance retain their own diagnostics. Never suggest the command
  PID as a liveness handle or let the explicit recovery impersonate a foreign
  owner. Leave Unix fallback for absence only. In provenance, wrap errors with
  `unknown harness provenance: `, keep inspection of Claude, both Codex inputs and
  the explicit override, remove unequal-ID rejection, and select:

```rust
let codex = thread.map(|id| (id, "CODEX_THREAD_ID"))
    .or_else(|| session.map(|id| (id, "CODEX_SESSION_ID")));
```

  Retain mixed-harness and qualified-override conflict handling. Do not reuse
  claim resolution to produce provenance.

- [x] **Extend existing tables, then run focused tests.** In claims and provenance
  cover equal IDs, unequal IDs, thread-only, session-only, empty and absent values;
  newline, tab, DEL and non-Unicode in either Codex value, including an invalid
  root with a valid thread. Existing explicit/Claude precedence controls must
  still pass. Claims error before mutation; provenance warns and omits its stamp.
  Change equal-ID provenance expectations to `CODEX_THREAD_ID`; retain both old
  source names in the parser/serializer tests. Use an override of `codex:root`
  against worker `worker` to keep a genuine conflict control after split IDs
  become valid. Assert errors name variables but contain no private test values.
  Malformed claim errors must also name `TASKS_SESSION` and explain repair or
  explicit-identity recovery; assert a matching explicit override still takes
  precedence over malformed native inputs when resuming an owned claim.

  Run `just test-one claims::tests`, `just test-one provenance::tests`, and
  `just test-one format::tests::provenance`. Update any old split-ID CLI assertions
  in `tests/cli.rs` to the new behavior, preserving distinct malformed/conflicting
  controls rather than deleting their coverage. Add `delegated_native_commands`
  there using TestEnv commands in separate processes: start, show, note --stamp,
  show; require stable worker key, absent PID, live claim and worker note source.
  Run `just test-one --test cli delegated_native_commands` red then green.

- [x] **Ship the handoff instructions with the behavior.** Add the final Task 4
  skill wording now, before anyone uses persistent worker claims. Update README
  and the skill's provenance paragraph: thread-first source even for equal IDs,
  no pair-shape change, and session-only worker isolation remains unsupported.
  Keep the detailed relay/root documentation for the steps that implement it.

- [x] **Verify and commit.** Run `cargo fmt`, `just test-fast`, `tasks check`,
  and `git diff --check`; mark this implementation step done with the code commit
  `fix: resolve native Codex worker identity by thread`. Do not install it yet.

### Task 2: Carry and preserve root association on claims

**Interfaces:** add `root_session: Option<String>` to `claims::Identity`,
`claims::Claim`, `relay::resolve::Resolved`, and `output::ClaimInfo`.
Only Claim and ClaimInfo serialize it, with absent values omitted and Claim
deserialization defaulting to None. No park, escalation or note field is added.

- [x] **Write a failing persistence regression.** Add this test to `tests/cli.rs`:

```rust
#[test]
fn delegated_root_refresh_and_recovery() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Root association"]));
    let run = |root: Option<&str>, args: &[&str]| {
        let mut cmd = env.cmd(&dir);
        cmd.env("CODEX_THREAD_ID", "worker");
        if let Some(root) = root { cmd.env("CODEX_SESSION_ID", root); }
        cmd.args(args).output().unwrap()
    };
    assert!(run(Some("root"), &["start", &id]).status.success());
    assert_eq!(env.json(&dir, &["show", &id])["claim"]["root_session"], "codex:root");
    assert!(run(None, &["start", &id]).status.success());
    assert_eq!(env.json(&dir, &["show", &id])["claim"]["root_session"], "codex:root");
    let record = std::fs::read(dir.join(format!("tasks/{id}.md"))).unwrap();
    let store = std::fs::read(env.claim_store("sci")).unwrap();
    let refused = run(Some("other"), &["start", &id]);
    assert!(!refused.status.success());
    assert_eq!(std::fs::read(dir.join(format!("tasks/{id}.md"))).unwrap(), record);
    assert_eq!(std::fs::read(env.claim_store("sci")).unwrap(), store);
    assert!(run(Some("other"), &["park", &id, "intentional reassociation"]).status.success());
    assert!(run(Some("other"), &["start", &id]).status.success());
    assert_eq!(env.json(&dir, &["show", &id])["claim"]["root_session"], "codex:other");
}
```

  Run `just test-one --test cli delegated_root_refresh_and_recovery`; expect the
  first root assertion to fail because the field is absent.

- [x] **Add the fields and populate each constructor.** Native Codex sets
  `root_session = session.as_ref().map(|id| format!("codex:{id}"))` before moving
  the thread/session options. Explicit, Claude and Unix identity use None.
  Relay identity copies `resolved.root_session`; until Task 3 the resolver emits
  None. `continuation_identity` copies the held claim's root. `ClaimInfo::of`
  copies it. Existing struct-update operations retain it automatically.

  Add `#[serde(default, skip_serializing_if = "Option::is_none")]` on Claim's
  field and `#[serde(skip_serializing_if = "Option::is_none")]` on ClaimInfo's.
  Find all literals with `rg -n 'Identity \{|Claim \{|Resolved \{' src tests`
  and update their intended root explicitly; do not add a blanket Default impl.

- [x] **Enforce refresh rules only on claim acquisition.** Inside
  `Ctx::claim_guard`, in the `to == Status::Doing` branch before constructing
  `Claim`, compute:

```rust
let root_session = if mine {
    let old = existing.as_ref().and_then(|claim| claim.root_session.as_ref());
    match (old, me.root_session.as_ref()) {
        (Some(old), Some(new)) if old != new => return Err(Error::Config(format!(
            "{id}: CODEX_SESSION_ID disagrees with the recorded root; inspect tasks show \
             and the environment; restore the exports, or park this task and start it \
             again to intentionally reassociate it"
        ))),
        (Some(old), None) => Some(old.clone()),
        _ => me.root_session.clone(),
    }
} else {
    me.root_session.clone()
};
```

  Put the result in the acquired claim. This refuses changed known roots before
  any persistent write; it does not block an authorized park/done, and same-owner
  `start --force` cannot bypass it. Takeovers use the new owner's root, including
  None for an explicit override. Proof-only identity preserves the recorded root.

- [x] **Validate stored roots and cover every projection.** In
  `ClaimStore::load_from`'s claim validation loop reject a present root unless
  `strip_prefix("codex:")` yields a nonempty suffix with no control characters.
  The typed error names the task and field, not the value. Use old-store and new
  TOML round-trip fixtures, an invalid-root fixture, and ClaimInfo serialization
  tests for present/absent roots. Do not invent a relation to raw claim keys or
  require a root on historical claims.

  `codex:codex:x` is intentionally valid: the outer `codex:` is the scheme and
  the opaque native ID is `codex:x`. Neither writer nor validator interprets or
  strips a scheme-looking prefix inside a native ID. Add a stored-root fixture
  that round trips this value and append this producer check to the CLI test
  above, using its existing `run` closure:

```rust
let opaque = id_of(env.json(&dir, &["add", "Opaque root"]));
assert!(run(Some("codex:x"), &["start", &opaque]).status.success());
assert_eq!(env.json(&dir, &["show", &opaque])["claim"]["root_session"],
    "codex:codex:x");
```

  The CLI check proves the writer can produce it; the store fixture proves it
  survives reload. Keep rejection controls for bare `codex:`, wrong outer scheme,
  and control characters, without introducing UUID-only validation.

  Extend the CLI test with explicit override and different-thread forced takeover
  cases, each asserting the displaced root does not survive; exercise same-owner
  force refusal and recovery guidance without ID values. Through `ClaimInfo::of`
  cover show, prime/doing, and claims views using their existing response shapes.
  Test a malformed native root on a read command with an exclusive hold: views
  still succeed but count no holds as this caller's. Use existing hold fixtures.

  Run `just test-one delegated_root`, `just test-one claims::tests`, and
  `just test-one output::tests`. Update README to define optional claim-only root
  metadata, retention and reassociation recovery, and mixed-writer loss limits.

- [x] **Verify and commit.** Run `cargo fmt`, `just test-fast`, `tasks check`,
  `git diff --check`; close the step with `feat: record Codex root association on claims`.

### Task 3: Validate relay hints and require concrete Codex ownership proof

**Interfaces:** change the environment callback from `Option<String>` to
`Option<OsString>` in `relay::resolve::{hint_for, resolve}` and
`claims::proves_ownership`. Real adapters in `relay::level` and
`commands::ownership` pass `std::env::var_os` directly. Update existing test
environment closures to return OsString. The return types and non-Codex policies
stay unchanged. `Resolved.root_session` now carries validated native root data
only for a selected Codex registry row.

- [x] **Pin the changed acceptance before editing the resolver.** In the existing
  relay unit module add, using its `env` helper:

```rust
#[test]
fn delegated_relay_hint_selects_thread() {
    let get = env(&[("CODEX_SESSION_ID", "root"), ("CODEX_THREAD_ID", "worker")]);
    assert_eq!(hint_for("codex", &get).unwrap().as_deref(), Some("worker"));
}
```

  Run `just test-one delegated_relay_hint_selects_thread`; baseline rejects the
  pair. Add proof cases to the existing claims proof tests: held `codex:worker`,
  same process/boot/host, and each of matching thread, sibling thread, session-only,
  no hints, invalid root with valid thread. Only the matching valid thread proves.
  Retain Claude's no-hint proof control. Run the affected proof test red.

- [x] **Use the shared validator without collapsing malformed values to absence.**
  The Codex hint branch becomes:

```rust
let session = crate::native::value(get, "CODEX_SESSION_ID").map_err(refuse)?;
let thread = crate::native::value(get, "CODEX_THREAD_ID").map_err(refuse)?;
Ok(thread.or(session))
```

  Retain existing non-Codex conversion/scoping behavior. Resolve hints before
  registry load as today. After selecting the sole qualifying row and checking
  its actor hint, populate root_session from the validated Codex session export;
  leave it None for other harnesses. Do not synthesize worker rows, prefer a hint
  among multiple qualifying rows, or change snapshot schema/cardinality rules.

  Replace the existing mismatch refusal that prints `hint` and `agent.id`.
  Inside the existing `hint != agent.session_id` branch, distinguish a root-only
  Codex match using the validated root export; keep other mismatches generic:

```rust
let root = if harness == "codex" {
    crate::native::value(get, "CODEX_SESSION_ID").map_err(refuse)?
} else {
    None
};
let detail = if harness == "codex" && root.as_deref() == Some(agent.session_id.as_str()) {
    "only the Codex root is registered in relay; worker registration is unavailable \
     (relay-f56e30), so the worker cannot claim as its controller"
} else {
    "the native actor hint does not match the qualifying relay row"
};
return Err(refuse(detail.into()));
```

  The surrounding mismatch condition plus the root match distinguishes the
  worker case from session-only input. Do not assert missing worker registration
  for an unrelated registry mismatch. `refuse` keeps its `TASKS_SESSION` recovery
  guidance; neither message interpolates native IDs or registry row IDs.

  In proof, keep all current process checks, then use this Codex-only branch:

```rust
if boundary.harness == "codex" {
    if crate::relay::resolve::hint_for("codex", get).is_err() { return false; }
    return match crate::native::value(get, "CODEX_THREAD_ID") {
        Ok(Some(thread)) => crate::relay::resolve::same_session(
            &claim.session, "codex", &thread,
        ),
        _ => false,
    };
}
```

  Leave other harness proof behavior and explicit-override short-circuiting alone.
  Existing raw/tagged/relay key equivalence remains confined to same_session.

- [x] **Audit all callers and update meaningful controls.** Search every
  `hint_for`, `resolve`, `proves_ownership` and relevant environment helper call.
  Existing Codex shim tests which relied on absent hints to continue their own
  claim now export the matching thread explicitly in those tests; do not globally
  inject it in `shim_env`, which would erase absent-input coverage. Keep dedicated
  session-only/absent-hint refusals and exact-identity successes.

  Add `delegated_relay_ownership` CLI coverage with existing `relay_on`, `shim_env`
  and `common::harness_shim`: root-only row refuses a worker; matching synthetic
  worker row acquires qualified worker key/root; missing registry allows matching
  held-thread proof, but not sibling/session-only/absent/malformed hints. Verify
  the registered root row and shared PID never grant worker ownership. Exercise
  start, park, done, protected edit, heartbeat and exclusive holds through shared
  ownership; refused writes preserve task/claim contents, while note's documented
  write-with-skipped-heartbeat behavior is preserved.

  Include a legitimate owner using exact held `TASKS_SESSION` to recover from
  missing-thread proof failure. No PID is fabricated. Verify malformed relay
  input errors include variable names/recovery guidance and no input values.
  Assert the root-row/worker-hint refusal names missing worker registration,
  `relay-f56e30` and `TASKS_SESSION`, with neither test ID present. Also test an
  unrelated row mismatch: it uses the generic value-free refusal, not the
  root-only diagnosis, and still leaves the claim/record untouched.
  Keep invalid registry, ambiguity, wrong host/boot/platform and other-harness
  controls intact. Run `just test-one --test cli delegated_relay_ownership`,
  `just test-one relay::`, and `just test-one claims::tests`.

- [x] **Document the break, verify and commit.** Amend README and the relay
  ancestry spec's hints/proof paragraphs, preserving historical context and
  linking the approved design. Spell out missing-thread proof loss, explicit
  legitimate-owner recovery, root-only registry refusal and relay-f56e30.
  Run `cargo fmt`, `just test-fast`, `tasks check`, `git diff --check`; close the
  step with `fix: require concrete Codex thread for relay ownership proof`.

### Task 4: Verify worker handoff, run live acceptance and close the remedy

**Interfaces:** no new CLI or authorization API. Use `start`, `park`, `done`,
`note --stamp`, `show`, `claims`, and existing takeover recording. Native live
checks use actual exported IDs, while deterministic tests use TestEnv inputs.

- [x] **Add the reverse-direction handoff regression.** In `tests/cli.rs`:

```rust
#[test]
fn delegated_worker_handoff_and_takeover() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Worker handoff"]));
    let run = |thread: &str, args: &[&str]| env.cmd(&dir)
        .env("CODEX_SESSION_ID", "root").env("CODEX_THREAD_ID", thread)
        .args(args).output().unwrap();
    assert!(run("worker", &["start", &id]).status.success());
    let held = env.json(&dir, &["show", &id]);
    assert_eq!(held["claim"]["session"], "worker");
    assert_eq!(held["claim"]["live"], true);
    assert!(!run("root", &["done", &id, "premature"]).status.success());
    assert!(!run("root", &["park", &id, "premature"]).status.success());
    assert!(run("worker", &["park", &id, "controller reviews", "--waiting-on", "agent"])
        .status.success());
    assert!(run("root", &["start", &id]).status.success());
    assert!(run("root", &["done", &id, "reviewed"]).status.success());

    let abandoned = id_of(env.json(&dir, &["add", "Worker left claim"]));
    assert!(run("worker", &["start", &abandoned]).status.success());
    assert!(!run("root", &["done", &abandoned, "premature"]).status.success());
    assert!(run("root", &["start", &abandoned, "--force", "--reason",
        "finished worker left a live claim"]).status.success());
    assert!(run("root", &["done", &abandoned, "reviewed"]).status.success());
    let shown = env.json(&dir, &["show", &abandoned]);
    assert!(shown["task"]["notes"].as_array().unwrap().iter().any(|n|
        n["text"].as_str().unwrap().contains("finished worker left a live claim")));
}
```

  Run `just test-one --test cli delegated_worker_handoff_and_takeover`. This
  locks in existing lifecycle commands under the new identity contract; it is
  expected to pass after Tasks 1–3. Confirm its identity premise fails against
  the pre-remedy binary or temporary reverted native selector, then restore it.
  Extend with record/store snapshots on refusals, protected-edit refusal, hold
  ownership, worker-owned done release, and a separate park picked up by a third
  distinct agent. Check the parked task appears to that agent in next/ready and
  that its start succeeds; this is a limit, not a desired reservation feature.

- [x] **Use the final skill text below.** Task 1 must already have shipped it;
  here verify it agrees with tested behavior and the approved spec:

> A worker releases its claims before ending its assignment: `done` if responsible
> for closing completed work, otherwise `park --waiting-on agent` naming the
> controller's next action and authoritative worktree. Verify release before
> reporting completion. The controller runs `start` before review edits or closure.
> An agent-waiting park is visible to every agent and is not reserved for the
> controller; its next-step text is a convention, not an ownership restriction.
> If a worker exited without release, the controller may use explicit `start
> --force --reason` recovery only after its own wait/completion result for that
> specific worker establishes the assignment has stopped, with no pending tool,
> check or background process. A timeout, silence, the root session's state, and
> the shared harness PID are not that evidence. If completion reports work still
> running or the state is uncertain, reconcile it with the worker and keep waiting;
> do not take over. Inspect the worker's result and claim before takeover. The
> worker may be resumable later; do not resume it on the task after taking over
> without a new ownership handoff. No worker closes a controller's live claim,
> and no controller closes a worker's live claim merely by being its parent.

  Use the harness's tracked waits; no detached monitor. The evidence is the
  controller's worker-specific completion result, not a nonexistent relay worker
  row or root Stop hook. This does not change tasks-f8c9cc's authorization scope.

- [x] **Check native live-smoke preconditions.** The controller must be an actual
  Codex session with working worker-spawn and tracked wait/completion tools, and
  native Codex IDs available to its command environment. The controller and its
  real worker must have no nonempty `CLAUDE_CODE_SESSION_ID`. A Claude Code
  implementer cannot substitute its own session for this smoke. Codex launched
  beneath Claude may inherit that variable: Claude then wins claim precedence,
  and mixed-harness provenance is intentionally omitted.

  Check only presence and equality relationships, not private values. If running
  under the wrong harness, lacking spawn support, or observing an inherited
  Claude ID in either participant, record **unverified: setup precondition not
  met**, not a product test failure. Preserve the observed exports; do not unset
  the Claude ID in-place to manufacture a clean native capture. Continue
  independent deterministic checks, then park this live gate for an eligible
  clean Codex controller session. A future clean session may resume the same
  implementation worktree; implementation need not be repeated.

- [x] **Run the native live smoke as a bounded controller/worker exchange.** Build
  the worktree debug binary through the focused test recipe. Use a temporary
  scratch directory with private XDG config/state directories and a scratch
  project initialized by that explicit binary. Keep native Codex exports exactly
  as delivered; unset explicit TASKS_SESSION/PID overrides only within the smoke
  command environment. Do not mutate the harness or real host config. Record the
  Codex version and whether the runtime's ID relationships match the audit.
  Missing or non-distinct worker thread input leaves native live acceptance
  unverified; do not inject IDs to turn it into a passing real capture.

  The controller creates three scratch tasks (controller-owned, normal handoff,
  intentional unreleased-worker control), starts its own, and dispatches one
  worker with the binary and scratch locations. The worker starts its two tasks
  in one command, reads/refreshes them and stamps a note in another, and attempts
  the prohibited close of the controller task. It returns a checkpoint so the
  controller can assert live distinct worker claims and fail its own premature
  closure. It then parks the normal-handoff task and ends, explicitly identifying
  the one deliberately unreleased control. The controller records its actual
  wait/completion result; after completion it resumes/closes the parked task,
  verifies unforced closure of the unreleased task still refuses, and uses forced
  start with the recorded reason before closing it. Finally close the controller
  scratch task. Check exact note provenance, root associations, live-after-command
  behavior and takeover notes. No four-hour wait is needed; TTL tests are pure.

  A checkpoint is not termination evidence. The controlled unreleased task is a
  test exception, not guidance for ordinary workers. On any failure, collect the
  partial verdict before cleanup; wait for the worker to stop, release remaining
  scratch claims, and remove scratch registry/state. Do not leave jobs detached.

- [x] **Capture real relay behavior separately.** Check where the installed relay
  executable resolves before using it. Inspect its supported isolated state/config
  invocation and arrange actual root hook delivery to a private registry for the
  scratch Codex session, with the user's host-use/config permissions if needed.
  The current hook entry is
  `relay hook --harness codex --subscribers "$scratch_subscribers" SessionStart`
  with an absolute scratch subscriber-file path (a Node
  script invocation uses `node bin/relay.js` instead of the launcher), with
  `RELAY_STATE_DIR` pointing at the scratch state. Invoke it through a genuinely
  delivered, trusted scratch-session hook with an explicit private subscriber
  file via `--subscribers`; do not replay manufactured input as a live event.
  Reuse a live source capture only if it actually supplies this run's registry
  evidence. Do not use `WRITE_REGISTRY` or a harness shim as a real capture.

  With relay identity enabled only for scratch tasks commands, prove the real
  root-only registry refuses worker acquisition and does not adopt the controller
  row. Record event/row presence and sanitized identity relationships, not raw
  IDs. A synthetic matching-worker row remains unit/integration coverage only.
  If trusted isolated real hook delivery cannot be arranged, stop this step at
  the documented live gate and park with the exact prerequisite; keep the remedy
  reports open. No global launcher/config repointing or quiet-host request is
  implied by plan approval.
  The real relay smoke likewise needs an eligible Codex controller/worker pair;
  inherited mixed-harness IDs leave its setup unverified as above.

- [x] **Record verdicts and update contracts.** Write the acceptance note with
  columns: case, native/synthetic/live-relay evidence, expected result, observed
  result, pass/fail/unverified. Preserve the root-vs-immediate-parent distinction.
  Update the old work-claims equality premise, final README/skill guidance, and
  the brief's resolved-question links. Cite acceptance without claiming worker
  Stop/SessionEnd/Interrupt delivery or relay worker registration was fixed.

- [x] **Review, integrate and deploy through the normal gates.** Run focused tests
  for any changed logic, then `just test-fast`, `tasks check`, `git diff --check`.
  Obtain a final whole-branch independent review of code, documents and live
  verdicts under requesting-code-review; log its outcome before acting. Correct
  Important/Critical findings and re-review under repository policy. Prepare the
  final code/closeout commit below, run `tasks check`, then commit on the shared
  `feat/delegated-identity` branch and merge that complete branch locally only
  after required checks and review pass. Tasks 1–3 stay on that branch until this
  single implementation merge. Follow the repository's explicit
  reinstall requirement: run `cargo install --path .` from the main checkout
  on main after the final merge, so installation builds the merged commit;
  verify `command -v tasks` and `tasks --version`, and ensure participating sessions
  use the new binary. Report an installation failure separately; do not claim it
  was deployed. No push, PR, hook installation or global configuration change is
  authorized by this instruction.

  Close the implementation step and the six reports only when the reviewed remedy
  and required acceptance are complete, using CLI writes in their code/closeout
  commit: tasks-5745bb, tasks-616cd7, tasks-cf8687, tasks-290fa3, tasks-6e2d15 and
  tasks-f537a8. Preserve sources. Give tasks-0c2c39 a note explicitly distinguishing
  the fixed verified split-ID symptom from uncaptured missing-export history and
  the still-unfixed session-only worker limit. Close that historical report only
  with that bounded disposition, never claiming missing exports were repaired.
  Keep relay-f56e30, tasks-f8c9cc and tasks-56b450 outside this closeout.

## Design-task handoff

The user resumed execution on 2026-10-06. The document/task-only handoff was
integrated locally and tasks-5e2472 closed as the reviewed design/plan deliverable.
Linked direct steps are tasks-3809b8, tasks-2047cd, tasks-17c844 and tasks-a5a502.
The first three are committed; all four are closed after reviewed native/relay acceptance, local merge and installation. The implementation
steps then proceed sequentially on the single `feat/delegated-identity` branch
in `.worktrees/delegated-identity`, using the workspace sequence above. The
document/task-only integration is not an intermediate implementation merge.

## Worktree cleanup

After the document/task-only merge, remove `.worktrees/tasks-5e2472` once all its
changes are committed and integrated. Keep `.worktrees/delegated-identity` through
all implementation steps, acceptance and final review; remove it after the final
merge and installation from main. A blocked installation or acceptance run keeps
the implementation worktree available for recovery.

For either removal, run `tt-report` in that worktree first. From the main checkout,
check that the target is clean and integrated and that no host pointer resolves
into it: use `readlink -f` on launchers under `~/bin` and `~/.local/bin`, and inspect
user systemd units and known configuration includes. Refuse removal while any
pointer targets the worktree; restore a recorded test pointer before retrying.
Then run `git worktree unlock` followed by `git worktree remove` with that
worktree's `.worktrees/` path. Do not use force removal or discard unrelated work.
