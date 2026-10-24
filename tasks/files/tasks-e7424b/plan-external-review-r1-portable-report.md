# Portable derivative of external plan review round 1

This is a placeholder-substituted derivative, not verbatim raw evidence. The exact original and placeholder mapping are retained in the task evidence record for plan-external-review-r1-report.md.

# External plan review, round 1

Frozen revision: fbb9e6aab5baad757a2f2742bb09a73ec1874155
Repository: tasks, branch design/tasks-e7424b
Path: docs/plans/2026-10-24-bounded-session-summary.md
Reviewer: claude-code/claude-opus-5-5 (identity supplied with the review)
Round label: the user's supplied external-review round 1; distinct from the earlier controller self-review labelled round 1.

Full returned review:

I recommend **revise**, but only Task 3 needs to change. I reviewed plan revision `fbb9e6a`. Tasks 1 and 2 are sound and match the code and the accepted spec. Task 3 adopts bounded reads only in the tasks skill, while 11 project `AGENTS.md` files also tell agents to start with `tasks prime`, and those are where the truncation was hit. Task 3 is held by the trial gate until 2027-02-16, so this doesn't delay Tasks 1–2. Inline execution is the right call. Nothing is recorded yet. The Codex session should note `review: plan round 1 — verdict: revise; findings: Important 1, Minor 2; reviewer: claude-code/claude-opus-5-5` and then freeze a new revision for you.

**What I checked (all correct)**
- The working file is identical to `fbb9e6a`, and the spec is unchanged since `76501bd`.
- **Byte counting:** `Output` is `#[serde(untagged)]` (`src/output.rs:1206`). So serializing `PrimeOut` inside the new `bound()` function produces exactly the bytes `main` writes. `paint_projects` only touches fields that are left out of JSON.
- **Code the plan relies on exists as described:**
  - `start_dir` is at `src/commands/mod.rs:908`, and prime's single production caller is at `:2065`.
  - The `ScopeArgs` fields (`project`, `all_projects`, `group`) are as the plan assumes.
  - `Error::with_suffix` is the exhaustive match the plan expects to extend.
  - `TestEnv` is in `tests/common/mod.rs`, and `err_kind` and `err_detail` are in `tests/cli.rs`.
- **Ops side:** the `CliTable.test_file_is_valid` test exists, and the proposed row format matches the existing prime rows next to `--closed` in ops's `cli.toml`.
- **Both round-3 minors are covered:** the README text goes in its own section, checked against every link from the skill, and the `(deleted)` path case is a typed error with no suffix-stripping.
- **The plan matches your rules:** confined builds, no pushes, and asking before any change to shared tooling (Task 3 Step 5) all follow your Git and Decisions rules.

**Important**

1. **Task 3 updates one of several startup instructions.** Eleven project guides under `<SYNC_ROOT>/*/AGENTS.md` tell agents to start with `tasks prime`, this repo's own `AGENTS.md` among them ("Start with `tasks prime`"). The Codex session hit the truncation by following one of them. Lore's global instructions also name `tasks prime`, though only for halts. With only `skills/tasks/SKILL.md` changed, sessions guided by project files keep doing the full read. That breaks spec §8's adoption intent while the plan's goal line says guidance has adopted the summary.
   - Suggested fix: Task 3 Step 1 lists every instruction surface that prescribes `tasks prime`. This repo's `AGENTS.md` goes into Task 3's files. Each other project gets its own owner task, filed with the same gate condition. Lore gets its own task if its halt wording should change.

**Minor**

2. **Resolve the directory without resolving symlinks.** The plan resolves the directory with full symlink resolution (`canonicalize`). Under `.worktrees`, which is a symlink into WORK_ROOT storage, that turns the recovery `-C` path into the storage path rather than the directory the read actually used. Spec §4 asks only for an absolute path. `std::path::absolute(start_dir(..))` keeps the directory as given, needs no extra filesystem access, and still meets the spec. I found no correctness difference in how projects or claims are resolved (`locations.rs` normalizes paths itself), so this is about matching the spec and about the path agents see.
3. **Reconcile with this repo's reinstall rule.** `AGENTS.md` says "Rebuild and reinstall after CLI changes" (`cargo install --path .`), but Task 2 Step 6 says no shared binary install is implied. After Task 2 merges, the README would describe `--summary` while the installed `tasks` lacks it. The plan should say what happens after the merge: either the documented reinstall as routine, or a named step that asks you first, given that it replaces a tool on PATH.

I wrote nothing to the task or the worktree, ran no tests, and left nothing running.
