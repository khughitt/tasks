# Creation Provenance Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan
> task-by-task in the existing worktree. Steps use checkboxes for tracking.

Status: implemented (2026-09-13); Tasks 1–2 and Pieces A/B complete, live acceptance passed.
Cross-project pieces (ops hook, ai wiring) are filed in those projects and block the
goal; they are described under **Pieces in other projects**, not as numbered tasks.

**Goal:** Every task an agent files carries an opaque `agent` stamp naming the harness
and, when known, the model that created it — and the harness side actually exports it.

**Architecture:** Extend the optional-string field path `source` and `model` already
use: record, codec, shared add/edit flags, output projections. One resolver decides the
creation value (flag first, then `TASKS_AGENT`), and `add::blank()` takes it so `add`
and `feedback` share the write path. Harnesses export the variable: Claude Code via an
env-file preamble that resolves a scratchpad state file at command time, Codex
harness-only, others by instruction.

**Tech Stack:** Rust, clap, serde, `tests/cli.rs` integration tests, `just` recipes;
Python hook and `unittest` in ops; JSON/TOML settings in ai.

**Spec:** `docs/specs/2026-09-13-creation-provenance-design.md` (approved after two
review rounds).

## Global constraints

- `agent` is optional, opaque, single-line, non-empty; `null` in JSON when absent.
  Never interpreted, validated against a registry, inferred, or guessed.
- Creation only: `add` (flag, else `TASKS_AGENT`) and `feedback` (`TASKS_AGENT` only).
  `edit` never reads the variable. Explicit flag resolves before the environment is read.
- `model` and `TASKS_MODEL` keep their rules; the two fields are independent.
- Frontmatter key order: after `model`, before `spec`. JSON is additive; no existing key
  changes. Tables gain no column; `show --pretty` prints the frontmatter line.
- No `check` warning, no readiness, ordering, or `start` effect. No retroactive stamping.
- Unknown-model paths in the harness wiring clear stale model state; harness-only
  attribution is preferred over a possibly wrong model.
- Test builders scrub `TASKS_AGENT` alongside `TASKS_MODEL`.
- `just gate` before every commit that touches `src/`; reinstall with
  `cargo install --path .` after the CLI lands so later pieces run the new binary.

---

### Task 1: Record and expose the agent stamp

**Files:**
- Modify: `src/model.rs` (Task struct, after `model`; test fixture)
- Modify: `src/format.rs` (KEYS, parse, validate, serialize; unit test)
- Modify: `src/cli.rs` (`FieldArgs.agent`, `EditArgs.no_agent`)
- Modify: `src/commands/mod.rs` (`creation_agent`, shared env reader, `apply_fields`)
- Modify: `src/commands/add.rs` (`blank` signature, resolver call)
- Modify: `src/commands/feedback.rs` (`blank` call)
- Modify: `src/commands/list.rs:482` (test fixture `blank` call gains `None`)
- Modify: `src/commands/edit.rs` (`has_flags`, `--no-agent`)
- Modify: `src/output.rs` (`TaskSummary`, `ParkedRow`, both constructors, test fixture)
- Modify: `src/complexity.rs`, `src/hierarchy.rs`, `src/periodic.rs`, `src/query.rs`,
  `src/repo.rs`, `src/similarity.rs` (test fixtures gain `agent: None`)
- Modify: `tests/common/mod.rs` (scrub `TASKS_AGENT` in `cmd` and `raw`)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `validate_line(field, s)` in `src/format.rs:226`; `completion_model()` in
  `src/commands/mod.rs:589`; `blank(project, title, status)` in `src/commands/add.rs:12`.
- Produces: `Task.agent: Option<String>`; `TaskSummary.agent`, `ParkedRow.agent`;
  `pub fn creation_agent(explicit: Option<&str>) -> Result<Option<String>>` and
  `fn provenance_var(name: &str) -> Result<Option<String>>` in `src/commands/mod.rs`;
  `blank(project, title, status, agent: Option<String>)`; flags `--agent <value>` on
  `add`/`edit` and `--no-agent` on `edit`.

- [ ] **Step 1: Write the failing integration tests**

Append to `tests/cli.rs`, after `show_pretty_prints_the_model_line`:

```rust
#[test]
fn add_stamps_agent_from_the_flag_then_the_variable_and_absence_records_nothing() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");

    let from_env = id_of(
        serde_json::from_slice(
            &env.cmd(&sci)
                .args(["add", "Env", "-p", "2"])
                .env("TASKS_AGENT", "crush/kimi-k3")
                .assert()
                .success()
                .get_output()
                .stdout,
        )
        .unwrap(),
    );
    assert_eq!(env.json(&sci, &["show", &from_env])["task"]["agent"], "crush/kimi-k3");
    assert!(env.read(&sci, &format!("tasks/{from_env}.md")).contains("\nagent: crush/kimi-k3\n"));

    let from_flag = id_of(
        serde_json::from_slice(
            &env.cmd(&sci)
                .args(["add", "Flag", "-p", "2", "--agent", "codex/gpt-6"])
                .env("TASKS_AGENT", "crush/kimi-k3")
                .assert()
                .success()
                .get_output()
                .stdout,
        )
        .unwrap(),
    );
    assert_eq!(env.json(&sci, &["show", &from_flag])["task"]["agent"], "codex/gpt-6");

    let plain = id_of(env.json(&sci, &["add", "Plain", "-p", "2"]));
    let shown = env.json(&sci, &["show", &plain]);
    assert!(shown["task"].get("agent").is_some(), "key present when absent: {shown}");
    assert!(shown["task"]["agent"].is_null());

    let empty = id_of(
        serde_json::from_slice(
            &env.cmd(&sci)
                .args(["add", "Empty", "-p", "2"])
                .env("TASKS_AGENT", "")
                .assert()
                .success()
                .get_output()
                .stdout,
        )
        .unwrap(),
    );
    assert!(env.json(&sci, &["show", &empty])["task"]["agent"].is_null());
}

#[test]
fn invalid_tasks_agent_fails_add_unless_the_flag_names_a_valid_agent() {
    use std::os::unix::ffi::OsStrExt;
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let before = std::fs::read_dir(sci.join("tasks")).unwrap().count();

    let out = env
        .cmd(&sci)
        .args(["add", "Bad", "-p", "2"])
        .env("TASKS_AGENT", std::ffi::OsStr::from_bytes(b"\xff\xfe"))
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(err_detail(&out).contains("TASKS_AGENT is not valid Unicode"), "{}", err_detail(&out));

    let out = env
        .cmd(&sci)
        .args(["add", "Bad", "-p", "2"])
        .env("TASKS_AGENT", "a\nb")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(err_detail(&out).contains("TASKS_AGENT must be a single line"), "{}", err_detail(&out));
    assert_eq!(
        std::fs::read_dir(sci.join("tasks")).unwrap().count(),
        before,
        "a failed add writes nothing"
    );

    for bad in [std::ffi::OsStr::from_bytes(b"\xff\xfe"), std::ffi::OsStr::new("a\nb")] {
        let id = id_of(
            serde_json::from_slice(
                &env.cmd(&sci)
                    .args(["add", "Good", "-p", "2", "--agent", "codex/gpt-6"])
                    .env("TASKS_AGENT", bad)
                    .assert()
                    .success()
                    .get_output()
                    .stdout,
            )
            .unwrap(),
        );
        assert_eq!(env.json(&sci, &["show", &id])["task"]["agent"], "codex/gpt-6");
    }
}

#[test]
fn feedback_stamps_agent_from_the_variable_only() {
    let (env, _target, reporter) = feedback_env();
    let out: serde_json::Value = serde_json::from_slice(
        &env.cmd(&reporter)
            .args(["feedback", "the flag is hard to find", "--category", "friction"])
            .env("TASKS_AGENT", "claude-code/claude-opus-5")
            .assert()
            .success()
            .get_output()
            .stdout,
    )
    .unwrap();
    let id = out["id"].as_str().unwrap();
    assert_eq!(
        env.json(&reporter, &["show", id])["task"]["agent"],
        "claude-code/claude-opus-5"
    );
    // feedback has no --agent flag
    env.cmd(&reporter)
        .args(["feedback", "x", "--category", "friction", "--agent", "codex"])
        .assert()
        .code(2);
}

#[test]
fn edit_never_reads_tasks_agent_and_agent_flags_replace_and_clear() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "Plain", "-p", "2"]));

    env.cmd(&sci)
        .args(["edit", &id, "--title", "Renamed"])
        .env("TASKS_AGENT", "crush/kimi-k3")
        .assert()
        .success();
    assert!(
        env.json(&sci, &["show", &id])["task"]["agent"].is_null(),
        "an edit under the variable does not claim creation"
    );

    env.json(&sci, &["edit", &id, "--agent", "codex/gpt-6"]);
    assert_eq!(env.json(&sci, &["show", &id])["task"]["agent"], "codex/gpt-6");
    env.json(&sci, &["edit", &id, "--no-agent"]);
    assert!(env.json(&sci, &["show", &id])["task"]["agent"].is_null());
    assert!(!env.read(&sci, &format!("tasks/{id}.md")).contains("agent:"));

    assert_eq!(env.fail(&sci, &["edit", &id, "--agent", ""]), "validation");
    assert_eq!(env.fail(&sci, &["edit", &id, "--agent", "a\nb"]), "validation");
    env.cmd(&sci)
        .args(["edit", &id, "--agent", "x", "--no-agent"])
        .assert()
        .code(2);

    // an editor save is validated through the record parser: an empty value is refused
    // and the record is left as it was
    env.json(&sci, &["edit", &id, "--agent", "codex/gpt-6"]);
    let editor = editor_script(&sci, "sed -i 's|^agent: .*$|agent: \"\"|' \"$1\"");
    let out = env
        .cmd(&sci)
        .args(["edit", &id])
        .env("EDITOR", &editor)
        .output()
        .unwrap();
    assert!(!out.status.success(), "{}", err_detail(&out));
    assert_eq!(env.json(&sci, &["show", &id])["task"]["agent"], "codex/gpt-6");

    // a value with reserved characters is quoted on disk and round-trips byte for byte
    env.json(&sci, &["edit", &id, "--agent", "crush/kimi-k3 [nightly]: b"]);
    assert_eq!(
        env.json(&sci, &["show", &id])["task"]["agent"],
        "crush/kimi-k3 [nightly]: b"
    );
    assert!(
        env.read(&sci, &format!("tasks/{id}.md")).contains("\nagent: \"crush/kimi-k3 [nightly]: b\"\n")
    );
}

#[test]
fn summary_rows_and_parked_rows_carry_agent() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let stamped = id_of(
        serde_json::from_slice(
            &env.cmd(&sci)
                .args(["add", "Stamped", "-p", "2"])
                .env("TASKS_AGENT", "codex/gpt-6")
                .assert()
                .success()
                .get_output()
                .stdout,
        )
        .unwrap(),
    );
    let plain = id_of(env.json(&sci, &["add", "Plain", "-p", "2"]));

    for view in [vec!["list"], vec!["ready"]] {
        let rows = env.json(&sci, &view);
        let rows = rows["tasks"].as_array().unwrap();
        let row = rows.iter().find(|row| row["id"] == stamped).unwrap();
        assert_eq!(row["agent"], "codex/gpt-6", "{view:?}");
        let row = rows.iter().find(|row| row["id"] == plain).unwrap();
        assert!(row.get("agent").is_some() && row["agent"].is_null(), "{view:?}");
    }

    env.json(&sci, &["park", &stamped, "resume here"]);
    let prime = env.json(&sci, &["prime"]);
    let parked = prime["parked"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == stamped)
        .unwrap()
        .clone();
    assert_eq!(parked["agent"], "codex/gpt-6");

    let out = env
        .cmd(&sci)
        .args(["--pretty", "show", &stamped])
        .assert()
        .success();
    let text = String::from_utf8_lossy(&out.get_output().stdout);
    assert!(text.contains("agent: codex/gpt-6\n"), "{text}");
    let out = env.cmd(&sci).args(["--pretty", "list"]).assert().success();
    let text = String::from_utf8_lossy(&out.get_output().stdout);
    assert!(!text.contains("codex/gpt-6"), "tables gain no column: {text}");
}
```

- [ ] **Step 2: Write the failing format unit test**

In `src/format.rs` tests, after `process_round_trips_and_rejects_invalid_values`:

```rust
    #[test]
    fn agent_round_trips_after_model_and_rejects_multi_line() {
        let text = MINIMAL.replace(
            "tags: []\n",
            "tags: []\nsource: mind6-1\nmodel: claude-opus-5\nagent: codex/gpt-6\n",
        );
        let task = parse_task(&text, "x").unwrap();
        assert_eq!(task.agent.as_deref(), Some("codex/gpt-6"));
        assert_eq!(serialize_task(&task), text);

        let mut bad = task.clone();
        bad.agent = Some("a\nb".into());
        assert!(validate_task(&bad).is_err());
    }
```

Check `MINIMAL` in the existing tests for the exact `tags: []` line and whether `spec:`
follows; the replacement must keep `agent` between `model` and `spec`.

- [ ] **Step 3: Run the new tests to verify they fail**

Run: `cargo test agent 2>&1 | tail -20`
Expected: compile errors — `Task` has no field `agent`, `--agent` is unknown.

- [ ] **Step 4: Add the field to the record and codec**

`src/model.rs`, after `pub model: Option<String>,`:

```rust
    /// The harness and model that filed the task (`<harness>/<model>`, or the harness
    /// alone), from `add --agent` or `TASKS_AGENT` at creation; `None` when unknown.
    /// Stored and returned, never interpreted. Independent of `model`, which is the
    /// completion side. See docs/specs/2026-09-13-creation-provenance-design.md.
    pub agent: Option<String>,
```

`src/format.rs`: `KEYS` becomes `[&str; 24]` with `"agent"` inserted after `"model"`;
in `parse_task` add `agent: scalar("agent")?,` after `model`; in `validate_task` add

```rust
    if let Some(agent) = &t.agent {
        validate_line("agent", agent)?;
    }
```

after the `model` check; in `serialize_task` add

```rust
    if let Some(v) = &t.agent {
        pairs.push(("agent".into(), s(v)));
    }
```

after the `model` pair. Add `agent: None,` to every `Task { … }` literal the compiler
reports: `src/model.rs`, `src/complexity.rs`, `src/hierarchy.rs`, `src/periodic.rs`,
`src/query.rs`, `src/repo.rs`, `src/similarity.rs`, and `src/commands/add.rs::blank`
(replaced in Step 6).

- [ ] **Step 5: Add the flags and the resolver**

`src/cli.rs`, in `FieldArgs` after `source`:

```rust
    /// The harness and model filing the task, `<harness>/<model>` or the harness alone.
    /// On `add` this overrides `TASKS_AGENT`; on `edit` it replaces the stamp. Never
    /// interpreted; see `--no-agent`.
    #[arg(long)]
    pub agent: Option<String>,
```

In `EditArgs` after `no_source`:

```rust
    /// Clear the agent stamp.
    #[arg(long, conflicts_with = "agent")]
    pub no_agent: bool,
```

`src/commands/mod.rs`: replace `completion_model` with a shared reader and two callers:

```rust
/// A provenance variable the harness exports (`TASKS_MODEL`, `TASKS_AGENT`). Unset or
/// empty records nothing; a non-Unicode or multi-line value is an explicit error, never
/// a silent skip.
fn provenance_var(name: &str) -> Result<Option<String>> {
    match std::env::var_os(name) {
        None => Ok(None),
        Some(value) => {
            let value = value
                .into_string()
                .map_err(|_| Error::Validation(format!("{name} is not valid Unicode")))?;
            if value.is_empty() {
                return Ok(None);
            }
            crate::format::validate_line(name, &value)?;
            Ok(Some(value))
        }
    }
}

/// The model the harness reports for this completion, from `TASKS_MODEL`.
fn completion_model() -> Result<Option<String>> {
    provenance_var("TASKS_MODEL")
}

/// The agent filing a task: an explicit `--agent` when given, validated first so a bad
/// environment cannot fail an add that names a valid agent; otherwise `TASKS_AGENT`.
pub fn creation_agent(explicit: Option<&str>) -> Result<Option<String>> {
    match explicit {
        Some(agent) => {
            validate_line("agent", agent)?;
            Ok(Some(agent.to_string()))
        }
        None => provenance_var("TASKS_AGENT"),
    }
}
```

In `apply_fields`, after the `source` block:

```rust
    if let Some(agent) = &fields.agent {
        validate_line("agent", agent)?;
        task.agent = Some(agent.clone());
    }
```

`src/commands/edit.rs`: add `|| fields.agent.is_some() || args.no_agent` to `has_flags`
beside the `source` pair, and after the `no_source` block:

```rust
    if args.no_agent {
        task.agent = None;
    }
```

- [ ] **Step 6: Route both creators through the resolver**

`src/commands/add.rs::blank` gains a fourth parameter and stores it:

```rust
/// … The single constructor behind `add` and `feedback`, so a file created in another
/// project is shaped exactly as one created locally. `agent` is the resolved creation
/// stamp (`creation_agent`), passed in so both creators share one rule.
pub fn blank(project: &Project, title: String, status: Status, agent: Option<String>) -> Result<Task> {
    …
        model: None,
        agent,
        spec: None,
    …
```

In `add::run`, replace `let mut task = blank(&ctx.project, title, status)?;` with:

```rust
    let agent = super::creation_agent(fields.agent.as_deref())?;
    let mut task = blank(&ctx.project, title, status, agent)?;
```

In `src/commands/feedback.rs:141`:

```rust
    let agent = super::creation_agent(None)?;
    let mut task = super::add::blank(target, summary, Status::Idea, agent)?;
```

The third caller is a unit-test fixture, `src/commands/list.rs:482`; it passes `None`:

```rust
        let dependency =
            crate::commands::add::blank(&project, "Dependency".into(), Status::Todo, None).unwrap();
```

- [ ] **Step 7: Project the field into JSON**

`src/output.rs`: add `pub agent: Option<String>,` after `model` in both `TaskSummary`
and `ParkedRow`; `agent: task.agent.clone(),` in `TaskSummary::from`;
`agent: summary.agent,` in `ParkedRow`'s resolved constructor and `agent: None,` in the
unresolved one; `agent: None,` in the test fixture at the bottom of the file.

- [ ] **Step 8: Scrub the variable in the test builders**

`tests/common/mod.rs`: add `.env_remove("TASKS_AGENT")` after `.env_remove("TASKS_MODEL")`
in both `cmd` and `raw`.

- [ ] **Step 9: Run the new tests to verify they pass**

Run: `cargo test agent 2>&1 | tail -20`
Expected: the five integration tests and the format unit test pass.

- [ ] **Step 10: Run the gate and reinstall**

Run: `just gate && cargo install --path .`
Expected: fmt, clippy, `tasks check`, and the full suite pass; `tasks add --help` lists
`--agent`.

- [ ] **Step 11: Commit**

```bash
git add src tests
git commit -m "feat(add): record the filing agent from --agent or TASKS_AGENT"
```

---

### Task 2: Instruct harnesses and document the field

**Files:**
- Modify: `skills/tasks/SKILL.md` (session protocol step 6 area, `TASKS_MODEL` paragraph
  at line 87; the "A scoped task" bullet's flag list)
- Modify: `README.md` (`TASKS_MODEL` paragraph at line 66; the flag examples block)
- Modify: `docs/specs/2026-08-29-tasks-design.md` (the field table or provenance section
  that lists `model`)
- Modify: `docs/specs/2026-09-13-creation-provenance-design.md` (status line)

**Interfaces:**
- Consumes: the flags and variable from Task 1.
- Produces: the one-rule instruction other harnesses follow; the README paragraph the
  ops and ai pieces link to.

- [ ] **Step 1: Add the skill rule**

In `skills/tasks/SKILL.md`, directly after the `TASKS_MODEL` paragraph (line 87–90):

```markdown
   Every task an agent files carries `agent`, the harness and model that created it
   (`<harness>/<model>`, or the harness alone), from `TASKS_AGENT` when the harness
   exports it. When it is unset and you know your harness and model ids, pass
   `--agent <harness>/<model>` on `add`; when you know only the harness, pass that;
   when unsure, pass nothing — never guess. `feedback` has no flag: supply the variable
   on that invocation, `TASKS_AGENT=<harness>/<model> tasks feedback …`. Under Claude
   Code the model half is the enclosing session's model. `tasks edit --agent/--no-agent`
   corrects a stamp; an edit never reads the variable.
```

Add `[--agent <harness>/<model>]` to the "A scoped task" `tasks add` line and
`--agent/--no-agent` to the `tasks edit` flag list beside `--source/--no-source`.

- [ ] **Step 2: Document the variable in the README**

After the `TASKS_MODEL` paragraph (line 66–68):

```markdown
`TASKS_AGENT` per harness process records which harness and model filed each task
(`<harness>/<model>`, or the harness alone): `add` and `feedback` stamp the record's
`agent` field from it, `add --agent` overrides it, and `tasks edit --agent`/`--no-agent`
corrects a stamp. The harness owns exporting it: the design wires Claude Code through
an ops session-start and model-switch hook and exports the harness alone for Codex
(both tracked as separate pieces in those projects until they land); other harnesses
pass `--agent` when they know their ids. Design:
`docs/specs/2026-09-13-creation-provenance-design.md`.
```

Add `--agent codex/gpt-6` to one `tasks add` example in the examples block.

- [ ] **Step 3: Update the base design reference**

The field table in `docs/specs/2026-08-29-tasks-design.md` §3.1 (line 109) lists
`source` but never received `model`. Insert two rows after the `source` row:

```markdown
| `model`    | string              | no       | Model id the harness reported (`TASKS_MODEL`) for the latest completion; single line; never interpreted. Written after `source`. See `2026-09-10-model-provenance-design.md`. |
| `agent`    | string              | no       | Harness and model that filed the task (`<harness>/<model>`, or the harness alone), from `add --agent` or `TASKS_AGENT`; single line; never interpreted. Written after `model`. See `2026-09-13-creation-provenance-design.md`. |
```

- [ ] **Step 4: Verify the docs against the binary**

Run: `grep -n -- "--agent" skills/tasks/SKILL.md README.md && tasks add --help | grep -- --agent && tasks edit --help | grep -- --no-agent`
Expected: every documented flag exists in the installed binary.

- [ ] **Step 5: Mark the spec locally implemented and run the check**

Set the spec status to `locally implemented (2026-09-13); harness pieces ops-… and
ai-… open`. Run: `just check`. Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add skills README.md docs
git commit -m "docs(agent): document the creation stamp and the harness rule"
```

---

## Pieces in other projects

Filed as tasks in ops and ai with `tasks add --project <prefix>`; the goal depends on
each (`tasks dep tasks-dc599b --on <id>`). They are executed in those checkouts against
the installed binary from Task 1, after this plan's tasks merge.

### Piece A (ops): Claude Code provenance hook

**Files:** create `hooks/claude-provenance` (one script for both events, dispatching on
`hook_event_name`), create `tests/test_provenance.py`, modify `justfile` `check_cmd`
(add the new hook to the `py_compile` list), modify `README.md` hooks section.

Behaviour (spec, Harness wiring): read stdin JSON; on `SessionStart`, which is the
only event that receives `CLAUDE_ENV_FILE`, require it (unset: write nothing); with
`scratchpad_dir` present, write `model` to
`<scratchpad_dir>/tasks-model` (delete the file when `model` is absent) and append

```sh
export TASKS_MODEL="$(cat '<state>' 2>/dev/null)"
export TASKS_AGENT="claude-code${TASKS_MODEL:+/$TASKS_MODEL}"
```

with `<state>` single-quote-escaped; without `scratchpad_dir`, append
`export TASKS_AGENT=claude-code` and `export TASKS_MODEL=`. On `PostModelSwitch`, with
`scratchpad_dir`: write `to_model` to the state file, or delete it when `to_model` is
absent — `CLAUDE_ENV_FILE` is never set for this event and must not be required. Any
other event, or any exception: write nothing, exit 0.

Tests: SessionStart with/without `model` and with/without `scratchpad_dir`, each
starting from an existing state file and `TASKS_MODEL=old` in the hook environment;
PostModelSwitch with and without `to_model`, run with `CLAUDE_ENV_FILE` removed from
the hook environment and still asserting the state file changes; a
`bash -c 'source $ENV; echo $TASKS_AGENT $TASKS_MODEL'` assertion before and after a
switch payload with `TASKS_MODEL` pre-set; a malformed stdin exits 0 and writes nothing.

### Piece B (ai): register the hook and set Codex harness-only

**Files:** modify `claude/settings.json` (add `~/d/ops/hooks/claude-provenance` under
`SessionStart` with matcher `startup|resume` and under a new `PostModelSwitch` entry);
modify `codex/config.toml` `[shell_environment_policy]` `set` to include
`TASKS_AGENT = "codex"`. Depends on Piece A.

Acceptance (spec, live transition), in a scratch project so no real record or registry
entry is touched: in a fresh Claude Code session, `D=$(mktemp -d) && cd $D && git init -q
&& export XDG_CONFIG_HOME=$(mktemp -d) && tasks init --prefix prb`; under model A,
`tasks add "probe A"` and confirm `agent` is `claude-code/<A>`; `/model` to B;
`tasks add "probe B"` and confirm `claude-code/<B>`; `tasks start` then `tasks done`
the second and confirm `model` is `<B>`; delete the scratch directories (`done` →
`dropped` is not a permitted transition, so the probes are not dropped). Record the
result on the piece. If the second add still shows A, apply the spec's fallback
(harness-only exports) and note why in the spec. In a Codex session, the same scratch
recipe with `tasks add "probe"` shows `agent: codex` and no `model`.

### Goal closeout

After both pieces close: correct the creation-provenance spec status to
`implemented`, `tasks done tasks-dc599b`, and `git worktree remove` the branch.
