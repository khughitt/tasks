# Halt enforcement implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task by task. Track the checkboxes below. Read the spec before editing code.

**Goal:** Make open `halt` tasks stop lower-priority new starts across worktrees, expose the halt in entry views, and record justified overrides.

**Architecture:** The registered checkout supplies halt records when the project is registered; otherwise the local checkout does. A small shared halt module computes one per-project snapshot and the local allowed-work graph. `start` applies that decision under its existing mutation lock; `ready`, `next`, and `prime` use the same decision on their local candidate pools.

**Tech Stack:** Rust CLI with clap and serde; integration tests in `tests/cli.rs`; Python/TOML shared CLI inventory in ops.

**Spec:** `docs/specs/2026-09-30-halt-enforcement-design.md`. The ops contract is `docs/specs/2026-09-29-test-latency-escalation-design.md` §6 in the ops repository.

## Global Constraints

- Work in the existing locked tasks `.worktrees/halt-enforcement` worktree. For ops code, run `work-link --ensure .worktrees`, create a separate `.worktrees/` worktree with `git worktree add`, lock it, and run `just setup` if defined.
- Update ops `cli.toml` before any vendored `tools/cli.toml` copy. Park's `--reason` remains its fixed enum; start's `--reason` is free text and has no `complete::reason` completer.
- Recheck the separate record-home change before touching `open_id_write_ctx` or task routing. Keep a registered checkout as halt authority even if record-home lands.
- Keep `--force`'s claim takeover behavior. A registered but unreadable authority is fatal to `start`, regardless of `--force`; an unregistered checkout uses its local records.
- Use `just test-one <runner args>` for each focused Rust test, then `just test-fast` and `just check` before commit. Do not call cargo test directly.
- Preserve unhalted JSON by omitting empty `halts`. Ordinary `list` and `show` remain record queries. Use no new dependency or state file.
- Commit design and plan documents in `docs/specs/` and `docs/plans/` under the personal profile. Use conventional commits, without attribution trailers.

## Review Focus

- A registered root with a missing or malformed config must fail a start before a claim is saved; test in Task 2.
- A cycle or repeated edge in local parent/dependency links must terminate and yield one allowed-work result; test in Task 1.
- Equal-priority halts must produce stable metadata and allow a same-priority start; test in Tasks 2 and 3.
- A reason prepared for an override must be accepted with a warning when the halt disappears before `start`; test in Task 2.
- A claimed halt missing from the local worktree must still appear in read metadata; test in Task 3.

---

### Task 1: Read authoritative halts and compute allowed work

**Files:**
- Create: `src/halt.rs` (authority scan, graph, decision, metadata)
- Modify: `src/lib.rs` or `src/main.rs` to declare the module, matching this crate's existing module layout
- Test: `tests/cli.rs` (authority and graph cases)

**Interfaces:** `halt::snapshot(project: &Project, registry: &Registry, local: &[Task]) -> Result<HaltSnapshot>` reads the authority and retains sorted open halt records. `HaltSnapshot::allows(&self, target: &Task) -> bool` and `blocking(&self, target: &Task) -> Vec<&Task>` drive Task 2 and Task 3. `HaltSnapshot::halts() -> &[Task]` supplies sorted records for Task 3's output mapping. For read commands, wrap snapshot errors per project into one warning and an empty filter decision, while `start` propagates the error.

- [ ] **Step 1: Add failing predicate tests.** Add tests in `src/halt.rs` using parsed in-memory `Task` values for open versus closed statuses and the mixed child/dependency graph. Run `just test-one halt::tests`; expect failure until the module is implemented. Leave the end-to-end start cases for Task 2 so this task can commit with a green suite.
- [ ] **Step 2: Implement authority scan.** Use `registry.project_root(&project.prefix)` to distinguish no entry from an existing entry. For an entry, call `scope::open_registered` then `Project::scan`; otherwise use the supplied local tasks. Retain tasks with `tags` containing `halt` and `status.is_open()`, including idea, blocked, and shelved. Sort by `(priority, id)`; do not use a subprocess or a second state file.

  ```rust
  let authority = match registry.project_root(&project.prefix) {
      Some(_) => scope::open_registered(registry, &project.prefix, Origin::Prefix)?.scan()?,
      None => local.to_vec(),
  };
  ```

- [ ] **Step 3: Implement the local graph.** Start each walk at the authoritative halt ID. For every visited local task, follow its `depends` edges and find local children whose `parent` matches that ID. Use a `HashSet<TaskId>` visited set, so cycles and duplicate edges terminate. A task in another prefix is never exempted by this graph; its own project's snapshot decides its start. A local-only child of a halt can enter the walk even when the halt record itself is absent locally.

  ```rust
  while let Some(id) = pending.pop() {
      if !visited.insert(id.clone()) { continue; }
      if let Some(task) = local.iter().find(|task| task.id == id) {
          pending.extend(task.depends.iter().filter(|dep| dep.prefix == id.prefix).cloned());
      }
      pending.extend(local.iter().filter(|task| task.parent.as_ref() == Some(&id)).map(|task| task.id.clone()));
  }
  ```

- [ ] **Step 4: Pin graph and status behavior.** Extend the focused module tests for all open statuses versus done/dropped, deferred halt, a subtask's dependency, a dependency's subtask, local-only remedy child, cross-project dependency, repeated links/cycle, and accepted local `priority`, `parent`, and `dep` edits. Run `just test-one halt::tests`; expect pass.
- [ ] **Step 5: Commit.** Run `just test-fast`, then commit the module and focused tests with `feat: read authoritative project halts`. If the source module layout differs from the interface sketch, update this plan's interface names before Task 3.

### Task 2: Add CLI vocabulary, enforce starts, and record overrides

**Files:**
- Modify: ops `cli.toml` and registered projects' clean vendored `tools/cli.toml` copies
- Modify: `src/cli.rs`, `src/commands/mod.rs`, `src/commands/status.rs`, `src/error.rs`, `tools/cli.toml`
- Test: ops `tests/test_cli.py`; tasks `tests/cli.rs`, `src/surface.rs`

**Interfaces:** `Command::Start { id, force, reason: Option<String> }` dispatches to `status::start(ctx, id, force, reason)`. Use Task 2's `snapshot` and `blocking`. Add `Error::Halted(String)` mapped to kind `halted`.

- [ ] **Step 1: Add failing CLI tests.** Use `repo_with_worktree` and `TestEnv::init` to assert an uncommitted main halt blocks a side start immediately; closing only the side's halt copy does not lift it; an unreadable registered root leaves task and claim unchanged; an unregistered clone starts normally. Also assert lower-priority todo, blocked, and recurring done starts get `halted`; already doing resumes; priority numerically equal to the most urgent halt starts; work for any halt is allowed even when another halt is more urgent; multiple blockers are named in priority/id order. Assert `--reason` without `--force`, blank reason under a halt, and `--force` without reason under a halt fail before task or claim changes. Run `just test-one --test cli halt_start`; expect failure.
- [ ] **Step 2: Change ops's source first, then this worktree's parser and copy.** Add `{ names = ["--reason"], value = "string" },` beside `force` on the ops `tasks start` row; leave park's enum row unchanged. Run ops `just test-one test_cli`. Copy that revised source into this tasks worktree's `tools/cli.toml`, then add a free-text `Option<String>` with `#[arg(long)]` to `Command::Start` and pass it through dispatch. Add `Halted(String)` to `Error`, `with_suffix`, and `kind`. Run tasks `just test-one surface`; expect pass. Keep the ops source change in its worktree until Task 4 publishes copies after tasks main receives the parser.

  ```rust
  #[arg(long)]
  reason: Option<String>,
  // Error::kind(): Error::Halted(_) => "halted"
  ```

- [ ] **Step 3: Guard the transition under the existing lock.** In `status::start`, load the target locally first and keep its not-found and shelved errors. Scan local tasks, read the authoritative snapshot while the lock is held, then decide: an already doing target resumes; an open halt, graph member, or priority `<=` the most urgent halt starts; otherwise return `Halted` with blocker IDs and an override command. Do not add fallback routing for a halt absent from the worktree. Validate `reason` with the existing single-line validator plus `trim().is_empty()`.

  ```rust
  if reason.is_some() && !force { return Err(Error::Validation("--reason requires --force".into())); }
  if force && !halts.is_empty() && reason.as_deref().is_none_or(|r| r.trim().is_empty()) {
      return Err(Error::Validation("--force under a halt requires --reason".into()));
  }
  ```

- [ ] **Step 4: Write audit notes before the target save.** Resolve the current session through existing claim identity, and use `owner_name` for note authorship. For a blocked forced start, append `halt override: attempted <target> by <session>: <reason>` to each blocking halt in the registered authority and save each; then append `halt override: started past <halt ids> by <session>: <reason>` to the target before `transition`/`save`. If a later save fails, attempted notes remain. For an allowed forced takeover, include its reason in the existing takeover note; otherwise add one unused-reason warning. Do not alter `claim_guard` takeover semantics.
- [ ] **Step 5: Verify notes and failure ordering.** Tests inspect both sides' task records and JSON error kind; include missing halt in local worktree, an injected failure after attempted notes, and a halt that lifted after a prior read. Run `just test-one --test cli halt_override`; expect pass. Run `just test-fast` and commit with `feat: enforce halt on task start`.

### Task 3: Expose halts and filter entry views

**Files:**
- Modify: `src/commands/list.rs`, `src/output.rs`
- Test: `tests/cli.rs`

**Interfaces:** Add `HaltRow { id, title, owner, priority }` and optional `halts: Vec<HaltRow>` to `NextOut`, `ListOut`, and `PrimeOut`, serialized only when nonempty. Task 1's per-project snapshot provides rows and eligibility. Keep other `ListOut` producers' `halts` empty.

- [ ] **Step 1: Add failing view tests.** In a registered main/side pair, assert `prime`, `ready`, and `next` show the uncommitted main halt in JSON and pretty output begins with `halt:`. If the only local ready task is hidden and the halt is absent locally, assert `next: null`, the halt row, and exactly one hidden-count warning. Assert claimed, shelved, and deferred halts remain in metadata; unhalted output has no `halts` key. Run `just test-one --test cli halt_views`; expect failure.
- [ ] **Step 2: Extend output structs and pretty rendering.** Add `halts` with `#[serde(skip_serializing_if = "Vec::is_empty")]`. Prepend one `halt:` line listing all sorted halts and allowed work to the existing `Next`, `List`, and `Prime` pretty text; when the halt is absent locally, say to start it from the registered checkout. Do not alter ordinary list/show output or their JSON shape.

  ```rust
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub halts: Vec<HaltRow>,
  ```

- [ ] **Step 3: Filter at each picker's final candidate pool.** `ready` filters `picked.tasks`; `next` filters its deduplicated parked-plus-ready pool; `prime` filters its `ready` vector. Apply the same `HaltSnapshot::allows` decision only to new starts and emit one warning with the hidden count per command. Keep locally present eligible halt tasks in their existing ready order. Read one authority snapshot per project, keyed by prefix; for `--all-projects`, a halt filters only tasks of that prefix. If an authority scan fails, warn that the halt state is unknown and keep ordinary rows; propagate no read error.
- [ ] **Step 4: Verify scope and errors.** Add tests with two registered scratch projects under `--all-projects`; one halt must not filter the other's rows. Make one registered root unreadable and assert the read warning and ordinary rows; an unreachable project remains skipped with an unknown-state warning. Assert the metadata still contains a halt already claimed in another checkout. Run `just test-one --test cli halt_views`, then `just test-fast` and `just check`; commit with `feat: show halts in task entry views`.

### Task 4: Document and verify the integrated contract

**Files:**
- Modify: `skills/tasks/SKILL.md` and the CLI usage section in `README.md` if it documents `start`
- Test: `tests/cli.rs` and `src/surface.rs`

**Interfaces:** No new code interface. The published CLI behavior and the ops §6 contract must agree.

- [ ] **Step 1: Update the task skill and user-facing help.** Document that open `halt` tasks block lower-priority new starts, how to find allowed work in `prime`/`ready`/`next`, and `tasks start <id> --force --reason "..."` for an audited override. Keep park's enum reason distinct from start's free text.
- [ ] **Step 2: Verify the two ops contract assertions verbatim in tests.** Name tests for: (1) an uncommitted incident in the registered checkout refuses a start from an existing linked worktree at once; (2) a halt closed in a worktree does not lift until the registered checkout's record is closed. Recheck that both assertions inspect claims and task status, not just the command exit code.
- [ ] **Step 3: Run gates and inspect changes.** Run `just test-one --test cli halt_`, `just test-one surface`, `just test-fast`, and `just check` in the tasks worktree. Run ops `just test-one test_cli`, `just test-fast`, and `just check` for the source/vendored inventory. Inspect `git diff --check` and both worktrees' status. Record any known unrelated gate drift separately; do not describe a blocked gate as passing.
- [ ] **Step 4: Commit and hand off.** Commit docs and any final integration corrections with `docs: explain halt enforcement`, then request an implementation review. After review acceptance, integrate tasks according to repository instructions. Once tasks main contains the parser and its revised `tools/cli.toml`, inspect registered vendor destinations for unrelated edits; run ops `python3 bin/vendored publish --force` from its worktree, commit the ops source with `feat: add start reason to cli inventory`, and commit clean vendor copies in their owning repositories. Recheck ops `bin/vendored check` and name any remaining unrelated drift. Do not activate the ops latency timer until this task and its companion hook task are complete.
