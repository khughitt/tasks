# Detached Records Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a project keep its records and working documents outside its code checkout, at `<records-root>/<prefix>/`, as a third project shape (`detached`) beside in-tree and linked, with one model and one code path for all three.

**Architecture:**
- **One model.** `Project` carries `root` (the code checkout, what the registry holds today), `checkout` (always present; equal to `root` unless linked), `records` (where `tasks/` and `docs/` live) and `kind`. Every record, attachment and document path derives from `records`; every resume path from `checkout`; every git query about records runs in the repository holding `records`. In-tree and linked projects keep today's behaviour because for them `records == root`.
- **Declaration and discovery.** The records config declares `[checkout] path` and `[checkout] kind = "detached"`, so `Project::open` on it is self-describing. The host registry gains `[records] <prefix> = <records>/<prefix>`, which `locate` uses to open a detached project from its code checkout and worktrees, and to refuse an in-tree config whose prefix is paired elsewhere.
- **The CLI surface.** `init --records <records-root>` registers the pairing; `rename` moves the records directory; `projects`, `where` and `resolve` report the three locations and the kind.

**Tech Stack:** Rust (clap, serde, toml, toml_edit, assert_cmd integration tests).

**Status:** draft for user review (round 2).

Revision 2 (2026-10-22, after plan review round 1): rename keeps the project's registered identity apart from the current records directory in the invocation, the inventory, the snapshot and the classifier (Task 9); the test fixtures build their git environment explicitly and assert the fixture repository before writing (Task 2); the registered-copy comparisons behind retirement compare records locations (Task 3); `init --records` on an existing config enables record commits and carries the registered-root overlap checks (Task 5); `projects` rows always carry `records` (Task 6, decision 3); the exhaustive classifier enumeration is scheduled (Task 9, Rollout).

**Spec:** `docs/specs/2026-10-22-records-home-design.md` (approved 2026-10-22 at revision 3, commit c03869d). Read it with this plan; section numbers below refer to it. This plan implements §3, §5.1–§5.4 and §12 cases 1–6 and the CLI side of case 9. `tasks docs` (§5.5) is tasks-a8e28e; skill wording is tasks-00723a.

## Decisions this plan makes beyond the spec

The reviewer should rule on each; none changes the spec's contract.

1. **`root` of a detached project comes from its config.** `Project::open` on `<records>/<prefix>` derives `root` from `[checkout] path`, exactly as a linked overlay derives `checkout`. The registry's `[projects]` and `[records]` rows pair the two for discovery (§3.2 step 2) and for the `records_moved` refusal (§3.3); `open_registered` reports a `config` error when the registry's root and the config's path disagree. §3.2 step 1 says "root from the registry"; reading it from the config keeps `open` registry-free and makes the disagreement visible instead of silent.
2. **`rename` moves the records directory with `std::fs::rename`, not `git mv`.** Every other rename stage is a plain filesystem operation and `rename` never commits; git records the rename at the user's commit. The move is one stage of the inventory with its own recovery state (§5.4).
3. **`projects` rows always carry `records`; `checkout` and `kind` whenever the config was read.** `records` comes from the registry (the `[records]` pairing, else the root), so an unreachable row still says where its records should be. `checkout` and `kind` are facts of the config: an unreachable row has none to read, and inventing `checkout = root` would be wrong for an unreachable linked overlay. This is the one exception to §5.2's "`checkout` always present", stated in the README (Task 10); ops's tolerant `linked.py` accepts a row without `checkout`.
4. **`where` applies the `records_moved` refusal inside git only.** `where` reads no registry outside git (linked-checkouts decision 4) and an old branch is by definition a git tree. `locate` reads the registry on every walk hit, as §3.2 says. Outside git, on a copied historical config, `where` and `locate` can therefore differ; the README states that limitation (Task 10).
5. **`init --records` edits an existing config with `toml_edit`**, the way `retire` writes `[retired]`, so a migrated config keeps its comments. It adds `[checkout]` and sets `[git] commit = true` (§3.4) when that is absent or false.
6. **Plain `tasks init` in a checkout paired in `[records]` refuses with `records_moved`.** Not in the spec; without it an in-tree config could be created in a detached checkout and refused only on the next command. The detail names the re-pairing command (`tasks init --prefix <p> --records <records-root> --force`).
7. **`InitOut` gains `records`** (omitted unless `--records`), so the migration script reads the derived directory from the command that made it.

## Global Constraints

- **Every commit passes the pre-commit hook.** `just check` runs `cargo clippy --all-targets -- -D warnings`, and this is a binary crate, so an unused function, variant or field fails it.
  - A helper lands in the same task as its first production caller.
  - Never add `#[allow(dead_code)]` or another lint allowance to get past this.
  - Each task's commit step runs `just check` first.
- **Three locations** (spec §3.1): `root` is the registered root and what `tasks root` prints, unchanged in meaning; `checkout` is the code checkout sessions run in; `records` is where `tasks/` and `docs/` live. In-tree: all three equal. Detached: `records = <records-root>/<prefix>`, the others equal. Linked: `records = root = <overlay>/<prefix>`, `checkout` the foreign repository.
- **Kind strings** (spec §3.1, §5.2): `in-tree`, `detached`, `linked`, in JSON and in pretty output.
- **Config** (spec §3.2): `[checkout] path` relative to the records directory, `[checkout] kind = "detached"`. Absence of `kind` on a `[checkout]` table is a linked overlay. Any other value is a `config` error.
- **Registry** (spec §3.2): `[records] <prefix> = "<records-root>/<prefix>"`, absolute. A key that is not a live prefix in `[projects]` is a `config` error on load. The value is the project's records directory, not the records root.
- **Error kinds** (JSON `error.kind`, spec §5.2): `records_moved` (§3.3), `records_unpaired` (§3.2), `records_in_tree` (§5.1). Each variant goes in three places in `src/error.rs`: the enum, `with_suffix`, and `kind()`.
- **JSON additions** (spec §5.2): `projects` rows gain `records` and `kind`, `checkout` always present on reachable rows; `where` gains `records` and `kind`, `checkout` always a string, `linked` stays and equals `kind == "linked"`; `resolve` rows gain `records`, and `via` gains `records`. No key changes meaning.
- **Resolution order** (spec §3.1, §7.3): a path reference resolves under `records`, then under the code tree (the worktree the command runs in, then the main checkout).
- **No command writes under a detached project's `root`** (spec §5.3). This is a test (Task 4), not a convention.
- **CLI surface:** `init --records <PATH>`, mutually exclusive with `--checkout`. The row lands in ops's `cli.toml` first (Task 1); this repo adopts it with `vendored adopt` in the parser commit (Task 5). Never edit `tools/cli.toml` by hand.
- **Release order** (spec §5.2): the branch merges only after ops-2a0024 (`linked.py` tolerant of both shapes) is installed on both hosts; see Rollout.
- **Tests:** run them only through `just test-one <args>` and `just test-fast`, never `cargo test` directly (AGENTS.md). `just test` (the `#[ignore]`d enumeration included) runs after Task 9's classifier change and on the branch head before integration.
- **Test git environments are built, not inherited.** A hook exports `GIT_DIR`, `GIT_INDEX_FILE` and `GIT_WORK_TREE`, which win over `git -C`; `GIT_CONFIG_GLOBAL` alone neutralizes none of them. The fixture's git helper starts from `env_clear()` and sets `PATH`, `HOME` (the test home), `LC_ALL=C`, `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_CONFIG_NOSYSTEM=1`, `GIT_AUTHOR_NAME/EMAIL` and `GIT_COMMITTER_NAME/EMAIL`; before a fixture repository's first write it asserts that `rev-parse --show-toplevel` is the fixture directory and `--absolute-git-dir` lies inside it. `TestEnv::cmd` and `raw` remove every inherited `GIT_*` variable except `GIT_CEILING_DIRECTORIES`, which the justfile sets on purpose.

## Review Focus

1. A registry whose `[records]` names a prefix absent from `[projects]` (a hand edit, or a half-finished `unregister`) must be a `config` error on load, not a silent pairing. Test in Task 3.
2. `init --records` run from a linked worktree of the code repository, not its main checkout, must refuse and name the main checkout. Test in Task 5.
3. `tasks commit` run in the code checkout of a detached project must commit in the records repository and leave the code repository's index alone. Test in Task 2.
4. A detached project whose code checkout is absent on this host (the second host between the sync and its own `init --records`) must still be readable from `-C <records>/<prefix>`, with `check` warning `checkout_missing`. Test in Task 7.
5. A task's `--ref` output declared as a records-relative path must satisfy the completion gate when the file is under `records`, and a retained spec in the code tree must satisfy it through `root`. Test in Task 8.

## Tracking

- **This plan belongs to tasks-211e28** (process `planned`). After the user approves it, in `.worktrees/records-detached`:
  1. `tasks edit tasks-211e28 --plan 2026-10-22-records-detached`.
  2. Under it, add one child per `### Task N:` heading below, with `--parent tasks-211e28`, `--plan 2026-10-22-records-detached --step "Task N: <title>"`, `--process direct`, `--size` and the complexity on each task's **Complexity:** line, and `--depends` on the previous step.
- **Run the implementation** in `.worktrees/records-detached` on `records-detached`, which holds the spec and this plan. Run the worktree skill's baseline (`just test`) before the first code change. The Rollout below is the goal's last step.

## Concurrent work

- **ops-2a0024** (`linked.py`) lands and is installed on both hosts before this branch merges; it has no code dependency on this branch. Task 6 changes the `where` shape that its old validator rejects.
- **tasks-a8e28e** (`tasks docs`) follows this branch and reads `Project::records`.
- **tasks-00723a** (skill wording) follows this branch; the tasks skill's `init --checkout` and `where` paragraphs gain the detached form there, not here.
- **Rebase onto main** before Task 2 and before the final review. Expect conflicts near `Project`, `scan_checkouts` and the output structs.

---

### Task 1: The CLI row in ops

**Complexity:** low

The vendored `tools/cli.toml` arrives only from ops main, and `parser_surface_equals_table` fails until it matches the parser. This row lands in ops before Task 5 adopts it.

**Files:**
- Modify (in an ops worktree, then ops main): `cli.toml`

**Interfaces:**
- Produces: ops main's `cli.toml` with, on `init`, `{ names = ["--records"], value = "path" }` after the `--checkout` row.

- [ ] **Step 1: Create the ops worktree**

In the ops main checkout (`tasks --pretty root ops-88a0fa` prints it):

```bash
work-link --ensure .worktrees
git worktree add -b feat/cli-init-records .worktrees/cli-init-records main
git worktree lock --reason "on WORK_ROOT storage (host: $(uname -n))" .worktrees/cli-init-records
cd .worktrees/cli-init-records && just setup
```

- [ ] **Step 2: Add the row**

Edit `cli.toml`: in the `[[cli.tasks.commands]]` entry with `path = ["init"]`, add `{ names = ["--records"], value = "path" },` after the `--checkout` row. Leave the summary as it is.

Run: `git diff --stat`
Expected: `cli.toml` only.

- [ ] **Step 3: Run ops's gate**

Run: `just test-fast`
Expected: PASS.

- [ ] **Step 4: Commit, merge locally, remove the worktree**

```bash
git add cli.toml
git commit -m "feat(cli): tasks init --records"
cd "$(tasks --pretty root ops-88a0fa)"
git merge --ff-only feat/cli-init-records
git -C .worktrees/cli-init-records status --ignored --porcelain   # copy out anything still needed
git worktree unlock .worktrees/cli-init-records
git worktree remove .worktrees/cli-init-records
git branch -d feat/cli-init-records
```

Expected: the merge fast-forwards. ops is a personal-profile repository, so a local merge needs no further approval.

---

### Task 2: The project model: `records`, `kind`, and every path under `records`

**Complexity:** mid

The model change and the mechanical sweep land together: once `tasks_dir()` reads `records`, every call site that still joins `root` is a bug the sweep removes. In-tree and linked projects are unchanged because `records == root` for them; the new integration tests open a detached project only by `-C <records>/<prefix>` (the walk hit), since discovery through the registry is Task 3.

**Files:**
- Modify: `src/repo.rs` (`Kind`, `Project`, `CheckoutConfig`, `Project::open`, `Project::init`, `tasks_dir`, `git`, `worktree_records`, `sibling_task_copies`, `uncommitted_task_files`, `missing_checkout_advice`, the two `written::record` calls)
- Modify: every `project.root` site that reaches `tasks/`, `docs/` or `CONFIG_REL`, or re-opens the project (the list in Step 3)
- Modify: `tests/common/mod.rs` (`cmd` and `raw` drop inherited `GIT_*` variables)
- Create: `tests/detached.rs`
- Test: unit tests in `src/repo.rs`; `tests/detached.rs`; `tests/linked.rs` and `tests/cli.rs` must keep passing unchanged

**Interfaces:**
- Produces, in `src/repo.rs`:
  - `pub enum Kind { InTree, Detached, Linked }`, deriving `Debug, Clone, Copy, PartialEq, Eq, Serialize`, `#[serde(rename_all = "kebab-case")]`, with `pub fn as_str(self) -> &'static str` returning `in-tree`, `detached`, `linked`.
  - `Project` fields: `pub root: PathBuf`, `pub checkout: PathBuf` (no longer `Option`), `pub records: PathBuf`, `pub kind: Kind`, `pub worktree: Option<PathBuf>` (unchanged meaning: the code tree the command runs in when the project was found through its link or pairing). `records` is canonical and is the directory holding `tasks/.config.toml`.
  - `pub fn open(dir: &Path) -> Result<Project>`: `dir` holds the config. `[checkout]` absent: in-tree, `root = checkout = records = dir`. `[checkout] path` without `kind`: linked, `root = records = dir`, `checkout = normalize(path, dir)`. `kind = "detached"`: `records = dir`, `root = checkout = normalize(path, dir)`. Any other `kind` value: `Error::Config("tasks/.config.toml: [checkout] kind must be \"detached\" or absent")`.
  - `CheckoutConfig { path: String, #[serde(default, skip_serializing_if = "Option::is_none")] kind: Option<String> }`, still `deny_unknown_fields`.
  - `pub fn tasks_dir(&self) -> PathBuf` is `self.records.join("tasks")`; `pub fn doc_path(&self, rel: &str) -> PathBuf` is `self.records.join(rel)` (the one builder for spec, plan and `--ref` paths under records).
  - `fn git(&self, args)` runs in `self.records`; `worktree_records`, `worktree_count`, `main_checkout_root`, `sibling_task_copies`, `uncommitted_task_files`, `linked_worktree_head_has` and `git_toplevel` therefore describe the repository holding the records.
  - Every `written::record(...)` call passes `&self.records` (or the records of the project written), never `root`.
- Consumes: nothing new.

- [ ] **Step 1: Write the failing unit tests in `src/repo.rs`**

```rust
#[test]
fn a_detached_config_opens_with_root_from_its_checkout_path() {
    let base = tempfile::tempdir().unwrap();
    let code = base.path().join("code");
    let records = base.path().join("records/det");
    std::fs::create_dir_all(&code).unwrap();
    std::fs::create_dir_all(records.join("tasks")).unwrap();
    std::fs::write(
        records.join("tasks/.config.toml"),
        "prefix = \"det\"\n\n[git]\ncommit = true\n\n[checkout]\npath = \"../../code\"\nkind = \"detached\"\n",
    )
    .unwrap();
    let project = Project::open(&records).unwrap();
    assert_eq!(project.kind, Kind::Detached);
    assert_eq!(project.records, records.canonicalize().unwrap());
    assert_eq!(project.root, code.canonicalize().unwrap());
    assert_eq!(project.checkout, project.root);
    assert_eq!(project.tasks_dir(), project.records.join("tasks"));
    assert!(project.commit_records);
}

#[test]
fn in_tree_and_linked_configs_keep_their_shape() {
    // in-tree: root == checkout == records == dir, Kind::InTree
    // linked ([checkout] path, no kind): root == records == dir, checkout == normalized path, Kind::Linked
}

#[test]
fn an_unknown_checkout_kind_is_a_config_error() {
    // kind = "overlay" -> Error::Config whose message contains "[checkout] kind"
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `just test-one a_detached_config_opens`
Expected: FAIL (compile errors: no `Kind`, no `records` field).

- [ ] **Step 3: Implement the model and sweep every path to `records`**

In `src/repo.rs`, add `Kind`, the fields, the `kind` parsing in `open`, and `doc_path`. `Project::init` (plain and `--checkout`) sets `kind` through `open` as today; Task 5 adds the detached writer.

Then replace `root` with `records` at every site that reaches `tasks/`, `docs/` or the config, or re-opens the project. The sites (line numbers from the branch point; the mechanical rule is "a path under `tasks/` or `docs/`, a `CONFIG_REL` join, a `Project::open(&project.root)`, a journal key, or a git query about records uses `records`"):

- `src/repo.rs`: `init` 388, 422–423; `missing_checkout_advice` 746; `sibling_task_copies` 842–848; `worktree_records` offset 768–819; `uncommitted_task_files` 876–925; the two `written::record` calls 625, 1030.
- `src/resolve.rs`: 188, 202, 293, 337, 544, and `resolve_registered` 592/607 (`has_config`) — the `records`-then-code order itself is Task 8; here only the base becomes `records`.
- `src/refs.rs:34`; `src/commands/show.rs:163,175`; `src/attachments.rs:33-38`; `src/commands/edit.rs:337,348,364`; `src/commands/feedback.rs:222`; `src/commands/check.rs:352,387`; `src/commands/retire.rs:113-114,162`; `src/commands/release.rs:243` (a claim's worktree is a code tree: read the record from `ctx.project.task_path(id)` instead); `src/commands/commit.rs:10,13,25`; `src/commands/status.rs:131,148,162`; `src/commands/list.rs:870` (already through `uncommitted_task_files`).
- `src/commands/mod.rs:733` (`Project::open(&ctx.project.records)`, keeping `worktree`); `src/moving/mod.rs:125-126,228,235,267,270,282`; `src/moving/preflight.rs:72` (`open_ctx(Some(&ctx.target.records))`).
- `src/rename/mod.rs:143,172,195,198,252,336`; `src/rename/adopt.rs:54`; `src/rename/inventory.rs:69,74-79,161`; `src/rename/snapshot.rs:116,155-157,220`; `src/rename/classify.rs:134-141` (these read `invocation.root`/`inventory.root`; Task 9 makes that root the records directory for a detached project, so leave the field names and only route through `records` where a `Project` is in hand).
- `src/record_commit.rs:422,435`: the journal key is now `records`; `Project::open(&root)` on it works for every kind. Rename the loop variable to `records`.
- Leave on `root`: `src/commands/init.rs:68` (`skill_installed`), `src/commands/root.rs`, `src/commands/feedback.rs:228,234` (display). The "is this copy the registered one" comparisons at `src/commands/mod.rs:784-807` (`effective_retirement`), `mod.rs:762-780` (`reject_retired`'s authority), `src/commands/check.rs:113-125` and `src/commands/retire.rs:69-82` (`authority`) stay as they are in this task and move to records locations in Task 3, which introduces the registry pairing they need.

`project.checkout` is now a `PathBuf`: update `src/commands/init.rs:375-386` (compare `existing.kind == Kind::Linked && existing.checkout != checkout`), `src/commands/check.rs:154-166` (`checkout_missing` when `kind != InTree && !checkout.is_dir()`), `src/locations.rs:52-57` (`if project.kind != Kind::InTree`), `src/commands/where_.rs:56-66` and `src/commands/projects.rs` (keep today's output: `checkout` only when `kind == Linked`; Task 6 changes the shape).

- [ ] **Step 4: Run the unit tests and the existing suites**

Run: `just test-one repo::` then `just test-fast`
Expected: PASS. `tests/linked.rs` and `tests/cli.rs` pass without edits: nothing observable changed for in-tree or linked projects.

- [ ] **Step 5: Write the failing integration tests in `tests/detached.rs`**

The fixture, kept in this file and reused by every later task (Task 3 adds the registry lines, Task 5 replaces the hand-written config and registry with `init --records`):

```rust
mod common;
use common::TestEnv;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Git with an environment built key by key (Global Constraints): nothing a hook exports
/// reaches the fixture, and `-C` is the only thing that chooses the repository.
fn git(home: &Path, dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("HOME", home)
        .env("LC_ALL", "C")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t")
        .arg("-C").arg(dir).args(args)
        .output().unwrap();
    assert!(out.status.success(), "git {args:?} in {}: {}", dir.display(), String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

/// `git init -b main`, then, before the first write, the assertion that this repository
/// is the fixture: top level == dir and the git directory inside it. One README commit.
fn repo(home: &Path, base: &Path, name: &str) -> PathBuf {
    let dir = base.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    git(home, &dir, &["init", "-q", "-b", "main"]);
    let dir = dir.canonicalize().unwrap();
    let top = git(home, &dir, &["rev-parse", "--show-toplevel"]);
    assert_eq!(Path::new(top.trim()), dir);
    let git_dir = git(home, &dir, &["rev-parse", "--absolute-git-dir"]);
    assert!(Path::new(git_dir.trim()).starts_with(&dir));
    std::fs::write(dir.join("README"), "x\n").unwrap();
    git(home, &dir, &["add", "README"]);
    git(home, &dir, &["commit", "-qm", "init"]);
    dir
}
fn porcelain(home: &Path, dir: &Path) -> String { git(home, dir, &["status", "--porcelain", "--ignored"]) }

pub struct Layout {
    _base: TempDir,
    pub base: PathBuf,
    /// The records root, a git repository.
    pub records: PathBuf,
    /// `<records>/<prefix>`, the project's records directory.
    pub project: PathBuf,
    /// The code checkout, a git repository with `.worktrees` linked to `base/storage/code`.
    pub code: PathBuf,
    /// A linked worktree of `code` on branch `wt`.
    pub wt: PathBuf,
}

/// A detached project named `prefix`: config written by hand and committed in the records
/// repository. Nothing is registered. Every git call takes `env.home.path()` as its HOME.
pub fn layout(env: &mut TestEnv, prefix: &str) -> Layout {
    // base/records (repo), base/code (repo), base/storage/code; code/.worktrees -> ../storage/code
    // records/<prefix>/tasks/.config.toml:
    //   prefix = "<prefix>"\n\n[git]\ncommit = true\n\n[checkout]\npath = "../../code"\nkind = "detached"\n
    // git -C records add -A && commit
    // git -C code worktree add -q -b wt .worktrees/wt
}
```

`Layout` also carries `pub home: PathBuf` (the test home) and the wrappers `l.git(dir, args)` and `l.porcelain(dir)` that pass it; the test outlines below write `git(&l.x, …)` and `porcelain(&l.x)` for those wrappers.

In `tests/common/mod.rs`, `cmd` and `raw` remove every variable whose name starts with `GIT_` except `GIT_CEILING_DIRECTORIES` (iterate `std::env::vars_os()`); every existing suite must still pass.

The tests:

```rust
#[test]
fn opened_from_its_records_directory_a_detached_project_writes_and_commits_there() {
    let mut env = TestEnv::new();
    let home = env.home.path().to_path_buf();
    let l = layout(&mut env, "det");
    let before = porcelain(&home, &l.code);
    let added = env.json(&l.project, &["add", "first"]);
    let id = added["id"].as_str().unwrap().to_string();
    assert!(l.project.join(format!("tasks/{id}.md")).is_file());
    assert!(!l.code.join("tasks").exists());
    env.json(&l.project, &["note", &id, "n1"]);
    let doc = l.project.join("docs/specs/x-design.md");
    std::fs::create_dir_all(doc.parent().unwrap()).unwrap();
    std::fs::write(&doc, "# x\n").unwrap();
    git(&home, &l.records, &["add", "-A"]);
    git(&home, &l.records, &["commit", "-qm", "spec"]);   // a record commit commits only what the command wrote
    env.json(&l.project, &["edit", &id, "--spec", "docs/specs/x-design.md"]);
    let shown = env.json(&l.project, &["show", &id]);
    assert_eq!(shown["spec_path"], doc.display().to_string());
    assert_eq!(git(&home, &l.records, &["log", "--format=%s", "-n", "1"]).trim(), format!("chore(tasks): edit {id}"));
    assert_eq!(git(&home, &l.records, &["show", "--stat", "--format=", "HEAD"]).trim().lines().count(), 2); // the record alone
    assert_eq!(porcelain(&home, &l.records), "");
    assert_eq!(porcelain(&home, &l.code), before);
}

#[test]
fn start_and_park_record_the_code_checkout_as_the_resume_root() {
    // tasks -C <project> add, start, park "next" --waiting-on agent
    // claim store (env.claim_store("det")) worktree == l.code (resume_root is root, the code checkout)
}

#[test]
fn commit_from_a_detached_project_commits_in_the_records_repository() {
    // disable [git] commit in the hand-written config (commit = false), add a task,
    // then `tasks -C <project> commit`: the records repo gains one commit; porcelain(code) unchanged
}
```

- [ ] **Step 6: Run them to verify they fail, then pass**

Run: `just test-one --test detached`
Expected: before Step 3 these would fail on the config parse; after it, PASS. If `commit_from_a_detached_project…` writes into `code`, a `root` site was missed: fix it.

- [ ] **Step 7: Check and commit**

Run: `just test-fast` then `just check`
Expected: PASS, no clippy findings.

```bash
git add src tests/detached.rs tests/common/mod.rs
git commit -m "feat(repo): a project's records, kind and checkout; every record path under records"
```

---

### Task 3: The registry `[records]` table and discovery from the code checkout

**Complexity:** high

**Files:**
- Modify: `src/registry.rs` (`Registry.records`, load validation and `~/` expansion, `records_dir`, `pair_records`, `unregister`, `rename`, `adopt`)
- Modify: `src/scope.rs` (`config_dir`, `is_reachable`, `open_registered`)
- Modify: `src/locations.rs` (`Linked` gains `kind` and `records`; `scan_checkouts` pushes every non-in-tree project)
- Modify: `src/link.rs` (`paired_project`, `project_of_tree`, `refuse_moved`; `linked_project` filters by kind; `holds_records` uses `config_dir`)
- Modify: `src/repo.rs` (`Project::locate`)
- Modify: `src/commands/where_.rs` (`walk` applies `refuse_moved` inside git; `project_of_tree` on a miss)
- Modify: `src/commands/parked.rs` (`open_recorded` uses `project_of_tree`)
- Modify: `src/commands/mod.rs` (`effective_retirement`, `reject_retired`), `src/commands/check.rs:113-125`, `src/commands/retire.rs` (`authority`): the registered-copy comparisons
- Modify: `src/error.rs` (`RecordsMoved`, `RecordsUnpaired`)
- Modify: every `is_reachable(root)` caller: `src/commands/check.rs:222`, `src/complete.rs:448`, `src/commands/feedback.rs:48`, `src/commands/group.rs:38,97`, `src/commands/projects.rs:75`, `src/commands/move_.rs:34`, `src/commands/init.rs:136`, `src/locations.rs:41`
- Test: unit tests in `src/registry.rs`; `tests/detached.rs`

**Interfaces:**
- Produces, in `src/registry.rs`:
  - `pub records: BTreeMap<String, PathBuf>` with `#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]`, placed after `aliases`. `load_with_home` expands a leading `~/` as it does for `projects` and refuses a key absent from `projects` with `Error::Config("[records] names unregistered prefix \"x\"")`.
  - `pub fn records_dir(&self, prefix: &str) -> Option<&Path>`.
  - `pub fn pair_records(&mut self, prefix: &str, dir: &Path)` (insert or replace).
  - `unregister` removes the entry; `rename` moves the key (value unchanged; Task 9 updates it); `adopt` moves the key the same way.
- Produces, in `src/scope.rs`:
  - `pub fn config_dir(registry: &Registry, prefix: &str) -> Option<PathBuf>`: the records directory when paired, else the registered root; `None` when unregistered. Takes the canonical prefix.
  - `pub fn is_reachable(registry: &Registry, prefix: &str) -> Result<bool>`: `config_dir` exists and holds a config.
  - `open_registered` opens `config_dir`; for a paired prefix it also requires `project.kind == Kind::Detached` and `project.root == registered root` (canonical), else `Error::Config("registry pairs {prefix} with {records}, whose config declares checkout {root}; fix the registry or the config")`.
- Produces, in `src/locations.rs`: `pub struct Linked { pub prefix: String, pub checkout: PathBuf, pub records: PathBuf, pub kind: Kind }`; `scan_checkouts` pushes a row for every project with `kind != InTree`.
- Produces, in `src/link.rs`:
  - `pub fn paired_project(registry: &Registry, tree: &GitTree) -> Result<Option<Project>>`: `tree.main_checkout()` equals a registered root (canonical compare) → if `[records]` pairs that prefix, `open_registered` and set `worktree = Some(tree.top)`; else if that root holds a config, `Ok(None)` (an in-tree project reached from a worktree without the file: today's fall-through); else `Err(Error::RecordsUnpaired(...))` naming the prefix, the root and `tasks init --prefix <p> --records <records-root>`.
  - `pub fn project_of_tree(registry: &Registry, tree: &GitTree) -> Result<Option<Project>>`: `paired_project`, then `linked_project`. `linked_project` keeps only `kind == Linked` rows.
  - `pub fn refuse_moved(registry: &Registry, project: &Project) -> Result<()>`: when `project.kind == InTree` and `registry.records_dir(&project.prefix)` is `Some(dir)` with `dir != project.records`, `Err(Error::RecordsMoved(...))` naming the prefix, `dir`, and the remedy "this tree's tasks/ is read-only history: rebase or merge the branch onto the main branch".
  - `holds_records` iterates `registry.projects.keys()` and tests each prefix's `config_dir` against `tree.common`.
- Produces, in `src/repo.rs`: `Project::locate` is §3.2's three steps: walk (then `refuse_moved` with `Registry::load()?`), then on a miss inside git `project_of_tree`, then `NoProject`.
- Produces, in `src/error.rs`: `RecordsMoved(String)` → `records_moved`; `RecordsUnpaired(String)` → `records_unpaired`.
- Produces: **the registered copy is decided by records location.** "Is this copy the registered one" means `scope::config_dir(registry, prefix) == Some(project.records)`, never `project_root == project.root`: two records worktrees of a detached project share one code root, so the old comparison would let a worktree made before a retirement skip the registered config. Change `effective_retirement` (`mod.rs:788-789`), `reject_retired`'s `authority` (`mod.rs:767-769`, which names `config_dir`), `check.rs:113-125` (`authority == ctx.project.records`) and `retire::authority` (`retire.rs:72-80`: run in the registered records directory, which the refusal names; `Project::open(config_dir)`). In-tree and linked projects are unchanged, since their `config_dir` is their root.

- [ ] **Step 1: Write the failing registry unit tests in `src/registry.rs`**

```rust
#[test]
fn records_table_round_trips_and_expands_home() {
    // load_with_home over: [projects]\ndet = "~/code"\n[records]\ndet = "~/records/det"\n
    // records_dir("det") == home.join("records/det"); save_to writes the table back
}

#[test]
fn records_for_an_unregistered_prefix_is_a_config_error() {
    // [records]\nghost = "/x"\n with no [projects] ghost -> Error::Config mentioning "[records]" and "ghost"
}

#[test]
fn unregister_and_rename_carry_the_records_entry() {
    // pair_records("det", ...); rename("det","dtx") moves the key; unregister("dtx") removes it
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `just test-one registry::records`
Expected: FAIL (compile errors: no `records` field).

- [ ] **Step 3: Implement the registry, `scope`, `scan_checkouts`, `link` and `locate` changes**

As the Interfaces block says. In `Project::locate`, the walk hit now reads the registry: a missing registry file is the default (empty) registry, a malformed one is the `config` error it already is for every registry read.

- [ ] **Step 4: Run the unit tests**

Run: `just test-one registry::records`
Expected: PASS.

- [ ] **Step 5: Write the failing discovery tests in `tests/detached.rs`**

Extend the fixture with a registry writer. It stays after Task 5 for the tests that need a registry state `init --records` would refuse (Task 3's refusals, Task 7); Task 5 adds `register`, the `init --records` route, and switches every other test to it:

```rust
/// Registers `prefix` as a detached project by writing the registry file directly:
/// [projects] prefix = code, [records] prefix = project.
pub fn register_by_hand(env: &TestEnv, l: &Layout, prefix: &str) {
    let path = env.home.path().join(".config/tasks/projects.toml");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, format!(
        "[projects]\n{prefix} = {:?}\n\n[records]\n{prefix} = {:?}\n",
        l.code.display().to_string(), l.project.display().to_string()
    )).unwrap();
}
```

```rust
#[test]
fn a_detached_project_opens_from_its_checkout_its_worktree_and_its_records_directory() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register_by_hand(&env, &l, "det");
    let id = env.json(&l.code, &["add", "from code"])["id"].as_str().unwrap().to_string();
    assert!(l.project.join(format!("tasks/{id}.md")).is_file());
    let from_wt = env.json(&l.wt, &["show", &id]);
    assert_eq!(from_wt["task"]["title"], "from code");
    let from_deep = env.json(&l.code.join("src"), &["list"]); // create code/src first
    assert_eq!(from_deep["tasks"].as_array().unwrap().len(), 1);
    env.json(&l.project, &["note", &id, "from records"]);
    // start from the worktree records the worktree as the resume root
    env.json(&l.wt, &["start", &id]);
    let store = std::fs::read_to_string(env.claim_store("det")).unwrap();
    assert!(store.contains(&format!("worktree = {:?}", l.wt.display().to_string())));
}

#[test]
fn a_branch_that_still_tracks_the_config_is_records_moved() {
    // layout + register_by_hand; in l.wt: mkdir tasks, write an in-tree config with prefix det, git add, commit
    // env.fail(&l.wt, &["list"]) == "records_moved"; env.fail(&l.wt, &["note", "det-000000", "x"]) == "records_moved"
    // the error detail names l.project (read stderr through env.raw)
    // the main checkout (no config) still opens: env.json(&l.code, &["list"]) succeeds
}

#[test]
fn a_registered_root_without_config_or_pairing_is_records_unpaired() {
    // registry with [projects] det = code and no [records]: env.fail(&l.code, &["list"]) == "records_unpaired"
    // detail names `tasks init --prefix det --records`
}

#[test]
fn an_in_tree_project_reached_from_a_worktree_without_its_tasks_dir_stays_no_project() {
    // env.init("itr") in a repo, commit, worktree on an orphan branch without tasks/: fail == "no_project"
}

#[test]
fn a_retirement_declared_in_the_registered_records_binds_an_older_records_worktree() {
    // register_by_hand; `git -C l.records worktree add -b old <base>/records-old` (a records worktree made before the retirement);
    // tasks -C l.project retire --reason "over" (from the registered records directory; l.code is refused with not_registered_checkout
    // naming l.project); then env.fail(&<base>/records-old/det, &["add", "late"]) == "retired_project";
    // env.fail(&l.code, &["add", "late"]) == "retired_project"
}

#[test]
fn a_registry_pairing_a_prefix_the_projects_table_lacks_is_a_config_error() {
    // Review Focus 1: write [records] det = ... with no [projects]; env.fail(&l.project, &["list"]) == "config"
}

#[test]
fn a_config_whose_checkout_disagrees_with_the_registry_is_a_config_error() {
    // register_by_hand with [projects] det = <another repo> and [records] det = l.project;
    // from outside every project, `tasks show <id>` reaches open_registered: env.fail(&l.base, &["show", &id]) == "config"
    // from l.code (unpaired by that registry): "no_project"; from l.project the walk still opens it
}
```

- [ ] **Step 6: Run them to verify they fail, then pass**

Run: `just test-one --test detached`
Expected: FAIL before Step 3 (`no_project` from the code checkout), PASS after.

- [ ] **Step 7: Check and commit**

Run: `just test-fast` then `just check`
Expected: PASS.

```bash
git add src tests/detached.rs
git commit -m "feat(locate): open a detached project from its code checkout through [records]"
```

---

### Task 4: No command writes under a detached project's root

**Complexity:** mid

The §5.3 guarantee as a test, mirroring `tests/linked.rs::the_lifecycle_in_a_code_worktree_keeps_records_in_the_overlay`, plus the stale-copy acceptance (§12 case 5). Whatever the lifecycle test finds still writing under `root` is fixed here.

**Files:**
- Test: `tests/detached.rs`
- Modify: whatever the test exposes (expected: nothing beyond Task 2's sweep; `src/commands/release.rs` and `src/commands/parked.rs` are the likely stragglers)

**Interfaces:**
- Consumes: Task 3's `register_by_hand` fixture (Task 5 switches these tests to `register`).

- [ ] **Step 1: Write the lifecycle test**

```rust
#[test]
fn the_lifecycle_from_the_code_checkout_and_its_worktree_writes_only_under_records() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    register_by_hand(&env, &l, "det");
    let code_before = porcelain(&l.code);
    let wt_before = porcelain(&l.wt);
    // from l.code: add, start, note, edit --spec (write the spec under l.project first), attach (a file under base), park "next" --waiting-on agent
    // from l.wt: start (resume), note, done "landed"
    // from l.base (outside every project): quiet, next --all-projects, show <id>, prime --project det
    // from l.code: add a second task, block, unblock, shelve, unshelve, drop
    assert_eq!(porcelain(&l.code), code_before, "nothing written into the code checkout");
    assert_eq!(porcelain(&l.wt), wt_before, "nothing written into the code worktree");
    assert_eq!(porcelain(&l.records), "", "every record write was committed in the records repository");
    // tasks/files/<id>/ lives under l.project
}
```

- [ ] **Step 2: Write the stale-copy test**

```rust
#[test]
fn stale_copy_never_fires_across_code_worktrees_and_fires_across_records_worktrees() {
    // two code worktrees (l.wt and a second) note the same id in turn: both succeed
    // then: git -C l.records worktree add -b side <base>/records-side; edit the record there by `tasks -C <side>/det note`;
    // a note from l.code now fails with "stale_copy"
}
```

- [ ] **Step 3: Run them, fix what they expose, run again**

Run: `just test-one --test detached lifecycle` and `just test-one --test detached stale_copy`
Expected: PASS. A `porcelain` diff names the command that wrote under `root`; fix its path to `records` and rerun.

- [ ] **Step 4: Check and commit**

Run: `just test-fast` then `just check`
Expected: PASS.

```bash
git add src tests/detached.rs
git commit -m "test(detached): the lifecycle writes only under records; stale copies follow the records repository"
```

---

### Task 5: `tasks init --records`

**Complexity:** high

**Files:**
- Modify: `src/cli.rs` (`Init.records`)
- Modify: `src/commands/init.rs` (`run` signature, `validate_records`, `reject_linked_destination` kind-aware)
- Modify: `src/repo.rs` (`Project::init_detached`, `add_checkout_table`)
- Modify: `src/link.rs` (`config_in_index`)
- Modify: `src/registry.rs` (nothing new; `pair_records` from Task 3)
- Modify: `src/output.rs` (`InitOut.records`)
- Modify: `src/error.rs` (`RecordsInTree`)
- Modify: `src/commands/mod.rs:1934-1938` (dispatch)
- Modify: `tools/cli.toml` by `vendored adopt` only
- Test: `tests/detached.rs` (the fixture now uses `init --records`), `tests/linked.rs::init_checkout_refuses_what_it_cannot_link` (unchanged, must pass)

**Interfaces:**
- Produces, in `src/cli.rs`: on `Init`, `/// Keep this checkout's records at <PATH>/<prefix> (a records root kept elsewhere); registers the pairing and writes nothing here.` `#[arg(long, value_name = "PATH", conflicts_with = "checkout")] records: Option<PathBuf>`.
- Produces, in `src/commands/init.rs`: `pub fn run(dir, prefix, checkout, records: Option<PathBuf>, force) -> Result<Output>`. With `records`:
  1. `root` = `dir` or cwd, canonical; `prefix` as today; locks and pending-operation checks as today.
  2. `validate_records(&root, &records_root, &registry, &prefix) -> Result<PathBuf>` (the derived `<records-root>/<prefix>`, canonical after `create_dir_all`): `records_root` is resolved against `root`, must exist and be a directory (`Error::Config("--records <p>: does not exist")`); `root` must be a git top level and a main checkout (`GitTree::of`, `is_main`, `main_checkout() == root`), with the `--checkout` wording ("is a linked worktree; pass its main checkout"); `root` must not lie inside, contain, or share `common` with the derived directory; **the overlap checks of `validate_checkout` (init.rs:227-243) apply to `root`**: for every other registered prefix (the prefix being paired is skipped, so `--force` re-pairing works), `root` must not contain or lie inside its registered root or its records directory, nor share `common` with either (`Error::Config("--records: <root> overlaps registered project <p> at <path>")`), so a repository nested under an in-tree project cannot detach while `locate` would walk up to the enclosing config; `Error::RecordsInTree` when `root.join(CONFIG_REL)` is a file or `config_in_index(&tree.top)?`, with the detail "tasks/.config.toml is in the working tree or the index; `git rm -r tasks` (and the docs you are moving) first, then rerun; a staged deletion is allowed"; `Error::CheckoutConflict` when another prefix's `Linked` row names `root` as its checkout, or another prefix's registered root equals `root` and is paired in `[records]`.
  3. `--force` repoints as today; without it `register` as today (a prefix registered to a different root refuses naming `--force`).
  4. `Project::init_detached(&derived, &prefix, &root)` writes the config, then `registry.pair_records(&prefix, &derived)`, `note_root_change` and `save` as today.
  5. Output: `InitOut { prefix, root, records: Some(derived), warnings, aliases }`.
  Without `records`, `reject_linked_destination` becomes kind-aware: a `Linked` row of kind `Detached` whose `checkout` is this tree's main checkout refuses with `Error::RecordsMoved` whose detail is "`<prefix>`'s records live at <dir>; to pair this checkout again run `tasks init --prefix <prefix> --records <records-root> --force`" (decision 6); a `Linked` row keeps today's `config` refusal.
- Produces, in `src/repo.rs`: `pub fn init_detached(records: &Path, prefix: &str, checkout: &Path) -> Result<Project>`: `create_dir_all(records.join("tasks"))`; when no config exists, write `prefix`, `[git] commit = true`, `[checkout] path = <relative_path(records, checkout)>`, `kind = "detached"`; when one exists, open it, refuse a different prefix (`Config`, as `init` does), then `detach_config(text, path) -> Result<String>` (a `toml_edit::DocumentMut` edit, as `retire::write_table` does, so comments and unrelated tables survive): add `[checkout]` with `path` and `kind = "detached"` when the table is absent, else require `kind = "detached"` and the same `path` (`Config("init does not relink an existing project")`); set `[git] commit = true` when the table or the key is absent or the value is `false` (§3.4). Write the file only when the text changed. Then create `docs/specs` and `docs/plans` under `records` and return `open(records)`.
- Produces, in `src/link.rs`: `pub fn config_in_index(top: &Path) -> Result<bool>`: `git -C top ls-files -z --cached -- tasks/.config.toml` prints a path. A failing git run is an `Io` error, never "absent".
- Produces, in `src/output.rs`: `InitOut.records: Option<String>` with `skip_serializing_if = "Option::is_none"`.
- Produces, in `src/error.rs`: `RecordsInTree(String)` → `records_in_tree`.

- [ ] **Step 1: Add the `init --records` route to the fixture**

In `tests/detached.rs`, `layout` no longer writes the config or commits; add:

```rust
/// Registers `prefix` the way a person does: `init --records`, then the records commit.
pub fn register(env: &TestEnv, l: &Layout, prefix: &str) {
    env.json(&l.code, &["init", "--prefix", prefix, "--records", l.records.to_str().unwrap()]);
    git(&l.records, &["add", "-A"]);
    git(&l.records, &["commit", "-qm", "init"]);
}
```

`register_by_hand` now also writes the config `layout` used to write (the tests that need a registry state `init --records` refuses keep using it). Switch every test of Tasks 2–4 that does not test a refusal to `register`, keeping their assertions. The config `init_detached` writes must parse to the same `Project` the hand-written one did.

- [ ] **Step 2: Write the failing init tests in `tests/detached.rs`**

```rust
#[test]
fn init_records_derives_the_directory_writes_the_config_and_pairs_the_registry() {
    let mut env = TestEnv::new();
    let l = layout(&mut env, "det");
    let out = env.json(&l.code, &["init", "--prefix", "det", "--records", l.records.to_str().unwrap()]);
    assert_eq!(out["records"], l.project.display().to_string());
    let config = std::fs::read_to_string(l.project.join("tasks/.config.toml")).unwrap();
    assert!(config.contains("prefix = \"det\""));
    assert!(config.contains("kind = \"detached\""));
    assert!(config.contains("path = \"../../code\""));
    assert!(config.contains("commit = true"));
    assert!(l.project.join("docs/specs").is_dir() && l.project.join("docs/plans").is_dir());
    assert!(!l.code.join("tasks").exists());
    let registry = std::fs::read_to_string(env.home.path().join(".config/tasks/projects.toml")).unwrap();
    assert!(registry.contains("[records]"));
    assert_eq!(env.json(&l.code, &["where"])["project"]["prefix"], "det"); // the pairing opens it from the code checkout
}

#[test]
fn init_records_refuses_while_the_config_is_in_the_tree_and_allows_a_staged_deletion() {
    // in-tree project in l.code: `tasks -C l.code init --prefix det`, commit; then:
    // (a) env.fail(&l.code, &["init", "--prefix", "det", "--records", ..]) == "records_in_tree"   (working tree)
    // (b) `rm tasks/.config.toml` (the index still holds it): still "records_in_tree"
    // (c) `git rm -r -q tasks` (index deletes, HEAD tracks): succeeds without --force, since the prefix is
    //     registered to this same root; warnings name no previous root
    // (d) git commit -qm "detach": the config is gone from HEAD; env.json(&l.code, &["list"]) opens the detached project
}

#[test]
fn init_records_force_keeps_aliases_groups_and_location_history() {
    // register an in-tree project, rename it (alias), group set g <p>, then detach with --force:
    // registry keeps [aliases], [groups], [locations.<p>]; `tasks resolve <old alias>` still resolves
}

#[test]
fn init_records_on_an_existing_records_config_adds_checkout_and_enables_commits() {
    // write records/<p>/tasks/.config.toml by hand with prefix, a comment line, [tags] x = "y", no [checkout], no [git]:
    //   init --records: the file keeps the comment and [tags], gains [checkout] path + kind and [git] commit = true
    // the same with `[git]\ncommit = false\n`: commit becomes true, the comment survives
    // the same with commit = true and the right [checkout] already present: bytes unchanged
    // a prefix mismatch is "config"; a [checkout] with another path is "config"
}

#[test]
fn init_records_refuses_a_checkout_that_overlaps_another_registered_project() {
    // an in-tree project at base/outer (env.init); a git repo at base/outer/inner:
    // env.fail(&inner, &["init", "--prefix", "inn", "--records", ..]) == "config", detail names "outer";
    // a repo containing a registered root, and one sharing a common dir (a worktree's main), refuse the same way
}

#[test]
fn init_records_is_idempotent_on_the_second_host() {
    // after register(): remove the registry file; rerun init --records --force: config bytes unchanged, registry paired again
}

#[test]
fn init_records_refusals() {
    // from l.wt (a linked worktree): "config", detail names the main checkout                 (Review Focus 2)
    // --records pointing at a missing directory: "config"
    // --records with --checkout: exit 2 (env.usage)
    // another prefix linked to l.code (tests/linked.rs::project fixture): "checkout_conflict"
    // plain `init --prefix det` in l.code after register(): "records_moved"                   (decision 6)
}
```

- [ ] **Step 3: Run them to verify they fail**

Run: `just test-one --test detached init_records`
Expected: FAIL (`--records` is unexpected: exit 2).

- [ ] **Step 4: Adopt the CLI row and implement**

```bash
python3 "$(tasks --pretty root ops-88a0fa)/bin/vendored" adopt cli.toml
```

Then implement the Interfaces block. `parser_surface_equals_table` must pass with the adopted table.

- [ ] **Step 5: Run the tests**

Run: `just test-one --test detached` then `just test-one --test linked init_checkout` then `just test-one surface`
Expected: PASS.

- [ ] **Step 6: Check and commit**

Run: `just test-fast` then `just check`
Expected: PASS.

```bash
git add src tests tools/cli.toml
git commit -m "feat(init): tasks init --records pairs a checkout with its records directory"
```

---

### Task 6: `projects`, `where` and `resolve` report the three locations

**Complexity:** mid

**Files:**
- Modify: `src/output.rs` (`WhereProject`, `ProjectRow`, `ResolveRow`, pretty output for `where`, `projects --paths`, `resolve`)
- Modify: `src/commands/where_.rs`, `src/commands/projects.rs` (and its `row()` test helper), `src/commands/resolver.rs`
- Modify: `src/locations.rs` (`Via::Records`, `Index::build`)
- Modify: `src/link.rs` (`holds_records` already by `config_dir` from Task 3; nothing new)
- Test: `tests/detached.rs`; `tests/linked.rs::where_answers_linked_in_tree_and_no_project` and `projects_rows_carry_checkout_only_when_linked` are rewritten for the new shape

**Interfaces:**
- Produces, in `src/output.rs`:
  - `WhereProject { prefix, root: String, checkout: String, records: String, kind: Kind, linked: bool, color }`.
  - `ProjectRow` gains `records: String` (always: the registry's pairing, else the root) and `kind: Option<Kind>` (`skip_serializing_if = "Option::is_none"`, set when the config was read); `checkout` keeps its attribute and is set on every row whose config was read (decision 3).
  - `ResolveRow` gains `records: Option<String>` (`skip_serializing_if`), set whenever `root` is.
  - Pretty `where`: `<prefix> <root>`, `<prefix> <root> (linked to <checkout>)`, or `<prefix> <root> (records at <records>)`.
  - Pretty `projects --paths`: the root column stays; a detached row appends ` → <records>` after the root.
- Produces, in `src/locations.rs`: `Via` becomes `Root, Storage, Checkout, Records, FormerRoot, FormerStorage, Git` (declaration order is the ranking; `Records` is `current()`). `Index::build` adds `Via::Records` at every `registry.records` value, and adds `Via::Checkout` only for `Linked` rows with `kind == Linked`.
- Produces, in `src/commands/resolver.rs`: every row that sets `root` also sets `records` from `registry.records_dir(prefix)` falling back to the root.

- [ ] **Step 1: Rewrite the two linked-shape tests and write the failing detached tests**

In `tests/linked.rs`: `where_answers_linked_in_tree_and_no_project` expects, for in-tree, `checkout == root`, `records == root`, `kind == "in-tree"`, `linked == false`; for linked, `records == root`, `kind == "linked"`. `projects_rows_carry_checkout_only_when_linked` becomes `projects_rows_carry_checkout_records_and_kind_on_every_reachable_row`.

In `tests/detached.rs`:

```rust
#[test]
fn where_reports_kind_records_and_checkout_for_every_shape() {
    // detached from l.code, l.wt, l.code/src and l.project:
    //   project.{prefix:"det", root:l.code, checkout:l.code, records:l.project, kind:"detached", linked:false}
    //   holds_records: false from l.code and l.wt; true from l.project (the records repository holds records)
    // from l.records (above every project directory): project null
    // pretty: "det <code> (records at <project>)"
}

#[test]
fn projects_rows_carry_the_three_locations_and_the_kind() {
    // row for det: root == l.code, checkout == l.code, records == l.project, kind == "detached"
    // after `rm -r l.project` (the records directory is gone): the row is reachable false, records == l.project,
    // and has no checkout and no kind keys (decision 3)
}

#[test]
fn resolve_matches_a_path_under_records_via_records_and_under_the_checkout_via_root() {
    // resolve l.project.join("tasks/det-000000.md") -> prefix det, via "records", records == l.project, relpath "tasks/det-000000.md"
    // resolve l.code.join("src/main.rs") -> via "root", records == l.project
    // resolve l.wt.join("x") -> via "storage" (the code checkout's .worktrees link), worktree "wt"
    // resolve "det" (a prefix) -> root l.code, records l.project
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `just test-one --test detached where_reports` and `just test-one --test linked where_answers`
Expected: FAIL (no `kind` key).

- [ ] **Step 3: Implement the output changes**

As the Interfaces block says. `where_.rs::shown(project)` drops its `linked` parameter and derives `linked` from `project.kind`.

- [ ] **Step 4: Run the tests**

Run: `just test-one --test detached` then `just test-one --test linked` then `just test-one --test resolve`
Expected: PASS.

- [ ] **Step 5: Check and commit**

Run: `just test-fast` then `just check`
Expected: PASS.

```bash
git add src tests
git commit -m "feat(output): projects, where and resolve report records and kind"
```

---

### Task 7: `check` in a detached code checkout

**Complexity:** low

**Files:**
- Modify: `src/commands/check.rs` (the `records_moved` error for an index-held config; `checkout_missing` wording for detached)
- Test: `tests/detached.rs`

**Interfaces:**
- Consumes: `link::config_in_index` (Task 5), `Project.worktree` (set when the project was reached from a code tree).
- Produces: `check` run with `ctx.project.kind == Kind::Detached` and `ctx.project.worktree == Some(top)` returns `Err(Error::RecordsMoved(...))` when `config_in_index(&top)?`, with the §3.3 detail. A HEAD that tracks the config while the index deletes it passes (the removal commit's window, §5.1). `checkout_missing`'s detail reads `[checkout] path names <root>, which is not on this machine` for both linked and detached.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn check_in_the_code_checkout_refuses_an_index_held_config_and_passes_a_staged_deletion() {
    // in-tree project in l.code committed, then register_by_hand (init --records would refuse this state):
    // (a) `rm tasks/.config.toml` (index still holds it; the walk misses; the pairing opens the detached project):
    //     env.fail(&l.code, &["check"]) == "records_moved"
    // (b) `git rm -r -q tasks` (index deletes, HEAD tracks): env.check(&l.code) has no errors
    // (c) commit the removal: env.check(&l.code) still clean; env.check(&l.project) clean
}

#[test]
fn check_warns_when_the_code_checkout_is_missing() {
    // Review Focus 4: register(); remove l.code entirely (and base/storage/code); env.json(&l.project, &["list"]) succeeds;
    // env.check(&l.project)["warnings"] contains kind "checkout_missing"
}
```

- [ ] **Step 2: Run them to verify they fail, implement, run again**

Run: `just test-one --test detached check_`
Expected: FAIL on (a) (check passes today), then PASS.

- [ ] **Step 3: Check and commit**

```bash
just test-fast && just check
git add src tests/detached.rs
git commit -m "feat(check): a detached checkout whose index still holds the config is records_moved"
```

---

### Task 8: Documents resolve under `records`, then the code tree

**Complexity:** mid

**Files:**
- Modify: `src/resolve.rs` (`Located::Code`, `LocatedEntry::Code`, `bases`, `locate`, `locate_entry`, `find`/`doc_matches`, `read`, `abs`)
- Modify: consumers of the new variants: `src/refs.rs:33`, `src/inventory.rs:36`, `src/commands/check.rs:654`, `src/commands/show.rs:173`, `src/moving/preflight.rs:195`
- Test: `tests/detached.rs`

**Interfaces:**
- Produces, in `src/resolve.rs`:
  - `pub enum Located { Here(PathBuf), Main { root, path }, Code { root: PathBuf, path: PathBuf }, Missing }` and the same `Code` variant on `LocatedEntry`. `Code` is a hit under the project's code tree for a detached project: readers treat it as `Here` (no `ref_in_main_checkout` warning: a retained reference document lives there by design, §7.3).
  - `fn bases(&self) -> Result<Vec<Base>>` where `enum Base { Here, Main(PathBuf), Code(PathBuf) }`: `Here` (records), then `Main` when the records directory is a worktree of its repository (today's `main_root`), then for `kind == Detached` `Code(resume_root)` and `Code(root)` when different. `locate`, `locate_entry`, `find` (bare-name search through `doc_matches`), `read` and `abs` walk `bases` in order and stop at the first hit.
- Consumes: `Project::doc_path`, `Project::resume_root`, `Project::kind`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_spec_under_records_and_a_retained_spec_in_the_code_tree_both_resolve() {
    // register(); write l.project/docs/specs/a-design.md and l.code/docs/specs/kept-design.md (commit in code)
    // add t1 --spec docs/specs/a-design.md; add t2 --spec docs/specs/kept-design.md: both succeed from l.code
    // show t1: spec_path == l.project/docs/specs/a-design.md; show t2: spec_path == l.code/docs/specs/kept-design.md
    // from l.wt, a doc that exists only on the wt branch resolves there (the worktree before the main checkout)
    // tasks check from l.code: no errors, no ref_in_main_checkout warning for t2
    // plan with a step: add t3 --plan docs/plans/p.md --step "Task 1: x" (plan under records): show step_found == true (§12 case 4)
}

#[test]
fn an_output_ref_under_records_satisfies_the_completion_gate() {
    // Review Focus 5: add t --ref 'rep (output): docs/notes/r.md'; done refuses ref_output_missing;
    // write l.project/docs/notes/r.md; done succeeds. The same with the file under l.code/docs/notes/ also succeeds.
}
```

- [ ] **Step 2: Run them to verify they fail, implement, run again**

Run: `just test-one --test detached a_spec_under_records`
Expected: FAIL (the retained spec is `doc_not_found`), then PASS.

- [ ] **Step 3: Check and commit**

```bash
just test-fast && just check
git add src tests/detached.rs
git commit -m "feat(resolve): documents resolve under records, then the code worktree and checkout"
```

---

### Task 9: `rename` moves the records directory

**Complexity:** high

**Files:**
- Modify: `src/commands/rename.rs` (`invocation` builds `root` from `scope::config_dir`; the adopt route finds the renamed directory)
- Modify: `src/rename/inventory.rs` (`Inventory.records_to`)
- Modify: `src/rename/snapshot.rs` (observe which of the two directories exists)
- Modify: `src/rename/classify.rs` (the move stage's states and refusals)
- Modify: `src/rename/mod.rs` (the move stage; `registry.pair_records` after `registry.rename`)
- Modify: `src/rename/adopt.rs` (`registry.pair_records` after `registry.adopt`)
- Test: unit tests in `src/rename/classify.rs` (the boundary enumeration gains the new state), `tests/detached.rs`

**Interfaces:**

Two paths run through rename and are kept apart everywhere: the project's **identity**, the registered root the registry's `[projects]` holds (the code checkout for a detached project; the overlay directory for a linked one; the root for in-tree), and its **current records directory**, where `tasks/` and the config are now. Today both are `root`; a detached project is the first where they differ, and where the second one changes during the rename.

- Produces, in `src/rename/snapshot.rs`: `pub struct Invocation { pub source: String, pub target: String, pub root: PathBuf /* identity */, pub records: PathBuf /* current records directory */ }`. `observe` reads `tasks/`, `tasks/files/`, strays and the config under `invocation.records`; `registry.old_key`/`new_key` keep comparing `[projects]` with `invocation.root`; `Snapshot.registry` gains `records_old: bool` (`registry.records_dir(source) == inventory.records_from`) and `records_new: bool` (`registry.records_dir(target) == inventory.records_to`); `Snapshot.records: RecordsDir` with `pub enum RecordsDir { Old, New, Both, Neither }` from the existence of `records_from` and `records_to` (`Old` whenever the inventory has no `records_from`, and before an inventory exists when `config_dir` exists).
- Produces, in `src/rename/inventory.rs`: `Inventory.root` stays the identity; `pub records_from: Option<PathBuf>` and `pub records_to: Option<PathBuf>`, both `#[serde(default, skip_serializing_if = "Option::is_none")]`: `Some(<records-root>/<old>)` and `Some(<records-root>/<new>)` for a detached project, `None` otherwise. An inventory from an older binary has neither and means "records at `root`", which is what it was. `Inventory::build(project, target)` fills them from `project.kind` and `project.records`.
- Produces, in `src/rename/classify.rs`: `registry_old` additionally requires `records_old` when `records_from` is set, and `registry_new` requires `records_new` likewise. Under `RecordsDir::Old` today's rules stand (files, then config, under `records_from`); `New` with the config digest `config_to` and the registry old → `ResumeRegistry`; `New` with the registry new → `ResumeCleanup`; `Both` → `Refuse("both <old> and <new> exist; remove the one that is not the renamed project")`; `Neither` → `Refuse("neither <old> nor <new> exists")`. `refusal()`'s R3 and every path it builds use the snapshot's current records directory.
- Produces, in `src/rename/mod.rs`: `Project::open(&invocation.records)` (never `root`); `authorize`'s worktree count and the dirty-tree check run in the records repository through that project (Task 2). After the `config` boundary and before `registry`, when `records_to` is `Some(to)`: `std::fs::rename(&records_from, to)?`, boundary name `records` for `TASKS_RENAME_STOP_AFTER`; from here on nothing opens `records_from`. In the `registry` stage, after `registry.rename`, `registry.pair_records(&invocation.target, to)` in the same save, so the inventory's old path and the registry's new path reconcile in one step.
- Produces, in `src/commands/rename.rs`: `invocation` sets `root` as today (the identity from `[projects]`, else the inventory's) and `records` as: a pending inventory's `records_to` when that exists and its `records_from` does not; else the pending inventory's `records_from`; else `scope::config_dir(registry, source)`; else `root`. The pending-rename identity check at `rename/mod.rs:134` (`root_identity(&pending.root) == invocation.root`) stays an identity comparison. For `--adopt`, when `registry.records_dir(old)` is `Some(dir)`, `dir` does not exist and `dir.parent().join(new)` holds a config whose prefix is `new`, `records` is that directory; `adopt::run` opens it and calls `registry.pair_records(new, &project.records)` after `registry.adopt`.
- Produces, in `src/registry.rs`: nothing new (`rename` and `adopt` move the key from Task 3; `pair_records` updates the value).

- [ ] **Step 1: Write the failing classify unit tests**

Extend the boundary table in `src/rename/classify.rs` with the `records` state: every existing row with `RecordsDir::Old` and `records_old`/`records_new` true, plus rows for `New`+`config_to`+registry old → `ResumeRegistry`, `New`+registry new → `ResumeCleanup`, `Old`+registry key old but `records_old` false → `Refuse`, `Both` → `Refuse`, `Neither` → `Refuse`. The exhaustive `#[ignore]` enumeration gains the dimension.

Run: `just test-one classify::`
Expected: FAIL (no `records` field on `Snapshot`).

- [ ] **Step 2: Write the failing integration tests**

```rust
#[test]
fn rename_moves_the_records_directory_and_the_registry_pairing() {
    // register(); add two tasks; commit the records repo;
    // tasks -C l.code rename det dtx (a fresh detached rename is not refused: the registry's [projects] still
    // names l.code, which is the invocation's identity): l.records/dtx/tasks/dtx-*.md exist, l.records/det gone,
    // registry [projects] dtx = code, [records] dtx = l.records/dtx, [aliases] det = "dtx";
    // tasks -C l.code list opens dtx; tasks show det-<hex> resolves through the alias;
    // [checkout] path in the moved config is still "../../code"; `git -C l.records status --porcelain` shows the rename for the user to commit
}

#[test]
fn rename_resumes_across_the_records_boundary() {
    // as tests/cli.rs::rename_every_mutation_boundary_resumes…: TASKS_RENAME_STOP_AFTER=inventory, files, config, records, registry, store, claims,
    // each followed by --explain (the verdict names the next stage; after `records` it is resume_registry and nothing opens l.records/det)
    // and a rerun without the variable: Complete, with the same end state as the uninterrupted rename
}

#[test]
fn rename_refuses_a_second_worktree_of_the_records_repository_not_of_the_code_repository() {
    // l.wt exists (a code worktree): rename succeeds. Then `git -C l.records worktree add …`: rename back is "validation"
}

#[test]
fn adopt_pairs_the_renamed_records_directory() {
    // simulate the other host: register(); then `mv l.records/det l.records/dtx` and rewrite its prefix by hand (as the sync delivers it);
    // tasks -C l.code rename det dtx --adopt: registry [records] dtx = l.records/dtx; list opens dtx
}
```

- [ ] **Step 3: Implement**

As the Interfaces block says. Keep the stage order: files, config, records, registry, store, claims.

- [ ] **Step 4: Run the tests, the exhaustive enumeration included**

Run: `just test-one classify::` then `just test-one --test detached rename` then `just test-one --test cli rename`
Expected: PASS, including every existing rename and adopt test.

Run: `just test`
Expected: PASS. `test-fast` skips the `#[ignore]`d `rename::classify` enumeration this task extends, and the branch's only other full run is the baseline before any change; this run is the one that exercises it.

- [ ] **Step 5: Check and commit**

```bash
just test-fast && just check
git add src tests/detached.rs
git commit -m "feat(rename): move a detached project's records directory and its registry pairing"
```

---

### Task 10: README and the spec status

**Complexity:** low

**Files:**
- Modify: `README.md` (`## Linked projects` becomes `## Linked and detached projects`; the registry shape under `### Location history`; `## Layout`'s registry line; the error kinds list)
- Modify: `docs/specs/2026-10-22-records-home-design.md` (line 3 status)

**Interfaces:** none.

- [ ] **Step 1: Document the detached shape**

In `README.md`, after the linked paragraphs: the three locations table from spec §3.1; `tasks init --prefix <p> --records <records-root>` with what it writes (the config, `[git] commit = true` enabled on an existing config too) and refuses (`records_in_tree`, the staged-deletion window, `checkout_conflict`, overlap with another registered project); the `[records]` registry table with a TOML example; the `where` shape for a detached project (`kind: "detached"`, `records`, `checkout`, `linked: false`); the `projects` row shape, including that an unreachable row carries `records` but no `checkout` or `kind` (decision 3); that `where` outside git does not consult the registry, so a copied historical config outside git opens there while `locate` refuses it (decision 4); `records_moved` and `records_unpaired` with their remedies; that `rename` moves the directory and leaves the rename for the user to commit; that each host's binary must be current before the first detached project (an older binary drops `[records]` on its next registry save).

- [ ] **Step 2: Mark the spec**

Line 3 of the spec becomes `Task: tasks-dc16eb. Status: approved 2026-10-22 at revision 3; the tasks CLI part implemented by docs/plans/2026-10-22-records-detached.md (tasks-211e28).`

- [ ] **Step 3: Check and commit**

```bash
just check
git add README.md docs/specs/2026-10-22-records-home-design.md
git commit -m "docs: detached records in the README; spec status"
```

---

## Rollout (after the branch is reviewed; operator steps, not implementer tasks)

0. **The full suite on the branch head.** `just test` in `.worktrees/records-detached` after the final review's fixes; the pre-push hook would run it, but a local merge never pushes, and `test-fast` skips the enumeration Task 9 extended.
1. **ops-2a0024 first.** Confirm `linked.py`'s tolerant version is installed on both hosts (`tests/test_linked.py` in ops covers both shapes). Until then the `where` shape change in Task 6 would make the foreign-write guard refuse every write.
2. **Merge locally** (personal profile): `git merge --no-ff records-detached` on main, then `cargo install --path .` on this host, and on the other host from the same commit before any project is detached: an older binary cannot read `[records]` and drops the table on its next registry save.
3. **Close the task**: `tasks done tasks-211e28` on the branch before the merge, as the session protocol says.
4. **Downstream**: tasks-a8e28e (`tasks docs`) and tasks-00723a (skill wording) in this repository; the ops migration script and hook pieces (ops-423f23 and its dependencies) read `kind` and `records` from `tasks projects` and `tasks where`.
