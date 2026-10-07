# Linked Projects (tasks piece 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A tasks project whose code lives in another checkout: `[checkout] path` in its
config, `locate` that finds it from that checkout on a miss, claims and parks that record
the code worktree while records stay in the overlay, `tasks init --checkout`, `tasks where`,
and `checkout` on linked rows of `tasks projects`.

**Architecture:** One new module, `src/link.rs`, owns everything git-shaped about a link:
a path's git facts (`GitTree`), the registered project whose `[checkout]` names a
repository's main checkout (`linked_project`), and whether a repository keeps records
(`holds_records`). `src/locations.rs` gains the one strict scan of registered checkouts
(`linked_checkouts`), which `link.rs` and `resolve`'s index both read. `Project` gains two
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
- `holds_records` counts a top-level `tasks/.config.toml` only when it is tracked in `HEAD` (spec decision 5, plan review round 1).
- `tasks projects --json` rows carry `checkout` (absolute path) only for linked projects; the key is absent on every other row.
- `where` never crosses a git boundary: inside a git repository its walk for `tasks/.config.toml` stops at that repository's top level, and its link step matches that repository's main checkout. Outside git it walks as the CLI does today.
- `Project::locate` keeps its walk up unchanged (spec §4.1 "keeps the walk up"); only on a miss, inside git, does it try the link. An in-tree project never loads the registry in `locate`.
- No command writes into a linked checkout. This is a test (`git status --porcelain --ignored` in the code repository identical before and after), not a convention.
- Errors stay errors: *no project* is the only fall-through. `checkout_conflict`, a malformed config of a registered project, and an unreadable registry are errors from `locate` and `where` alike.
- `[checkout] path` is written relative to the project root by `init --checkout`; on read, an absolute path is accepted as written (ops's test stub writes one).
- A new CLI option or command is a `cli.toml` change in ops first; this repo runs `vendored adopt cli.toml` in the commit that adds the parser surface (ops Rules).
- Never call `cargo test` directly: `just test-one <filter>` or `just test-one --test linked <filter>` while working, `just test-fast` before each commit.
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
5. **`resolve` reads checkouts leniently.** `Index::build` adds `via: "checkout"`
   locations from the same scan but turns its error into a warning, as it already does for
   unreadable storage links: `resolve` answers history questions and never refuses for one
   project's broken config.

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
- Modify `src/locations.rs` — `Linked`, `linked_checkouts` (strict scan); `Via::Checkout`
  and its locations in `Index::build`.
- Modify `src/repo.rs` — `[checkout]` config table; `Project.checkout`, `Project.worktree`,
  `resume_root()`; `locate`'s link step; `Project::init` takes the checkout.
- Modify `src/error.rs` — `Error::CheckoutConflict` (`checkout_conflict`).
- Modify `src/commands/mod.rs` — claim/park/owner use `resume_root()`;
  `lock_and_revalidate` keeps the worktree; dispatch for `where` and `init --checkout`.
- Modify `src/commands/park.rs`, `src/commands/parked.rs` — park worktree; `open_recorded`
  link step; `recorded_is_scanned` compares the resume root.
- Create `src/commands/where_.rs` — the `where` command (`where` is a keyword).
- Modify `src/commands/init.rs` — `--checkout` validation.
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
ops main has landed. The ops plan allows its Tasks 1–3 to merge to main at any time.

**Files:**
- Modify (in ops main): `cli.toml`, `tests/test_cli.py`

**Interfaces:**
- Produces: ops main's `cli.toml` with `{ names = ["--checkout"], value = "path" }` on
  `init` and a `where` command row (`summary = "The project a path belongs to, in-tree or
  linked, and whether its repository keeps records"`, one optional `path` arg of value
  `path`). Task 4 adopts it.

- [ ] **Step 1: Apply only the vocabulary hunks to ops main**

In the ops main checkout (clean tree on `main`):

```bash
git checkout 91f191b -- cli.toml tests/test_cli.py
git diff --cached --stat
```

Expected: only `cli.toml` and `tests/test_cli.py` staged. If main's `cli.toml` has moved
since `91f191b` (it has: obs rows changed), `git checkout <commit> -- cli.toml` would
revert those. Instead:

```bash
git reset -q
git show 91f191b -- cli.toml tests/test_cli.py | git apply --index
```

Expected: applies cleanly (the hunks touch only the `init` and `root`/`where` rows and
their test). If it does not, stop and report the conflict.

- [ ] **Step 2: Run ops's gate for the change**

Run: `just test-one tests.test_cli`
Expected: PASS.
Run: `just test-fast`
Expected: PASS.

- [ ] **Step 3: Commit on ops main**

```bash
git commit -m "feat(cli): declare tasks where and init --checkout"
```

Do not push (a push is outside this repository's authority); the adoption in Task 4 reads
the local main.

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
  - `Project.worktree: Option<PathBuf>` — code working tree top, set only by the link.
  - `Project::resume_root(&self) -> &Path`.
  - `crate::link::GitTree { top: PathBuf, common: PathBuf, git_dir: PathBuf }` with
    `GitTree::of(dir: &Path) -> Result<Option<GitTree>>`,
    `fn main_checkout(&self) -> Option<PathBuf>`, `fn is_main(&self) -> bool`,
    `fn worktrees(&self) -> Result<Vec<PathBuf>>`.
  - `crate::link::linked_project(registry: &Registry, tree: &GitTree) -> Result<Option<Project>>`.
  - `crate::locations::Linked { prefix: String, root: PathBuf, checkout: PathBuf }` and
    `crate::locations::linked_checkouts(registry: &Registry) -> Result<Vec<Linked>>`.
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
    /// The code working tree this command runs in, when `locate` found the project through
    /// its `[checkout]` link; `None` otherwise. Claims and parks record it (§4.1, "record
    /// location and resume location are separate").
    pub worktree: Option<PathBuf>,
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

In `Project::open`, set the new fields (the root is already canonical there):

```rust
            checkout: checkout_path(&root, config.checkout)?,
            worktree: None,
```

Note the order: `root` is moved into the struct; compute `let checkout =
checkout_path(&root, config.checkout)?;` before the `Ok(Project { ... })` and use it.

In `Project::init`, the `Config` literal gains `checkout: None` (Task 4 changes this).

Add the accessor in `impl Project`:

```rust
    /// Where a session working on this project resumes: the code working tree for a
    /// project found through its link, otherwise the root. Claims, parks, and the owner
    /// fallback use it; records are never read from it.
    pub fn resume_root(&self) -> &Path {
        self.worktree.as_deref().unwrap_or(&self.root)
    }
```

Every other place that builds a `Project` literal (search `Project {` in `src/` and tests)
gains `checkout: None, worktree: None`.

- [ ] **Step 5: The strict scan in `src/locations.rs`**

```rust
/// One registered project that declares a `[checkout]` (linked-checkouts §4.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Linked {
    pub prefix: String,
    pub root: PathBuf,
    pub checkout: PathBuf,
}

/// Every registered project on this host that declares a checkout. A project whose root
/// is gone or holds no config is skipped: it cannot be opened here, so nothing can be
/// linked to it. One whose config exists and does not open is an error, never skipped
/// (§4.1a: errors stay errors).
pub fn linked_checkouts(registry: &crate::registry::Registry) -> crate::error::Result<Vec<Linked>> {
    let mut found = Vec::new();
    for (prefix, root) in &registry.projects {
        if !crate::scope::is_reachable(root)? {
            continue;
        }
        let project =
            crate::scope::open_registered(registry, prefix, crate::scope::Origin::Prefix)?;
        if let Some(checkout) = project.checkout {
            found.push(Linked {
                prefix: prefix.clone(),
                root: project.root,
                checkout,
            });
        }
    }
    Ok(found)
}
```

- [ ] **Step 6: `src/link.rs`**

```rust
//! Linked checkouts: a path's git facts and the registered project whose `[checkout]`
//! names its repository (ops docs/specs/2026-10-01-linked-checkouts-design.md §4.1).

use crate::error::{Error, Result};
use crate::registry::Registry;
use crate::repo::Project;
use std::path::{Path, PathBuf};

/// git's view of a directory: its working tree's top level, the repository's common git
/// directory, and this working tree's own git directory, all canonical.
#[derive(Debug, Clone, PartialEq)]
pub struct GitTree {
    pub top: PathBuf,
    pub common: PathBuf,
    pub git_dir: PathBuf,
}

impl GitTree {
    /// `None` when git says `dir` is not inside a repository, or there is no git
    /// executable; any other git failure is an `io` error. `LC_ALL=C` pins the one
    /// message matched, as `Project::git_toplevel` does.
    pub fn of(dir: &Path) -> Result<Option<GitTree>> {
        let output = match std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "rev-parse",
                "--path-format=absolute",
                "--show-toplevel",
                "--git-common-dir",
                "--absolute-git-dir",
            ])
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
                .ok_or_else(|| Error::Io(format!("git rev-parse in {} printed too little", dir.display())))?
                .map_err(Error::from)
        };
        Ok(Some(GitTree {
            top: next()?,
            common: next()?,
            git_dir: next()?,
        }))
    }

    /// The repository's main checkout: the common directory's parent when that is a
    /// `.git` directory. `None` for a bare repository or a separate git dir.
    pub fn main_checkout(&self) -> Option<PathBuf> {
        (self.common.file_name()? == ".git").then(|| self.common.parent().map(Path::to_path_buf))?
    }

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
}

/// The registered project whose `[checkout]` is `tree`'s main checkout, opened with
/// `worktree` set to `tree`'s top level. Two such projects are `checkout_conflict`.
pub fn linked_project(registry: &Registry, tree: &GitTree) -> Result<Option<Project>> {
    let Some(main) = tree.main_checkout() else {
        return Ok(None);
    };
    let matches: Vec<_> = crate::locations::linked_checkouts(registry)?
        .into_iter()
        .filter(|linked| linked.checkout == main)
        .collect();
    match matches.as_slice() {
        [] => Ok(None),
        [one] => {
            let mut project =
                crate::scope::open_registered(registry, &one.prefix, crate::scope::Origin::Prefix)?;
            project.worktree = Some(tree.top.clone());
            Ok(Some(project))
        }
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
        let tree = |common: &str| GitTree {
            top: "/r".into(),
            common: common.into(),
            git_dir: common.into(),
        };
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
file whose `Project` literal gained the two fields.)

---

### Task 3: Record location and resume location

**Files:**
- Modify: `src/commands/mod.rs` (`claim_guard`, `follow_holder`, `transition`,
  `raw_owner_name`, `lock_and_revalidate`), `src/commands/park.rs`,
  `src/commands/parked.rs` (`open_recorded`, the `recorded_is_scanned` comparison)
- Test: `tests/linked.rs`

**Interfaces:**
- Consumes: `Project::resume_root`, `Project.worktree`, `link::GitTree::of`,
  `link::linked_project` (Task 2).
- Produces: claims and parks whose `worktree` is the code working tree for a linked
  project; `parked::open_recorded(id, worktree)` that opens the overlay project for a
  recorded code worktree.

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
        let id = env.json(tree, &["add", &format!("Linked work {n}")])["id"]
            .as_str()
            .unwrap()
            .to_string();
        env.json(tree, &["start", &id]);
        let shown = env.json(tree, &["show", &id]);
        assert_eq!(shown["claim"]["worktree"], top, "{n}: the claim names the code tree");
        assert_eq!(shown["task"]["owner"], branch, "{n}: the owner reads the code branch");
        env.json(tree, &["note", &id, "a note"]);
        env.json(tree, &["edit", &id, "--spec", "topic"]);
        env.json(tree, &["attach", &id, shot.to_str().unwrap(), "--caption", "a shot"]);
        env.json(tree, &["park", &id, "resume here"]);

        for view in [
            env.json(&l.code, &["quiet", "--project", "nexc"]),
            env.json(&l.code, &["next"]),
            env.json(&l.code, &["prime"]),
        ] {
            let text = string(&view);
            assert!(text.contains(&id) && text.contains(top), "{n}: {text}");
        }
        let found = env.json(elsewhere.path(), &["show", &id]);
        assert_eq!(found["park"]["worktree"], top, "{n}");
        assert_eq!(found["task"]["spec"], "docs/specs/topic.md", "{n}");

        env.json(tree, &["start", &id]);
        env.json(tree, &["done", &id, "landed"]);
        assert!(l.nexc.join(format!("tasks/{id}.md")).is_file(), "{n}");
        assert!(l.nexc.join(format!("tasks/files/{id}")).is_dir(), "{n}: attachments in the overlay");
    }
    assert_eq!(porcelain(&l.code), before, "nothing written into the code repository");
    assert_eq!(porcelain(&l.wt), before_wt, "nothing written into the code worktree");
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

If `quiet` refuses a park that is not `--reason quiet`, replace that entry with `env.json(&l.code,
&["list", "--parked"])`; the assertion stays the same. Check `tasks quiet --help` first and
keep whichever view lists agent-waiting parks.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `just test-one --test linked lifecycle`
Expected: FAIL on the `claim.worktree` assertion (it names the overlay project root).

- [ ] **Step 3: Record the resume root**

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

- [ ] **Step 4: Keep the link across the lock**

In `lock_and_revalidate`, the local arm reopens the root and would drop `worktree`:

```rust
            Routing::Local => Project {
                worktree: ctx.project.worktree.clone(),
                ..Project::open(&ctx.project.root)?
            },
```

- [ ] **Step 5: `open_recorded` through the link**

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

- [ ] **Step 6: Run the tests to verify they pass**

Run: `just test-one --test linked`
Expected: PASS.

- [ ] **Step 7: Run the fast suite**

Run: `just test-fast`
Expected: PASS (in-tree projects have `worktree: None`, so `resume_root()` is the root and
every existing claim and park test is unchanged).

- [ ] **Step 8: Commit**

```bash
git add src/commands/mod.rs src/commands/park.rs src/commands/parked.rs tests/linked.rs
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
- Produces: `Output::Where(WhereOut)`; `link::holds_records(registry: &Registry, tree:
  &GitTree) -> Result<bool>`; `link::relative_path(from: &Path, to: &Path) -> PathBuf`;
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

- [ ] **Step 5: `holds_records` and `relative_path` in `src/link.rs`**

```rust
/// Linked-checkouts decision 5 and §8.1: the repository holding `tree` keeps our records
/// when a registered project's root is in it (the same common git directory), or its top
/// level holds a `tasks/.config.toml` tracked in `HEAD`. Only roots inside one of the
/// repository's working trees can share its common directory, so git runs only for those.
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
    let tracked = std::process::Command::new("git")
        .arg("-C")
        .arg(&tree.top)
        .args(["cat-file", "-e", "HEAD:tasks/.config.toml"])
        .env("LC_ALL", "C")
        .stderr(std::process::Stdio::null())
        .status()?;
    Ok(tracked.success())
}

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
loaded and before `registry.register`:

```rust
    let checkout = checkout
        .map(|checkout| validate_checkout(&root, &root.join(checkout), &registry, &prefix))
        .transpose()?;
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
    if root.starts_with(&checkout) {
        return refuse("contains this project; records must live outside the code tree".into());
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
    let taken: Vec<_> = crate::locations::linked_checkouts(registry)?
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

The refused-init assertion (`!dir.join("tasks").exists()`) holds because validation runs
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
- Consumes: `Project.checkout`, `locations::linked_checkouts` (Task 2).
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
```

Run: `just test-one --test linked projects_rows check_reports resolve_matches`
Expected: FAIL on all three.

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

At the end of `Index::build`'s loop (after the per-project loop, once):

```rust
        match linked_checkouts(registry) {
            Ok(linked) => {
                for l in linked {
                    add(&l.prefix, Via::Checkout, &l.checkout, &mut warnings);
                }
            }
            Err(e) => warnings.push(format!("linked checkouts not matched: {e}")),
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
