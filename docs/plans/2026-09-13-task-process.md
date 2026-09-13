# Explicit Task Process Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan
> task-by-task in the existing worktree. Steps use checkboxes for tracking.

Status: locally implemented (2026-09-13); both tasks complete on this branch.
External rollout remains tracked by ai-69ccac and ai-e8dcc5.

**Goal:** Store and display an explicit direct/planned process choice without
changing which tasks can be selected or started.

**Architecture:** Extend the existing optional-field path used by complexity:
model, record codec, shared add/edit fields, and output projections. Add one
doing-only check warning; keep execution policy in the agent instructions.

**Tech Stack:** Rust, clap, serde, existing integration tests and `just` recipes.

**Spec:** `docs/specs/2026-09-13-task-process-design.md` (approved with edits).

## Global constraints

- `process` is optional and accepts only `direct` and `planned`; null is unassessed.
- No derivation, inheritance, default, sorting/filtering change, start gate,
  approval-state field, claim-store extension, or new dependency.
- JSON adds keys without changing existing keys. `phase` keeps its current meaning.
- `process_missing` warns only on doing records, including goals and plan steps.
- No bulk backfill or historical record rewrite. Task records are CLI-written only.
- Both code paths use worktree isolation; the CLI does not enforce Git policy.
- Existing document links and unchecked boxes do not establish approval or progress.
- Keep the spec status accurate when work lands. Report local implementation and
  cross-project rollout separately.
- Run checks through the timing wrapper. A focused Rust test invocation is
  `python3 tools/tt test-fast -- cargo test process_`; the final gate is `just gate`.
- Reinstall after CLI changes with `cargo install --path .` before using new flags.

## Scope and handoff

The checkout is `.worktrees/tasks-61cc5c-process`, branch
`feat/tasks-61cc5c-process`. It already exists; its justfile has no setup recipe.
Commands below run there. Start each child before its changes, record scope shifts
with `tasks note`, and close it in its implementation commit after checks pass.

This plan has two sequential, independently reviewable deliverables: the CLI
contract, then the agent policy and documentation. Execute inline; no parallel
dispatch is needed. Both children now record process **direct**, since this
reviewed plan supplies their decisions. Task 1 installed the supporting binary
and assigned those fields explicitly; the parent records **planned**. Unrelated
records remain unassessed, including tasks-7ba741 until the merge checkout handles it.

External follow-ups already filed:

- **ai-69ccac:** widen the global worktree trigger. Independent of this CLI; it is
  the fix for the ran-on-main half of the prism incident.
- **ai-e8dcc5:** update and distribute the canonical shared writing-plans skill's
  child-creation command after tasks-61cc5c lands. The tasks repo owns only its own
  integration prose. Do not edit plugin caches or claim the shared rollout is done.

### Task 1: Persist and expose the process contract

Task record: tasks-ccbdf2 (intended process: direct; complexity: mid).

**Files:**

- Modify `src/model.rs`: `Process`, `Task.process`, model test constructors.
- Modify `src/format.rs`: `KEYS`, `parse_task`, `serialize_task`, record tests.
- Modify `src/cli.rs`: `FieldArgs.process`, `EditArgs.no_process`.
- Modify `src/complete.rs`: `processes` candidates and test constructors.
- Modify `src/commands/add.rs`: `blank` default.
- Modify `src/commands/mod.rs`: `apply_fields` and test constructors.
- Modify `src/commands/edit.rs`: flag detection and explicit clearing.
- Modify `src/output.rs`: summary/parked projections and pretty renderers.
- Modify `src/commands/check.rs`: one warning predicate.
- Modify `tests/cli.rs`: process contract coverage using `common::TestEnv`.
- Update existing `Task` literals as required in
  `src/complexity.rs`, `src/hierarchy.rs`, `src/query.rs`, `src/repo.rs`,
  `src/similarity.rs`, and any remaining constructors identified by
  `rg -n 'Task \{' src tests`. Set their new field to `None`; do not refactor them.

**Interfaces:** Consumes existing `Task`, `FieldArgs`, `EditArgs`, and output
projections. Produces `Process::{Direct, Planned}`, `Process::ALL`,
`Process::parse(&str) -> Result<Process>`, `Process::as_str(self) -> &'static str`,
and `process: Option<Process>` on `Task`, `TaskSummary`, and `ParkedRow`.
Produces `complete::processes() -> Vec<CompletionCandidate>`.

- [x] **Write a failing end-to-end field test in `tests/cli.rs`.**

```rust
#[test]
fn process_round_trips_and_rejects_invalid_edits() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "T", "--process", "direct"]));
    let path = dir.join(format!("tasks/{id}.md"));
    assert_eq!(env.json(&dir, &["show", &id])["task"]["process"], "direct");
    env.json(&dir, &["edit", &id, "--process", "planned"]);
    env.json(&dir, &["note", &id, "retain choice"]);
    assert_eq!(env.json(&dir, &["show", &id])["task"]["process"], "planned");
    let before = std::fs::read(&path).unwrap();
    assert_eq!(env.fail(&dir, &["edit", &id, "--process", "auto"]), "validation");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let out = env.cmd(&dir)
        .args(["edit", &id, "--process", "direct", "--no-process"])
        .output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    env.json(&dir, &["edit", &id, "--no-process"]);
    env.json(&dir, &["note", &id, "retain absence"]);
    let task = env.json(&dir, &["show", &id]);
    assert!(task["task"].get("process").unwrap().is_null());
    assert!(!std::fs::read_to_string(&path).unwrap().contains("\nprocess:"));
    assert_eq!(env.fail(&dir, &["add", "Bad", "--process", "auto"]), "validation");
    assert_eq!(env.json(&dir, &["list"])["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(env.complete_values(&dir, "bash", 4,
        &["tasks", "add", "T", "--process", ""]), vec!["direct", "planned"]);
    assert!(env.complete_values(&dir, "zsh", 4,
        &["tasks", "edit", &id, "--process", ""])
        .iter().any(|value| value.starts_with("planned")));
}
```

Run `python3 tools/tt test-fast -- cargo test process_round_trips` and confirm the
unknown `--process` flag causes failure before implementing it.

- [x] **Implement the model and record codec using the existing field pattern.**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Process { Direct, Planned }

impl Process {
    pub const ALL: [Process; 2] = [Process::Direct, Process::Planned];
    pub fn parse(s: &str) -> Result<Process> {
        Self::ALL.into_iter().find(|p| p.as_str() == s).ok_or_else(|| {
            Error::Validation(format!(
                "unknown process {s:?}; expected one of direct, planned"
            ))
        })
    }
    pub fn as_str(self) -> &'static str {
        match self { Self::Direct => "direct", Self::Planned => "planned" }
    }
}
```

Add `pub process: Option<Process>` beside complexity on `Task` and update all
constructors. Add `process` to `format::KEYS`. Parse and serialize it as follows,
retaining the existing file-specific parse-error mapping:

```rust
process: scalar("process")?
    .map(|s| Process::parse(&s))
    .transpose()
    .map_err(|e| perr(file, e.to_string()))?,
```

```rust
if let Some(process) = t.process {
    pairs.push(("process".into(), s(process.as_str())));
}
```

The existing editor calls this parser: do not add another validator or introduce a
separate serialization path. Add a record-level test covering both values,
absence, and an invalid process scalar in the existing format test module.

- [x] **Wire the shared CLI fields, mutation path, and completion.**

```rust
// FieldArgs
#[arg(long, add = ArgValueCandidates::new(crate::complete::processes))]
pub process: Option<String>,
// EditArgs
#[arg(long, conflicts_with = "process")]
pub no_process: bool,
// complete.rs
pub fn processes() -> Vec<CompletionCandidate> {
    plain(Process::ALL.iter().map(|process| process.as_str()))
}
// apply_fields
if let Some(process) = &fields.process {
    task.process = Some(Process::parse(process)?);
}
// edit::run, before apply_fields
if args.no_process {
    task.process = None;
}
```

Include `fields.process.is_some()` and `args.no_process` in `has_flags`, and add
help text explaining missing means unassessed. Do not copy complexity's
`ctx.reassess` call: process has no shared-store overlay.

- [x] **Write output and warning regression tests, then implement the projections.**

Start with these two test bodies; run them red before changing those paths:

```rust
#[test]
fn process_survives_ready_and_parked_next() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "T", "--process", "planned"]));
    assert_eq!(env.json(&dir, &["ready"])["tasks"][0]["process"], "planned");
    assert_eq!(env.json(&dir, &["prime"])["ready"][0]["process"], "planned");
    assert_eq!(env.json(&dir, &["next"])["next"]["task"]["process"], "planned");
    assert!(env.pretty(&dir, &["ready"]).contains("planned"));
    assert!(env.pretty(&dir, &["next"]).contains("Process: planned"));
    env.json(&dir, &["park", &id, "resume here"]);
    let parked = env.json(&dir, &["list", "--parked"]);
    assert_eq!(parked["tasks"][0]["process"], "planned");
    assert_eq!(parked["tasks"][0]["phase"], "implementing");
    assert_eq!(env.json(&dir, &["next"])["next"]["task"]["process"], "planned");
    assert!(env.pretty(&dir, &["list", "--parked"]).contains("planned"));
    env.json(&dir, &["edit", &id, "--no-process"]);
    assert!(env.pretty(&dir, &["show", &id]).contains("Process: unassessed"));
}

#[test]
fn process_missing_warns_only_while_doing() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "T"]));
    let missing = |v: &serde_json::Value| v["warnings"].as_array().unwrap()
        .iter().any(|w| w["kind"] == "process_missing");
    assert!(!missing(&env.json(&dir, &["check"])));
    env.json(&dir, &["start", &id]);
    assert!(missing(&env.json(&dir, &["check"])));
    env.json(&dir, &["edit", &id, "--process", "direct"]);
    assert!(!missing(&env.json(&dir, &["check"])));
    env.json(&dir, &["edit", &id, "--no-process"]);
    assert!(missing(&env.json(&dir, &["check"])));
    env.json(&dir, &["done", &id]);
    assert!(!missing(&env.json(&dir, &["check"])));
}
```

Extend coverage using the same fixture patterns: compare ready ids before and
after process edits with distinct priorities; verify an unassessed task remains
selectable/startable; inspect null with `get("process").unwrap()` so a missing
JSON key cannot pass as null. Cover a doing goal/step, quiet idea/shelved/dropped
records, and an unresolved parked row by extending the existing parked tests.
Use `editor_script` for a valid process edit and an invalid edit that leaves the
original bytes intact. No live registry or task-corpus mutations in tests.

Add optional process fields and direct copies in `TaskSummary::of`,
`ParkedRow::resolved`, and `ParkedRow::unresolved` (null for unresolved). The raw
`Task` supplies show/next JSON automatically. No changes to `ready_tasks`,
`Phase::of`, or claim routing are required.

Use `row.process.map(Process::as_str).unwrap_or("-")` as a seven-character column
beside complexity in `table` and `parked_table`; preserve pad-before-paint behavior.
`show_text` already renders stored frontmatter, so add the required explicit
summary outside the record text, including when the key is absent:

```rust
rendered.push_str(&format!("\nProcess: {}\n",
    o.task.process.map(Process::as_str).unwrap_or("unassessed")));
```

Add the warning beside `unrated_step`, with no hierarchy or status inference:

```rust
if task.status == Status::Doing && task.process.is_none() {
    warnings.push(finding(Some(task), file.clone(), "process_missing",
        "doing task without a process decision".into()));
}
```

- [x] **Verify, install, record the intended process, and commit.**

Run `cargo fmt`, then `python3 tools/tt test-fast -- cargo test process_` and
`just gate`. Existing exact pretty-output assertions may need the new column;
change expected text without weakening their alignment/color assertions.
Install with `cargo install --path .`. Use `tasks edit` to set this parent to
planned and these two plan children to direct. Run `tasks check`, close Task 1
with its actual outcome, and commit the code and task record together using
`feat: record and display explicit task process`. Keep the design status approved,
not implemented, until the policy/documentation piece lands.

### Task 2: Adopt and document the process policy

Task record: tasks-d37cf5 (intended process: direct; complexity: low).

**Files:** `skills/tasks/SKILL.md`, `skills/scope/SKILL.md`,
`skills/curate/SKILL.md`, `AGENTS.md`, `README.md`,
`docs/specs/2026-08-29-tasks-design.md`, this spec and plan, and CLI-managed task
records. Depends on Task 1. No shared skill or harness source edits in this task.

**Interfaces:** Consumes Task 1's installed flags and output contract. Produces
agent instructions that select the recorded path, preserve both written-document
review gates, and explain the separate global rollout tasks.

- [x] **Update the tasks protocol and its planning integration.**

Place this rule before implementation starts in the session protocol:

> Read process before implementation and state the chosen path and workspace.
> When it is unassessed, inspect the task and relevant code, set direct or planned
> explicitly, and note the reason. Direct executes the scoped task or reviewed
> plan without new design/plan documents. Planned obtains a reviewed written spec,
> then a reviewed implementation plan, before execution. Document links alone do
> not prove approval. Discovery outside a direct task's scope requires a note and
> reassessment to planned. Ideas still require scoping.

Add the flags to recording/editing guidance and explain doing-only warnings. Both
paths retain appropriate debugging, validation, and review skills. In the
writing-plans integration, use this concrete form for a settled plan step:

```sh
tasks add "Implement the process field" --parent tasks-61cc5c \
  --plan task-process --step "Task 1: Persist and expose the process contract" \
  --size m --complexity mid --process direct
```

This is documentation of command form, not a command to run and create a duplicate
of the already-filed child. A planner chooses both fields explicitly; process is
not inherited. Add equivalent choice criteria and `--process` to scoping and to
curation's allowed field edits. Do not change the curation pass's scope or gates.

- [x] **Adopt the policy in this repo and document the CLI.**

Add a repo instruction making the task's process the selector for creating new
design/plan artifacts, with generic brainstorming triggers subordinate to that
choice. Preserve the written spec and plan review gates on planned work. Require
an isolated task worktree for either code path, reusing an existing one on resume,
and preserve `git worktree add`, `.worktrees/`, setup, and explicit user overrides.

README and the base design reference gain the field, flags, JSON null behavior,
doing-only finding, and an example contrasting process with phase. Explain that
other projects must adopt the instruction policy and that ai-69ccac fixes global
worktree coverage without this CLI. Name ai-e8dcc5 for the shared planner update;
do not mark either follow-up done from this checkout.

- [x] **Check policy consistency and finish the local implementation.**

```sh
rg -n 'process|worktree|brainstorm|writing-plans|complexity' \
  AGENTS.md README.md skills/tasks/SKILL.md skills/scope/SKILL.md \
  skills/curate/SKILL.md docs/specs/2026-08-29-tasks-design.md
git diff --check
just check
```

Walk the three scenarios from the spec against the final prose: direct small fix
still isolated; planned change reaches both written reviews; missing choice is
recorded before implementation. Confirm both the tasks integration and the linked
ai follow-up specify `--complexity` plus `--process` on child creation. This is a
manual instruction review, not evidence of a completed live-agent rollout.

Mark the feature spec locally implemented when this piece lands, while retaining
the outstanding ai rollout references. Mark completed plan steps only after
checking their actual diff and test results. Close Task 2 in the docs commit with
`docs: adopt explicit task process policy`. Confirm the parent appears in prime's
closeout and close it with the local outcome, explicitly naming any outstanding
external follow-ups. Run `tasks check` before that commit as well.
