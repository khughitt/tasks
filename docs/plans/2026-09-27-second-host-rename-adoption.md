# Second-host rename adoption implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a host adopt a prefix rename already present in a synced checkout, carrying its local state and preserving old IDs as aliases.

**Architecture:** `tasks rename old new --adopt` routes to a separate adoption path. It reads the local checkout, locks both prefix stores and the registry, classifies pre-registry and post-registry states, then writes only the target store, registry, and old-store cleanup. Ordinary rename stays on its current path.

**Tech Stack:** Rust, clap, TOML, existing `Registry`, `ClaimStore`, `Project`, and `tests/cli.rs` harness. No new dependency.

**Spec:** `docs/specs/2026-09-27-second-host-rename-adoption-design.md`

## Global constraints

- Do not write the synced checkout's `tasks/.config.toml` or task files.
- Both `XDG_CONFIG_HOME` and `XDG_STATE_HOME` must be isolated in every two-host test.
- Keep `claims/old.lock`; only remove `claims/old.toml`.
- Preserve ordinary rename's JSON and behavior. Add `mode: "adopt"` only for adoption.
- Run checks via `just test-fast`, `just check`, and `just gate`, never direct `cargo test`.

## Review focus

1. A `start` after registry save consumes a carried park: cleanup succeeds and preserves the new claim (Task 3).
2. `init --prefix new --force` ran before adoption: both registry keys converge to one live key (Task 1 and Task 3).
3. An orphaned old park has no matching synced task: it survives with a warning and appears unresolved in `list --parked` (Task 3).
4. A stale old root path remains as an empty directory: adoption succeeds; a different root with `tasks/.config.toml` refuses (Task 2).
5. A live old claim cannot be released through the missing checkout: adoption names the owner and requires the claim to become stale before retry (Task 2 and Task 3).
6. Local `start` consumes a carried park after the `store` stop but before the registry switch: retry accepts the remaining subset and preserves target claims (Task 3).

---

### Task 1: Prepare the registry transition and rename output

Task record: `tasks-54df5e`. Start it before code changes.

**Files:** Modify `src/output.rs`, `src/registry.rs`, `src/rename/mod.rs`; test `src/registry.rs` and `tests/cli.rs`.

**Interfaces:** Add `Registry::adopt(source, target, root)` to handle both a free target and the partial `init --force` state. Add optional `RenameOut.mode`; ordinary rename sets it to `None`.

- [ ] **Step 1: Write failing registry and output tests.** In `src/registry.rs`, cover `adopt("old", "new", root)` when `new` is free and when `new` is already registered at `root`; in both cases assert `projects` has only `new`, `old -> new`, and any `older -> old` alias becomes `older -> new`. Assert a target at another root and an unrelated live key at `root` refuse. Register `new` through a non-canonical path to the same root and assert it is accepted. In `tests/cli.rs`, assert ordinary rename output omits `mode`.

  ```rust
  assert_eq!(registry.project_root("new"), Some(root));
  assert!(registry.project_root("old").is_none());
  assert_eq!(registry.canonical_prefix("older"), "new");
  assert!(env.json(&dir, &["rename", "dot", "dots"])["mode"].is_null());
  ```
- [ ] **Step 2: Run `just test-fast adopt` and confirm the new tests fail.** Existing ordinary rename tests may run separately with `just test-fast rename`.
- [ ] **Step 3: Implement the registry operation and optional output field.** `Registry::adopt` checks target validity and collisions before mutating. Compare all roots through `rename::root_identity`, including an already-live target and any other live key. If target is free, call `rename`, then `repoint`; if already live at the same root, remove only `source`, retarget aliases naming `source`, and insert `source -> target`. Preserve the existing `Registry::load_from` invariants. Add `#[serde(skip_serializing_if = "Option::is_none")] mode: Option<String>` to `RenameOut`; ordinary rename sets `None`.

  ```rust
  // Only after all conflict checks succeed, inside Registry::adopt:
  if !self.projects.contains_key(target) {
      self.rename(source, target)?;
  } else {
      self.projects.remove(source);
      for live in self.aliases.values_mut() {
          if live == source { *live = target.into(); }
      }
      self.aliases.insert(source.into(), target.into());
  }
  self.repoint(target, root)?;
  ```
- [ ] **Step 4: Run `just test-fast adopt` and `just test-fast rename`; fix only observed failures.**
- [ ] **Step 5: Complete and commit:** `tasks done tasks-54df5e 'Registry transition and output field landed'`, then `git add src/output.rs src/registry.rs src/rename/mod.rs tests/cli.rs tasks/tasks-54df5e.md && git commit -m 'feat(tasks): prepare rename adoption registry transition'`.

### Task 2: Classify and execute adoption under locks

Task record: `tasks-06abc9`, dependent on Task 1. Start it before code changes.

**Files:** Create `src/rename/adopt.rs`; modify `src/rename/mod.rs`, `src/commands/rename.rs`, `src/cli.rs`, `src/commands/mod.rs`, `src/claims.rs`; test `tests/cli.rs`.

**Interfaces:** `commands::rename::run(dir, old, new, explain, adopt)` dispatches to ordinary rename or `rename::adopt::run(registry: &mut Registry, project: &Project, old: &str, new: &str, explain: bool) -> Result<RenameOut>`. `commands::rename` acquires old/new mutation locks and the registry lock for mutation; `--explain` calls without locks. Adoption returns `fresh`, `resume_registry`, `resume_cleanup`, `complete`, or `refuse` in `RenameOut.recovery`.

- [ ] **Step 1: Write one failing CLI test for each preflight refusal.** First assert `rename --help` advertises `--adopt`. Use `TestEnv` with explicit scratch `XDG_CONFIG_HOME` and `XDG_STATE_HOME`: older alias as `old`, config/file prefix mismatch, pending ordinary rename inventory, target store conflict, live old claim, target registered elsewhere, and an old root at another location that still has `tasks/.config.toml`. Assert `--explain` reports `refuse` without creating lock/store files or changing checkout bytes. Assert an empty old directory does not refuse. Add a target-store test with a stale claim and no parks: adoption must preserve that claim, even when it writes carried parks.

  ```rust
  let explained = env.json(&dir, &["rename", "old", "new", "--adopt", "--explain"]);
  assert_eq!(explained["mode"], "adopt");
  assert_eq!(explained["recovery"], "refuse");
  assert_eq!(std::fs::read(&config_path).unwrap(), config_before);
  ```
- [ ] **Step 2: Run `just test-fast adopt` and confirm failures.**
- [ ] **Step 3: Wire the command and implement observation.** Add `#[arg(long)] adopt: bool` to `Command::Rename` and pass global `dir`. Locate the project from `dir`/cwd with `Project::locate`; never call ordinary `invocation()`, which resolves the old registry root. Read task paths using `inventory::task_paths` and `inventory::validate_task_file`; use the parsed task's ID prefix instead of unwrapping a filename stem. Call existing `reject_pending_rename_at(Some(&project.root), old)` and again for `new`. Validate registry state and target store before any write. Reuse `ClaimStore::carried_renamed_text`; reject live claims before registry save, discard stale claims, and warn for carried IDs absent from the synced task set. Target parks/escalations must be a value-equal subset of the carried entries. A target entry absent from the carried set is a conflict.

  ```rust
  let project = Project::locate(dir.unwrap_or(&std::env::current_dir()?))?;
  super::reject_pending_rename_at(Some(&project.root), old)?;
  super::reject_pending_rename_at(Some(&project.root), new)?;
  for path in inventory::task_paths(&project.tasks_dir())? {
      let text = std::fs::read_to_string(&path)?;
      inventory::validate_task_file(&path, &text)?;
      let task = crate::format::parse_task(&text, &path.display().to_string())?;
      if task.id.prefix != new { return Err(Error::Validation(format!("foreign task {}", task.id))); }
  }
  ```
- [ ] **Step 4: Implement the three writes and stage classification.** If the target has none of the carried parks/escalations and there is state to carry, write their bytes merged with target claims, then verify. If the target already has a matching subset, skip the write and continue to `Registry::adopt` and `save`; missing entries may have been consumed by local commands. Add a small `ClaimStore` method that serializes carried parks/escalations with destination claims and refuses an ID collision. Then remove only `claims/old.toml`. Reuse `stop_after` for `store`, `registry`, and `claims`. Once the registry is settled, check only that target state exists and parses if the source carried parks or escalations; accept new claims or consumed parks. With no carried state, skip `store` and never emit `resume_registry`. `--explain` classifies but does not authorize or write. Return `tasks: 0` and `mode: "adopt"`.

  ```rust
  atomic_write(&target_path, carried.as_bytes())?;
  if stop_after("store") { return Ok(out); }
  registry.adopt(old, new, &project.root)?;
  registry.save()?;
  if stop_after("registry") { return Ok(out); }
  std::fs::remove_file(&old_path)?;
  if stop_after("claims") { return Ok(out); }
  ```

  Handle absent `old_path` as already cleaned up, as ordinary rename does. The code above is the mutation order; the stage classifier skips completed steps.
- [ ] **Step 5: Run `just test-fast adopt` and `just check`; fix only observed failures.**
- [ ] **Step 6: Complete and commit:** `tasks done tasks-06abc9 'Adoption state machine landed'`, then `git add src/rename/adopt.rs src/rename/mod.rs src/commands/rename.rs src/cli.rs src/commands/mod.rs src/claims.rs tests/cli.rs tasks/tasks-06abc9.md && git commit -m 'feat(tasks): adopt synced renames without checkout writes'`.

### Task 3: Prove two-host recovery and document the command

Task record: `tasks-10afde`, dependent on Task 2. Start it before code changes.

**Files:** Modify `tests/cli.rs`, `README.md`, `skills/tasks/SKILL.md`.

**Interfaces:** Uses the finished `tasks rename old new --adopt` command; no new runtime interface.

- [ ] **Step 1: Write the two-host test with one checkout and two config/state pairs.** Create the checkout under a scratch parent; host A and B each get separate `XDG_CONFIG_HOME` and `XDG_STATE_HOME`. Before host A renames, register `old` on B, park a task on B, and record B's registry/store bytes. Host A runs ordinary `rename`, moves the checkout to a new path, and runs `init --prefix new --force`; B adopts from the moved root. Assert `show old-<hex>` resolves to the new task, the park and escalation have new keys, B's registry has only live `new` plus aliases, and checkout bytes are unchanged by B. Repeat B after first running the partial `init --force` workaround.

  ```rust
  let adopted = host_b_cmd(&moved_root)
      .args(["rename", "old", "new", "--adopt"])
      .output().unwrap();
  assert!(adopted.status.success(), "{adopted:?}");
  assert_eq!(std::fs::read(&synced_task).unwrap(), task_before_adoption);
  ```

  Define `host_b_cmd` as a local test closure wrapping `env_b.cmd` and setting both XDG variables; do the same for host A. Keep the parent temp directory alive while moving its nested checkout.
- [ ] **Step 2: Test each stop boundary and local writes on both sides of the switch.** For `TASKS_RENAME_STOP_AFTER=store`, `registry`, and `claims`, assert the next `--explain` verdict is `resume_registry`, `resume_cleanup`, and `complete` respectively, then rerun adoption. After `store`, run `tasks start new-<hex>` from the synced checkout to consume the carried park, then retry: it must accept the target subset, preserve the new claim, and settle the registry. Repeat after `registry` before cleanup. With no carried state, assert `store` is skipped and `resume_registry` never appears. Test an orphaned park reaches `list --parked` as unresolved with the adoption warning. For a live old claim, assert `tasks claims` lists it despite the missing root, then rewrite its `seen` to more than four hours ago with no PID proof and retry; adoption must then discard it and succeed.

  ```rust
  assert_eq!(explained["recovery"], expected_stage);
  assert_eq!(resumed["mode"], "adopt");
  assert_eq!(host_b_json(&moved_root, &["show", &new_id])["claim"]["live"], true);
  ```
- [ ] **Step 3: Run `just test-fast adopt` and read the results.** Fix only concrete failures. Add a short README example and the same operational instruction to `skills/tasks/SKILL.md`: run adoption from the renamed checkout before starting work there, use scratch config **and** state for rehearsal, and explain the live-claim refusal.
- [ ] **Step 4: Run `just gate`, `tasks check`, and inspect `git diff --check`.** The gate includes the full test suite and clippy. Stop testing after it passes.
- [ ] **Step 5: Reinstall the changed CLI with `cargo install --path .`, then smoke-test `tasks rename --help` using the installed command.** Do not run adoption against a live project.
- [ ] **Step 6: Complete Task 3 and its parent in the same commit as the code:** run `tasks done tasks-10afde 'Two-host recovery verified and documented'`, then `tasks done tasks-7f1596 'Second-host rename adoption landed'`. Run `tasks check`, then `git add tests/cli.rs README.md skills/tasks/SKILL.md tasks/tasks-10afde.md tasks/tasks-7f1596.md && git commit -m 'feat(tasks): verify and document second-host adoption'`.

## Finish

Review the branch against the spec, inspect `git status`, and report the commits and `just gate` result. Keep the worktree until the user chooses integration.
