# Lifecycle-note provenance implementation plan

> **For agentic workers:** Use `superpowers:executing-plans` to implement this plan task-by-task, with independent diff review before completion.

Status: approved with the user's three review adjustments, 2026-09-17. Task 1 is merged and installed on titan; independent review and all 499 tests passed. Task 2 is parked pending Europa's reader rollout.

**Goal:** Persist the approved harness-session key on task lifecycle notes without changing claim identity or liveness.

**Architecture:** Extend the existing note representation with optional provenance. Resolve it from command environment metadata, then stamp lifecycle notes at the existing transition and park boundaries. Reuse task-file persistence, locking, JSON output and warnings.

**Tech stack:** Rust; existing serde/serde_json; existing unit and CLI test infrastructure. No new dependency, store, registry reader or hook.

**Spec:** ops `docs/specs/2026-09-17-session-provenance-design.md`, approved in this conversation, with installed-harness evidence in ops `docs/reports/2026-09-17-session-provenance-probe.md`. These are sibling-project references, not copies of the contract. Existing claim semantics: `docs/specs/2026-09-05-work-claims-design.md` and current `src/claims.rs`.

Workspace: `.worktrees/lifecycle-provenance`, branch `feat/lifecycle-provenance`.
The narrower implementation is part of tasks-c9199a; its broader opt-in relay-ancestry design remains separate and is not a prerequisite for obs's note consumer.

Task graph: tasks-d51eda (Task 1) → tasks-b07adc (Task 2) → obs-09cb62.
tasks-8921f4 holds the ancestry design independently; tasks-c9199a remains the parent of both tracks.

Confirmed reader hosts: titan and europa (Europa). Europa's reader installation and
isolated stamped-fixture check require user confirmation before the writer proceeds.

Rollout evidence: titan installed reader-only commit `57311fc` with `cargo install
--path .` on 2026-09-17. The installed binary passed `show`, a title-only `edit`,
and another `show` against an isolated stamped fixture, preserving the complete
note (including both provenance fields); config and state were isolated from the
real registry. Europa is pending. No production lifecycle stamp writer is enabled.

## Global constraints

- Fixed serialized fields: `harness_session` and `harness_session_source`, both optional strings, omitted together when unknown.
- Keys: `claude-code:<native-id>` / `codex:<native-id>`. IDs are opaque, nonempty strings, not necessarily UUIDs.
- Source names: `CLAUDE_CODE_SESSION_ID`, `CODEX_SESSION_ID`, or `CODEX_THREAD_ID`. Agreeing Codex variables select `CODEX_SESSION_ID`; a matching claim override never replaces the native source name.
- `claims::identity_from`, PID/start/boot capture, Live/Stale, TTL, refresh, acquisition/release ordering and mutation locking retain their behavior. In particular, Codex provenance does not change its `sid:<pid>` claim.
- Missing/conflicting provenance never prevents an otherwise valid lifecycle mutation. Invalid/conflicting inputs produce existing CLI warnings; never dump environment values into a diagnostic.
- Do not export environment variables, infer from PID/cwd/model/transcripts, rewrite historical notes, or install/change hooks.
- Claude subagents may share the parent's session key. This is provenance, not authorization or child identity.
- obs-1ad448 owns unknown-provenance-rate diagnostics; this producer does not implement them.
- Use `just test-fast [filter]` and `just gate`, never bare `cargo test`. No setup recipe is defined in this checkout.

## Concrete representation and resolution

Retain each existing note bullet; add one optional indented JSON continuation:

```markdown
- 2026-09-17T20:00:00Z (worker): started
  provenance: {"harness_session":"codex:example","harness_session_source":"CODEX_SESSION_ID"}
```

The continuation belongs only to the immediately preceding note. Its object has exactly the two required string fields. Reject an orphan, a second continuation, malformed JSON, duplicate/unknown keys, missing/null/non-string fields, an unsupported harness/source pairing, or an empty native suffix. Plain notes retain their byte representation; user note text containing `provenance:` is just text. JSON escaping preserves opaque IDs containing punctuation or whitespace without inventing delimiter rules. Reject control characters in metadata; do not trim or normalize IDs.

Internally use `Option<HarnessProvenance>` with serde flattening on `Note`, so the public note object gains the two fields directly, not a nested `provenance` object. `None` emits neither field. The pair type is the persisted contract, not a generic metadata bag.

| Available input | Provenance result |
| --- | --- |
| Claude native ID alone | `claude-code:<id>`, Claude source |
| One Codex variable | `codex:<id>`, that variable's source |
| Both Codex variables equal | `codex:<id>`, `CODEX_SESSION_ID` |
| Both Codex variables differ, or both harnesses have native IDs | Unknown plus warning naming the conflicting variable names |
| Supported qualified override differs from native key | Unknown plus warning |
| Arbitrary claim override plus valid native input | Native provenance; the independent claim override remains untouched |
| No native input, with or without a claim override | Unknown; neither field emitted |
| Non-Unicode or control-containing relevant input | Unknown plus warning naming the input; no partial stamp |

Unset and empty variables count as absent. Native input conflicts are checked before any override; an override cannot rescue an ambiguous native identity. `TASKS_SESSION` is checked only for a conflict with a resolved native key, never used as a source. `TASKS_SESSION_PID` is not read by this resolver.

## File map

- `src/model.rs`: the paired provenance type and optional flattened `Note` member.
- `src/provenance.rs` (new), `src/main.rs`: pure allowlisted environment resolution plus its tests; no process or registry operations.
- `src/format.rs`: continuation read/write and pair validation, with format tests.
- `src/commands/mod.rs`: lifecycle append helper; start/resume/close evidence in shared `transition`.
- `src/commands/park.rs`, `src/commands/status.rs`: stamp existing park and close-message notes.
- `tests/common/mod.rs`: remove both Codex variables in both command constructors.
- `tests/cli.rs`: lifecycle/claim regression scenarios using existing `TestEnv`.
- `README.md`, `skills/tasks/SKILL.md`: document the note contract and separation from claim overrides.

### Task 1: Persist and resolve optional note provenance

Task: tasks-d51eda.

**Interfaces:**

```rust
// model.rs
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarnessProvenance {
    pub harness_session: String,
    pub harness_session_source: String,
}

// New member on Note (existing at/by/text stay unchanged):
#[serde(flatten, skip_serializing_if = "Option::is_none")]
pub provenance: Option<HarnessProvenance>,

// provenance.rs: Err is a diagnostic, not a failed task mutation.
pub fn resolve_from(
    get: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> std::result::Result<Option<HarnessProvenance>, String>;
```

- [x] Add failing table-driven resolver tests in `src/provenance.rs` for every row above. Use an injected closure, not mutation of the test process environment. Include a non-UUID ID and a Claude child sharing its parent's value.

```rust
let resolved = resolve_from(|name| match name {
    "CODEX_SESSION_ID" | "CODEX_THREAD_ID" => Some("session:a".into()),
    _ => None,
}).unwrap().unwrap();
assert_eq!(resolved.harness_session, "codex:session:a");
assert_eq!(resolved.harness_session_source, "CODEX_SESSION_ID");
```

- [x] Add format tests alongside the existing `FULL` and `MINIMAL` roundtrips. Attach the example continuation to a valid note fixture, parse, serialize and parse again; assert both flattened JSON fields and unchanged text/time/owner. Exercise all malformed-continuation cases above and an opaque ID containing quote/backslash/colon. Assert an unstamped note omits both fields and old `FULL` roundtrips byte-for-byte.

```rust
let note = &parsed.notes[0];
let json = serde_json::to_value(note).unwrap();
assert_eq!(json["harness_session"], "codex:example");
assert!(json.get("provenance").is_none());
assert_eq!(parse_task(&serialize_task(&parsed), "roundtrip").unwrap(), parsed);
```

- [x] Run `just test-fast provenance` and the new format checks; observe failure before implementing the missing behavior. A missing member/module compile failure is the initial red check.
- [x] Add the paired type and initialize `provenance: None` at the existing two `Note` constructors (`parse_note_line`, `append_note`). Register the resolver module in `main.rs`; implement exactly the resolution table using the four allowlisted variable names and owned `OsString` conversion. Do not change the existing `provenance_var` used for task-level model/agent stamps: its failure semantics differ.
- [x] Extend `split_body_notes`: recognize the exact `  provenance: ` prefix only inside Notes, deserialize into the strict pair type, validate it, and attach it to the preceding note once. Other non-bullet lines still fail. Include pair validation in `validate_task`, so construction and parsing obey the same rules. Valid native sources must match the harness prefix.
- [x] Extend the existing note serialization loop, using the existing JSON library:

```rust
if let Some(provenance) = &n.provenance {
    out.push_str("  provenance: ");
    out.push_str(&serde_json::to_string(provenance)
        .expect("a pair of strings serializes as JSON"));
    out.push('\n');
}
```

- [x] Run `just test-fast provenance`, `just test-fast format::tests`, and `just gate`. Obtain independent diff review; commit as `feat: persist optional harness provenance on notes`. The reviewer found positional-array acceptance in Serde; the regression failed before the object guard and passed after it. Final full gate: 188 unit tests and 311 CLI tests passed; review cleared.
- [x] Merge Task 1 separately and `cargo install --path .` on titan. This task adds format support, not a production stamp writer. Rollout evidence is recorded above; Europa remains Task 2's gate.

### Task 2: Stamp lifecycle notes without changing claims

Task: tasks-b07adc; depends on tasks-d51eda.

**Consumes:** Task 1's paired type, format support and `resolve_from`.
**Produces:** Durable start/resume, park and close associations in `tasks show` and task files, surviving removal of the shared claim/park.

- [ ] **Cross-host reader gate:** confirm Task 1 is merged and `cargo install --path .` has installed its reader on every host reading the Dropbox-synced task files. Record the host inventory, installed revision and a read check using a stamped fixture in isolated storage. Get the user's confirmation for hosts not inspected here; missing evidence keeps this task parked. Do not merge or install Task 2, or write any stamped note into synced task files, before every host passes.
- [ ] First isolate both `TestEnv::cmd` and `TestEnv::raw` from `CODEX_SESSION_ID` and `CODEX_THREAD_ID`, alongside their existing Claude/override removals. Add failing CLI tests using explicit test-only native values:

```rust
let mut env = TestEnv::new();
let dir = env.init("sci");
let id = id_of(env.json(&dir, &["add", "Provenance", "--status", "todo"]));
env.cmd(&dir)
    .env("CODEX_SESSION_ID", "codex-a")
    .env("CODEX_THREAD_ID", "codex-a")
    .args(["start", &id])
    .assert().success();
let shown = env.json(&dir, &["show", &id]);
let note = shown["task"]["notes"].as_array().unwrap().last().unwrap();
assert_eq!(note["text"], "started");
assert_eq!(note["harness_session"], "codex:codex-a");
assert_eq!(note["harness_session_source"], "CODEX_SESSION_ID");
assert!(shown["claim"]["session"].as_str().unwrap().starts_with("sid:"));
```

Extend this fixture through park, resumed start and done in separate CLI processes. Capture the notes before release; assert their timestamps and pairs persist after both shared entries disappear. Test done with/without a message, drop, recurring completion and cleanup retry. Repeat the identity assertion with Claude's raw claim key and a live test-process `CLAUDE_PID`.

- [ ] Run the focused checks before wiring stamp creation; verify they fail for absent lifecycle notes/fields.
- [ ] Add one helper in `commands/mod.rs`:

```rust
pub fn append_lifecycle_note(
    ctx: &mut Ctx, task: &mut Task, by: &str, text: &str,
) -> Result<()> {
    append_note(task, by, text)?;
    match crate::provenance::resolve_from(std::env::var_os) {
        Ok(provenance) => task.notes.last_mut().expect("just appended").provenance = provenance,
        Err(diagnostic) => ctx.warnings.push(diagnostic),
    }
    Ok(())
}
```

Use a closure `|name| std::env::var_os(name)` if required by generic function lifetime inference. Plain `tasks note`, feedback, shelf notes and takeover commentary keep their current behavior; this slice adds provenance only at the named lifecycle boundaries.

- [ ] At the shared `transition` boundary, after existing guards and before persistence, append `started` on first entry into doing, or `resumed` when `task.started` was already present. Capture that boolean before modifying `started`. Every successful explicit `start` records activity, including a same-owner refresh; it does not reset the task's first-start timestamp or claim's acquisition timestamp. Same-status `edit` still bypasses transition and adds nothing.
- [ ] In `transition`, on a genuine change into done/dropped, append a stamped `done`/`dropped` note even without a user message. For a recurring completion, stamp the existing `completed; next due ...` note instead of adding a second generated close note. The `closing`/`completing` predicates already distinguish a real transition from cleanup retry; do not manufacture a second lifecycle transition on retry. Flag and interactive status edits already call this function and receive the same coverage.
- [ ] In `status::close`, stamp the existing caller-supplied message when it is appended; preserve its text, existing `!ctx.recovered` guard and ordinary retry behavior. This can leave a generated transition note plus a message note: only the generated transition is a new lifecycle boundary, not every stamped note. Do not deduplicate arbitrary user messages. In `park::run`, replace its existing `append_note` call with `append_lifecycle_note`, preserving the park vocabulary and write order.
- [ ] Add/extend checks for both flag and editor status paths, same-status edit preserving a park, denied foreign claims producing no note, acquisition rollback, and release cleanup retry. Reuse existing failure-injection fixtures in `tests/cli.rs`; do not build a second harness. Adjust old note-count/index assertions only where the new documented start/close note is the reason.
- [ ] Prove provenance independence with fixtures for Claude, Codex, absent native data, conflicting native data, and explicit claim overrides with/without a PID. Compare session/PID/start/boot and liveness outcomes against the same claim inputs without Codex provenance. Existing `claims.rs` unit tests continue to cover TTL boundaries and confirmed death/live behavior unchanged. Exercise note heartbeat and guarded release under both known and unknown provenance. Conflicts must return successful lifecycle output with a warning and an unstamped note; pretty output must carry that warning too.
- [ ] Update README and the shipped tasks skill: native provenance is automatic and independent of claims; do not set `TASKS_SESSION` merely to improve obs. Document the two optional JSON fields, metadata continuation, empty/conflicting behavior, and override-conflict checking (the source always names a native variable). State the canonical lifecycle text contract in README: exact `started`, `resumed`, `done`, `dropped`; `parked (waiting on …): <next step>` using the existing park vocabulary; and `completed; next due <YYYY-MM-DD>` for recurring completion. obs-09cb62 derives transition kinds from these generated texts, not from the presence of provenance fields. User-message notes are stamped but are not lifecycle markers. Document reader-first deployment: every host must have Task 1 before Task 2 merges, because an older binary rejects the whole stamped task file.
- [ ] Run `just test-fast`, `just gate`, `tasks check` and `git diff --check`. Obtain independent diff review, fix findings, rerun affected checks. Commit the completed task record with the implementation as `feat: stamp lifecycle notes with native harness provenance`.
- [ ] Integrate the reviewed branch, then `cargo install --path .` from the integrated checkout per AGENTS.md. Verify the installed reader against a new isolated test task store with an isolated registry, never the real registry. Report note-stamping delivery to obs-09cb62; do not close the separate relay-ancestry deliverable or implement obs diagnostics here.

## Coverage and handoff

| Approved requirement | Coverage |
| --- | --- |
| Fixed optional pair, source and key semantics | Task 1 resolver/format checks |
| Unknown/conflict diagnostics, arbitrary overrides | Task 1 matrix; Task 2 CLI warnings |
| Durable start/resume/park/close evidence | Task 2 shared transition, park and release scenarios |
| No claim identity/liveness changes | Task 2 regression fixtures plus existing claim suite |
| Existing notes unmodified | Task 1 byte roundtrips; Task 2 persistence assertions |
| Native capability evidence | Completed ops-79f409 report, not repeated here |
| Unknown-rate diagnostics | Remains obs-1ad448, outside this plan |

Execute the approved plan inline in task order, with independent code review before each delivery. Deliver Task 1 alone first, then stop at Task 2's cross-host reader gate until the user supplies any outstanding host installation evidence. Task 1 being merged or installed on titan does not satisfy that gate. No remote installation is assumed authorized by this plan. If a reviewer is unavailable, park explicitly with the owner and next action; do not imply that review is running.
