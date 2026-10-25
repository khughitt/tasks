# Creation and occurrence provenance Implementation Plan

**Status:** approved (2026-10-25, user, after plan review round 1 by codex/gpt-6.1-sol). Execution: subagent-driven, with sequential implementers and a fresh reviewer per task.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every new task records the session that filed it and, optionally, whose ask it is (`user`, `agent`, `mixed`); each feedback occurrence is stamped separately; the skills say how to follow a record back to its originating ask.

**Architecture:** A generated `created` note, stamped like lifecycle notes, is written by `add` and `feedback` creation; feedback recurrence notes are stamped too. The note continuation JSON gains optional occurrence keys (`root_session`, `origin`, `origin_session`), and the record gains a correctable frontmatter `origin`. Retrieval is guidance (scope and tasks skills), not code.

**Tech Stack:** Rust 2024 crate, clap, serde/serde_json; tests in `tests/cli.rs` against the built binary via `tests/common::TestEnv`.

**Spec:** `docs/specs/2026-10-25-creation-occurrence-provenance-design.md` (revision 4, `00f58ea`). Section references below (§n) are to it.

## Global Constraints

- Origin values exactly `user`, `agent`, `mixed`; absent means unknown. Filters also take `none`.
- The tool never derives origin from `agent`, `parent`, `source`, tags, session presence or commit time.
- Creation note text exactly `created`; its `at` equals the record's `created`; author exactly `add` (from `add`) or `feedback` (from `feedback`).
- Feedback recurrence note texts unchanged: `feedback from <prefix>: <summary>`, `detail from <prefix>: <body>`.
- Occurrence keys only on occurrence notes (`created`, `feedback from`); a `detail` note carries the native pair alone.
- Continuation JSON key order: `harness_session`, `harness_session_source`, `root_session`, `origin`, `origin_session`; the line keeps the name `provenance:`; the pair stays all-or-nothing.
- `root_session` is a `codex:` key, only with a pair whose source is `CODEX_THREAD_ID`.
- A `reused` add writes nothing and has no `at`.
- Conflicting or invalid identity warns naming variables, never values; creation is never blocked by identity.
- `TASKS_SESSION` is never a provenance source.
- New CLI rows land in ops's `cli.toml` first; adopt with `python3 "$(tasks --pretty root ops-88a0fa)/bin/vendored" adopt cli.toml`; never edit `tools/cli.toml` by hand.
- Tests run through the front door only: `just test-one …`, `just test-fast` before each commit; never `cargo test`.
- Each test subprocess gets an environment built key by key (`TestEnv` already does this; harness variables are added per command with `.env(...)`).
- No backfill of existing records. JSON changes are additive.

## Review Focus

1. `--origin` or `--origin-session` under unknown or conflicting native identity: both are still recorded on the creation note without the pair, and a conflict warns without values. Test: `origin_survives_unknown_identity` (Task 4).
2. `feedback` filed from outside every project (`from:unregistered`) and `add --project X` from another checkout: both still write a creation note in the target, with the fixed author. Tests: `feedback_outside_projects_writes_creation_note`, `add_into_other_project_writes_creation_note` (Task 3).
3. `tasks move` of a record carrying a creation note and occurrence keys: notes, keys and `created` survive unchanged. Test: `move_preserves_creation_note_and_occurrence_keys` in `tests/move.rs` (Task 3).
4. `tasks edit --editor` on a record with occurrence keys: an unchanged save round-trips byte for byte, and an editor change to a continuation line is refused like any note edit. Test: `editor_preserves_occurrence_keys` (Task 4).
5. Cross-project reads (`list --all-projects --origin none`) see records from projects without any origin data and treat them as unknown. Test: `origin_filter_spans_projects` (Task 4).

---

### Task 1: Declare the new CLI rows in ops

Lands outside this repository; it gates Task 4. Follow ops's own guide (its AGENTS.md) for worktree, gate and integration; file the work as an ops task with `tasks add --project ops "Declare tasks origin flags in cli.toml" --process direct` and run it under that task.

**Files (in ops):**
- Modify: `cli.toml` — rows under `path = ["add"]`, `["feedback"]`, `["edit"]`, `["list"]`, `["search"]`, `["ready"]`.

**Rows:**
- add: `{ names = ["--origin"], value = "enum", values = ["user", "agent", "mixed"] }`, `{ names = ["--origin-session"], value = "string" }`
- feedback: the same two rows.
- edit: `{ names = ["--origin"], value = "enum", values = ["user", "agent", "mixed"] }`, `{ names = ["--no-origin"], value = "none" }`
- list, search, ready: `{ names = ["--origin"], value = "enum", values = ["user", "agent", "mixed", "none"], repeatable = true }`

- [ ] **Step 1:** Add the rows beside the existing `--process` rows of each command; run ops's gate; expected: green.
- [ ] **Step 2:** Commit in ops as `feat(cli): declare tasks origin and origin-session options`, integrate to ops main per its guide; record the ops commit sha in a note on this plan's goal task.

---

### Task 2: Origin type and occurrence keys in the record format

**Files:**
- Modify: `src/model.rs` (new `Origin`, `NoteOccurrence`; `Note` and `Task` fields)
- Modify: `src/format.rs` (key list, parse, serialize, validate; unit tests in its `mod tests`)
- Modify: `src/provenance.rs` (`validate_session_key`)

**Interfaces:**
- Produces:
  - `pub enum Origin { User, Agent, Mixed }` with `ALL`, `parse(&str) -> Result<Origin>` (error `unknown origin {s:?}; expected one of user, agent, mixed`), `as_str()`, `combine(self, other: Origin) -> Origin` (equal → self; either `Mixed` → `Mixed`; different → `Mixed`). Serde lowercase.
  - `pub struct NoteOccurrence { pub root_session: Option<String>, pub origin: Option<Origin>, pub origin_session: Option<String> }`, `Default`, `is_empty()`, each field `skip_serializing_if = "Option::is_none"`.
  - `Note` gains `#[serde(flatten)] pub occurrence: NoteOccurrence` after `provenance`.
  - `Task` gains `pub origin: Option<Origin>` after `agent` (serialized after `agent`, skipped when `None`); `add::blank` sets `None`.
  - `pub fn provenance::validate_session_key(key: &str) -> Result<(), String>`: `claude-code:` or `codex:` prefix, nonempty, control-free remainder.

- [ ] **Step 1: Write failing unit tests in `src/format.rs`**

```rust
#[test]
fn occurrence_keys_roundtrip_in_fixed_order() {
    let line = r#"  provenance: {"harness_session":"codex:w","harness_session_source":"CODEX_THREAD_ID","root_session":"codex:r","origin":"mixed","origin_session":"claude-code:old"}"#;
    let text = format!("{FULL}- 2026-10-25T12:00:00Z (add): created\n{line}\n");
    let task = parse_task(&text, "t").unwrap();
    assert_eq!(task.notes[0].occurrence.origin, Some(Origin::Mixed));
    assert_eq!(serialize_task(&task), text); // byte for byte
}

#[test]
fn occurrence_keys_may_stand_without_the_pair() {
    // origin supplied under unknown identity
    let text = format!("{FULL}- 2026-10-25T12:00:00Z (add): created\n  provenance: {{\"origin\":\"user\"}}\n");
    assert!(parse_task(&text, "t").unwrap().notes[0].provenance.is_none());
}

#[test]
fn occurrence_keys_reject_bad_forms() {
    for json in [
        r#"{"origin":"maybe"}"#,
        r#"{"origin_session":"slack:abc"}"#,
        r#"{"origin_session":"codex:"}"#,
        r#"{"root_session":"codex:r"}"#,                                   // no pair
        r#"{"harness_session":"claude-code:a","harness_session_source":"CLAUDE_CODE_SESSION_ID","root_session":"codex:r"}"#,
        r#"{"harness_session":"codex:w","harness_session_source":"CODEX_THREAD_ID","root_session":"claude-code:r"}"#,
        r#"{"harness_session":"codex:w"}"#,                               // half a pair
        r#"{"surprise":1}"#,
    ] {
        let text = format!("{FULL}- 2026-10-25T12:00:00Z (add): created\n  provenance: {json}\n");
        assert!(parse_task(&text, "t").is_err(), "{json}");
    }
}

#[test]
fn a_second_continuation_is_refused_even_without_a_pair() {
    let text = format!(
        "{FULL}- 2026-10-25T12:00:00Z (add): created\n  provenance: {{\"origin\":\"user\"}}\n  provenance: {{\"origin\":\"agent\"}}\n"
    );
    let error = parse_task(&text, "t").unwrap_err().to_string();
    assert!(error.contains("duplicate provenance line"), "{error}");
}

#[test]
fn origin_frontmatter_roundtrips_after_agent_and_rejects_unknown_values() {
    // `origin: user` placed directly after `agent:` survives parse+serialize unchanged;
    // `origin: someone` fails to parse with "unknown origin".
}
```

`FULL` and `serialize_task` stand for the module's existing fixture and serializer; use their actual names.

- [ ] **Step 2: Run `just test-one occurrence_keys`, `just test-one origin_frontmatter` and `just test-one second_continuation`.** Expected: FAIL to compile (no `Origin`).
- [ ] **Step 3: Implement.** Parse the continuation into one private struct holding all five keys (`deny_unknown_fields`), then split it into `provenance` (both pair keys or neither, else `invalid note provenance`) and `occurrence`; validate `origin_session`/`root_session` with `validate_session_key` and the `root_session` source rule. The duplicate guard tracks whether the current note has already had a continuation line, independent of which keys it held: today's `note.provenance.is_some()` check misses an origin-only first line, so replace it with that per-note flag (`duplicate provenance line` stays the error). Serialize the same struct so key order follows the Global Constraint. Add `"origin"` after `"agent"` in the key list, parse it with `Origin::parse`, and serialize it after `agent`. `validate_task` re-checks the note keys, so `check` reports invalid values.
- [ ] **Step 4: Run `just test-one format::` and `just test-fast`.** Expected: PASS, with no other test changed.
- [ ] **Step 5: Commit** `feat(format): origin field and note occurrence keys`.

---

### Task 3: Creation notes, stamped recurrences, and `at` in write output

**Files:**
- Modify: `src/provenance.rs` (`resolve_root_from`)
- Modify: `src/commands/mod.rs` (`Stamp`, `occurrence_stamp`, `append_occurrence_note`)
- Modify: `src/commands/add.rs`, `src/commands/feedback.rs`
- Modify: `src/output.rs` (`AddOut.at`, `FeedbackOut.at`)
- Modify: `tests/cli.rs`, `tests/move.rs`, `tests/review.rs`, `tests/attachments.rs` (existing positional note assertions)
- Modify: `README.md` (Lifecycle notes: add `created` to the generated texts; creation provenance paragraph: the creation note, recurrence stamps, `root_session`, `at`, and the reader's identification rule: the first note, text `created`, `at` equal to `created`)

**Interfaces:**
- Consumes: `NoteOccurrence`, `Origin` (Task 2).
- Produces:
  - `pub fn provenance::resolve_root_from(get: impl Fn(&str) -> Option<OsString>) -> Option<String>`: `Some("codex:<CODEX_SESSION_ID>")` only when `resolve_from` selects `CODEX_THREAD_ID` and a valid `CODEX_SESSION_ID` differs from it.
  - `pub struct Stamp { pub provenance: Option<HarnessProvenance>, pub root_session: Option<String> }`
  - `pub fn occurrence_stamp(warnings: &mut Vec<String>) -> Stamp`: resolves once per command from the process environment; on a resolver error it pushes the diagnostic and returns an empty stamp.
  - `pub fn append_occurrence_note(task: &mut Task, at: &str, by: &str, text: &str, stamp: &Stamp, origin: Option<Origin>, origin_session: Option<&str>) -> Result<()>`: sets the pair and `root_session` from `stamp`, and `origin`/`origin_session` from the arguments.
  - `AddOut { …, #[serde(skip_serializing_if = "Option::is_none")] pub at: Option<String> }`; the same field on `FeedbackOut`.
  - `add::run` and `feedback::create` call `append_occurrence_note(task, &task.created, "add" | "feedback", "created", …)` before `create`. Task 4 fills the origin arguments; this task passes `None`.

- [ ] **Step 1: Write failing tests in `tests/cli.rs`**

```rust
#[test]
fn add_writes_one_stamped_creation_note() {
    // cases: Claude id; Codex thread; Codex worker (CODEX_THREAD_ID=w, CODEX_SESSION_ID=r);
    // none; conflict (CLAUDE_CODE_SESSION_ID + CODEX_THREAD_ID)
    // for each: out = env.json(add ...), show = env.json(show id)
    // assert show.notes.len() == 1, notes[0].text == "created", notes[0].by == "add",
    //   notes[0].at == show.created == out.at
    // Claude:   harness_session "claude-code:native-a", source CLAUDE_CODE_SESSION_ID, no root_session
    // worker:   harness_session "codex:w", source CODEX_THREAD_ID, root_session "codex:r"
    // none:     no harness_session, no root_session, warnings []
    // conflict: no harness_session; warnings name both variables, contain neither value
}

#[test]
fn reused_add_writes_nothing_and_has_no_at() {
    // add --source S under session A, then the same add under session B:
    // second out.action == "reused", out.at absent; file bytes unchanged; notes.len() == 1,
    // notes[0].harness_session == "claude-code:A"
}

#[test]
fn feedback_creation_and_recurrence_are_stamped_per_occurrence() {
    // accept_feedback(target); file under session A → created; notes[0] "created", by "feedback",
    //   stamped A, out.at == notes[0].at
    // recur twice under session B with -b detail: each recurrence adds
    //   "feedback from <p>: <summary>" (stamped B, out.at == its at) and
    //   "detail from <p>: <body>" (stamped B, no occurrence keys); texts unchanged;
    //   the two outputs' `at` name two different notes (sleep 1s between, or assert
    //   index-wise equality of at to the k-th "feedback from" note)
}

#[test]
fn feedback_outside_projects_writes_creation_note() { /* from a dir outside every project */ }

#[test]
fn add_into_other_project_writes_creation_note() { /* add --project other from this checkout */ }

#[test]
fn pretty_add_still_prints_the_bare_id() { /* add --pretty stdout == id + newline; no action, no at */ }
```

Also, in `tests/move.rs`: `move_preserves_creation_note_and_occurrence_keys` (Review Focus 3).

- [ ] **Step 2: Run `just test-one --test cli creation_note`, `just test-one --test cli feedback_creation`, `just test-one --test move creation_note`.** Expected: FAIL (no note, no `at`).
- [ ] **Step 3: Implement** the Produces items. In `recur_into`, compute `occurrence_stamp` before `guarded_update`, so retries reuse the same stamp and push no duplicate warnings. Write the `feedback from` note with `append_occurrence_note` and the `detail` note with the pair only. Return the occurrence note's `at` so `run` can put it in `FeedbackOut`.
- [ ] **Step 4: Run `just test-fast`.** Expected: only the old positional note assertions fail. Update each one to account for the leading `created` note (shift indexes, or select by text). Never delete an assertion. Re-run until green.
- [ ] **Step 5: Update the README** paragraphs listed under Files, so every claim matches the tests.
- [ ] **Step 6: Run `just test-fast`.** Expected: PASS. Commit `feat(add,feedback): stamp creation and recurrence occurrences`.

---

### Task 4: Origin flags, combine, correction, filters and display

**Files:**
- Modify: `tools/cli.toml` (adopted from ops after Task 1; never hand-edited)
- Modify: `src/cli.rs` (`FieldArgs.origin`, `EditArgs.no_origin` and the editor conflict list, `Command::Add.origin_session`, `Command::Feedback.origin`/`origin_session`, `FilterArgs.origins`)
- Modify: `src/complete.rs` (`origins`, `filter_origins` candidates)
- Modify: `src/commands/add.rs`, `src/commands/feedback.rs`, `src/commands/mod.rs` (`apply_fields`), `src/commands/edit.rs` (or wherever `--no-process` is applied)
- Modify: `src/filter.rs` (`TaskFilter.origins: Vec<Option<Origin>>`, `Fields.origin`)
- Modify: `src/output.rs` / `src/format.rs` (pretty `show`: an `origin:` line after `agent:`; note rows show `origin` and `origin_session`)
- Modify: `README.md` (origin: values and definitions copied from §4.3; who supplies it; correction; **the reading rule** with the correction case as its example; `--origin-session`; filters; rollout §4.9)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Origin`, `Origin::combine`, `validate_session_key` (Task 2); `append_occurrence_note`, `occurrence_stamp` (Task 3).
- Produces:
  - `add --origin <user|agent|mixed>` and `--origin-session <key>`: both validated before any write. The origin goes on `task.origin` and on the creation note; the origin session goes on the creation note only.
  - `feedback --origin` and `--origin-session`. On creation, as for `add`. On recurrence, the note gets both, and the field is updated by `pub fn recur_origin(field: Option<Origin>, occurrence: Option<Origin>) -> (Option<Origin>, bool)` in `src/commands/feedback.rs`, where the `bool` means "warn":
    - `(f, None)` → `(f, false)`
    - `(None, Some(_))` → `(None, true)`
    - `(Some(f), Some(x))` → `(Some(f.combine(x)), false)`
  - Warning text: `origin unchanged: {id} has no recorded origin and this occurrence says {x}; set it with \`tasks edit {id} --origin\` once its occurrences are read`.
  - `edit --origin <v>` sets the field; `edit --no-origin` clears it. Notes are never touched.
  - Filters: `--origin <user|agent|mixed|none>`, repeatable and widening, on list, search and ready.

- [ ] **Step 1: Adopt the ops table:** `python3 "$(tasks --pretty root ops-88a0fa)/bin/vendored" adopt cli.toml`. Run `just test-one parser_surface_equals_table`; expected: FAIL (parser lacks the rows). It turns green in Step 4.
- [ ] **Step 2: Write failing tests in `tests/cli.rs`**

```rust
#[test]
fn add_records_origin_on_field_and_creation_note() {
    // add --origin user --origin-session claude-code:old → show.origin == "user",
    // notes[0].origin == "user", notes[0].origin_session == "claude-code:old";
    // add without --origin → no "origin" key anywhere
}

#[test]
fn bad_origin_inputs_are_refused_before_writing() {
    // --origin maybe → exit 2 (clap); --origin-session slack:x → validation, no file created
}

#[test]
fn recurrence_combines_origin_per_rule() {
    // field user + occ agent → mixed; field mixed + occ user → mixed;
    // field agent + occ agent → agent; field user + occ absent → user;
    // field absent + occ user → absent, one warning with the exact text, note keeps "user"
}

#[test]
fn correction_supersedes_a_mistaken_label() {
    // add --origin user; edit --origin agent → show.origin "agent", notes[0].origin "user";
    // edit --no-origin → no show.origin, notes[0].origin still "user"
}

#[test]
fn origin_filter_selects_and_none_means_unknown() {
    // three tasks user / mixed / none: list --origin user → 1; --origin user --origin mixed → 2;
    // --origin none → 1; ready and search accept the same
}

#[test]
fn pretty_show_prints_origin_after_agent() { /* "origin:   user" line follows "agent:" */ }
```

Also add `origin_survives_unknown_identity`, `editor_preserves_occurrence_keys` and `origin_filter_spans_projects` (Review Focus 1, 4 and 5).

- [ ] **Step 3: Run `just test-one --test cli origin`.** Expected: FAIL.
- [ ] **Step 4: Implement** the Produces items. Model `FieldArgs.origin` and the filter on the `process` / `processes` pattern (`ValueSet`, candidates, `optional(value, Origin::parse)`). Add `no_origin` to `--editor`'s conflict list and `origin` to `FieldArgs`' list there.
- [ ] **Step 5: Update the README** sections listed under Files.
- [ ] **Step 6: Run `just test-fast` and `just check`.** Expected: PASS, `parser_surface_equals_table` included. Commit `tools/cli.toml` with the parser: `feat(cli): origin, origin-session and origin filters`.

---

### Task 5: Skills: filing, pickup, and retrieval

**Files:**
- Modify: `skills/tasks/SKILL.md`. Under "A scoped task" / filing, pass `--origin` when the filer knows whose ask it is, with the three definitions and the "classifies asks, not the filing" sentence from §4.3, and `--origin-session` for an older ask. In the pickup step (Session protocol 3), read origin by the reading rule before asking the user about intent, and follow the scope skill's retrieval procedure for a non-`user` origin.
- Modify: `skills/scope/SKILL.md`, §2 Gather evidence. Add the §4.7 procedure in its two stages: candidate rules, the full / id-only / unreadable result classes, and the outcomes matched, inferred, ambiguous (with candidates), unavailable and lookup failed, recorded per stage. Ambiguous includes a single candidate whose creation-or-reuse action stays unsettled. Scope and brief notes are written with `--stamp`. Correct `origin` with `tasks edit --origin` plus a stamped evidence note.
- Test: none in Rust. `tasks check` and `just check` (hygiene) must stay green.

- [ ] **Step 1:** Write the scope-skill text by condensing §4.7 without changing any rule. Every outcome name and candidate rule matches the spec's words.
- [ ] **Step 2:** Write the tasks-skill text.
- [ ] **Step 3:** Run `just check`. Expected: green. Run `just install-skills` only if the project's guide asks for it after skill edits; otherwise leave the installed copy alone.
- [ ] **Step 4:** Commit `docs(skills): origin at filing and retrieval of the originating ask`.

---

### Task 6: Retrieval demonstrations

Runs after Tasks 2–5, before merge, from a real Claude Code session (the implementing session). Use the worktree binary by explicit path, `<worktree>/target/debug/tasks` after `cargo build` via `just test-fast`, in a scratch project under a fresh temp dir with `XDG_CONFIG_HOME` and `XDG_STATE_HOME` set to fresh temp dirs. It never touches the tracker on `PATH`. Record each outcome as a stamped note on this task. Attach one `retrieval-demo-report.md` listing each case, its commands, the transcript evidence, and its outcome per stage.

**Files:** none committed; the report is attached with `tasks attach`.

- [ ] **Step 1: Live creation.** `add` a task in the scratch project with JSON output. Retrieve it through the session-logs tooling from this session's transcript. Expected: stage 1 matched.
- [ ] **Step 2: Two recurrences.** Make the scratch project accept feedback and file one report. Then:
  - Two JSON recurrences in different seconds (`sleep 2` between them, and check that the two outputs' `at` differ). Each binds to its own call by `at`. Expected: matched twice.
  - Two JSON recurrences with equal summaries in the same second (one shell line, and check that the two `at` values are equal; if they are not, retry). Expected: ambiguous, listing both: `at` has whole-second precision.
  - The same pair of cases with `--pretty`. Expected: matched where time and arguments separate the calls, otherwise ambiguous.
- [ ] **Step 3: Pretty creation, then pretty reuse.** Run `add --pretty --source S` twice. Expected: the first binds as inferred and the second is excluded by its reuse warning. For the no-stderr variant, reason over a transcript excerpt with stderr removed. Expected: ambiguous, listing both calls. Then a lone sourced `add --pretty --source S2` in a fresh scratch project, read from an excerpt with stderr removed. Expected: ambiguous with one candidate and the action unsettled (spec §4.7).
- [ ] **Step 4: Older request.** File in this session (B) with `--origin-session claude-code:572ee51e-2f53-4729-96f8-f122a2cc8305`, after reading that session's 10:59:13Z human turn (the user's "Are you able to tell who created the task?"). Expected: stage 1 matched in B; stage 2 matched (B's session-logs read printed the turn) or inferred. Then file a second record naming a nonexistent `codex:` key. Expected: stage 2 unavailable, stage 1 still matched.
- [ ] **Step 5: Limits.** Record one case for each:
  - An unstamped older record (any pre-change task): unavailable.
  - An archived Codex session under `~/.codex/archived_sessions`: reachable or not with the current tooling; name the reader gap if not.
  - The parent-thread path on one audited Codex subagent filing from tasks-fa3525's attachments.
  - Another host: an explicit limitation.
- [ ] **Step 6:** Attach the report and note each outcome. A failed expectation is a defect against Task 5's text or the code: fix it, and rerun that case.

---

### Task 7: Hand-offs and wake notes

- [ ] **Step 1:** File the lore follow-up:

```bash
tasks add "Pass --origin in the feedback rule and cite origin in the intent clause" --project lore \
  --process direct --status todo \
  -b "tasks now records origin (user|agent|mixed). The global Feedback rule passes --origin user or --origin mixed when a report relays the user's words, beside lore-c01bc8's body rule; the Decisions clause on intent may read origin before asking. Spec: tasks docs/specs/2026-10-25-creation-occurrence-provenance-design.md §4.8."
```

- [ ] **Step 2:** File the post-merge rollout as an open child of this plan's goal, owned by the agent that merges the branch:

```bash
tasks add "Roll out the provenance binary to every host" --parent tasks-053e89 --status todo \
  --process direct --complexity low \
  -b "After the implementation branch merges: on this host, run cargo install --path . from main and verify end to end (one add and one feedback show a stamped created note). Every other host that reads synced task records installs a current binary before records carrying origin or the new note keys reach it; older binaries reject them (spec §4.9). Done when each such host is named in a note as updated."
```

- [ ] **Step 3:** File the blind pilot as a child of this plan's goal that depends on the rollout task. Copy its body from §5 "Blind at-filing pilot", and set `--process direct --complexity mid`.
- [ ] **Step 4:** Before the branch merges, write the wake notes the source tasks ask for:
  - `tasks note tasks-516ab4 --no-reviewed "<what landed and how a scope pass now finds a report's origin>"`
  - `tasks note tasks-2e7d03 --no-reviewed "<what landed: creation note, origin field, retrieval procedure>"`
- [ ] **Step 5:** Close this step with `tasks done` on the branch, before merge. Installation is not part of it: it belongs to the rollout task filed in Step 2.
