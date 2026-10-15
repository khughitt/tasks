# Codex Worker Rows Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task, inline (Native, the execution method chosen at plan round 1), then dispatch one fresh read-only reviewer on the most capable model for the whole branch. Steps use checkbox (`- [ ]`) syntax for tracking.

**Status:** approved by Keith on 2026-10-15 after plan round 1 (fix: a session export equal to the worker's id is tested against the worker-thread guard).

**Goal:** tasks accepts relay's Codex worker rows (`scope: "worker"`, `root: "codex:<R>"`) and resolves a Codex caller among rows sharing one process by its thread hint. A worker row is selected only by that worker's own `CODEX_THREAD_ID`.

**Architecture:** `snapshot::parse_agent` mirrors relay's planned validator for `scope` and `root`. `relay::resolve` reads both Codex exports once, uses the hint to break ties among several qualifying Codex rows, and refuses a worker row unless the caller's thread names it. A shared `hint_mismatch` diagnosis replaces the `relay-f56e30` wording. `Agent` gains no field.

**Tech Stack:** Rust (serde_json), unit tests in `src/relay/{snapshot,resolve}.rs`, `assert_cmd` shim tests in `tests/cli.rs`.

**Spec:** `docs/specs/2026-10-06-delegated-identity-design.md`, section "Relay worker rows (2026-10-15 amendment)" and §6 item 4, approved after spec round 1. Producer contract: relay `docs/specs/2026-10-15-codex-worker-rows.md`. Read the amendment with this plan.

## Global Constraints

- The snapshot schema stays 2. `Agent` gains no field, and `root` is validated, never stored.
- `scope` is `process` for OpenCode, `session` for Claude Code, and `session` or `worker` for Codex.
- `root` is required on a `worker` row. Any other row must omit the key entirely, so `root: null` is refused. On a worker row, `root` is text `codex:<suffix>` with a suffix that is non-blank under the `trim()` rule, and it differs from the row's `id`.
- Tie-break: only when the boundary harness is Codex, a hint is present and more than one row qualifies. The selection does not depend on `scope`. Claude Code and OpenCode are unchanged.
- A selected `worker` row requires a valid, present `CODEX_THREAD_ID` equal to its `sessionId`, whether it was the sole qualifying row or selected by the tie-break.
- Refusals never name a native session value or a worker row's session. Every refusal keeps the `TASKS_SESSION` recovery (`refuse()` appends it).
- The claim's `root_session` stays `codex:<CODEX_SESSION_ID>` (§2), never the row's `root`.
- The fixture is byte-for-byte relay's `test/fixtures/registry-workers.json` as fixed in relay's plan at relay revision `74ce0d8` (2422 bytes).
- Tests run only through `just test-one <args>` and `just test-fast`, never `cargo test`. `just check` runs before each commit.

## Review Focus

1. **A sole worker row with no Codex exports.** A controller or sibling command must not resolve as that worker. It is refused before any claim or record write (spec round 1 finding). Pinned in Task 2 (unit test and CLI control).
2. **A sole worker row with only `CODEX_SESSION_ID`.** The root's id is refused with the generic, value-free hint diagnosis. The worker's own id passes the ordinary hint check and is refused by the worker-thread guard: a session export never selects a worker row (plan round 1 finding). Pinned in Task 2 (unit test, CLI control, and the mutation check in Step 7).
3. **`root: null` on a root row**, which a careless producer might emit. It is refused as relay refuses it, not read as absent. Pinned in Task 1.
4. **Two Claude Code rows on one process with a matching hint.** They still refuse as ambiguous, because the tie-break is Codex-only. Pinned in Task 2.
5. **A worker thread that relay has not registered, under its root.** It gets the worker-row refusal, never the controller's identity, and the error carries no ids. Pinned in Task 2 (fixture test and the updated `delegated_relay_ownership`).

---

### Task 1: The snapshot accepts worker rows

**Files:**
- Create: `tests/fixtures/relay/registry-workers.json` (vendored bytes)
- Modify: `tests/fixtures/relay/README.md`
- Modify: `src/relay/snapshot.rs` (`parse_agent`, the scope check at lines 235-245; tests module)

**Interfaces:**
- Produces: `snapshot::parse` accepts Codex rows with `scope == "worker"`. `Agent.scope` carries `"worker"` for them, and resolution reads that value in Task 2. The fixture at `tests/fixtures/relay/registry-workers.json` holds ids `R = 0199f000-0000-7000-8000-000000000001`, `W1 = …0002`, `W2 = …0003` (nested; root `R`), and Claude `…00c1`. Every Codex row's handle is `{platform: linux, host: "host", bootId: "12345678-1234-4234-8234-123456789abc", pid: 4242, start: "123456"}`.

- [ ] **Step 1: Vendor the fixture**

Copy the bytes out of relay's approved plan. The branch is shared by every relay worktree, so this runs from the relay checkout:

```bash
RELAY=$(tasks root relay-635fdc | jq -r .root)
git -C "$RELAY" show 74ce0d8:docs/plans/2026-10-15-codex-worker-rows.md \
  | awk '/^`test\/fixtures\/registry-workers.json`:$/{f=1;next} f&&/^```json$/{p=1;next} p&&/^```$/{exit} p' \
  > tests/fixtures/relay/registry-workers.json
wc -c tests/fixtures/relay/registry-workers.json   # expect 2422
jq '.agents | length' tests/fixtures/relay/registry-workers.json   # expect 4
```

If `74ce0d8` is unreachable on this host, stop and park the task `--reason dependency`. Do not retype the fixture.

Append to `tests/fixtures/relay/README.md`:

```markdown

`registry-workers.json` is relay's shared worker-row snapshot: a Codex root row, a worker
and a nested worker (whose `root` names the root, not its parent) on one handle, and an
unrelated Claude Code row. Relay commits it as `test/fixtures/registry-workers.json` in
relay-635fdc Task 1, after tasks has landed, so it was copied byte for byte from that
task's step in relay's plan `docs/plans/2026-10-15-codex-worker-rows.md` at revision
`74ce0d8`. When relay's commit lands, compare the bytes, then replace this source with the
relay revision of `test/fixtures/registry-workers.json`. A difference is a contract change
to settle with relay, not a local edit. `src/relay/snapshot.rs` parses it, and
`src/relay/resolve.rs` resolves each of its Codex threads against it.
```

- [ ] **Step 2: Write the failing tests**

Append inside `mod tests` in `src/relay/snapshot.rs`:

```rust
    const WORKERS: &str = include_str!("../../tests/fixtures/relay/registry-workers.json");

    #[test]
    fn relay_worker_rows_fixture_parses() {
        let snapshot = parse(WORKERS).unwrap();
        let scopes: Vec<(&str, &str)> = snapshot
            .agents
            .iter()
            .map(|agent| (agent.id.as_str(), agent.scope.as_str()))
            .collect();
        assert_eq!(
            scopes,
            vec![
                ("claude-code:0199f000-0000-7000-8000-0000000000c1", "session"),
                ("codex:0199f000-0000-7000-8000-000000000001", "session"),
                ("codex:0199f000-0000-7000-8000-000000000002", "worker"),
                ("codex:0199f000-0000-7000-8000-000000000003", "worker"),
            ]
        );
        let handles: Vec<Handle> = snapshot
            .agents
            .iter()
            .filter(|agent| agent.harness == "codex")
            .map(|agent| agent.process.clone().unwrap())
            .collect();
        assert!(handles.windows(2).all(|pair| pair[0] == pair[1]), "{handles:?}");
    }

    /// A valid worker row; tests override one field at a time from here.
    fn worker_json(overrides: &[(&str, &str)]) -> String {
        let mut fields: Vec<(&str, &str)> = vec![
            ("id", "\"codex:w1\""),
            ("sessionId", "\"w1\""),
            ("scope", "\"worker\""),
            ("root", "\"codex:r\""),
        ];
        fields.extend_from_slice(overrides);
        agent_json(&fields)
    }

    #[test]
    fn relay_worker_row_is_a_codex_row_with_a_root() {
        let snapshot = parse(&snapshot_json("codex:w1", &worker_json(&[]))).unwrap();
        assert_eq!(snapshot.agents[0].scope, "worker");

        let other_harness = |harness: &str| {
            agent_json(&[
                ("id", format!("\"{harness}:w1\"").as_str()),
                ("harness", format!("\"{harness}\"").as_str()),
                ("sessionId", "\"w1\""),
                ("scope", "\"worker\""),
                ("root", "\"codex:r\""),
            ])
        };
        let cases: Vec<(&str, String, &str)> = vec![
            ("codex:w1", worker_json(&[("root", "\u{0}")]), "without a root"),
            ("codex:w1", worker_json(&[("root", "\"codex:w1\"")]), "invalid root"),
            ("codex:w1", worker_json(&[("root", "\"claude-code:r\"")]), "invalid root"),
            ("codex:w1", worker_json(&[("root", "\"codex:\"")]), "invalid root"),
            ("codex:w1", worker_json(&[("root", "\"codex: \"")]), "invalid root"),
            ("codex:w1", worker_json(&[("root", "7")]), "invalid root"),
            ("codex:w1", worker_json(&[("root", "null")]), "invalid root"),
            ("codex:s1", agent_json(&[("root", "\"codex:r\"")]), "not a worker row"),
            ("codex:s1", agent_json(&[("root", "null")]), "not a worker row"),
            ("claude-code:w1", other_harness("claude-code"), "expected \"session\""),
            ("opencode:w1", other_harness("opencode"), "expected \"process\""),
        ];
        for (key, agent, expected) in cases {
            let error = parse(&snapshot_json(key, &agent)).unwrap_err().to_string();
            assert!(error.contains(expected), "{key} {agent}: {error}");
        }
    }
```

`agent_json` replaces a field already present in place, and a later override of the same key wins. `"\u{0}"` omits the field.

- [ ] **Step 3: Run them to verify they fail**

Run: `just test-one relay_worker`
Expected: FAIL. `relay_worker_rows_fixture_parses` panics with `has scope "worker", expected "session"`. `relay_worker_row_is_a_codex_row_with_a_root` panics on its first `unwrap` with the same message.

- [ ] **Step 4: Implement**

In `src/relay/snapshot.rs` `parse_agent`, replace the scope check:

```rust
    let scope = text_field(value, "scope")?;
    let expected_scope = if harness == "opencode" {
        "process"
    } else {
        "session"
    };
    if scope != expected_scope {
        return Err(invalid(&format!(
            "agent {key:?} has scope {scope:?}, expected {expected_scope:?}"
        )));
    }
```

with:

```rust
    let scope = text_field(value, "scope")?;
    // A Codex worker shares its controller's process and names the root session it belongs
    // to; no other row carries a root, not even a null one (relay spec 2026-10-15, mirrored
    // from relay's validator).
    let worker = harness == "codex" && scope == "worker";
    let expected_scope = if harness == "opencode" {
        "process"
    } else {
        "session"
    };
    if !worker && scope != expected_scope {
        return Err(invalid(&format!(
            "agent {key:?} has scope {scope:?}, expected {expected_scope:?}"
        )));
    }
    match (worker, value.get("root")) {
        (true, Some(root)) => {
            let valid = root.as_str().is_some_and(|root| {
                root != id
                    && root
                        .strip_prefix("codex:")
                        .is_some_and(|suffix| !suffix.trim().is_empty())
            });
            if !valid {
                return Err(invalid(&format!("agent {key:?} has an invalid root")));
            }
        }
        (true, None) => {
            return Err(invalid(&format!(
                "agent {key:?} is a worker row without a root"
            )));
        }
        (false, Some(_)) => {
            return Err(invalid(&format!(
                "agent {key:?} has a root but is not a worker row"
            )));
        }
        (false, None) => {}
    }
```

- [ ] **Step 5: Run them to verify they pass**

Run: `just test-one relay_worker`
Expected: PASS (2 tests).

- [ ] **Step 6: Fast suite, check, commit**

Run: `just test-fast` (expected: all pass), then `just check` (expected: clean).

```bash
git add tests/fixtures/relay/registry-workers.json tests/fixtures/relay/README.md src/relay/snapshot.rs
git commit -m "feat(relay): accept Codex worker rows in the registry snapshot"
```

---

### Task 2: Select a Codex row by its thread

**Files:**
- Modify: `src/relay/resolve.rs` (`resolve` from `let hint = hint_for(...)` to the end; new `hint_mismatch`; tests module)
- Modify: `tests/common/mod.rs:373-389` (`WRITE_REGISTRY`)
- Modify: `tests/cli.rs` (`delegated_relay_ownership` assertions, near line 28345; new test after it)
- Modify: `README.md:283-286`, `skills/tasks/SKILL.md:376-377`, `docs/specs/2026-09-22-relay-ancestry-identity-design.md:194-199`

**Interfaces:**
- Consumes: Task 1's acceptance of `scope == "worker"`, and the fixture ids and handle listed there.
- Produces: `fn hint_mismatch(root: Option<&str>, candidates: &[&crate::relay::snapshot::Agent]) -> Error` (private). The `write_registry` shell function reads optional `SCOPE` (default `session`) and `ROOT_ID` (adds `"root"` when set). Existing callers set neither and produce identical bytes.

- [ ] **Step 1: Write the failing unit tests**

Append inside `mod tests` in `src/relay/resolve.rs`:

```rust
    const WORKERS: &str = include_str!("../../tests/fixtures/relay/registry-workers.json");
    const FIXTURE_BOOT: &str = "12345678-1234-4234-8234-123456789abc";
    const R: &str = "0199f000-0000-7000-8000-000000000001";
    const W1: &str = "0199f000-0000-7000-8000-000000000002";
    const W2: &str = "0199f000-0000-7000-8000-000000000003";

    /// Resolve against the vendored worker fixture from its shared Codex process.
    fn workers(e: &impl Fn(&str) -> Option<OsString>) -> Result<Option<Resolved>> {
        resolve(
            ancestor("codex", 4242, 123456),
            || crate::relay::snapshot::parse(WORKERS),
            "host",
            Some(FIXTURE_BOOT),
            e,
        )
    }

    fn worker(session: &str) -> Agent {
        Agent {
            scope: "worker".into(),
            ..agent("codex", session, "linux", 42, 900, BOOT)
        }
    }

    #[test]
    fn relay_worker_rows_select_by_thread() {
        for thread in [R, W1, W2] {
            let resolved = workers(&env(&[("CODEX_SESSION_ID", R), ("CODEX_THREAD_ID", thread)]))
                .unwrap()
                .unwrap();
            assert_eq!(resolved.session, format!("codex:{thread}"));
            assert_eq!(resolved.root_session, Some(format!("codex:{R}")));
            assert_eq!((resolved.pid, resolved.pid_start), (4242, 123456));
        }
    }

    #[test]
    fn relay_worker_rows_refuse_an_unregistered_thread_without_values() {
        let unknown = "0199f000-0000-7000-8000-0000000000ff";
        let error = workers(&env(&[("CODEX_SESSION_ID", R), ("CODEX_THREAD_ID", unknown)]))
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("no row for this Codex worker thread") && error.contains("TASKS_SESSION"),
            "{error}"
        );
        assert!(!error.contains("0199f000"), "{error}");
    }

    #[test]
    fn relay_worker_rows_without_a_hint_stay_ambiguous() {
        let error = workers(&env(&[])).unwrap_err().to_string();
        assert!(error.contains("several relay agents match pid 4242"), "{error}");
    }

    #[test]
    fn relay_worker_rows_session_only_selects_the_root() {
        // §1's known limit: without a thread, the root's session names the root row.
        let resolved = workers(&env(&[("CODEX_SESSION_ID", R)])).unwrap().unwrap();
        assert_eq!(resolved.session, format!("codex:{R}"));
    }

    #[test]
    fn relay_worker_row_alone_needs_its_own_thread() {
        let rows = || vec![worker("w1")];
        let absent = go(ancestor("codex", 42, 900), rows(), &env(&[]))
            .unwrap_err()
            .to_string();
        assert!(
            absent.contains("Codex worker row")
                && absent.contains("CODEX_THREAD_ID")
                && absent.contains("TASKS_SESSION"),
            "{absent}"
        );
        assert!(!absent.contains("w1"), "{absent}");
        let session_only = go(ancestor("codex", 42, 900), rows(), &env(&[("CODEX_SESSION_ID", "r")]))
            .unwrap_err()
            .to_string();
        assert!(session_only.contains("native actor hint"), "{session_only}");
        // A session export equal to the worker's id passes the ordinary hint check, so only
        // the worker-thread guard refuses it: a session export never selects a worker row.
        let matching_session = go(ancestor("codex", 42, 900), rows(), &env(&[("CODEX_SESSION_ID", "w1")]))
            .unwrap_err()
            .to_string();
        assert!(
            matching_session.contains("Codex worker row")
                && matching_session.contains("CODEX_THREAD_ID"),
            "{matching_session}"
        );
        assert!(!matching_session.contains("w1"), "{matching_session}");
        let own = go(
            ancestor("codex", 42, 900),
            rows(),
            &env(&[("CODEX_SESSION_ID", "r"), ("CODEX_THREAD_ID", "w1")]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(own.session, "codex:w1");
    }

    #[test]
    fn relay_worker_selection_is_codex_only_and_ignores_scope() {
        let claude = go(
            ancestor("claude", 42, 900),
            vec![
                agent("claude-code", "c1", "linux", 42, 900, BOOT),
                agent("claude-code", "c2", "linux", 42, 900, BOOT),
            ],
            &env(&[("CLAUDE_CODE_SESSION_ID", "c1")]),
        )
        .unwrap_err()
        .to_string();
        assert!(claude.contains("several relay agents match"), "{claude}");
        // Two root sessions hosted by one codex process are separated the same way.
        let resolved = go(
            ancestor("codex", 42, 900),
            vec![
                agent("codex", "s1", "linux", 42, 900, BOOT),
                agent("codex", "s2", "linux", 42, 900, BOOT),
            ],
            &env(&[("CODEX_THREAD_ID", "s2")]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(resolved.session, "codex:s2");
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `just test-one relay_worker`
Expected: Task 1's two tests pass and the six new ones fail. `select_by_thread`, `session_only_selects_the_root` and the second half of `codex_only_and_ignores_scope` fail with "several relay agents match". `refuse_an_unregistered_thread` fails on the message. `alone_needs_its_own_thread` fails because `go(..., &env(&[]))` returns `Ok`.

- [ ] **Step 3: Implement the resolver**

In `src/relay/resolve.rs`, add after `no_match`:

```rust
/// The actor hint names none of the qualifying rows. When one of them is this caller's own
/// Codex root, the caller is a worker relay has not registered, and it must not claim as its
/// controller (delegated identity §4, 2026-10-15 amendment). Neither message names a session.
fn hint_mismatch(root: Option<&str>, candidates: &[&crate::relay::snapshot::Agent]) -> Error {
    let detail = if root.is_some_and(|root| candidates.iter().any(|agent| agent.session_id == root))
    {
        "relay has no row for this Codex worker thread (the relay writing the registry predates \
         worker rows, or its Codex hooks are not installed), so the worker cannot claim as its \
         controller"
    } else {
        "the native actor hint does not match any qualifying relay row"
    };
    refuse(detail.into())
}
```

In `resolve`, directly after `let hint = hint_for(harness, get)?;` add:

```rust
    // Both Codex exports, already validated by `hint_for`. The root is the claim's
    // association (§2) and names this caller's own root row in a refusal; only the thread
    // can select a worker row.
    let (root, thread) = if harness == "codex" {
        (
            crate::native::value(get, "CODEX_SESSION_ID").map_err(refuse)?,
            crate::native::value(get, "CODEX_THREAD_ID").map_err(refuse)?,
        )
    } else {
        (None, None)
    };
```

Replace everything from `let agent = match qualifying.as_slice() {` through the end of the `Ok(Some(Resolved { … }))` expression with:

```rust
    let agent = match qualifying.as_slice() {
        [one] => *one,
        [] => {
            return Err(no_match(
                &snapshot.agents,
                &same_process,
                &boundary,
                boot_id,
                hint.as_deref(),
            ));
        }
        // Codex workers share their controller's process (relay spec 2026-10-15), so the
        // actor hint tells their rows apart. Snapshot keys are unique and each id is
        // `codex:<sessionId>`, so at most one row carries the hint.
        many => match (harness, hint.as_deref()) {
            ("codex", Some(hint)) => match many.iter().find(|agent| agent.session_id == hint) {
                Some(agent) => *agent,
                None => return Err(hint_mismatch(root.as_deref(), many)),
            },
            _ => {
                let ids: Vec<&str> = many.iter().map(|agent| agent.id.as_str()).collect();
                return Err(refuse(format!(
                    "several relay agents match pid {}: {}",
                    nearest.pid,
                    ids.join(", ")
                )));
            }
        },
    };

    if let Some(hint) = hint.as_deref()
        && hint != agent.session_id
    {
        return Err(hint_mismatch(root.as_deref(), &[agent]));
    }
    // Process evidence cannot tell a worker from its controller or siblings, which share the
    // handle: only the worker's own thread selects its row, sole or not. Reaching here with
    // a worker row means the caller exports no thread.
    if agent.scope == "worker" && thread.as_deref() != Some(agent.session_id.as_str()) {
        return Err(refuse(
            "the qualifying relay row is a Codex worker row, and only that worker's own \
             CODEX_THREAD_ID selects it; this caller exports none"
                .into(),
        ));
    }

    let process = agent
        .process
        .as_ref()
        .expect("filtered on a present handle");
    Ok(Some(Resolved {
        session: agent.id.clone(),
        root_session: root.map(|id| format!("codex:{id}")),
        pid: process.pid,
        pid_start: process.start,
        boot_id: boot_id.to_string(),
        host: host.to_string(),
    }))
```

- [ ] **Step 4: Run the unit tests to verify they pass**

Run: `just test-one relay_worker`
Expected: PASS (8 tests).
Run: `just test-one relay::resolve`
Expected: PASS. The existing `a_codex_actor_hint_must_match_the_row` and `a_match_is_ambiguous_only_among_fully_qualifying_candidates` are unchanged.

- [ ] **Step 5: Let the shim write a worker row**

In `tests/common/mod.rs`, replace `WRITE_REGISTRY` with:

```rust
pub const WRITE_REGISTRY: &str = r#"
write_registry() {
  start=$(awk '{print $22}' "/proc/$$/stat")
  boot=$(cat /proc/sys/kernel/random/boot_id)
  host=$(cat /proc/sys/kernel/hostname)
  root_field=""
  if [ -n "${ROOT_ID:-}" ]; then root_field=",\"root\":\"$ROOT_ID\""; fi
  mkdir -p "$RELAY_STATE_DIR"
  chmod 700 "$RELAY_STATE_DIR"
  cat > "$RELAY_STATE_DIR/agents.json" <<EOF
{"schema":2,"generation":"11111111-2222-4333-8444-555555555555","revision":1,
 "agents":{"$AGENT_ID":{"id":"$AGENT_ID","harness":"$HARNESS","sessionId":"$SESSION",
  "scope":"${SCOPE:-session}"$root_field,"cwd":"/w","repoRoot":null,"remote":null,"projectKey":"k",
  "project":"p","state":"idle","updatedAt":1,
  "process":{"platform":"linux","host":"$host","bootId":"$boot","pid":$$,"start":"$start"}}}}
EOF
  chmod 600 "$RELAY_STATE_DIR/agents.json"
}
"#;
```

Keep the doc comment above it, and add one sentence to it: "`SCOPE` (default `session`) and `ROOT_ID` (adds `root`) let a test write a Codex worker row."

- [ ] **Step 6: Write the failing CLI control, and update the old wording's assertions**

In `tests/cli.rs`, in `delegated_relay_ownership`, replace:

```rust
    assert!(
        root_error.contains("worker registration")
            && root_error.contains("relay-f56e30")
            && root_error.contains("TASKS_SESSION"),
        "{root_error}"
    );
```

with:

```rust
    assert!(
        root_error.contains("no row for this Codex worker thread")
            && root_error.contains("TASKS_SESSION"),
        "{root_error}"
    );
```

and replace:

```rust
    assert!(
        !unrelated.contains("relay-f56e30")
```

with:

```rust
    assert!(
        !unrelated.contains("worker thread")
```

Add after `delegated_relay_ownership`:

```rust
/// Spec round 1 finding: relay creates a worker's row even when its root row is absent, so a
/// lone worker row can qualify on the shared process. A command without that worker's thread
/// (a controller or sibling missing its exports) must not resolve as the worker, and its
/// refusals must leave the claim store and every record byte-identical.
#[test]
fn delegated_relay_sole_worker_row_needs_its_thread() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let free = id_of(env.json(&dir, &["add", "Free"]));
    let held = id_of(env.json(&dir, &["add", "Held by the worker"]));
    let state = relay_on(&env);
    let script = format!(
        r#"set -e
{}
SCOPE=worker ROOT_ID=codex:private-root
export SCOPE ROOT_ID
write_registry
CODEX_SESSION_ID=private-root CODEX_THREAD_ID=private-worker "$TASKS_BIN" start {held}
"$TASKS_BIN" show {held} > "$HOME/held.json"
refuse() {{
    cp tasks/{free}.md "$HOME/before-free.md"
    cp tasks/{held}.md "$HOME/before-held.md"
    cp "$HOME/.local/state/tasks/claims/sci.toml" "$HOME/before.toml"
    if "$TASKS_BIN" "$@" > "$HOME/refuse.out" 2> "$HOME/refuse.err"; then exit 93; fi
    cmp tasks/{free}.md "$HOME/before-free.md"
    cmp tasks/{held}.md "$HOME/before-held.md"
    cmp "$HOME/.local/state/tasks/claims/sci.toml" "$HOME/before.toml"
}}
for mode in absent session-only matching-session; do
    unset CODEX_SESSION_ID CODEX_THREAD_ID
    case "$mode" in
        absent) : ;;
        session-only) export CODEX_SESSION_ID=private-root ;;
        matching-session) export CODEX_SESSION_ID=private-worker ;;
    esac
    refuse start {free}
    cp "$HOME/refuse.err" "$HOME/start-$mode.err"
    refuse park {held} refused
    refuse done {held} refused
done
"#,
        shim_env(&state, "codex", "private-worker")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert!(
        out.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let held: serde_json::Value =
        serde_json::from_str(&env.read(env.home.path(), "held.json")).unwrap();
    assert_eq!(held["claim"]["session"], "codex:private-worker");
    let absent = env.read(env.home.path(), "start-absent.err");
    assert!(
        absent.contains("Codex worker row")
            && absent.contains("CODEX_THREAD_ID")
            && absent.contains("TASKS_SESSION"),
        "{absent}"
    );
    let session_only = env.read(env.home.path(), "start-session-only.err");
    assert!(session_only.contains("native actor hint"), "{session_only}");
    // The worker's own id as a session export passes the ordinary hint check; only the
    // worker-thread guard refuses it.
    let matching_session = env.read(env.home.path(), "start-matching-session.err");
    assert!(
        matching_session.contains("Codex worker row")
            && matching_session.contains("CODEX_THREAD_ID"),
        "{matching_session}"
    );
    for error in [&absent, &session_only, &matching_session] {
        assert!(
            !error.contains("private-worker") && !error.contains("private-root"),
            "{error}"
        );
    }
}
```

- [ ] **Step 7: Run the CLI tests**

Run: `just test-one --test cli delegated_relay`
Expected: PASS, with `delegated_relay_ownership` under its new wording and `delegated_relay_sole_worker_row_needs_its_thread`. To confirm the control has teeth, make two temporary mutations to the `agent.scope == "worker"` guard from Step 3, one at a time, and run both `just test-one relay_worker_row_alone` and this command after each:
1. Delete the guard. The CLI control must fail with exit 93 from `refuse start` in the `absent` mode, and the unit test must fail at `absent`.
2. Restore the guard but compare `hint.as_deref()` instead of `thread.as_deref()`. The CLI control must fail with exit 93 from `refuse start` in the `matching-session` mode, and the unit test must fail at `matching_session`.

Restore the guard exactly as in Step 3 before continuing.

- [ ] **Step 8: Update the docs**

`README.md`: replace

```markdown
association separately. A worker presented only with its registered root row
refuses acquisition: relay worker registration remains unavailable (`relay-f56e30`).
It cannot claim as its controller; other mismatches use a value-free diagnosis.
No registry schema or producer behavior changes here.
```

with

```markdown
association separately. Relay publishes a row per Codex worker (`scope: "worker"`,
naming its `root`) on its controller's process. Among the rows on one process, the hint
selects the caller's own, and a worker row is selected only by that worker's
`CODEX_THREAD_ID`. A worker relay has not registered is refused rather than given its
controller's identity; other mismatches use a value-free diagnosis.
```

`skills/tasks/SKILL.md`: replace

```markdown
new owner's root. Older claim writers can discard it. Relay worker acquisition
still refuses a root-only row (`relay-f56e30`); Codex proof-only continuation needs
```

with

```markdown
new owner's root. Older claim writers can discard it. Relay worker acquisition
uses the worker's own row, selected only by its `CODEX_THREAD_ID`, and refuses a
worker relay has not registered; Codex proof-only continuation needs
```

`docs/specs/2026-09-22-relay-ancestry-identity-design.md`: replace

```markdown
root-only row refuses a worker without adopting its controller identity; its
value-free error names unavailable worker registration (`relay-f56e30`) and
`TASKS_SESSION` recovery. An unrelated row mismatch stays generic. Existing
cardinality, process, host, boot, platform and harness checks remain unchanged.
A selected Codex actor row records valid native `CODEX_SESSION_ID` separately as
optional `root_session`; no registry schema or producer change is implied.
```

with

```markdown
root-only row refuses a worker without adopting its controller identity; its
value-free error says relay has no row for that worker thread and gives
`TASKS_SESSION` recovery. An unrelated row mismatch stays generic. Process, host,
boot, platform and harness checks remain unchanged. Updated 2026-10-15: relay's
Codex worker rows share their controller's handle, so the hint selects among several
qualifying Codex rows, and a worker row needs its own `CODEX_THREAD_ID` (delegated
identity design, "Relay worker rows"). A selected Codex actor row records valid
native `CODEX_SESSION_ID` separately as optional `root_session`.
```

- [ ] **Step 9: Fast suite, check, commit**

Run: `just test-fast` (expected: all pass), then `just check` (expected: clean).

```bash
git add src/relay/resolve.rs tests/common/mod.rs tests/cli.rs README.md skills/tasks/SKILL.md docs/specs/2026-09-22-relay-ancestry-identity-design.md
git commit -m "feat(relay): select a Codex worker row by its own thread"
```

---

## Closeout (controller, after the final review)

- Mark the amendment as landed in the delegated-identity spec's header line ("Amended 2026-10-15 … landed on main").
- After the merge, run `cargo install --path .` on this host, as AGENTS.md requires after CLI changes.
- Note on relay-635fdc that tasks-af3aac has landed and been installed, and that its Task 6 can run. Also note that tasks vendors the fixture from plan revision `74ce0d8` and will compare bytes when relay's Task 1 commit lands.
- `tasks done tasks-af3aac` on the branch before the merge.
