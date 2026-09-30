# Halt enforcement implementation plan

**Status:** revised for plan review round 2, 2026-09-30.

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

- A registered root with a missing or malformed config must fail a start before a claim is saved, even with `--force --reason`; test in Task 1.
- A cycle or repeated edge in local parent/dependency links must terminate and yield one allowed-work result; test in Task 1.
- Equal-priority halts must produce stable metadata and allow a same-priority start; test in Tasks 1 and 2.
- A reason prepared for an override must be accepted with a warning when the halt disappears before `start`; test in Task 1.
- A claimed halt missing from the local worktree must still appear in read metadata; test in Task 2.

---

### Task 1: Enforce starts from authoritative halts

**Files:**
- Create: `src/halt.rs` (authority scan, graph, decision)
- Modify: `src/main.rs`, `src/cli.rs`, `src/commands/mod.rs`, `src/commands/status.rs`, `src/error.rs`, `tools/cli.toml`
- Modify: ops `cli.toml` in a separate locked ops worktree; leave it uncommitted until Task 3 publishes the copies
- Test: ops `tests/test_cli.py`; tasks `src/halt.rs`, `tests/cli.rs`, `src/surface.rs`

**Interfaces:** `halt::snapshot(project: &Project, registry: &Registry, local: &[Task]) -> Result<HaltSnapshot>` reads the authority and retains sorted open halt records and its authority `Project`. `HaltSnapshot::allows(&self, target: &Task) -> bool` permits an already doing task to resume, then applies the halt graph and priority rule to new starts; `blocking(&self, target: &Task) -> Vec<&Task>` returns the more urgent halts. Both drive this task and Task 2. `HaltSnapshot::halts() -> &[Task]` supplies sorted records for Task 2's output mapping; `authority() -> &Project` supplies the note-write destination. `Command::Start { id, force, reason: Option<String> }` dispatches to `status::start(ctx, id, force, reason)`. Add `Error::Halted(String)` mapped to kind `halted`.

- [ ] **Step 1: Add failing predicate tests.** Add tests in `src/halt.rs` using parsed in-memory `Task` values for open versus closed statuses and the mixed child/dependency graph. Run `just test-one halt::tests`; expect failure until the module is implemented. This binary crate declares the module in `src/main.rs`; do not commit it until `start` uses its functions in Step 7, or clippy's dead-code check fails.
- [ ] **Step 2: Implement authority scan.** Use `registry.project_root(&project.prefix)` to distinguish no entry from an existing entry. For an entry, call `scope::open_registered` then `Project::scan`; otherwise use the supplied local tasks. Retain tasks with `tags` containing `halt` and `status.is_open()`, including idea, blocked, and shelved. Sort by `(priority, id)`; do not use a subprocess or a second state file.

  ```rust
  let authority = match registry.project_root(&project.prefix) {
      Some(_) => scope::open_registered(registry, &project.prefix, Origin::Prefix)?.scan()?,
      None => local.to_vec(),
  };
  ```

- [ ] **Step 3: Implement the local graph.** Seed the walk with the authority halt ID **and that authority record's dependencies**. For later nodes, follow `depends` from their local copies and find local children whose `parent` matches the visited ID. Never substitute a stale local halt's dependency list for the authority root's. Use a `HashSet<TaskId>` visited set, so cycles and duplicate edges terminate. A task in another prefix is never exempted by this graph; its own project's snapshot decides its start. A local-only child of a halt can enter the walk even when the halt record itself is absent locally.

  ```rust
  let mut pending = vec![halt.id.clone()];
  pending.extend(halt.depends.iter().filter(|dep| dep.prefix == halt.id.prefix).cloned());
  while let Some(id) = pending.pop() {
      if !visited.insert(id.clone()) { continue; }
      if id != halt.id {
          if let Some(task) = local.iter().find(|task| task.id == id) {
              pending.extend(task.depends.iter().filter(|dep| dep.prefix == id.prefix).cloned());
          }
      }
      pending.extend(local.iter().filter(|task| task.parent.as_ref() == Some(&id)).map(|task| task.id.clone()));
  }
  ```

- [ ] **Step 4: Pin graph and status behavior.** Extend the focused module tests for all open statuses versus done/dropped, deferred halt, a subtask's dependency, a dependency's subtask, authority-root dependencies with the halt absent locally or stale locally, local-only remedy child, cross-project dependency, repeated links/cycle, and accepted local `priority`, `parent`, and `dep` edits. Run `just test-one halt::tests`; expect pass.
- [ ] **Step 5: Add failing CLI tests.** Use `repo_with_worktree` and `TestEnv::init` to assert an uncommitted main halt blocks a side start immediately; closing only the side's halt copy does not lift it; an unreadable registered root leaves task and claim unchanged even with `--force --reason`; an unregistered clone starts normally. Also assert lower-priority todo, blocked, and recurring done starts get `halted`; already doing resumes; priority numerically equal to the most urgent halt starts; work for any halt is allowed even when another halt is more urgent; multiple blockers are named in priority/id order. Assert `--reason` without `--force`, blank reason under a halt, `--force` without reason under a halt, and multiline reason on an unhalted takeover fail before task or claim changes. Run `just test-one --test cli halt_start`; expect failure.
- [ ] **Step 6: Change ops's source first, then this worktree's parser and copy.** Add `{ names = ["--reason"], value = "string" },` beside `force` on the ops `tasks start` row; leave park's enum row unchanged. Run ops `just test-one test_cli`. Copy that revised source into this tasks worktree's `tools/cli.toml`, then add a free-text `Option<String>` with `#[arg(long)]` to `Command::Start` and pass it through dispatch. Add `Halted(String)` to `Error`, `with_suffix`, and `kind`. Run tasks `just test-one surface`; expect pass. Keep the ops source change in its worktree until Task 3 publishes copies after tasks main receives the parser.

  ```rust
  #[arg(long)]
  reason: Option<String>,
  // Error::kind(): Error::Halted(_) => "halted"
  ```

- [ ] **Step 7: Guard the transition under the existing lock.** In `status::start`, load the target locally first and keep its not-found and shelved errors. Scan local tasks, read the authoritative snapshot while the lock is held, then decide: an already doing target resumes; an open halt, graph member, or priority `<=` the most urgent halt starts; otherwise return `Halted` with blocker IDs and an override command. Do not add fallback routing for a halt absent from the worktree. Whenever `reason` is present, run the existing single-line validator; under a halt, require it to be nonblank for every `--force`, including resumes. Run `transition` before any note is persisted so invalid transitions and claim-identity failures leave no attempted note.

  ```rust
  if reason.is_some() && !force { return Err(Error::Validation("--reason requires --force".into())); }
  if let Some(reason) = reason.as_deref() { crate::format::validate_line("reason", reason)?; }
  if force && !snapshot.halts().is_empty() && reason.as_deref().is_none_or(|r| r.trim().is_empty()) {
      return Err(Error::Validation("--force under a halt requires --reason".into()));
  }
  ```

- [ ] **Step 8: Write audit notes after transition but before the target save.** `transition` prepares `ctx.pending_claim` in memory; take the session from its `ClaimIntent::Acquire` and use `owner_name` for note authorship. Set the target owner, then preflight `format::validate_task`, `Project::validate_docs`, and `hierarchy::{validate_parent, validate_periodic, validate_defer}` on the target, so predictable validation failures leave no halt note. For a blocked forced start, append `halt override: attempted <target> by <session>: <reason>` to each blocking halt loaded from the authority snapshot, bump its `updated` stamp, validate it, and write it through `snapshot.authority().write_task(&ctx.registry, &halt)`. Do **not** call `save(&mut ctx, &mut halt)`: that writes to the caller's worktree and consumes its prepared claim. Then append `halt override: started past <halt ids> by <session>: <reason>` to the target and call `save(&mut ctx, &mut task)` once. If that later write fails, attempted notes remain but the previous claim is restored by `save`. For an allowed forced takeover, include its reason in the existing takeover note; otherwise add one unused-reason warning. Do not alter `claim_guard` takeover semantics.

  ```rust
  transition(&mut ctx, &mut task, Status::Doing, force)?;
  task.owner = Some(owner.clone());
  crate::format::validate_task(&task)?;
  ctx.project.validate_docs(&task)?;
  crate::hierarchy::validate_parent(&ctx.project, &ctx.registry, &task)?;
  crate::hierarchy::validate_periodic(&ctx.project, &ctx.registry, &task)?;
  crate::hierarchy::validate_defer(&ctx.project, &ctx.registry, &task)?;
  let session = match &ctx.pending_claim {
      Some((_, ClaimIntent::Acquire(claim))) => claim.session.clone(),
      _ => unreachable!("start prepared an acquire claim"),
  };
  let reason = reason.as_deref().expect("override reason was validated");
  let halt_ids = blockers.iter().map(|halt| halt.id.to_string()).collect::<Vec<_>>().join(", ");
  for mut halt in blockers.into_iter().cloned() {
      append_note(&mut halt, &owner, &format!(
          "halt override: attempted {} by {session}: {reason}", task.id
      ))?;
      halt.updated = crate::time::now();
      crate::format::validate_task(&halt)?;
      snapshot.authority().validate_docs(&halt)?;
      // Append-only audit note: this direct authority write skips save's newer-sibling warning.
      snapshot.authority().write_task(&ctx.registry, &halt)?;
  }
  append_note(&mut task, &owner, &format!(
      "halt override: started past {halt_ids} by {session}: {reason}"
  ))?;
  save(&mut ctx, &mut task)?;
  ```

- [ ] **Step 9: Verify notes and failure ordering.** Tests inspect both sides' task records and JSON error kind; include missing halt in local worktree, invalid transition and identity failure leaving no attempted note, and a halt that lifted after a prior read. For the later-write failure, use a linked worktree fixture, make only its `tasks/` directory read-only after setup, run the override as a non-root test user, restore permissions in a cleanup guard, and assert the main checkout has attempted notes while the side target write fails and `save` restores the previous claim. Run `just test-one --test cli halt_override`; expect pass. Run `just test-fast` and `just check`, then commit the tasks module, parser, vendor copy, start guard, and focused tests together with `feat: enforce authoritative project halts`. The ops source stays uncommitted in its worktree until Task 3's rollout.

### Task 2: Expose halts and filter entry views

**Files:**
- Modify: `src/commands/list.rs`, `src/output.rs`
- Test: `tests/cli.rs`

**Interfaces:** Add `HaltRow { id, title, owner, priority }` and optional `halts: Vec<HaltRow>` to `NextOut`, `ListOut`, and `PrimeOut`, serialized only when nonempty. Task 1's per-project snapshot provides rows and eligibility. Keep other `ListOut` producers' `halts` empty. In `list.rs`, use one `retain_allowed(tasks: &mut Vec<Task>, snapshots: &HashMap<String, HaltSnapshot>) -> usize` helper for the three pickers.

- [ ] **Step 1: Add failing view tests.** In a registered main/side pair, assert `prime`, `ready`, and `next` show the uncommitted main halt in JSON and pretty output begins with `halt:`. If the only local ready task is hidden and the halt is absent locally, assert `next: null`, the halt row, and exactly one hidden-count warning. Assert claimed, shelved, and deferred halts remain in metadata; unhalted output has no `halts` key. Run `just test-one --test cli halt_views`; expect failure.
- [ ] **Step 2: Extend output structs and pretty rendering.** Add `halts` with `#[serde(skip_serializing_if = "Vec::is_empty")]`. Prepend one `halt:` line listing all sorted halts and allowed work to the existing `Next`, `List`, and `Prime` pretty text; when the halt is absent locally, say to start it from the registered checkout. Do not alter ordinary list/show output or their JSON shape.

  ```rust
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub halts: Vec<HaltRow>,
  ```

- [ ] **Step 3: Filter with one helper at the picker call sites.** Each picker already calls `ready_tasks`; keep that shared candidate construction and add one `retain_allowed` helper. In `ready`, call it on `picked.tasks` before complexity, size/parallel, and `--limit`. In `next`, merge parked candidates with ready tasks and deduplicate, then call it once on that pool before complexity; a parked `todo` is a new start and can be hidden, while a parked `doing` task is a resume and remains eligible. In `prime`, call it on the ready vector before complexity. Emit one warning with the count returned by the helper per command, so the count measures only halt filtering. Keep locally present eligible halt tasks in their existing ready order. Read one authority snapshot per project, keyed by prefix; for `--all-projects`, a halt filters only tasks of that prefix. If an authority scan fails, warn that the halt state is unknown and keep ordinary rows; propagate no read error.

  ```rust
  fn retain_allowed(tasks: &mut Vec<Task>, snapshots: &HashMap<String, HaltSnapshot>) -> usize {
      let before = tasks.len();
      tasks.retain(|task| {
          snapshots.get(&task.id.prefix).is_none_or(|halt| halt.allows(task))
      });
      before - tasks.len()
  }
  ```
- [ ] **Step 4: Verify scope and errors.** Add tests with two registered scratch projects under `--all-projects`; one halt must not filter the other's rows. Make one registered root unreadable and assert the read warning and ordinary rows; an unreachable project remains skipped with an unknown-state warning. Assert the metadata still contains a halt already claimed in another checkout. Run `just test-one --test cli halt_views`, then `just test-fast` and `just check`; commit with `feat: show halts in task entry views`.

### Task 3: Document, verify, and publish the integrated contract

**Files:**
- Modify: `skills/tasks/SKILL.md` and the CLI usage section in `README.md` if it documents `start`
- Test: `tests/cli.rs` and `src/surface.rs`

**Interfaces:** No new code interface. The published CLI behavior and the ops §6 contract must agree.

- [ ] **Step 1: Update the task skill and user-facing help.** Document that open `halt` tasks block lower-priority new starts, how to find allowed work in `prime`/`ready`/`next`, and `tasks start <id> --force --reason "..."` for an audited override. Keep park's enum reason distinct from start's free text.
- [ ] **Step 2: Verify the two ops contract assertions verbatim in tests.** Name tests for: (1) an uncommitted incident in the registered checkout refuses a start from an existing linked worktree at once; (2) a halt closed in a worktree does not lift until the registered checkout's record is closed. Recheck that both assertions inspect claims and task status, not just the command exit code.
- [ ] **Step 3: Run tasks gates and inspect changes.** Run `just test-one --test cli halt_`, `just test-one surface`, `just test-fast`, and `just check` in the tasks worktree. Inspect `git diff --check` and both worktrees' status. Commit docs and any final integration corrections with `docs: explain halt enforcement`, then request an implementation review. Do not run ops `just check` yet: its `check-vendored` compares the edited worktree source to every registered main checkout and must fail until publication.
- [ ] **Step 4: Integrate tasks, publish, then commit ops.** After implementation review acceptance, integrate tasks into main according to its repository instructions, ensuring main now has both the parser and revised `tools/cli.toml`. Inspect each registered vendor destination for unrelated edits; from the ops worktree run `just vendor-cli --force`, which publishes `cli.toml` and `cli_surface.py` from its unmerged branch. Then run ops `just test-one test_cli`, `just test-fast`, and `just check`; name any unrelated drift separately. Commit the ops source with `feat: add start reason to cli inventory` and commit clean vendor copies in their owning repositories. Record every vendor-copy commit outside tasks and ops for the rollout report. **Publish before committing the ops source:** ops pre-commit runs `vendored pre-commit` against the staged source and refuses it while copies are stale.
- [ ] **Step 5: Merge and recheck ops main.** Fast-forward the ops branch into main; if main has moved, rebase the branch first, then fast-forward. Run `just check-vendored` from ops main; the copies must now match a merged source. Run `tt-report` in the ops worktree, check that no host pointer resolves into it, unlock and remove that worktree, and delete its merged branch. Keep the ops latency timer inactive until this task and its companion hook task are complete.
