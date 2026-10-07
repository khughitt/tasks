# Linked Projects (tasks piece 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A tasks project whose code lives in another checkout: `[checkout] path` in its
config, `locate` that finds it from that checkout on a miss, claims and parks that record
the code worktree while records stay in the overlay, `tasks init --checkout`, `tasks where`,
and `checkout` on linked rows of `tasks projects`.

**Architecture:** One new module, `src/link.rs`, owns everything git-shaped about a link:
a path's git facts (`GitTree`), the registered project whose `[checkout]` names a
repository's main checkout (`linked_project`), and whether a repository keeps records
(`holds_records`). `src/locations.rs` gains the one scan of registered checkouts (`scan_checkouts`), which
keeps each project's success or failure: `link.rs` and `init` read it strictly,
`resolve`'s index leniently, per project. `Project` gains two
fields: `checkout` (declared, from the config) and `worktree` (the code working tree the
command ran in, set only when the project was found through the link); `resume_root()` is
the worktree when set, else the root, and every claim, park, and owner lookup uses it.

**Tech Stack:** Rust (clap, serde, toml), integration tests with `assert_cmd` in temp repos
under a temp `HOME`, `just test-one` / `just test-fast`.

**Spec:** ops `docs/specs/2026-10-01-linked-checkouts-design.md` (approved revision 5 with
the §8 amendments; on ops branch `feat/shared-repos` until that branch merges: read it in
ops's `.worktrees/shared-repos/`). Sections for this piece: decisions 4–5, §4.1, §4.1a
(the error table's `tasks where` column), §6 "tasks (1)", "Nested foreign checkout",
"Resolver outcomes", §8.1, §8.2. Also this repo's
`docs/specs/2026-10-06-project-resolution-design.md` §4.2 and §5 (the shared location
module and the `checkout` location kind). The ops plan's Global Constraints
(`docs/plans/2026-10-01-linked-checkouts.md` on the same branch) fix the JSON contract
consumers already test against.

## Global Constraints

- `tasks where [PATH] --json` outcomes: `{"project": {"prefix", "root", "checkout", "linked"}, "holds_records": bool, "warnings": []}`; `{"project": null, "holds_records": bool, "warnings": []}`; error: exit 1 with `{"error": {"kind", "detail"}}`.
- Stream contract: success JSON on stdout with exit 0; error JSON on stderr with exit 1 and nothing on stdout (the CLI's existing behaviour; do not change it).
- In `where`'s project object, `checkout` is a non-empty string when `linked` is true and JSON `null` (key present) when `linked` is false. ops's `bin/linked.py` rejects any other shape.
- `holds_records` counts a top-level `tasks/.config.toml` only when it is tracked in `HEAD` (spec decision 5, plan review round 1), and only as a regular file: `git ls-tree HEAD -- tasks/.config.toml` must name a `blob` of mode `100644` or `100755`. A tree or a symlink at that path is not a config (`git cat-file -e` accepts a tree, so it is not the test).
- No command writes into a linked checkout, and that includes `tasks init` run without `--checkout`: every init destination inside a repository that a registered project declares as its checkout is refused, judged by repository identity (common git directory), so a worktree of that repository stored anywhere is refused too.
- `tasks projects --json` rows carry `checkout` (absolute path) only for linked projects; the key is absent on every other row.
- `where` never crosses a git boundary: inside a git repository its walk for `tasks/.config.toml` stops at that repository's top level, and its link step matches that repository's main checkout. Outside git it walks as the CLI does today.
- `Project::locate` keeps its walk up unchanged (spec §4.1 "keeps the walk up"); only on a miss, inside git, does it try the link. An in-tree project never loads the registry in `locate`.
- No command writes into a linked checkout. This is a test (`git status --porcelain --ignored` in the code repository identical before and after), not a convention.
- Errors stay errors: *no project* is the only fall-through. `checkout_conflict`, a malformed config of a registered project, and an unreadable registry are errors from `locate` and `where` alike.
- `[checkout] path` is written relative to the project root by `init --checkout`; on read, an absolute path is accepted as written (ops's test stub writes one).
- A new CLI option or command is a `cli.toml` change in ops first; this repo runs `vendored adopt cli.toml` in the commit that adds the parser surface (ops Rules).
- Never call `cargo test` directly: `just test-one <filter>` or `just test-one --test linked <filter>` while working, `just test-fast` before each commit. Cargo takes one positional filter: run one invocation per filter, never several filters in one.
- The crate is a binary with private modules and pre-commit runs `cargo clippy --all-targets -- -D warnings`, so every function, method, field, and variant lands in the task that first calls or reads it. A task whose commit leaves one unused fails the gate.
- Ops is changed only in an ops worktree under `.worktrees/`, merged to ops main locally after its gate passes (a personal-profile repository).
- JSON shapes change only as this plan states. Conventional commits; no AI attribution.
- Avoid machine-specific absolute paths in code comments and docs.

## Decisions this plan makes (flag in review if wrong)

1. **`locate`'s walk stays unbounded; `where`'s walk is bounded.** The spec says both
   ("`Project::locate` keeps the walk up" in §4.1, "`where` never crosses a git boundary"
   in decision 4). They differ only for a foreign clone nested inside a registered
   project's tree: a command run there opens the parent, as today, while `where` reports
   no project. Bounding `locate` too would change every command run in such a clone;
   nothing in the spec asks for that.
2. **`open_recorded` tries the link only where the recorded worktree has no config.** It
   does not walk up from a recorded worktree: a recorded in-tree worktree whose config was
   deleted would otherwise walk up into the main checkout's copy, which is a behaviour
   change. A recorded code worktree is always a git top level, so the link step alone
   reaches it.
3. **`init --checkout` on an existing config only accepts the same checkout.** It does not
   rewrite an existing config (that would drop its comments); it refuses with the line to
   add by hand. `ops-projects link` always initializes a fresh directory.
4. **A registered project whose root is gone or holds no config is skipped by the link
   scan**, with no warning: it cannot be opened on this host, so nothing can be linked to
   it. A config that exists and does not parse is an error.
5. **`resolve` reads checkouts leniently, per project.** The scan returns every project it
   read and every project it could not (`CheckoutScan { linked, failed }`). `locate`,
   `where`, and `init` call `.strict()`, which fails on the first failure; `Index::build`
   adds `via: "checkout"` locations for every project that read and one warning per
   project that did not, as it already does for unreadable storage links. One broken
   config never hides another project's checkout from `resolve`.
6. **`init` refuses inside a linked checkout's repository**, with or without
   `--checkout` (plan review round 1, P1). The refusal names the linked project and its
   root, which is where that repository's records already live.

## Review Focus

1. **A hand-written or stub-written absolute `[checkout] path`.** ops's `tests/stub_tasks.py` writes `path = "<absolute>"`; the real CLI must accept it and match it the same as a relative one. Test in Task 2.
2. **The overlay is not on this host yet** (Dropbox has not synced it, or the project is registered but its root is missing). A command in the code checkout must answer `no_project`, not an error, and `where` must answer `project: null`. Test in Task 2 (locate) and Task 4 (`where`).
3. **The declared checkout reached through a symlink** (`~/d/...` vs the Dropbox realpath). The config may name the symlinked spelling; matching must compare normalized real paths. Test in Task 2.
4. **`where` given a file, a relative path, or a path that does not exist.** A file answers for its directory; a relative path resolves against `-C`/cwd; a missing path is an `io` error with exit 1, never `project: null`. Test in Task 4.
5. **`init --checkout` given a relative path or a path with a space.** The relative argument resolves against the directory `init` runs in (its `-C`), and the stored relative path round-trips through TOML so `where` from the checkout finds the project. Test in Task 4.

---

## File Structure

- Create `src/link.rs` — `GitTree` (a path's git top, common dir, git dir; main checkout;
  worktree list), `linked_project`, `holds_records`, `relative_path`.
- Modify `src/locations.rs` — `Linked`, `CheckoutScan`, `scan_checkouts`; `Via::Checkout`
  and its locations in `Index::build`.
- Modify `src/repo.rs` — `[checkout]` config table; `Project.checkout`, `Project.worktree`,
  `resume_root()`; `locate`'s link step; `Project::init` takes the checkout.
- Modify `src/error.rs` — `Error::CheckoutConflict` (`checkout_conflict`).
- Modify `src/commands/mod.rs` — claim/park/owner use `resume_root()`;
  `lock_and_revalidate` keeps the worktree; dispatch for `where` and `init --checkout`.
- Modify `src/commands/park.rs`, `src/commands/parked.rs` — park worktree; `open_recorded`
  link step; `recorded_is_scanned` compares the resume root.
- Create `src/commands/where_.rs` — the `where` command (`where` is a keyword).
- Modify `src/commands/init.rs` — the destination check on every init; `--checkout`
  validation.
- Modify `src/commands/projects.rs`, `src/output.rs` — `checkout` on rows; `WhereOut`.
- Modify `src/commands/check.rs` — `checkout_missing` finding.
- Modify `src/cli.rs`, `src/main.rs` — the parser surface.
- Modify `tools/cli.toml` — adopted from ops main with `vendored adopt cli.toml`.
- Create `tests/linked.rs` — every end-to-end test of this plan, with its fixtures.
- Modify `README.md`, `skills/tasks/SKILL.md` — linked projects, `where`, `init --checkout`.

---

### Task 1: Land the CLI vocabulary (ops piece 0) on ops main

The rows exist on ops branch `feat/shared-repos` (commit `91f191b`, "feat(cli): declare
tasks where and init --checkout") but not on ops main, and `vendored adopt` copies what
ops main has landed. The ops plan allows its Tasks 1–3 to merge to main at any time. The
change is made in an ops worktree and merged locally (Global Constraints); only the
commit's own hunks are applied, so vocabulary ops main has landed since `91f191b` (obs's
`tests` view, for one) is kept.

**Files:**
- Modify (in an ops worktree, then ops main): `cli.toml`, `tests/test_cli.py`

**Interfaces:**
- Produces: ops main's `cli.toml` with `{ names = ["--checkout"], value = "path" }` on
  `init` and a `where` command row (`summary = "The project a path belongs to, in-tree or
  linked, and whether its repository keeps records"`, one optional `path` arg of value
  `path`). Task 4 adopts it.

- [ ] **Step 1: Create the ops worktree**

In the ops main checkout (clean tree on `main`):

```bash
work-link --ensure .worktrees
git worktree add -b feat/cli-where .worktrees/cli-where main
git worktree lock --reason "on WORK_ROOT storage (host: $(uname -n))" .worktrees/cli-where
```

ops's justfile defines no `setup`; its guide names none beyond a checkout.

- [ ] **Step 2: Apply only the commit's hunks**

In `.worktrees/cli-where`:

```bash
git show 91f191b -- cli.toml tests/test_cli.py | git apply --index
git diff --cached --stat
```

Expected: `cli.toml` and `tests/test_cli.py` staged, nothing else; `git diff --cached
cli.toml` shows only the `--checkout` line and the `where` command block. If the patch
does not apply, stop and report the conflicting hunk: never resolve it by checking out a
whole file from `91f191b`, which would revert newer rows.

- [ ] **Step 3: Run ops's gate in the worktree**

Run: `just test-one tests.test_cli`
Expected: PASS.
Run: `just test-fast`
Expected: PASS.

- [ ] **Step 4: Commit in the worktree**

```bash
git commit -m "feat(cli): declare tasks where and init --checkout"
```

- [ ] **Step 5: Merge locally and remove the worktree**

From the ops main checkout:

```bash
git merge --ff-only feat/cli-where
tt-report
git worktree unlock .worktrees/cli-where
git worktree remove .worktrees/cli-where
git branch -d feat/cli-where
```

If `--ff-only` refuses (main moved), `git merge --no-ff feat/cli-where` and rerun `just
test-fast` on main. Do not push (a push is outside this repository's authority); Task 4's
adoption reads the local main. When `feat/shared-repos` later merges, git sees the same
hunks on both sides.

---

### Task 2: `[checkout]`, the location scan, and `locate` through the link

**Files:**
- Create: `src/link.rs`
- Modify: `src/repo.rs` (Config, Project, `open`, `locate`), `src/locations.rs`,
  `src/error.rs`, `src/main.rs` or `src/lib.rs` (whichever declares modules: add `mod link;`)
- Test: `tests/linked.rs` (new), unit tests in `src/link.rs`

**Interfaces:**
- Produces:
  - `Project.checkout: Option<PathBuf>` — declared checkout, absolute and normalized.
  - `crate::link::GitTree { common: PathBuf }` with
    `GitTree::of(dir: &Path) -> Result<Option<GitTree>>` and
    `fn main_checkout(&self) -> Option<PathBuf>`. Task 3 adds `top`; Task 4 adds
    `git_dir`, `is_main`, and `worktrees` — each with its first reader (Global
    Constraints: nothing lands unused).
  - `crate::link::linked_project(registry: &Registry, tree: &GitTree) -> Result<Option<Project>>`.
  - `crate::locations::Linked { prefix: String, checkout: PathBuf }`,
    `crate::locations::CheckoutScan { linked: Vec<Linked>, failed: Vec<(String, Error)> }`,
    `crate::locations::scan_checkouts(registry: &Registry) -> CheckoutScan`, and
    `CheckoutScan::strict(self) -> Result<Vec<Linked>>`.
  - `Error::CheckoutConflict(String)`, kind `"checkout_conflict"`.

- [ ] **Step 1: Write the test fixture and the failing locate tests**

Create `tests/linked.rs`:

```rust
mod common;
use common::TestEnv;
use std::path::{Path, PathBuf};
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "init.defaultBranch=main"])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A code repository with one commit, at `base/name`.
fn code_repo(base: &Path, name: &str) -> PathBuf {
    let dir = base.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q"]);
    std::fs::write(dir.join("README"), "code\n").unwrap();
    git(&dir, &["add", "README"]);
    git(&dir, &["commit", "-q", "-m", "init"]);
    dir.canonicalize().unwrap()
}

/// A project at `overlay/prefix`, registered, whose config declares `checkout` as written.
fn project(env: &TestEnv, overlay: &Path, prefix: &str, checkout: Option<&str>) -> PathBuf {
    let dir = overlay.join(prefix);
    std::fs::create_dir_all(&dir).unwrap();
    env.json(&dir, &["init", "--prefix", prefix]);
    if let Some(path) = checkout {
        let config = dir.join("tasks/.config.toml");
        let mut text = std::fs::read_to_string(&config).unwrap();
        text.push_str(&format!("\n[checkout]\npath = {path:?}\n"));
        std::fs::write(&config, text).unwrap();
    }
    dir.canonicalize().unwrap()
}

/// The standard layout: an overlay git repository holding `nexc` linked to `code`, with a
/// worktree of `code` under a `.worktrees` symlink into separate storage.
struct Layout {
    _base: tempfile::TempDir,
    base: PathBuf,
    overlay: PathBuf,
    nexc: PathBuf,
    code: PathBuf,
    wt: PathBuf,
}

fn layout(env: &TestEnv) -> Layout {
    let holder = tempfile::tempdir().unwrap();
    let base = holder.path().canonicalize().unwrap();
    let overlay = base.join("overlay");
    std::fs::create_dir_all(&overlay).unwrap();
    git(&overlay, &["init", "-q"]);
    let code = code_repo(&base, "code");
    let nexc = project(env, &overlay, "nexc", Some("../../code"));
    git(&overlay, &["add", "-A"]);
    git(&overlay, &["commit", "-q", "-m", "overlay"]);
    let storage = base.join("storage/code");
    std::fs::create_dir_all(&storage).unwrap();
    std::os::unix::fs::symlink(&storage, code.join(".worktrees")).unwrap();
    git(&code, &["worktree", "add", "-q", "-b", "wt", ".worktrees/wt"]);
    let wt = code.join(".worktrees/wt").canonicalize().unwrap();
    Layout { _base: holder, base, overlay, nexc, code, wt }
}

#[test]
fn a_linked_checkout_opens_its_overlay_project_from_every_tree() {
    let env = TestEnv::new();
    let l = layout(&env);
    let sub = l.code.join("src/deep");
    std::fs::create_dir_all(&sub).unwrap();
    for (n, dir) in [&l.code, &sub, &l.wt].into_iter().enumerate() {
        let added = env.json(dir, &["add", &format!("From tree {n}")]);
        let id = added["id"].as_str().unwrap().to_string();
        assert!(id.starts_with("nexc-"), "{dir:?} opened {id}");
        assert!(l.nexc.join(format!("tasks/{id}.md")).is_file(), "record lands in the overlay");
    }
    let listed = env.json(&l.wt, &["list"]);
    assert_eq!(listed["tasks"].as_array().unwrap().len(), 3);
}

#[test]
fn two_projects_declaring_one_checkout_is_a_conflict_naming_both() {
    let env = TestEnv::new();
    let l = layout(&env);
    project(&env, &l.overlay, "dup", Some(l.code.to_str().unwrap()));
    let out = env.cmd(&l.code).args(["list"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "checkout_conflict");
    let detail = error["error"]["detail"].as_str().unwrap();
    assert!(detail.contains("nexc") && detail.contains("dup"), "{detail}");
}

#[test]
fn an_in_tree_project_never_scans_the_registry() {
    let env = TestEnv::new();
    let l = layout(&env);
    // Two registered projects claim the in-tree project's own repository: if locate
    // scanned on a hit, every command here would be a checkout_conflict.
    let intree = code_repo(&l.base, "intree");
    env.json(&intree, &["init", "--prefix", "own"]);
    project(&env, &l.overlay, "aaa", Some(intree.to_str().unwrap()));
    project(&env, &l.overlay, "bbb", Some(intree.to_str().unwrap()));
    let added = env.json(&intree, &["add", "Here"]);
    assert!(added["id"].as_str().unwrap().starts_with("own-"));
}

#[test]
fn a_missing_declared_checkout_never_matches() {
    let env = TestEnv::new();
    let l = layout(&env);
    project(&env, &l.overlay, "gone", Some("../../nowhere"));
    let other = code_repo(&l.base, "other");
    assert_eq!(env.fail(&other, &["list"]), "no_project");
}

#[test]
fn a_malformed_config_in_a_registered_project_is_an_error_not_no_project() {
    let env = TestEnv::new();
    let l = layout(&env);
    std::fs::write(l.nexc.join("tasks/.config.toml"), "prefix = \"nexc\"\n[checkout]\npath = 7\n").unwrap();
    assert_eq!(env.fail(&l.code, &["list"]), "config");
}

#[test]
fn outside_git_a_miss_is_still_no_project_without_reading_the_registry() {
    let env = TestEnv::new();
    let plain = tempfile::tempdir().unwrap();
    let registry = env.home.path().join(".config/tasks/projects.toml");
    std::fs::create_dir_all(registry.parent().unwrap()).unwrap();
    std::fs::write(&registry, "not toml = [").unwrap();
    assert_eq!(env.fail(plain.path(), &["list"]), "no_project");
}

#[test]
fn an_absolute_checkout_path_matches_like_a_relative_one() {
    let env = TestEnv::new();
    let l = layout(&env);
    let abs = code_repo(&l.base, "abs");
    project(&env, &l.overlay, "absp", Some(abs.to_str().unwrap()));
    let added = env.json(&abs, &["add", "Absolute"]);
    assert!(added["id"].as_str().unwrap().starts_with("absp-"));
}

#[test]
fn a_checkout_declared_through_a_symlink_matches_its_real_path() {
    let env = TestEnv::new();
    let l = layout(&env);
    let real = code_repo(&l.base, "real");
    let alias = l.base.join("alias");
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    project(&env, &l.overlay, "sym", Some(alias.to_str().unwrap()));
    let added = env.json(&real, &["add", "Through the alias"]);
    assert!(added["id"].as_str().unwrap().starts_with("sym-"));
}

#[test]
fn an_overlay_missing_on_this_host_is_no_project() {
    let env = TestEnv::new();
    let l = layout(&env);
    std::fs::rename(&l.nexc, l.base.join("moved-away")).unwrap();
    assert_eq!(env.fail(&l.code, &["list"]), "no_project");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-one --test linked`
Expected: FAIL — the config parse rejects `[checkout]` (unknown field is not denied on
`Config`, so the first failures are `no_project` from the code checkout), and
`checkout_conflict` does not exist.

- [ ] **Step 3: Add the error kind**

In `src/error.rs`, add the variant after `NotRegisteredCheckout`:

```rust
    #[error("{0}")]
    CheckoutConflict(String),
```

its kind in `kind()`:

```rust
            Error::CheckoutConflict(_) => "checkout_conflict",
```

and its arm in `with_suffix`:

```rust
            Error::CheckoutConflict(detail) => Error::CheckoutConflict(detail + suffix),
```

- [ ] **Step 4: The `[checkout]` table and the Project fields**

In `src/repo.rs`, add beside `FeedbackConfig`:

```rust
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckoutConfig {
    path: String,
}
```

add to `Config` (after `retired`):

```rust
    #[serde(default, skip_serializing_if = "Option::is_none")]
    checkout: Option<CheckoutConfig>,
```

and to `Project` (after `retired`):

```rust
    /// The code checkout this project is linked to, from the config's `[checkout] path`,
    /// absolute and normalized; `None` for an in-tree project. It need not exist: `check`
    /// reports a missing one, and a missing one never matches (ops linked-checkouts §4.1).
    pub checkout: Option<PathBuf>,
```

Add the parse helper beside `retirement`:

```rust
/// A `[checkout] path`: relative to the project root as `init --checkout` writes it, or
/// absolute as written by hand. Normalized the way `resolve` normalizes paths, so a
/// symlinked spelling and its real path compare equal when the target exists.
fn checkout_path(root: &Path, raw: Option<CheckoutConfig>) -> Result<Option<PathBuf>> {
    let Some(table) = raw else {
        return Ok(None);
    };
    if table.path.is_empty() {
        return Err(Error::Config(format!("{CONFIG_REL}: [checkout] path must not be empty")));
    }
    crate::locations::normalize(Path::new(&table.path), root)
        .map(Some)
        .map_err(|problem| Error::Config(format!("{CONFIG_REL}: [checkout] path: {problem}")))
}
```

`locations::normalize(path, base)` joins a relative `path` onto `base`, canonicalizes the
longest existing ancestor, and appends the rest; it refuses only a `..` after a missing
component.

In `Project::open` (the root is already canonical there), compute the checkout before
the struct literal, since `root` moves into it:

```rust
        let checkout = checkout_path(&root, config.checkout)?;
```

and set `checkout,` in the `Ok(Project { ... })` literal.

In `Project::init`, the `Config` literal gains `checkout: None` (Task 4 changes this).

Every other place that builds a `Project` literal (search `Project {` in `src/` and tests)
gains `checkout: None`.

- [ ] **Step 5: The scan in `src/locations.rs`**

The scan keeps every project it read and every project it could not, so a strict caller
fails on any failure while `resolve` (Task 5) keeps the good entries (Decision 5).

```rust
/// One registered project that declares a `[checkout]` (linked-checkouts §4.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Linked {
    pub prefix: String,
    pub checkout: PathBuf,
}

/// Every registered project on this host that declares a checkout, and every registered
/// project whose config could not be read. A project whose root is gone or holds no config
/// is in neither: it cannot be opened here, so nothing can be linked to it.
#[derive(Debug, Default)]
pub struct CheckoutScan {
    pub linked: Vec<Linked>,
    pub failed: Vec<(String, crate::error::Error)>,
}

impl CheckoutScan {
    /// The linked projects, or the first failure, naming its project (§4.1a: errors stay
    /// errors for every caller that opens a project).
    pub fn strict(self) -> crate::error::Result<Vec<Linked>> {
        match self.failed.into_iter().next() {
            Some((prefix, error)) => {
                Err(error.with_suffix(&format!(" (registered project {prefix})")))
            }
            None => Ok(self.linked),
        }
    }
}

pub fn scan_checkouts(registry: &crate::registry::Registry) -> CheckoutScan {
    let mut scan = CheckoutScan::default();
    for (prefix, root) in &registry.projects {
        let opened = crate::scope::is_reachable(root).and_then(|reachable| {
            reachable
                .then(|| {
                    crate::scope::open_registered(registry, prefix, crate::scope::Origin::Prefix)
                })
                .transpose()
        });
        match opened {
            Ok(Some(project)) => {
                if let Some(checkout) = project.checkout {
                    scan.linked.push(Linked {
                        prefix: prefix.clone(),
                        checkout,
                    });
                }
            }
            Ok(None) => {}
            Err(error) => scan.failed.push((prefix.clone(), error)),
        }
    }
    scan
}
```

`Error::with_suffix` (`src/error.rs`, `pub`) rebuilds the same variant with the suffix
appended, so a malformed config stays kind `config` and keeps naming its project.

- [ ] **Step 6: `src/link.rs`**

`GitTree` starts with the one fact Task 2 reads, the common directory. Task 3 adds the
working tree's top level and Task 4 the git dir, each by adding a flag to the one
`rev-parse` call below and a field to the struct.

```rust
//! Linked checkouts: a path's git facts and the registered project whose `[checkout]`
//! names its repository (ops docs/specs/2026-10-01-linked-checkouts-design.md §4.1).

use crate::error::{Error, Result};
use crate::registry::Registry;
use crate::repo::Project;
use std::path::{Path, PathBuf};

/// git's view of a directory, canonical: the repository's common git directory.
#[derive(Debug, Clone, PartialEq)]
pub struct GitTree {
    pub common: PathBuf,
}

impl GitTree {
    /// `None` when git says `dir` is not inside a repository, or there is no git
    /// executable; any other git failure is an `io` error. `LC_ALL=C` pins the one
    /// message matched, as `Project::git_toplevel` does. One line per requested fact, in
    /// the order of the flags.
    pub fn of(dir: &Path) -> Result<Option<GitTree>> {
        let output = match std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
            .env("LC_ALL", "C")
            .output()
        {
            Ok(output) => output,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("not a git repository") {
                return Ok(None);
            }
            return Err(Error::Io(format!(
                "git rev-parse in {} failed ({}): {}",
                dir.display(),
                output.status,
                stderr.trim()
            )));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let mut lines = text.lines().map(|line| PathBuf::from(line).canonicalize());
        let mut next = || -> Result<PathBuf> {
            lines
                .next()
                .ok_or_else(|| {
                    Error::Io(format!("git rev-parse in {} printed too little", dir.display()))
                })?
                .map_err(Error::from)
        };
        Ok(Some(GitTree { common: next()? }))
    }

    /// The repository's main checkout: the common directory's parent when that is a
    /// `.git` directory. `None` for a bare repository or a separate git dir.
    pub fn main_checkout(&self) -> Option<PathBuf> {
        if self.common.file_name()? != ".git" {
            return None;
        }
        self.common.parent().map(Path::to_path_buf)
    }
}

/// The registered project whose `[checkout]` is `tree`'s main checkout. Two such projects
/// are `checkout_conflict`.
pub fn linked_project(registry: &Registry, tree: &GitTree) -> Result<Option<Project>> {
    let Some(main) = tree.main_checkout() else {
        return Ok(None);
    };
    let matches: Vec<_> = crate::locations::scan_checkouts(registry)
        .strict()?
        .into_iter()
        .filter(|linked| linked.checkout == main)
        .collect();
    match matches.as_slice() {
        [] => Ok(None),
        [one] => crate::scope::open_registered(registry, &one.prefix, crate::scope::Origin::Prefix)
            .map(Some),
        many => Err(Error::CheckoutConflict(format!(
            "{} is declared as the checkout of {}; keep one project's [checkout]",
            main.display(),
            many.iter().map(|l| l.prefix.as_str()).collect::<Vec<_>>().join(" and ")
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_checkout_is_the_parent_of_a_dot_git_common_dir_only() {
        let tree = |common: &str| GitTree { common: common.into() };
        assert_eq!(tree("/r/.git").main_checkout(), Some(PathBuf::from("/r")));
        assert_eq!(tree("/srv/r.git").main_checkout(), None, "a bare repository has none");
    }

    #[test]
    fn outside_a_repository_there_is_no_tree() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(GitTree::of(dir.path()).unwrap(), None);
    }
}
```

When Task 3 and Task 4 add fields, the unit test's `GitTree { ... }` literal gains them.

Register the module where the crate declares its modules (`grep -n "^mod \|^pub mod "
src/main.rs src/lib.rs`), alphabetically.

- [ ] **Step 7: `locate`'s link step**

Replace `Project::locate` in `src/repo.rs`:

```rust
    /// The project a command run in `start` opens: the nearest `tasks/.config.toml` at or
    /// above it, as always. Only on a miss, inside a git repository, the registered project
    /// whose `[checkout]` is that repository's main checkout (linked-checkouts §4.1). An
    /// in-tree project therefore never reads the registry here, and a miss outside git
    /// stays `no_project` whatever state the registry is in.
    pub fn locate(start: &Path) -> Result<Project> {
        let start = start.canonicalize()?;
        let mut directory = Some(start.as_path());
        while let Some(path) = directory {
            if path.join(CONFIG_REL).is_file() {
                return Self::open(path);
            }
            directory = path.parent();
        }
        if let Some(tree) = crate::link::GitTree::of(&start)? {
            let registry = Registry::load()?;
            if let Some(project) = crate::link::linked_project(&registry, &tree)? {
                return Ok(project);
            }
        }
        Err(Error::NoProject(start))
    }
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `just test-one --test linked`
Expected: PASS (all nine). Then `just test-one link::` for the unit tests: PASS.

- [ ] **Step 9: Run the fast suite**

Run: `just test-fast`
Expected: PASS. A failure in a test that expects `no_project` from inside a git
repository with a malformed registry is this change: such a test asserted the old
fall-through. Report it rather than weakening the new rule.

- [ ] **Step 10: Commit**

```bash
git add src/link.rs src/repo.rs src/locations.rs src/error.rs src/main.rs tests/linked.rs
git commit -m "feat(locate): open a linked project from its code checkout"
```

(Add `src/lib.rs` instead of `src/main.rs` if that is where modules are declared, and any
file whose `Project` literal gained the field.) Before committing, `just check` must be
clean: clippy runs with `-D warnings`, so nothing this task adds may be unused.

---

### Task 3: Record location and resume location

**Files:**
- Modify: `src/link.rs` (`GitTree.top`, `linked_project` sets the worktree),
  `src/repo.rs` (`Project.worktree`, `resume_root`), `src/commands/mod.rs`
  (`claim_guard`, `follow_holder`, `transition`, `raw_owner_name`, `lock_and_revalidate`),
  `src/commands/park.rs`, `src/commands/parked.rs` (`open_recorded`, the
  `recorded_is_scanned` comparison)
- Test: `tests/linked.rs`

**Interfaces:**
- Consumes: `link::GitTree::of`, `link::linked_project` (Task 2).
- Produces:
  - `GitTree.top: PathBuf` — the working tree's top level, canonical.
  - `Project.worktree: Option<PathBuf>` — code working tree top, set only by
    `linked_project`.
  - `Project::resume_root(&self) -> &Path`.
  - Claims and parks whose `worktree` is the code working tree for a linked project;
    `parked::open_recorded(id, worktree)` that opens the overlay project for a recorded
    code worktree.

- [ ] **Step 1: Write the failing lifecycle test**

Append to `tests/linked.rs`:

```rust
fn porcelain(dir: &Path) -> String {
    git(dir, &["status", "--porcelain", "--ignored"])
}

fn string(v: &serde_json::Value) -> String {
    serde_json::to_string(v).unwrap()
}

#[test]
fn the_lifecycle_in_a_code_worktree_keeps_records_in_the_overlay() {
    let env = TestEnv::new();
    let l = layout(&env);
    let sub = l.code.join("src/deep");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join(".keep"), "").unwrap();
    git(&l.code, &["add", "src/deep/.keep"]);
    git(&l.code, &["commit", "-q", "-m", "sub"]);
    let before = porcelain(&l.code);
    let before_wt = porcelain(&l.wt);
    std::fs::create_dir_all(l.nexc.join("docs/specs")).unwrap();
    std::fs::write(l.nexc.join("docs/specs/topic.md"), "# Topic\n").unwrap();
    let shot = l.base.join("shot.png");
    std::fs::write(&shot, b"\x89PNG\r\n\x1a\n").unwrap();
    let elsewhere = tempfile::tempdir().unwrap();

    // From the main checkout, a subdirectory of it, and the worktree: the claim and park
    // name the working tree's top level, and the owner is that tree's branch.
    for (n, (tree, top, branch)) in [
        (&l.code, &l.code, "main"),
        (&sub, &l.code, "main"),
        (&l.wt, &l.wt, "wt"),
    ]
    .into_iter()
    .enumerate()
    {
        let top = top.to_str().unwrap();
        let title = format!("Linked work {n}");
        let id = env.json(tree, &["add", &title])["id"].as_str().unwrap().to_string();
        env.json(tree, &["start", &id]);
        let shown = env.json(tree, &["show", &id]);
        assert_eq!(shown["claim"]["worktree"], top, "{n}: the claim names the code tree");
        assert_eq!(shown["task"]["owner"], branch, "{n}: the owner reads the code branch");
        env.json(tree, &["note", &id, "a note"]);
        env.json(tree, &["edit", &id, "--spec", "topic"]);
        env.json(tree, &["attach", &id, shot.to_str().unwrap(), "--caption", "a shot"]);

        // An agent-waiting park: next and prime list it with the code tree to open.
        env.json(tree, &["park", &id, "resume here"]);
        for view in [env.json(&l.code, &["next"]), env.json(&l.code, &["prime"])] {
            let text = string(&view);
            assert!(text.contains(&id) && text.contains(top), "{n}: {text}");
            assert!(!string(&view["warnings"]).contains("unavailable"), "{n}: {text}");
        }
        let found = env.json(elsewhere.path(), &["show", &id]);
        assert_eq!(found["park"]["worktree"], top, "{n}");
        assert_eq!(found["task"]["spec"], "docs/specs/topic.md", "{n}");

        // A quiet park, read from outside every project: quiet resolves the recorded code
        // tree through the link (`open_recorded`), so the row carries the record's title
        // and no warning calls the checkout unavailable.
        env.json(tree, &["start", &id]);
        env.json(
            tree,
            &["park", &id, "run it", "--reason", "quiet", "--waiting-on", "user", "--minutes", "5"],
        );
        let quiet = env.json(elsewhere.path(), &["quiet", "--project", "nexc"]);
        let text = string(&quiet);
        assert!(text.contains(&id) && text.contains(top) && text.contains(&title), "{n}: {text}");
        assert!(!string(&quiet["warnings"]).contains("unavailable"), "{n}: {text}");

        env.json(tree, &["start", &id]);
        env.json(tree, &["done", &id, "landed"]);
        assert!(l.nexc.join(format!("tasks/{id}.md")).is_file(), "{n}");
        assert!(l.nexc.join(format!("tasks/files/{id}")).is_dir(), "{n}: attachments in the overlay");
    }
    assert_eq!(porcelain(&l.code), before, "nothing written into the code repository");
    assert_eq!(porcelain(&l.wt), before_wt, "nothing written into the code worktree");
}

#[test]
fn open_recorded_reads_a_recorded_code_tree_through_the_link() {
    // The unit seam under quiet, next, prime, and show's fallback: with the overlay root
    // never named, a recorded code worktree still opens the overlay project.
    let env = TestEnv::new();
    let l = layout(&env);
    let id = env.json(&l.wt, &["add", "Recorded"])["id"].as_str().unwrap().to_string();
    env.json(&l.wt, &["start", &id]);
    env.json(
        &l.wt,
        &["park", &id, "run it", "--reason", "quiet", "--waiting-on", "user", "--minutes", "5"],
    );
    let elsewhere = tempfile::tempdir().unwrap();
    let quiet = env.json(elsewhere.path(), &["quiet"]);
    let text = string(&quiet);
    assert!(text.contains(&id) && text.contains("Recorded"), "{text}");
    assert!(!string(&quiet["warnings"]).contains("unavailable"), "{text}");
}

#[test]
fn a_holders_write_moves_its_one_claim_between_code_trees() {
    let env = TestEnv::new();
    let l = layout(&env);
    let id = env.json(&l.code, &["add", "Moving"])["id"].as_str().unwrap().to_string();
    env.json(&l.code, &["start", &id]);
    assert_eq!(env.json(&l.code, &["show", &id])["claim"]["worktree"], l.code.to_str().unwrap());
    env.json(&l.wt, &["note", &id, "from the worktree"]);
    let moved = env.json(&l.code, &["show", &id]);
    assert_eq!(moved["claim"]["worktree"], l.wt.to_str().unwrap(), "the claim follows its holder");
    env.json(&l.code, &["note", &id, "back in the main checkout"]);
    assert_eq!(env.json(&l.wt, &["show", &id])["claim"]["worktree"], l.code.to_str().unwrap());
    let claims = string(&env.json(&l.code, &["claims"]));
    assert_eq!(claims.matches(&format!("\"{id}\"")).count(), 1, "one claim, moved: {claims}");
}

#[test]
fn prime_warns_only_about_this_projects_directory_in_the_overlay() {
    let env = TestEnv::new();
    let l = layout(&env);
    let other = project(&env, &l.overlay, "oth", None);
    env.json(&other, &["add", "Uncommitted elsewhere"]);
    env.json(&l.wt, &["add", "Uncommitted here"]);
    let warnings = string(&env.json(&l.wt, &["prime"])["warnings"]);
    assert!(warnings.contains("nexc-"), "{warnings}");
    assert!(!warnings.contains("oth-"), "{warnings}");
}
```

Before running, check two shapes with `--help` and one scratch run, and adjust only the
lookup, never the assertion: `tasks quiet` lists only parks made with `--reason quiet`
(so the ordinary park above is deliberately not asserted there), and `tasks claims`
names each claimed id once as a JSON string. If `claims` keys its rows differently,
count rows whose id field equals `id` instead.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-one --test linked lifecycle`
Expected: FAIL on the `claim.worktree` assertion (it names the overlay project root).
Run: `just test-one --test linked holders_write`
Expected: FAIL on the first `claim.worktree` assertion.

- [ ] **Step 3: The working tree's top, and the resume root**

In `src/link.rs`, `GitTree` gains its first field and `of` its first flag:

```rust
pub struct GitTree {
    pub top: PathBuf,
    pub common: PathBuf,
}
```

```rust
            .args([
                "rev-parse",
                "--path-format=absolute",
                "--show-toplevel",
                "--git-common-dir",
            ])
```

```rust
        Ok(Some(GitTree {
            top: next()?,
            common: next()?,
        }))
```

The unit test's literal becomes `GitTree { top: "/r".into(), common: common.into() }`.
`linked_project`'s single-match arm sets the worktree:

```rust
        [one] => {
            let mut project =
                crate::scope::open_registered(registry, &one.prefix, crate::scope::Origin::Prefix)?;
            project.worktree = Some(tree.top.clone());
            Ok(Some(project))
        }
```

and its doc comment says so: "opened with `worktree` set to `tree`'s top level".

In `src/repo.rs`, `Project` gains (after `checkout`):

```rust
    /// The code working tree this command runs in, when `locate` found the project through
    /// its `[checkout]` link; `None` otherwise. Claims and parks record it (ops
    /// linked-checkouts §4.1, "record location and resume location are separate").
    pub worktree: Option<PathBuf>,
```

`Project::open` sets `worktree: None`; every other `Project` literal gains `worktree:
None`. Add the accessor in `impl Project`:

```rust
    /// Where a session working on this project resumes: the code working tree for a
    /// project found through its link, otherwise the root. Claims, parks, and the owner
    /// fallback use it; records are never read from it.
    pub fn resume_root(&self) -> &Path {
        self.worktree.as_deref().unwrap_or(&self.root)
    }
```

- [ ] **Step 4: Record the resume root**

In `src/commands/mod.rs`:
- `claim_guard`: `let worktree = self.project.resume_root().display().to_string();`
- `follow_holder`: `worktree: ctx.project.resume_root().display().to_string(),`
- `transition` (the recurring-done check): `let worktree = ctx.project.resume_root().display().to_string();`
- `raw_owner_name`: `.current_dir(project.resume_root())`

In `src/commands/park.rs` at the park entry: `worktree: ctx.project.resume_root().display().to_string(),`

Then search for any other place that writes or compares a claim or park `worktree`
against `project.root`:

```bash
grep -rn "project\.root" src/commands | grep -v "^.*//"
```

Each hit is either a *record* location (config, tasks dir, docs, git checks, rename,
retire — leave it) or a *resume* location (a claim or park `worktree` written or compared
— change it to `resume_root()`). The list above is every resume hit known when this plan
was written; report any other you change.

- [ ] **Step 5: Keep the link across the lock**

In `lock_and_revalidate`, the local arm reopens the root and would drop `worktree`:

```rust
            Routing::Local => Project {
                worktree: ctx.project.worktree.clone(),
                ..Project::open(&ctx.project.root)?
            },
```

- [ ] **Step 6: `open_recorded` through the link**

In `src/commands/parked.rs`:

```rust
/// Opens the checkout a store entry recorded and reads `id` there, with that checkout's
/// whole scan so the task's children, dependencies, and documents resolve there too (park
/// design §5.3). A recorded code worktree of a linked checkout holds no config; its
/// project is the one its repository is linked to (ops linked-checkouts §4.1). `None` when
/// the checkout is gone, belongs to another prefix, or lacks the record.
pub fn open_recorded(id: &TaskId, worktree: &Path) -> Result<Option<(Project, Task, Vec<Task>)>> {
    let project = if crate::scope::has_config(worktree)? {
        Project::open(worktree)?
    } else {
        if !worktree.try_exists()? {
            return Ok(None);
        }
        let Some(tree) = crate::link::GitTree::of(worktree)? else {
            return Ok(None);
        };
        match crate::link::linked_project(&Registry::load()?, &tree)? {
            Some(project) => project,
            None => return Ok(None),
        }
    };
    if project.prefix != id.prefix {
        return Ok(None);
    }
    let scan = project.scan()?;
    let Some(task) = scan.iter().find(|task| task.id == *id).cloned() else {
        return Ok(None);
    };
    Ok(Some((project, task, scan)))
}
```

(`Registry` is already imported in `parked.rs`; add `use crate::registry::Registry;` if not.)

And where `recorded_is_scanned` is computed:

```rust
            .any(|project| project.resume_root() == Path::new(&park.worktree));
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `just test-one --test linked`
Expected: PASS.

- [ ] **Step 8: Run the fast suite**

Run: `just test-fast`
Expected: PASS (in-tree projects have `worktree: None`, so `resume_root()` is the root and
every existing claim and park test is unchanged).

- [ ] **Step 9: Commit**

```bash
git add src/link.rs src/repo.rs src/commands/mod.rs src/commands/park.rs src/commands/parked.rs tests/linked.rs
git commit -m "feat(claims): record a linked project's code worktree as its resume location"
```

---

### Task 4: `tasks where`, `tasks init --checkout`, and the vocabulary

**Files:**
- Create: `src/commands/where_.rs`
- Modify: `src/cli.rs`, `src/commands/mod.rs` (module list, dispatch), `src/main.rs` (the
  read-command list before the width check), `src/commands/init.rs`, `src/repo.rs`
  (`Project::init`), `src/link.rs` (`holds_records`, `relative_path`), `src/output.rs`
  (`WhereOut`, pretty, warnings), `tools/cli.toml`
- Test: `tests/linked.rs`

**Interfaces:**
- Consumes: ops main's `cli.toml` rows (Task 1); `GitTree`, `linked_project` (Task 2).
- Produces: `Output::Where(WhereOut)`; `GitTree.git_dir: PathBuf`, `GitTree::is_main(&self)
  -> bool`, `GitTree::worktrees(&self) -> Result<Vec<PathBuf>>`;
  `link::holds_records(registry: &Registry, tree: &GitTree) -> Result<bool>`;
  `link::relative_path(from: &Path, to: &Path) -> PathBuf`;
  `Project::init(root: &Path, prefix: &str, checkout: Option<&Path>) -> Result<Project>`.

- [ ] **Step 1: Write the failing tests**

Append to `tests/linked.rs`:

```rust
#[test]
fn where_answers_linked_in_tree_and_no_project() {
    let env = TestEnv::new();
    let l = layout(&env);
    let linked = env.json(&l.wt, &["where", "--json"]);
    assert_eq!(linked["project"]["prefix"], "nexc");
    assert_eq!(linked["project"]["root"], l.nexc.to_str().unwrap());
    assert_eq!(linked["project"]["checkout"], l.code.to_str().unwrap());
    assert_eq!(linked["project"]["linked"], true);
    assert_eq!(linked["holds_records"], false, "the code repository keeps no records");
    assert_eq!(linked["warnings"], serde_json::json!([]));

    let intree = env.json(&l.nexc, &["where"]);
    assert_eq!(intree["project"]["linked"], false);
    assert!(intree["project"].as_object().unwrap().contains_key("checkout"));
    assert!(intree["project"]["checkout"].is_null());
    assert_eq!(intree["holds_records"], true, "the overlay holds a registered root");

    let plain = tempfile::tempdir().unwrap();
    let none = env.json(plain.path(), &["where"]);
    assert!(none["project"].is_null());
    assert_eq!(none["holds_records"], false);
}

#[test]
fn where_takes_a_path_a_file_and_refuses_a_missing_one() {
    let env = TestEnv::new();
    let l = layout(&env);
    let elsewhere = tempfile::tempdir().unwrap();
    let by_path = env.json(elsewhere.path(), &["where", l.wt.to_str().unwrap()]);
    assert_eq!(by_path["project"]["prefix"], "nexc");
    let by_file = env.json(&l.code, &["where", "README"]);
    assert_eq!(by_file["project"]["prefix"], "nexc", "a relative file answers for its directory");
    assert_eq!(env.fail(&l.code, &["where", "no/such/path"]), "io");
}

#[test]
fn where_never_crosses_into_an_enclosing_project() {
    let env = TestEnv::new();
    let l = layout(&env);
    let parent = code_repo(&l.base, "parent");
    env.json(&parent, &["init", "--prefix", "par"]);
    git(&parent, &["add", "-A"]);
    git(&parent, &["commit", "-q", "-m", "records"]);
    let nested = code_repo(&parent, "vendor/clone");
    let inside = env.json(&nested, &["where"]);
    assert!(inside["project"].is_null(), "a nested foreign clone is no project: {inside}");
    assert_eq!(inside["holds_records"], false);
    let sub = parent.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    assert_eq!(env.json(&sub, &["where"])["project"]["prefix"], "par");
}

#[test]
fn holds_records_needs_a_config_tracked_in_head() {
    let env = TestEnv::new();
    let l = layout(&env);
    let repo = code_repo(&l.base, "unregistered");
    std::fs::create_dir_all(repo.join("tasks")).unwrap();
    std::fs::write(repo.join("tasks/.config.toml"), "prefix = \"unr\"\n").unwrap();
    assert_eq!(env.json(&repo, &["where"])["holds_records"], false, "untracked");
    git(&repo, &["add", "tasks/.config.toml"]);
    assert_eq!(env.json(&repo, &["where"])["holds_records"], false, "staged only");
    git(&repo, &["commit", "-q", "-m", "records"]);
    assert_eq!(env.json(&repo, &["where"])["holds_records"], true, "tracked in HEAD");
}

#[test]
fn holds_records_refuses_a_tracked_tree_or_symlink_at_the_config_path() {
    let env = TestEnv::new();
    let l = layout(&env);
    // A directory named like the config: `git cat-file -e` would accept its tree object.
    let tree = code_repo(&l.base, "tree");
    std::fs::create_dir_all(tree.join("tasks/.config.toml")).unwrap();
    std::fs::write(tree.join("tasks/.config.toml/inner"), "x\n").unwrap();
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "a tree at the config path"]);
    assert_eq!(env.json(&tree, &["where"])["holds_records"], false, "a tree is not a config");
    let link = code_repo(&l.base, "link");
    std::fs::create_dir_all(link.join("tasks")).unwrap();
    std::fs::write(link.join("real.toml"), "prefix = \"lnk\"\n").unwrap();
    std::os::unix::fs::symlink("../real.toml", link.join("tasks/.config.toml")).unwrap();
    git(&link, &["add", "-A"]);
    git(&link, &["commit", "-q", "-m", "a symlink at the config path"]);
    assert_eq!(env.json(&link, &["where"])["holds_records"], false, "a symlink is not a config");
    let unborn = l.base.join("unborn");
    std::fs::create_dir_all(&unborn).unwrap();
    git(&unborn, &["init", "-q"]);
    assert_eq!(env.json(&unborn, &["where"])["holds_records"], false, "no HEAD tracks nothing");
}

#[test]
fn where_reports_resolver_errors_as_errors() {
    let env = TestEnv::new();
    let l = layout(&env);
    project(&env, &l.overlay, "dup", Some(l.code.to_str().unwrap()));
    assert_eq!(env.fail(&l.wt, &["where", "--json"]), "checkout_conflict");
    let env = TestEnv::new();
    let l = layout(&env);
    std::fs::write(l.nexc.join("tasks/.config.toml"), "prefix = \"nexc\"\n[checkout]\nbogus = 1\n").unwrap();
    assert_eq!(env.fail(&l.code, &["where"]), "config");
}

#[test]
fn where_with_the_overlay_missing_is_no_project() {
    let env = TestEnv::new();
    let l = layout(&env);
    std::fs::rename(&l.nexc, l.base.join("moved-away")).unwrap();
    assert!(env.json(&l.code, &["where"])["project"].is_null());
}

#[test]
fn init_checkout_writes_a_relative_link_outside_the_code_tree() {
    let env = TestEnv::new();
    let holder = tempfile::tempdir().unwrap();
    let base = holder.path().canonicalize().unwrap();
    let code = code_repo(&base, "my code");
    let before = porcelain(&code);
    let dir = base.join("overlay/mine");
    std::fs::create_dir_all(&dir).unwrap();
    env.json(&dir, &["init", "--prefix", "mine", "--checkout", "../../my code"]);
    let config = std::fs::read_to_string(dir.join("tasks/.config.toml")).unwrap();
    assert!(config.contains("[checkout]") && config.contains("path = \"../../my code\""), "{config}");
    assert!(dir.join("docs/specs").is_dir() && dir.join("docs/plans").is_dir());
    assert_eq!(porcelain(&code), before, "init wrote nothing into the checkout");
    assert_eq!(env.json(&code, &["where"])["project"]["prefix"], "mine");
}

#[test]
fn plain_init_refuses_every_tree_of_a_linked_checkout() {
    let env = TestEnv::new();
    let l = layout(&env);
    let sub = l.code.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    let before = porcelain(&l.code);
    let before_wt = porcelain(&l.wt);
    for (n, dir) in [&l.code, &sub, &l.wt].into_iter().enumerate() {
        let out = env.cmd(dir).args(["init", "--prefix", "intr"]).output().unwrap();
        assert_eq!(out.status.code(), Some(1), "{n}");
        let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(error["error"]["kind"], "config", "{n}");
        assert!(error["error"]["detail"].as_str().unwrap().contains("nexc"), "{n}: names the linked project");
        assert!(!dir.join("tasks").exists(), "{n}: nothing created");
    }
    assert_eq!(porcelain(&l.code), before);
    assert_eq!(porcelain(&l.wt), before_wt);
    assert!(env.json(&l.nexc, &["projects"])["projects"].as_array().unwrap().iter().all(|r| r["prefix"] != "intr"));
}

#[test]
fn init_checkout_refuses_what_it_cannot_link() {
    let env = TestEnv::new();
    let l = layout(&env);
    let fresh = |name: &str| {
        let dir = l.overlay.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    };
    let plain = l.base.join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    let intree = code_repo(&l.base, "intree");
    env.json(&intree, &["init", "--prefix", "own"]);
    let inside = intree.join("inner");
    std::fs::create_dir_all(&inside).unwrap();
    for (name, checkout, kind) in [
        ("p1", l.wt.to_str().unwrap(), "config"),        // a worktree, not the main checkout
        ("p2", l.code.join("src").to_str().unwrap(), "config"), // not the top level (missing dir: also config)
        ("p3", plain.to_str().unwrap(), "config"),       // not a git repository
        ("p4", intree.to_str().unwrap(), "config"),      // keeps records in-tree
        ("p5", l.code.to_str().unwrap(), "checkout_conflict"), // nexc already declares it
    ] {
        let dir = fresh(name);
        assert_eq!(env.fail(&dir, &["init", "--prefix", name, "--checkout", checkout]), kind, "{name}");
        assert!(!dir.join("tasks").exists(), "{name}: a refused init writes nothing");
    }
    // The project directory inside a worktree of the very repository passed as
    // --checkout, that worktree stored outside the checkout's tree: no project links the
    // repository yet, and containment misses it; repository identity does not.
    let other = code_repo(&l.base, "other");
    let other_store = l.base.join("storage/other");
    std::fs::create_dir_all(&other_store).unwrap();
    std::os::unix::fs::symlink(&other_store, other.join(".worktrees")).unwrap();
    git(&other, &["worktree", "add", "-q", "-b", "ow", ".worktrees/ow"]);
    let in_wt = other.join(".worktrees/ow").canonicalize().unwrap().join("overlay-here");
    std::fs::create_dir_all(&in_wt).unwrap();
    assert!(!in_wt.starts_with(&other), "the worktree really is outside the checkout's tree");
    assert_eq!(
        env.fail(&in_wt, &["init", "--prefix", "p7", "--checkout", other.to_str().unwrap()]),
        "config",
        "a project directory in a worktree of its own checkout's repository"
    );
    assert!(!in_wt.join("tasks").exists());
    let nested = code_repo(&l.nexc, "nested");
    let dir = fresh("p6");
    assert_eq!(
        env.fail(&dir, &["init", "--prefix", "p6", "--checkout", nested.to_str().unwrap()]),
        "config",
        "a checkout inside a registered project's tree"
    );
}
```

Note on `p2`: `code/src` does not exist in the layout; create it first
(`std::fs::create_dir_all(l.code.join("src"))`) so the case tests "not the top level"
rather than "does not exist". Add that line before the loop.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-one --test linked where`
Expected: FAIL — `where` is an unknown subcommand (exit 2).
Run: `just test-one --test linked plain_init`
Expected: FAIL — `init` succeeds inside the code checkout and creates `tasks/` there.

- [ ] **Step 3: Adopt the vocabulary**

From this worktree (ops main must hold Task 1's commit):

```bash
python3 <ops main checkout>/bin/vendored adopt cli.toml
git diff --stat tools/cli.toml
```

Expected: `tools/cli.toml` gains the `--checkout` and `where` rows (and whatever else ops
main has landed since the last adoption, such as obs's `tests` view). Do not edit it by hand.

- [ ] **Step 4: The parser surface**

In `src/cli.rs`, `Init` gains (between `prefix` and `force`):

```rust
        /// Link this project to a code checkout kept elsewhere: the repository's main
        /// checkout. Records stay here; nothing is written there.
        #[arg(long, value_name = "PATH")]
        checkout: Option<PathBuf>,
```

and a new command after `Root`:

```rust
    /// The project a path belongs to, in-tree or linked, and whether its repository keeps records.
    Where {
        /// A directory or file; the current directory when omitted.
        #[arg(value_name = "PATH")]
        path: Option<PathBuf>,
    },
```

(`use std::path::PathBuf;` at the top of `cli.rs` if it is not already imported.)

In `src/main.rs`, add `| cli::Command::Where { .. }` to the read-command list before the
width check. In `src/commands/mod.rs`, add `mod where_;` (or `pub mod`, matching the
others) and the dispatch:

```rust
        Command::Init { prefix, checkout, force } => init::run(dir, prefix, checkout, force),
        Command::Where { path } => where_::run(path, dir),
```

- [ ] **Step 5: `GitTree`'s git dir, `holds_records`, and `relative_path` in `src/link.rs`**

`GitTree` gains its last fact, and the two methods whose first callers are this task's
`holds_records` and `validate_checkout`:

```rust
pub struct GitTree {
    pub top: PathBuf,
    pub common: PathBuf,
    pub git_dir: PathBuf,
}
```

`of` adds `"--absolute-git-dir"` after `"--git-common-dir"` and `git_dir: next()?,` after
`common: next()?,`; the unit test's literal gains `git_dir: common.into()`. In `impl
GitTree`:

```rust
    /// This working tree is the main checkout, not a linked worktree.
    pub fn is_main(&self) -> bool {
        self.git_dir == self.common
    }

    /// Every working tree of the repository, canonical where it still exists, main first.
    pub fn worktrees(&self) -> Result<Vec<PathBuf>> {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&self.top)
            .args(["worktree", "list", "--porcelain", "-z"])
            .env("LC_ALL", "C")
            .output()?;
        if !output.status.success() {
            return Err(Error::Io(format!(
                "git worktree list in {} failed: {}",
                self.top.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .split('\0')
            .filter_map(|field| field.strip_prefix("worktree "))
            .map(|listed| {
                let listed = PathBuf::from(listed);
                listed.canonicalize().unwrap_or(listed)
            })
            .collect())
    }
```

Then the two free functions:

```rust
/// Linked-checkouts decision 5 and §8.1: the repository holding `tree` keeps our records
/// when a registered project's root is in it (the same common git directory), or its top
/// level holds a `tasks/.config.toml` tracked in `HEAD` as a regular file. Only roots
/// inside one of the repository's working trees can share its common directory, so git
/// runs only for those.
pub fn holds_records(registry: &Registry, tree: &GitTree) -> Result<bool> {
    let worktrees = tree.worktrees()?;
    for root in registry.projects.values() {
        if !root.try_exists()? {
            continue;
        }
        let root = root.canonicalize()?;
        if !worktrees.iter().any(|worktree| root.starts_with(worktree)) {
            continue;
        }
        if GitTree::of(&root)?.is_some_and(|theirs| theirs.common == tree.common) {
            return Ok(true);
        }
    }
    tracked_config(&tree.top)
}

/// `tasks/.config.toml` is tracked in `HEAD` at `top` as a regular file: `ls-tree` names a
/// `blob` of mode `100644` or `100755`. A tree at that path (which `git cat-file -e`
/// accepts) or a symlink (`120000`) is not a config. An unborn `HEAD` tracks nothing;
/// any other git failure is an `io` error.
fn tracked_config(top: &Path) -> Result<bool> {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(top)
            .args(args)
            .env("LC_ALL", "C")
            .output()
    };
    let listed = git(&["ls-tree", "-z", "HEAD", "--", CONFIG_PATH])?;
    if !listed.status.success() {
        if !git(&["rev-parse", "-q", "--verify", "HEAD"])?.status.success() {
            return Ok(false);
        }
        return Err(Error::Io(format!(
            "git ls-tree in {} failed: {}",
            top.display(),
            String::from_utf8_lossy(&listed.stderr).trim()
        )));
    }
    // `<mode> SP <type> SP <object> TAB <path> NUL`, or nothing when the path is untracked.
    let entry = String::from_utf8_lossy(&listed.stdout);
    let Some((meta, path)) = entry.trim_end_matches('\0').split_once('\t') else {
        return Ok(false);
    };
    let mut fields = meta.split(' ');
    Ok(path == CONFIG_PATH
        && matches!(
            (fields.next(), fields.next()),
            (Some("100644" | "100755"), Some("blob"))
        ))
}

const CONFIG_PATH: &str = crate::repo::CONFIG_REL;

/// `to` relative to `from`, both absolute and canonical: the shared prefix dropped, one
/// `..` per remaining component of `from`, then the rest of `to`.
pub fn relative_path(from: &Path, to: &Path) -> PathBuf {
    let from: Vec<_> = from.components().collect();
    let to: Vec<_> = to.components().collect();
    let shared = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut out = PathBuf::new();
    for _ in shared..from.len() {
        out.push("..");
    }
    for component in &to[shared..] {
        out.push(component.as_os_str());
    }
    out
}
```

Unit tests in `link.rs`'s test module:

```rust
    #[test]
    fn relative_path_climbs_then_descends() {
        assert_eq!(relative_path(Path::new("/d/o/nexc"), Path::new("/d/code")), PathBuf::from("../../code"));
        assert_eq!(relative_path(Path::new("/d/o"), Path::new("/d/o/x")), PathBuf::from("x"));
    }
```

- [ ] **Step 6: The `where` command**

`src/output.rs`, beside `RootOut`:

```rust
/// `tasks where` (ops linked-checkouts §4, §8.1). `checkout` is serialized as `null` when
/// absent: consumers require the key on every project object.
#[derive(Serialize)]
pub struct WhereOut {
    pub project: Option<WhereProject>,
    pub holds_records: bool,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct WhereProject {
    pub prefix: String,
    pub root: String,
    pub checkout: Option<String>,
    pub linked: bool,
}
```

Add `Where(WhereOut)` to `Output` (after `Root`), its warnings arm beside
`Output::Root(o) => o.warnings.clone()`, and its pretty arm beside `Output::Root(o) =>
o.root.clone()`:

```rust
        Output::Where(o) => match &o.project {
            None => "no project".into(),
            Some(p) => match &p.checkout {
                Some(checkout) => format!("{} {} (linked to {checkout})", p.prefix, p.root),
                None => format!("{} {}", p.prefix, p.root),
            },
        },
```

`src/commands/where_.rs`:

```rust
use crate::error::Result;
use crate::link::{GitTree, holds_records, linked_project};
use crate::output::{Output, WhereOut, WhereProject};
use crate::registry::Registry;
use crate::repo::{CONFIG_REL, Project};
use std::path::{Path, PathBuf};

/// Which project a command run at `path` would open, without crossing a git boundary
/// (ops linked-checkouts decision 4): the walk for `tasks/.config.toml` stops at the
/// repository's top level, then the link step matches its main checkout. Outside git it
/// walks as `locate` does and reads no registry.
pub fn run(path: Option<PathBuf>, dir: Option<&Path>) -> Result<Output> {
    let start = super::start_dir(dir)?;
    let path = match path {
        Some(path) => start.join(path),
        None => start,
    }
    .canonicalize()?;
    let dir = if path.is_dir() {
        path
    } else {
        path.parent().expect("a file has a parent").to_path_buf()
    };
    let tree = GitTree::of(&dir)?;
    let walked = walk(&dir, tree.as_ref().map(|tree| tree.top.as_path()))?;
    let (project, holds) = match tree {
        None => (walked.map(|p| shown(p, false)), false),
        Some(tree) => {
            let registry = Registry::load()?;
            let project = match walked {
                Some(project) => Some(shown(project, false)),
                None => linked_project(&registry, &tree)?.map(|p| shown(p, true)),
            };
            (project, holds_records(&registry, &tree)?)
        }
    };
    Ok(Output::Where(WhereOut {
        project,
        holds_records: holds,
        warnings: Vec::new(),
    }))
}

fn walk(dir: &Path, stop: Option<&Path>) -> Result<Option<Project>> {
    for path in dir.ancestors() {
        if path.join(CONFIG_REL).is_file() {
            return Project::open(path).map(Some);
        }
        if Some(path) == stop {
            break;
        }
    }
    Ok(None)
}

fn shown(project: Project, linked: bool) -> WhereProject {
    WhereProject {
        prefix: project.prefix,
        root: project.root.display().to_string(),
        checkout: linked
            .then(|| project.checkout.map(|c| c.display().to_string()))
            .flatten(),
        linked,
    }
}
```

- [ ] **Step 7: `init --checkout`**

`Project::init` in `src/repo.rs` takes the checkout and writes it relative to the root:

```rust
    pub fn init(root: &Path, prefix: &str, checkout: Option<&Path>) -> Result<Project> {
```

In the existing-config branch, after the prefix comparison:

```rust
            if let Some(checkout) = checkout
                && existing.checkout.as_deref() != Some(checkout)
            {
                return Err(Error::Config(format!(
                    "{} already exists without this checkout; init does not relink an existing \
                     project: add `[checkout] path = {:?}` by hand",
                    config.display(),
                    crate::link::relative_path(&existing.root, checkout).display().to_string()
                )));
            }
```

In the `Config` literal:

```rust
                checkout: checkout
                    .map(|checkout| {
                        let relative = crate::link::relative_path(&root.canonicalize()?, checkout);
                        relative
                            .to_str()
                            .map(|path| CheckoutConfig { path: path.to_string() })
                            .ok_or_else(|| Error::Config(format!(
                                "checkout {} is not valid UTF-8", checkout.display()
                            )))
                    })
                    .transpose()?,
```

Update every other caller of `Project::init` (tests included) to pass `None`.

In `src/commands/init.rs`, `run` gains `checkout: Option<PathBuf>`. After the registry is
loaded and before `registry.register`, every init checks its destination, and one with
`--checkout` also checks the checkout:

```rust
    reject_linked_destination(&root, &registry)?;
    let checkout = checkout
        .map(|checkout| validate_checkout(&root, &root.join(checkout), &registry, &prefix))
        .transpose()?;
```

```rust
/// No command writes into a linked checkout (ops linked-checkouts §4.1), `init` included:
/// a destination in a repository that a registered project declares as its checkout is
/// refused, whichever of that repository's working trees it is in, because the
/// repository's records already live in that project's root.
fn reject_linked_destination(root: &Path, registry: &Registry) -> Result<()> {
    let Some(tree) = crate::link::GitTree::of(root)? else {
        return Ok(());
    };
    let Some(main) = tree.main_checkout() else {
        return Ok(());
    };
    let owners: Vec<_> = crate::locations::scan_checkouts(registry)
        .strict()?
        .into_iter()
        .filter(|linked| linked.checkout == main)
        .map(|linked| linked.prefix)
        .collect();
    if owners.is_empty() {
        return Ok(());
    }
    Err(Error::Config(format!(
        "{} is in {}, the linked checkout of {}; its records live in that project's root \
         (`tasks where` names it), and nothing is written into the code tree",
        root.display(),
        main.display(),
        owners.join(" and ")
    )))
}
```

(`root.join` keeps an absolute argument as given and resolves a relative one against the
directory `init` runs in, which is `root`.) Pass `checkout.as_deref()` to `Project::init`.

```rust
/// Linked-checkouts §4.1: the checkout must be the main checkout of a git repository that
/// keeps no records, outside every registered project's tree, not containing this
/// project, and declared by no other project.
fn validate_checkout(root: &Path, checkout: &Path, registry: &Registry, prefix: &str) -> Result<PathBuf> {
    let refuse = |why: String| Err(Error::Config(format!("--checkout {}: {why}", checkout.display())));
    let Ok(checkout) = checkout.canonicalize() else {
        return refuse("does not exist".into());
    };
    let Some(tree) = crate::link::GitTree::of(&checkout)? else {
        return refuse("is not in a git repository".into());
    };
    if tree.top != checkout {
        return refuse(format!("is not the top level of its repository ({})", tree.top.display()));
    }
    if !tree.is_main() {
        let main = tree.main_checkout().map(|m| m.display().to_string()).unwrap_or_default();
        return refuse(format!("is a linked worktree; pass its main checkout {main}"));
    }
    if tree.main_checkout().as_deref() != Some(checkout.as_path()) {
        return refuse("has no main checkout (a bare repository or a separate git dir)".into());
    }
    if checkout.join(crate::repo::CONFIG_REL).exists() {
        return refuse("keeps records in-tree".into());
    }
    // Repository identity, not only containment: a worktree of this repository stored
    // anywhere (`.worktrees` linked into separate storage) is still its code tree.
    if root.starts_with(&checkout)
        || crate::link::GitTree::of(root)?.is_some_and(|here| here.common == tree.common)
    {
        return refuse("holds this project; records must live outside the code tree".into());
    }
    for (other, other_root) in &registry.projects {
        if other == prefix || !other_root.try_exists()? {
            continue;
        }
        let other_root = other_root.canonicalize()?;
        if checkout.starts_with(&other_root) || other_root.starts_with(&checkout) {
            return refuse(format!("overlaps registered project {other} ({})", other_root.display()));
        }
    }
    let taken: Vec<_> = crate::locations::scan_checkouts(registry)
        .strict()?
        .into_iter()
        .filter(|linked| linked.prefix != prefix && linked.checkout == checkout)
        .map(|linked| linked.prefix)
        .collect();
    if !taken.is_empty() {
        return Err(Error::CheckoutConflict(format!(
            "{} is already the checkout of {}",
            checkout.display(),
            taken.join(" and ")
        )));
    }
    Ok(checkout)
}
```

The refused-init assertions (`!dir.join("tasks").exists()`) hold because both checks run
before `Project::init` creates anything and before `registry.save()`.

- [ ] **Step 8: Run the tests to verify they pass**

Run: `just test-one --test linked`
Expected: PASS.
Run: `just test-one parser_surface_equals_table`
Expected: PASS (the parser and the adopted table agree).

- [ ] **Step 9: Run the fast suite**

Run: `just test-fast`
Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add tools/cli.toml src/cli.rs src/main.rs src/commands/mod.rs src/commands/where_.rs \
  src/commands/init.rs src/repo.rs src/link.rs src/output.rs tests/linked.rs
git commit -m "feat(where): tasks where and init --checkout for linked projects"
```

---

### Task 5: `projects` rows, `check`, `resolve`, docs, and install

**Files:**
- Modify: `src/output.rs` (`ProjectRow.checkout`), `src/commands/projects.rs`,
  `src/commands/check.rs`, `src/locations.rs` (`Via::Checkout` in `Index::build`),
  `README.md`, `skills/tasks/SKILL.md`
- Test: `tests/linked.rs`

**Interfaces:**
- Consumes: `Project.checkout`, `locations::scan_checkouts` and `CheckoutScan` (Task 2).
- Produces: `tasks projects --json` rows with `checkout` on linked rows (§8.2);
  a `checkout_missing` warning from `check`; `resolve` matches with `via: "checkout"`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn projects_rows_carry_checkout_only_when_linked() {
    let env = TestEnv::new();
    let l = layout(&env);
    let other = project(&env, &l.overlay, "oth", None);
    let rows = env.json(&other, &["projects"])["projects"].as_array().unwrap().clone();
    let row = |p: &str| rows.iter().find(|r| r["prefix"] == p).unwrap().clone();
    assert_eq!(row("nexc")["checkout"], l.code.to_str().unwrap());
    assert!(!row("oth").as_object().unwrap().contains_key("checkout"));
}

#[test]
fn check_reports_a_declared_checkout_that_is_missing() {
    let env = TestEnv::new();
    let l = layout(&env);
    let gone = project(&env, &l.overlay, "gone", Some("../../nowhere"));
    let report = env.check(&gone);
    let text = serde_json::to_string(&report).unwrap();
    assert!(text.contains("checkout_missing") && text.contains("nowhere"), "{text}");
    assert_eq!(env.check(&l.nexc)["warnings"], serde_json::json!([]));
}

#[test]
fn resolve_matches_a_linked_checkout() {
    let env = TestEnv::new();
    let l = layout(&env);
    let out = env.json(&l.base, &["resolve", l.code.join("README").to_str().unwrap()]);
    let text = serde_json::to_string(&out).unwrap();
    assert!(text.contains("\"nexc\"") && text.contains("\"checkout\""), "{text}");
}

#[test]
fn resolve_keeps_every_valid_checkout_when_one_project_is_broken() {
    let env = TestEnv::new();
    let l = layout(&env);
    let broken = project(&env, &l.overlay, "brk", None);
    std::fs::write(broken.join("tasks/.config.toml"), "prefix = \"brk\"\n[checkout]\npath = 7\n").unwrap();
    let out = env.json(&l.base, &["resolve", l.code.join("README").to_str().unwrap()]);
    let text = serde_json::to_string(&out).unwrap();
    assert!(text.contains("\"nexc\"") && text.contains("\"checkout\""), "nexc still resolves: {text}");
    let warnings = serde_json::to_string(&out["warnings"]).unwrap();
    assert!(warnings.contains("brk"), "one warning names the broken project: {warnings}");
    assert!(!warnings.contains("nexc"), "{warnings}");
}
```

Run each separately (Cargo takes one filter):
`just test-one --test linked projects_rows`, `just test-one --test linked check_reports`,
`just test-one --test linked resolve_`.
Expected: FAIL on all four tests.

- [ ] **Step 2: `checkout` on `projects` rows**

`ProjectRow` gains (after `retired`):

```rust
    /// A linked project's declared code checkout (ops linked-checkouts §8.2). Absent on
    /// every other row, and on an unreachable project's, whose config was not read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkout: Option<String>,
```

In `projects.rs`, the reachable arm also yields `project.checkout.map(|c|
c.display().to_string())`; the unreachable arm yields `None`; the test helper `row()`
sets `checkout: None`.

- [ ] **Step 3: `check`'s finding**

In `check.rs`'s `run`, after the parse errors:

```rust
    if let Some(checkout) = &ctx.project.checkout
        && !checkout.is_dir()
    {
        warnings.push(finding(
            None,
            crate::repo::CONFIG_REL.into(),
            "checkout_missing",
            format!("[checkout] path names {}, which is not on this machine", checkout.display()),
        ));
    }
```

A warning, not an error: the same synced config is correct on a host that has the
checkout (ops-projects check reports it the same way).

- [ ] **Step 4: `resolve`'s checkout locations**

Add `Checkout` to `Via` after `Storage`, and count it as current:

```rust
    fn current(self) -> bool {
        matches!(self, Via::Root | Via::Storage | Via::Checkout)
    }
```

After `Index::build`'s per-project loop, once (Decision 5: keep every project that read,
warn once for each that did not):

```rust
        let scan = scan_checkouts(registry);
        for l in scan.linked {
            add(&l.prefix, Via::Checkout, &l.checkout, &mut warnings);
        }
        for (prefix, error) in scan.failed {
            warnings.push(format!("{prefix}: its checkout is not matched ({error})"));
        }
```

Check that `Via`'s derived `Ord` still puts `Root` first for `min_by_key` (it does: it
is declared first). Update the project-resolution spec's §4.2 table row for `checkout` to
drop "once tasks-9b0a2e lands".

- [ ] **Step 5: Run the tests to verify they pass**

Run: `just test-one --test linked`
Expected: PASS.

- [ ] **Step 6: Docs**

`README.md`: a "Linked projects" subsection beside the registry/projects section:
what `[checkout] path` means, `tasks init --prefix <p> --checkout <path>` run in the
overlay project directory, that records and docs live in the overlay and nothing is
written into the checkout, that claims and parks name the code worktree, `tasks where
[PATH]` and its three outcomes with `holds_records`, and `checkout_conflict`.

`skills/tasks/SKILL.md`: one paragraph after the "Every write command that takes an id"
paragraph:

```markdown
A linked project keeps its records outside its code checkout: its `tasks/.config.toml`
carries `[checkout] path`, and every command run anywhere in that checkout or its
worktrees opens the linked project (`tasks init --prefix <p> --checkout <path>` in the
overlay directory makes one). Claims and parks name the code worktree, so resume there;
records, specs and plans are read and written in the project root, never in the code
tree. `tasks where [PATH]` prints which project a path opens (`linked` says whether
through the link) and whether its repository keeps records; two projects declaring one
checkout is `checkout_conflict`.
```

- [ ] **Step 7: Fast suite and the gate**

Run: `just test-fast`
Expected: PASS.
Run: `just check`
Expected: clean (fmt, clippy `-D warnings`, hygiene, `tasks check`).

- [ ] **Step 8: Commit**

```bash
git add src/output.rs src/commands/projects.rs src/commands/check.rs src/locations.rs \
  docs/specs/2026-10-06-project-resolution-design.md README.md skills/tasks/SKILL.md tests/linked.rs
git commit -m "feat(projects): checkout on linked rows; check and resolve know linked checkouts"
```

- [ ] **Step 9: Install after merge**

After the branch merges to main (the integration step, not this task): `cargo install
--path .` from the main checkout, then in a scratch directory `tasks where --json` exits 0
with `{"project": null, ...}`. That is the ops plan's deployment gate for this host; the second host's
check goes through the user.
