# Prefix rename implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename a project's prefix with one command, keeping every existing reference to
the old name resolving through a retired-prefix alias that lives as long as the project's
registration does.

**Architecture:** A retired prefix becomes an alias in the registry; because a rename
preserves the hex, resolution rewrites only the prefix component of an id, so no other
repository is ever written to. The rename itself runs in six phases behind an inventory
that records digests before anything moves, and recovery is a pure classifier over an
observation of the world plus that baseline.

**Tech Stack:** Rust 2024, clap, serde/toml, sha2 (new — see Global Constraints).

**Spec:** `docs/specs/2026-09-08-prefix-rename-design.md`

## Global Constraints

- **JSON output is the contract.** No existing shape changes meaning. New fields only.
- **Fail early with a typed error; no silent fallbacks.** Every refusal in this plan is an
  `Error` variant from `src/error.rs`, never a warning-and-continue.
- **`just check` before every commit; `just gate` before the final one.** `check` is
  `cargo fmt --check && cargo clippy --all-targets -- -D warnings && tasks check`.
- **Conventional commits, no AI-attribution trailers.**
- **Aliases never chain:** every alias targets a live prefix. Enforced at load.
- **Recovery is scoped to process death, not power loss** (spec §9). No `fsync` is added.
- **New dependency: `sha2 = "0.10"`.** The inventory's correctness argument rests on
  content identity, and `DefaultHasher` is explicitly unstable across Rust releases, so it
  cannot be written to a file that must survive a crash and a toolchain upgrade. Added in
  Task 9. **If the reviewer prefers zero new dependencies, stop and raise it there** — the
  alternative is storing whole file bodies in the inventory, which is a spec change.
- **`tasks check` must stay clean.** This plan's `### Task N:` headings are linked to child
  tasks of `tasks-8c9398`; renaming a heading without updating its task fails the gate.
- **There is no library target.** Unit tests in `src/` run under the binary:
  `cargo test --bin tasks <filter>`. `cargo test --lib` fails with
  "no library targets found in package `tasks`".
- **Every task ends by reinstalling and closing its task, in the same commit as the code**
  (AGENTS.md): `cargo install --path .` so the `tasks` binary used by the next task is the
  code under test, then `tasks done <id> "<what landed>"`. Each task's final step spells
  out both with its own id.

## File Structure

**New:**
- `src/rename/mod.rs` — the phases and their sequencing; the only module that writes.
- `src/rename/inventory.rs` — the `Inventory` record, its digests, read/write/remove.
- `src/rename/snapshot.rs` — observing the world into a `Snapshot`. Touches the filesystem.
- `src/rename/classify.rs` — `classify(&Snapshot) -> Recovery`. Pure; no filesystem, no locks.
- `src/rename/rewrite.rs` — the byte-preserving frontmatter rewrite.
- `src/commands/rename.rs` — CLI wiring, `--explain`, output shaping.

**Modified:**
- `src/registry.rs` — `aliases` table, load invariants, canonicalization, `rename`, `unregister`.
- `src/scope.rs` — alias-aware `open_registered`.
- `src/commands/mod.rs` — canonicalization at input, `add`'s lock, revalidation, the freeze.
- `src/resolve.rs`, `src/commands/dep.rs` — canonicalization at resolution and comparison.
- `src/repo.rs` — the stale-local-config check.
- `src/commands/check.rs` — `retired_prefix`.
- `src/commands/init.rs`, `src/commands/unregister.rs` — name reservation and alias drops.
- `src/cli.rs`, `src/output.rs`, `src/format.rs` — the subcommand, `RenameOut`, the shared
  timestamp helper.
- `tests/cli.rs` — end-to-end.

Tasks 1–7 deliver the alias half and are independently useful: after Task 7 a hand-rename
is safe, because every existing reference keeps resolving. Tasks 8–12 add the command.

---

### Task 1: Registry aliases and canonicalization

**Files:**
- Modify: `src/registry.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `Registry.aliases: BTreeMap<String, String>`;
  `Registry::canonical_prefix(&self, prefix: &str) -> &str`;
  `Registry::canonical_id(&self, id: &TaskId) -> TaskId`;
  `Registry::is_taken(&self, prefix: &str) -> bool`.

Purely additive: no existing signature changes, so every caller still compiles.

- [x] **Step 1: Write the failing tests**

In `src/registry.rs`, in the existing `mod tests`:

```rust
#[test]
fn aliases_canonicalize_and_reserve_names() {
    let mut r = Registry::default();
    r.register("dots", Path::new("/tmp/dotfiles")).unwrap();
    r.aliases.insert("dot".into(), "dots".into());

    assert_eq!(r.canonical_prefix("dot"), "dots");
    assert_eq!(r.canonical_prefix("dots"), "dots");
    assert_eq!(r.canonical_prefix("nope"), "nope", "an unknown prefix is left alone");

    let retired = TaskId::parse("dot-a00088").unwrap();
    assert_eq!(r.canonical_id(&retired).to_string(), "dots-a00088");
    let live = TaskId::parse("dots-a00088").unwrap();
    assert_eq!(r.canonical_id(&live).to_string(), "dots-a00088");

    assert!(r.is_taken("dots"), "a live prefix is taken");
    assert!(r.is_taken("dot"), "an alias is taken");
    assert!(!r.is_taken("free"));
}

#[test]
fn load_rejects_a_dangling_or_colliding_alias() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("projects.toml");

    std::fs::write(&path, "[projects]\ndots = \"/tmp/d\"\n\n[aliases]\ndot = \"gone\"\n").unwrap();
    let error = Registry::load_from(&path).unwrap_err();
    assert_eq!(error.kind(), "config");
    assert!(error.to_string().contains("dot"), "{error}");

    std::fs::write(&path, "[projects]\ndots = \"/tmp/d\"\n\n[aliases]\ndots = \"dots\"\n").unwrap();
    assert_eq!(Registry::load_from(&path).unwrap_err().kind(), "config");
}

#[test]
fn a_registry_without_an_aliases_table_still_loads() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("projects.toml");
    std::fs::write(&path, "[projects]\nsci = \"/tmp/a\"\n").unwrap();
    let r = Registry::load_from(&path).unwrap();
    assert!(r.aliases.is_empty());
    assert_eq!(r.project_root("sci").unwrap(), Path::new("/tmp/a"));
}
```

Add `use crate::model::TaskId;` to the test module if it is not already in scope.

- [x] **Step 2: Run the tests to verify they fail**

Run: `cargo test --bin tasks registry::`
Expected: FAIL — `no method named canonical_prefix`, `no field aliases`.

- [x] **Step 3: Add the field, the invariants, and the primitives**

```rust
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub projects: BTreeMap<String, PathBuf>,
    /// Retired prefix -> the live prefix it now resolves to. Never chains: every target is
    /// a live prefix, enforced on load, so resolution is always one hop.
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
}
```

In `load_from`, after the `~` expansion loop and before `Ok(registry)`:

```rust
    for (alias, target) in &registry.aliases {
        if registry.projects.contains_key(alias) {
            return Err(Error::Config(format!(
                "{}: alias {alias:?} collides with a live prefix",
                path.display()
            )));
        }
        if !registry.projects.contains_key(target) {
            return Err(Error::Config(format!(
                "{}: alias {alias:?} targets {target:?}, which is not a registered project",
                path.display()
            )));
        }
    }
```

In `impl Registry`:

```rust
    /// The live prefix `prefix` resolves to. One hop: aliases never chain (§2).
    pub fn canonical_prefix<'a>(&'a self, prefix: &'a str) -> &'a str {
        self.aliases.get(prefix).map_or(prefix, String::as_str)
    }

    /// The same rule applied to an id. Only the prefix component moves; the hex is
    /// preserved by a rename, which is what makes this a rule rather than an index.
    pub fn canonical_id(&self, id: &TaskId) -> TaskId {
        let prefix = self.canonical_prefix(&id.prefix);
        if prefix == id.prefix {
            return id.clone();
        }
        TaskId {
            prefix: prefix.to_string(),
            hex: id.hex.clone(),
        }
    }

    /// Whether a prefix may be claimed: a live prefix and a retired one are both taken,
    /// because an id must never mean two projects.
    pub fn is_taken(&self, prefix: &str) -> bool {
        self.projects.contains_key(prefix) || self.aliases.contains_key(prefix)
    }
```

Add `use crate::model::TaskId;` to the module imports.

- [x] **Step 4: Run the tests to verify they pass**

Run: `cargo test --bin tasks registry::` — Expected: PASS.
Run: `just check` — Expected: clean.

- [x] **Step 5: Reinstall, close the task, and commit**

```bash
cargo install --path .   # AGENTS.md: the `tasks` the next task uses must be this code
tasks done tasks-590d4d "Registry gains an aliases table with load-time invariants (every target live, no collision with a live prefix), plus canonical_prefix, canonical_id and is_taken. Purely additive: no caller changed."
git add src/registry.rs tasks/
git commit -m "feat(registry): record retired prefixes as aliases"
```

---

### Task 2: Alias-aware project resolution

**Files:**
- Modify: `src/scope.rs:27-50` (`open_registered`)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Registry::canonical_prefix`, `Registry::is_taken` (Task 1).
- Produces: `open_registered` accepts a retired prefix and opens its live project.

**Scope note:** this task resolves the *project* from a retired prefix. Reading a *task*
through a retired id additionally needs `canonical_id` at the input, which is Task 3 —
`show old-a00088` would otherwise open the right project and then look for
`tasks/old-a00088.md`, which does not exist. The test below therefore exercises `root` and
`--project`, and Task 3's test covers `show`.

- [x] **Step 1: Write the failing test**

In `tests/cli.rs`. `alias_registry` writes the alias directly because `tasks rename` does
not exist until Task 11; every later task reuses this helper.

```rust
/// Register `alias` as a retired prefix of `target` by writing the registry directly.
/// Until Task 11 there is no command that does this.
fn alias_registry(env: &TestEnv, alias: &str, target: &str) {
    let path = env.home.path().join(".config/tasks/projects.toml");
    let mut text = std::fs::read_to_string(&path).unwrap();
    if !text.contains("[aliases]") {
        text.push_str("\n[aliases]\n");
    }
    text.push_str(&format!("{alias} = {target:?}\n"));
    std::fs::write(&path, text).unwrap();
}

#[test]
fn a_retired_prefix_resolves_to_its_live_project() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let id = id_of(env.json(&fam, &["add", "Far", "-p", "2"]));
    alias_registry(&env, "old", "fam");

    let retired = format!("old-{}", id.split_once('-').unwrap().1);
    // `root` resolves the *project* from the prefix, which is all this task delivers.
    assert_eq!(env.json(&sci, &["root", &retired])["prefix"], "fam");

    // --project takes a retired name too: it is a name of the project.
    let v = env.json(&sci, &["list", "--project", "old"]);
    assert_eq!(v["tasks"][0]["id"], id, "{v}");
}
```

- [x] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli -- a_retired_prefix_resolves`
Expected: FAIL — `unresolvable_id`, no project registered as "old".

- [x] **Step 3: Follow the alias in `open_registered`**

In `src/scope.rs`, replace the body's opening lookup:

```rust
pub fn open_registered(registry: &Registry, prefix: &str, origin: Origin) -> Result<Project> {
    let fail = |detail: String| match origin {
        Origin::Id(id) => Error::UnresolvableId(format!("{id}: {detail}")),
        Origin::Prefix => Error::Config(detail),
    };
    // A retired prefix is a name of the project, so it resolves like the live one. The
    // guard below then compares against the *canonical* prefix, so it keeps catching a
    // registry that points a name at the wrong root.
    let live = registry.canonical_prefix(prefix);
    let Some(root) = registry.project_root(live) else {
        return Err(fail(format!("no project registered as {prefix:?}")));
    };
    if !has_config(root)? {
        return Err(fail(format!(
            "project {live:?} at {} has no {CONFIG_REL}; run `tasks init` there",
            root.display()
        )));
    }
    let project = Project::open(root)?;
    if project.prefix != live {
        return Err(Error::Config(format!(
            "registry maps {live:?} to {}, whose prefix is {:?}; fix the registry",
            root.display(),
            project.prefix
        )));
    }
    Ok(project)
}
```

- [x] **Step 4: Run the tests to verify they pass**

Run: `cargo test --test cli -- a_retired_prefix_resolves` — Expected: PASS.
Run: `just gate` — Expected: clean; no existing test regresses.

- [x] **Step 5: Reinstall, close the task, and commit**

```bash
cargo install --path .   # AGENTS.md: the `tasks` the next task uses must be this code
tasks done tasks-03a9c2 "open_registered follows an alias to its live project, so a retired prefix names the project in root and --project. Its misconfiguration guard now compares against the canonical prefix, so it still catches a registry pointing a name at the wrong root."
git add src/scope.rs tests/cli.rs tasks/
git commit -m "feat(scope): resolve a retired prefix to its live project"
```

---

### Task 3: Canonicalize input, resolution, and comparison

**Files:**
- Modify: `src/commands/mod.rs` (`open_id_write_ctx:174`, `open_id_read_ctx`, a parse helper)
- Modify: `src/resolve.rs` (`resolve_task`)
- Modify: `src/commands/dep.rs:29-60`
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Registry::canonical_id` (Task 1).
- Produces: `commands::parse_id(&Registry, &str) -> Result<TaskId>` — parses and
  canonicalizes user input. Every command that takes an id string uses it.

**Do not** canonicalize `task.depends` or `task.parent` in memory: `save` rewrites the whole
file from the model, so that would silently rewrite stored references during an unrelated
edit and erase the Task 5 nudge (spec §4).

- [x] **Step 1: Write the failing test**

```rust
#[test]
fn a_retired_id_is_one_task_for_routing_dedup_and_removal() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let a = id_of(env.json(&sci, &["add", "A", "-p", "2"]));
    let b = id_of(env.json(&sci, &["add", "B", "-p", "2"]));
    alias_registry(&env, "old", "sci");
    let retired_a = format!("old-{}", a.split_once('-').unwrap().1);
    let retired_b = format!("old-{}", b.split_once('-').unwrap().1);

    // Reading a task through a retired id (deferred here from Task 2).
    assert_eq!(env.json(&sci, &["show", &retired_a])["task"]["id"], a);
    // And through the list-backed resolution path, which Scope::resolve_task serves.
    assert_eq!(env.json(&sci, &["list", "--parent", &retired_a])["tasks"], serde_json::json!([]));

    // Routing: a retired id of the *current* project stays in this checkout.
    env.json(&sci, &["note", &retired_a, "written through the retired name"]);
    let shown = env.json(&sci, &["show", &a]);
    assert_eq!(shown["task"]["notes"][0]["text"], "written through the retired name");

    // New references store the live prefix.
    env.json(&sci, &["dep", &a, "--on", &retired_b]);
    assert_eq!(env.json(&sci, &["show", &a])["task"]["depends"][0], b);

    // Dedup: the same task by its other name is not a second dependency.
    env.json(&sci, &["dep", &a, "--on", &b]);
    assert_eq!(
        env.json(&sci, &["show", &a])["task"]["depends"].as_array().unwrap().len(),
        1
    );

    // Self-dependency and cycles see through the alias.
    assert_eq!(env.fail(&sci, &["dep", &a, "--on", &retired_a]), "cycle");

    // Removal matches either spelling.
    env.json(&sci, &["dep", &a, "--rm", &retired_b]);
    assert!(env.json(&sci, &["show", &a])["task"]["depends"].as_array().unwrap().is_empty());
}
```

- [x] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli -- a_retired_id_is_one_task`
Expected: FAIL — `task_not_found` on the first `note`.

- [x] **Step 3: Canonicalize at the three points**

In `src/commands/mod.rs`:

```rust
/// Parse a user-supplied id and canonicalize it, so a retired prefix names the same task
/// as the live one everywhere: routing, storage, dedup, removal, and cycle detection.
pub fn parse_id(registry: &Registry, id: &str) -> Result<TaskId> {
    Ok(registry.canonical_id(&TaskId::parse(id)?))
}
```

In `open_id_write_ctx`, replace `let id = TaskId::parse(id)?;` with a canonicalizing load.
The registry must be read before the id can be canonicalized, so build the ctx first:

```rust
pub fn open_id_write_ctx(dir: Option<&Path>, id: &str) -> Result<Ctx> {
    let mut ctx = open_ctx(dir)?;
    let id = parse_id(&ctx.registry, id)?;
    if id.prefix != ctx.project.prefix {
        ctx.project = crate::scope::open_registered(&ctx.registry, &id.prefix, Origin::Id(&id))?;
    }
    ctx.lock = Some(MutationLock::acquire(&ctx.project.prefix)?);
    Ok(ctx)
}
```

Apply the same change in `open_id_read_ctx` (Task: `tree`'s router) — canonicalize before
comparing against the local project's prefix.

In `src/resolve.rs`, canonicalize in **both** resolution entry points. `Resolver::resolve_task`
is only half of it: `Scope::resolve_task` (`src/scope.rs:141`) backs `list`, `ready`, `next`,
and `prime`, it compares `project.prefix == id.prefix` raw, and its fallback reaches
`resolve_registered` (`src/resolve.rs:125`), whose first act is a raw
`registry.project_root(&id.prefix)` that returns `None` for an alias and bails long before
the alias-aware `open_registered` of Task 2 is ever called.

```rust
// src/resolve.rs
pub fn resolve_registered(registry: &Registry, id: &TaskId) -> Result<Option<Task>> {
    let id = &registry.canonical_id(id);
    let Some(root) = registry.project_root(&id.prefix) else {
        return Ok(None);
    };
    // … unchanged from here
}
```

```rust
// src/scope.rs
    pub fn resolve_task(&self, registry: &Registry, id: &TaskId) -> Result<Option<Task>> {
        let id = &registry.canonical_id(id);
        match self.projects().iter().find(|p| p.prefix == id.prefix) {
            Some(project) => crate::resolve::read_present(project, id),
            None => crate::resolve::resolve_registered(registry, id),
        }
    }
```

**Every user-input parse site uses `parse_id`.** The ones that reparse a retired id and would
otherwise keep failing:

| Site | Command surface |
|---|---|
| `src/commands/mod.rs:361` (`load`) | every write command's task lookup |
| `src/commands/mod.rs:308`, `:323` (`apply_fields`) | `--depends`, `--parent` on `add` and `edit` |
| `src/commands/show.rs:11` | `show` |
| `src/commands/root.rs:11` | `root` |
| `src/commands/tree.rs:12` | `tree <id>` |
| `src/commands/edit.rs:95` | `edit` |
| `src/commands/list.rs:39` | `list --parent` |
| `src/commands/feedback.rs:61` | `feedback --recur` |
| `src/commands/dep.rs:34`, `:48` | `dep --on`, `dep --rm` |

`apply_fields` and `load` take a `&Ctx`, so the registry is already to hand; `list.rs` takes
a `ReadCtx`. None needs a new parameter.

In `src/commands/dep.rs`, canonicalize both sides of every comparison:

```rust
    if !on.is_empty() {
        let resolver = Resolver::new(&ctx.project, &ctx.registry);
        for value in &on {
            let dependency = super::parse_id(&ctx.registry, value)?;
            if dependency == task.id {
                return Err(Error::Cycle(format!("{dependency} -> {dependency}")));
            }
            if resolver.resolve_task(&dependency)?.is_none() {
                return Err(Error::UnresolvableId(dependency.to_string()));
            }
            // Stored refs may carry a retired prefix; compare canonically so the same task
            // under either name is one dependency.
            if !task
                .depends
                .iter()
                .any(|item| ctx.registry.canonical_id(item) == dependency)
            {
                task.depends.push(dependency);
            }
        }
        ensure_acyclic(&ctx, &task)?;
    } else {
        for value in &rm {
            let dependency = super::parse_id(&ctx.registry, value)?;
            let before = task.depends.len();
            task.depends
                .retain(|item| ctx.registry.canonical_id(item) != dependency);
            if task.depends.len() == before {
                return Err(Error::Validation(format!(
                    "{} does not depend on {dependency}",
                    task.id
                )));
            }
        }
    }
```

In `ensure_acyclic`, canonicalize the ids the edge function yields:

```rust
    let edges = |id: &TaskId| -> Result<Option<Vec<TaskId>>> {
        let id = ctx.registry.canonical_id(id);
        if id == candidate.id {
            return Ok(Some(candidate.depends.clone()));
        }
        Ok(resolver.resolve_task(&id)?.map(|task| {
            task.depends
                .iter()
                .map(|d| ctx.registry.canonical_id(d))
                .collect()
        }))
    };
```

Update `show::run` and `root` to use `parse_id` as well.

- [x] **Step 4: Run the tests to verify they pass**

Run: `cargo test --test cli -- a_retired_id_is_one_task` — Expected: PASS.
Run: `just gate` — Expected: clean.

- [x] **Step 5: Reinstall, close the task, and commit**

```bash
cargo install --path .   # AGENTS.md: the `tasks` the next task uses must be this code
tasks done tasks-66120e "canonical_id applied at every user-input parse site, at both resolution entry points (Resolver::resolve_task and Scope::resolve_task via resolve_registered, which backs list/ready/next/prime), and on both sides of dep's dedup, removal, self-check and cycle detection. Stored depends are deliberately left as written so an unrelated save does not rewrite them."
git add src/commands/mod.rs src/resolve.rs src/scope.rs src/commands/dep.rs \
        src/commands/show.rs src/commands/root.rs src/commands/tree.rs \
        src/commands/edit.rs src/commands/list.rs src/commands/feedback.rs \
        tests/cli.rs tasks/
git commit -m "feat(ids): treat a retired prefix as the same task everywhere"
```

---

### Task 4: Refuse a stale local checkout

**Files:**
- Modify: `src/commands/mod.rs` (`open_ctx`, `open_read_ctx`)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Registry::canonical_prefix` (Task 1).
- Produces: `commands::reject_stale_local(&Registry, &Project) -> Result<()>`.

No pre-existing guard catches this: `open_registered` validates the *registered* root,
which after a rename is consistent, and `Project::locate` never consults the registry
(spec §4). Reads refuse too — reporting a project under a name the registry retired is the
same lie, more quietly told.

- [x] **Step 1: Write the failing test**

```rust
#[test]
fn a_checkout_still_using_a_retired_prefix_refuses() {
    let mut env = TestEnv::new();
    let dots = env.init("dots");
    // A second root under the retired name: what a branch predating the rename looks like.
    let stale = env.init_forced("dots");
    std::fs::write(
        stale.join("tasks/.config.toml"),
        "prefix = \"dot\"\n",
    )
    .unwrap();
    let path = env.home.path().join(".config/tasks/projects.toml");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text = text.replace(&format!("dots = {:?}", stale.to_str().unwrap()), &format!("dots = {:?}", dots.to_str().unwrap()));
    std::fs::write(&path, text).unwrap();
    alias_registry(&env, "dot", "dots");

    // Both a read and a write refuse, rather than routing or minting an old-prefix file.
    assert_eq!(env.fail(&stale, &["list"]), "config");
    assert_eq!(env.fail(&stale, &["add", "New", "-p", "2"]), "config");
}
```

- [x] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli -- a_checkout_still_using_a_retired`
Expected: FAIL — both commands succeed.

- [x] **Step 3: Check the local prefix against the registry**

In `src/commands/mod.rs`:

```rust
/// A checkout whose own prefix the registry has retired. Nothing else sees this:
/// `open_registered` validates the registered destination, which after a rename is
/// perfectly consistent, and `Project::locate` never reads the registry at all.
pub fn reject_stale_local(registry: &Registry, project: &Project) -> Result<()> {
    let live = registry.canonical_prefix(&project.prefix);
    if live != project.prefix {
        return Err(Error::Config(format!(
            "this checkout's tasks/.config.toml says {:?}, which was renamed to {live:?}; \
             update the checkout (merge or rebase onto the rename) before using it here",
            project.prefix
        )));
    }
    Ok(())
}
```

Call it in `open_ctx` after loading the registry, and in `open_read_ctx`'s local arm.

- [x] **Step 4: Run the tests to verify they pass**

Run: `cargo test --test cli -- a_checkout_still_using_a_retired` — Expected: PASS.
Run: `just gate` — Expected: clean.

- [x] **Step 5: Reinstall, close the task, and commit**

```bash
cargo install --path .   # AGENTS.md: the `tasks` the next task uses must be this code
tasks done tasks-bdde89 "Every command checks its local project prefix against the registry and refuses when the registry has retired it. Nothing else saw this: open_registered validates the registered destination and Project::locate never reads the registry."
git add src/commands/mod.rs tests/cli.rs tasks/
git commit -m "feat(scope): refuse a checkout whose prefix the registry retired"
```

---

### Task 5: The `retired_prefix` check warning

**Files:**
- Modify: `src/commands/check.rs:75-95`
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Registry::canonical_id` (Task 1).
- Produces: `check` warning kind `retired_prefix`.

Raised in the *referring* project on a `depends` entry naming a retired prefix. Prose is
never examined, so the note mentions the spec measured stay silent and keep resolving.

- [x] **Step 1: Write the failing test**

```rust
#[test]
fn check_nudges_a_depends_naming_a_retired_prefix() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let far = id_of(env.json(&fam, &["add", "Far", "-p", "2"]));
    let here = id_of(env.json(&sci, &["add", "Here", "-p", "2"]));
    env.json(&sci, &["dep", &here, "--on", &far]);
    alias_registry(&env, "old", "fam");

    // Rewrite the stored ref to the retired spelling: what an inbound ref looks like after
    // the referred-to project is renamed.
    let path = sci.join(format!("tasks/{here}.md"));
    let text = std::fs::read_to_string(&path).unwrap().replace("fam-", "old-");
    std::fs::write(&path, text).unwrap();

    let v = env.json(&sci, &["check"]);
    assert!(
        v["warnings"].as_array().unwrap().iter().any(|w| {
            w["kind"] == "retired_prefix" && w["detail"].as_str().unwrap().contains(&far)
        }),
        "{v}"
    );
    assert!(v["errors"].as_array().unwrap().is_empty(), "{v}");
    // The renamed project itself is clean.
    assert_eq!(env.json(&fam, &["check"])["warnings"], serde_json::json!([]));
}
```

- [x] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli -- check_nudges_a_depends`
Expected: FAIL — no `retired_prefix` warning.

- [x] **Step 3: Raise the warning**

In `src/commands/check.rs`, in the dependency loop, before the reachability match:

```rust
                let canonical = ctx.registry.canonical_id(dependency);
                if &canonical != dependency {
                    warnings.push(finding(
                        Some(task),
                        file.clone(),
                        "retired_prefix",
                        format!(
                            "depends on {dependency} through retired prefix {:?}; it is now {canonical}",
                            dependency.prefix
                        ),
                    ));
                }
```

- [x] **Step 4: Run the tests to verify they pass**

Run: `cargo test --test cli -- check_nudges_a_depends` — Expected: PASS.
Run: `just gate` — Expected: clean.

- [x] **Step 5: Reinstall, close the task, and commit**

```bash
cargo install --path .   # AGENTS.md: the `tasks` the next task uses must be this code
tasks done tasks-52ffa3 "check raises retired_prefix in the referring project on a depends entry naming a retired prefix, quoting the current id. Prose is never examined."
git add src/commands/check.rs tests/cli.rs tasks/
git commit -m "feat(check): nudge a depends naming a retired prefix"
```

---

### Task 6: `unregister` and `init` honour aliases

**Files:**
- Modify: `src/registry.rs` (`unregister`), `src/commands/unregister.rs`, `src/commands/init.rs`
- Modify: `src/output.rs` (`InitOut` gains `aliases`)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Registry::is_taken` (Task 1).
- Produces: `Registry::unregister(&mut self, prefix: &str) -> Result<(PathBuf, Vec<String>)>`
  — the root and the aliases dropped with it.

Leaving an alias behind would dangle the Task 1 load invariant and fail every subsequent
command, so removal takes them with it (spec §5.4).

- [x] **Step 1: Write the failing test**

```rust
#[test]
fn unregister_takes_the_aliases_with_it_and_init_respects_them() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    env.init("fam");
    alias_registry(&env, "old", "fam");

    // A retired name is taken: init cannot claim it.
    let fresh = tempfile::tempdir().unwrap();
    assert_eq!(env.fail(fresh.path(), &["init", "--prefix", "old"]), "config");

    // Unregistering the alias itself is refused, pointing at the live name.
    assert_eq!(env.fail(&sci, &["unregister", "old"]), "config");

    // Unregistering the project drops its aliases and says so.
    let v = env.json(&sci, &["unregister", "fam"]);
    assert_eq!(v["aliases"], serde_json::json!(["old"]));
    // The registry is loadable afterwards: no dangling alias.
    assert_eq!(env.json(&sci, &["list"])["tasks"], serde_json::json!([]));
}
```

- [x] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli -- unregister_takes_the_aliases`
Expected: FAIL — `init --prefix old` succeeds.

- [x] **Step 3: Implement**

`src/registry.rs`:

```rust
    /// Removes a project and every alias that targeted it. Leaving an alias behind would
    /// dangle the load invariant and fail every later command; and once the project is
    /// gone its ids cannot resolve anyway, alias or not.
    pub fn unregister(&mut self, prefix: &str) -> Result<(PathBuf, Vec<String>)> {
        if let Some(target) = self.aliases.get(prefix) {
            return Err(Error::Config(format!(
                "{prefix:?} is a retired prefix of {target:?}; unregister {target:?} to \
                 remove the project"
            )));
        }
        let root = self
            .projects
            .remove(prefix)
            .ok_or_else(|| Error::Config(format!("no project registered as {prefix:?}")))?;
        let dropped: Vec<String> = self
            .aliases
            .iter()
            .filter(|(_, target)| target.as_str() == prefix)
            .map(|(alias, _)| alias.clone())
            .collect();
        for alias in &dropped {
            self.aliases.remove(alias);
        }
        Ok((root, dropped))
    }
```

In `register` and `repoint`, refuse a prefix that `is_taken` as an alias. In
`src/commands/unregister.rs`, destructure the pair and put `dropped` in the output. Add
`#[serde(default)] pub aliases: Vec<String>` to `InitOut` in `src/output.rs` and fill it
from both callers (empty from `init`).

- [x] **Step 4: Run the tests to verify they pass**

Run: `cargo test --test cli -- unregister_takes_the_aliases` — Expected: PASS.
Run: `just gate` — Expected: clean. Fix `registry.rs`'s existing
`unregister_removes_once_and_then_reports_the_prefix_is_absent` for the new return type.

- [x] **Step 5: Reinstall, close the task, and commit**

```bash
cargo install --path .   # AGENTS.md: the `tasks` the next task uses must be this code
tasks done tasks-6725e3 "unregister removes a project with every alias targeting it and names them in its output; unregistering an alias is refused with a pointer to the live name; init refuses a prefix taken as either a live prefix or an alias."
git add src/registry.rs src/commands/unregister.rs src/commands/init.rs src/output.rs tests/cli.rs tasks/
git commit -m "feat(registry): drop a project's aliases with the project"
```

---

### Task 7: Registry lock, `add`'s lock, and post-lock revalidation

**Files:**
- Modify: `src/registry.rs` (a lock path), `src/commands/mod.rs` (`add`'s ctx, revalidation)
- Modify: `src/commands/init.rs`, `src/commands/unregister.rs`
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `MutationLock::acquire_at` (`src/claims.rs:227`).
- Produces: `Registry::lock() -> Result<MutationLock>` — held across every registry
  read-modify-write.
- Produces: `commands::Routing { Local, Registered(String) }` and
  `commands::lock_and_revalidate(&mut Ctx, &Routing) -> Result<()>` — acquire, then
  re-resolve under the lock without changing which checkout is being written to. Used by
  `open_id_write_ctx` and by both `add` arms.

`add` takes no mutation lock at all today (`open_ctx` sets `lock: None`), relying on
`create_task`'s exclusive create, which guards a colliding id and nothing else. And
`Registry::load` → mutate → `save` is an unserialized read-modify-write: atomic replacement
prevents a torn file, not a lost update.

- [x] **Step 1: Write the failing test**

The window is microseconds wide, so spawning two processes and hoping they collide is not a
test. Hold the registry lock from the test, start both children, confirm they are blocked,
then release — forcing the interleaving instead of wishing for it. (The companion case,
an `add` waiting through a rename, needs the command and so lives in Task 11.)

```rust
#[test]
fn concurrent_registry_writes_do_not_lose_each_other() {
    use std::fs::File;
    let env = TestEnv::new();
    let dirs: Vec<_> = (0..2).map(|_| tempfile::tempdir().unwrap()).collect();

    let lock_path = env.home.path().join(".config/tasks/projects.lock");
    std::fs::create_dir_all(lock_path.parent().unwrap()).unwrap();
    let held: File = std::fs::OpenOptions::new()
        .create(true).truncate(false).write(true).open(&lock_path).unwrap();
    held.lock().unwrap();

    let mut children: Vec<_> = ["aaa", "bbb"]
        .iter()
        .zip(&dirs)
        .map(|(prefix, dir)| {
            env.raw(dir.path()).args(["init", "--prefix", prefix]).spawn().unwrap()
        })
        .collect();
    // Asserting that progress does NOT happen; there is no event to wait for.
    std::thread::sleep(std::time::Duration::from_millis(200));
    for child in &mut children {
        assert!(child.try_wait().unwrap().is_none(), "a child ran without the lock");
    }
    drop(held);

    for child in &mut children {
        assert!(child.wait().unwrap().success());
    }
    let text = std::fs::read_to_string(env.home.path().join(".config/tasks/projects.toml")).unwrap();
    assert!(text.contains("aaa = "), "{text}");
    assert!(text.contains("bbb = "), "{text}");
}

```

- [x] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli -- concurrent_registry_writes`
Expected: FAIL deterministically — with no `projects.lock` taken by the binary the children
do not block, so `try_wait` finds one already finished.

- [x] **Step 3: Serialize the registry and lock `add`**

In `src/registry.rs`:

```rust
    /// Held across a registry read-modify-write. Atomic replacement stops a torn file; it
    /// does not stop a lost update, which is what two concurrent `init`s produce.
    pub fn lock() -> Result<crate::claims::MutationLock> {
        let path = Self::path()?.with_file_name("projects.lock");
        crate::claims::MutationLock::acquire_at(&path)
    }
```

Take it in `init`, `unregister`, and (Task 11) `rename`, around load-mutate-save.

**First remove `add`'s inner lock.** `add::run` acquires the same prefix lock itself when
`--source` is given (`src/commands/add.rs:63-66`), to make the duplicate check and the write
one critical section. `MutationLock` is an flock, and a second open of the same inode blocks
even within one process, so handing the context a lock without removing that one deadlocks
every sourced `add`. Delete the inner acquisition; the duplicate check keeps its critical
section because the context's lock is already held around it.

```rust
// src/commands/add.rs — was `let _lock = match &fields.source { Some(source) => { … acquire … } }`
    // The context holds this project's mutation lock, so the duplicate check and the
    // create below are already one critical section.
    if let Some(source) = &fields.source {
        validate_line("source", source)?;
        let existing = duplicates_of(&ctx.project, source, &title)?;
        // … unchanged from here
    }
```

**One helper for every locking path**, so revalidation is not written three times and
forgotten in two of them:

**Revalidation must not change *which checkout* it writes to.** Re-resolving through
`open_registered` unconditionally would replace a worktree with the registered root the
moment their paths differ — which is always, for a worktree — silently routing the write out
of the checkout the caller is standing in, with no rename involved at all. That is the
failure `465b778` fixed, so the helper is told the caller's routing intent and preserves it:

```rust
/// Which project a command meant to write to, so revalidation refreshes it without
/// changing it. `-C` and worktrees win for local work; the registry decides for work
/// explicitly addressed to another project.
pub enum Routing {
    /// This checkout, whatever the registry says about its prefix.
    Local,
    /// The project the registry names: an id whose prefix is not the local project's, or
    /// an explicit `--project`.
    Registered(String),
}

/// Acquire the project's mutation lock and re-resolve under it.
///
/// A waiter resolved *before* the lock, and a rename may have completed while it waited —
/// including the cleanup that lifts the freeze — so acting on the pre-lock project would
/// write under a prefix that no longer exists. `Local` reopens the *same root*, because
/// only the config may have moved, never the checkout; `Registered` re-follows the
/// registry, because that is what the caller asked for.
fn lock_and_revalidate(ctx: &mut Ctx, routing: &Routing) -> Result<()> {
    for _ in 0..4 {
        ctx.lock = Some(MutationLock::acquire(&ctx.project.prefix)?);
        let registry = Registry::load()?;
        let project = match routing {
            Routing::Local => Project::open(&ctx.project.root)?,
            Routing::Registered(prefix) => {
                let live = registry.canonical_prefix(prefix).to_string();
                crate::scope::open_registered(&registry, &live, Origin::Prefix)?
            }
        };
        if project.prefix == ctx.project.prefix && project.root == ctx.project.root {
            reject_stale_local(&registry, &project)?; // Task 4
            ctx.project = project; // reassigned: spec_dirs and plan_dirs may have moved too
            ctx.registry = registry;
            return Ok(());
        }
        ctx.lock = None; // released before the next acquire; never held crossed
        ctx.project = project;
    }
    Err(Error::Io(
        "the project's identity kept changing while acquiring its lock; retry".into(),
    ))
}
```

The success branch assigns `ctx.project`, not just `ctx.registry`: the config may have been
rewritten under the waiter with the same prefix and root but different `spec_dirs` or
`plan_dirs`, and discarding the reopened `Project` would keep the stale ones.

Call sites and their routing:

| Caller | Routing |
|---|---|
| `open_id_write_ctx`, id prefix == local project's | `Local` |
| `open_id_write_ctx`, id prefix != local project's | `Registered(id.prefix)` |
| `add` local arm | `Local` |
| `add --project <prefix>` arm | `Registered(prefix)` |

Each builds its `Ctx` with `lock: None`, then calls `lock_and_revalidate(&mut ctx, &routing)?`.

The freeze (`reject_pending_rename`) is **not** called here yet — it arrives with the command
it protects, in Task 11, which adds the call to this helper. Task 7 must compile on its own.

- [x] **Step 4: Run the tests to verify they pass**

Run: `cargo test --test cli -- concurrent_registry_writes` — Expected: PASS, repeatedly.
Run: `just gate` — Expected: clean.

- [x] **Step 5: Reinstall, close the task, and commit**

```bash
cargo install --path .   # AGENTS.md: the `tasks` the next task uses must be this code
tasks done tasks-6a4742 "A registry lock serializes the read-modify-write that init, unregister and rename share, closing a pre-existing lost-update race. add takes the project mutation lock on both arms with its inner acquisition removed, and lock_and_revalidate re-resolves under the lock so a waiter cannot act on a pre-lock identity."
git add src/registry.rs src/commands/mod.rs src/commands/add.rs src/commands/init.rs \
        src/commands/unregister.rs tests/cli.rs tasks/
git commit -m "fix(registry): serialize read-modify-write and lock add"
```

---

### Task 8: Byte-preserving frontmatter rewrite

**Files:**
- Create: `src/rename/mod.rs`, `src/rename/rewrite.rs`
- Modify: `src/format.rs` (extract the timestamp helper), `src/main.rs` (`mod rename;`)

**Interfaces:**
- Produces: `rename::rewrite::rewrite_prefix(text: &str, old: &str, new: &str) -> Result<String>`
  — rewrites `id`, and any `depends`/`parent` entry whose prefix is `old`, preserving every
  byte after the closing frontmatter delimiter.
- Produces: `format::quote_timestamps(fm: &str) -> String` — the pre-quote pass
  `parse_task` already performs, extracted for reuse.

`RESERVED` contains `:` (`src/frontmatter.rs:11`), so the subset parser rejects every
`created:`/`updated:` line. `parse_task` pre-quotes them (`src/format.rs:24`) and
`serialize_task` emits them `Value::Raw` (`src/format.rs:295`). Reuse both halves or the
rewrite either fails to parse or re-quotes timestamps and changes bytes.

- [x] **Step 1: Write the failing test**

In `src/rename/rewrite.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const HAND_WRITTEN: &str = "---\nid: dot-a00088\ntitle: Hand written\nstatus: todo\n\
priority: 2\ncreated: 2026-09-01T00:00:00Z\nupdated: 2026-09-05T09:00:00Z\n\
depends: [dot-b11111, ops-c22222]\ntags: []\n---\n\n\nBody   with  odd    spacing.\n\n\n\
## Notes\n\n- 2026-09-01T00:00:00Z (keith):   two spaces after the colon\n";

    #[test]
    fn rewrites_ids_and_local_refs_and_keeps_every_other_byte() {
        let out = rewrite_prefix(HAND_WRITTEN, "dot", "dots").unwrap();
        assert!(out.contains("id: dots-a00088"));
        assert!(out.contains("depends: [dots-b11111, ops-c22222]"), "{out}");
        assert!(out.contains("created: 2026-09-01T00:00:00Z"), "timestamps stay unquoted");
        let body = |t: &str| t.split_once("\n---\n").unwrap().1.to_string();
        assert_eq!(body(&out), body(HAND_WRITTEN), "post-frontmatter bytes are untouched");
    }

    #[test]
    fn a_file_with_no_local_refs_changes_only_its_id() {
        let text = HAND_WRITTEN.replace("depends: [dot-b11111, ops-c22222]", "depends: [ops-c22222]");
        let out = rewrite_prefix(&text, "dot", "dots").unwrap();
        assert!(out.contains("depends: [ops-c22222]"), "{out}");
    }
}
```

- [x] **Step 2: Run the tests to verify they fail**

Run: `cargo test --bin tasks rename::rewrite`
Expected: FAIL — module does not exist.

- [x] **Step 3: Implement**

In `src/format.rs`, extract the existing inline pass and call it from `parse_task`:

```rust
/// Timestamps are the one schema scalar containing `:`; quote them for the strict subset
/// parser. Shared with the rename rewrite, which must round-trip a file it did not write.
pub fn quote_timestamps(fm: &str) -> String {
    fm.lines()
        .map(|line| {
            if line.starts_with("created: ") || line.starts_with("updated: ") {
                let (k, v) = line.split_once(':').expect("prefix matched");
                format!("{k}: \"{}\"", v.trim_start())
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
```

In `src/rename/rewrite.rs`:

```rust
use crate::error::{Error, Result};
use crate::frontmatter::{self, Value};

/// Rewrite `id` and any `depends`/`parent` entry whose prefix is `old`, preserving every
/// byte after the closing frontmatter delimiter. `serialize_task` is deliberately not used:
/// it rebuilds the whole file from the model, normalizing a hand-written body.
pub fn rewrite_prefix(text: &str, old: &str, new: &str) -> Result<String> {
    let rest = text
        .strip_prefix("---\n")
        .ok_or_else(|| Error::Parse { file: String::new(), detail: "missing opening ---".into() })?;
    let (fm, after) = rest
        .split_once("\n---\n")
        .ok_or_else(|| Error::Parse { file: String::new(), detail: "missing closing ---".into() })?;
    let pairs = frontmatter::parse(&format!("{}\n", crate::format::quote_timestamps(fm)))?;

    let move_one = |value: &str| -> String {
        match value.split_once('-') {
            Some((prefix, hex)) if prefix == old => format!("{new}-{hex}"),
            _ => value.to_string(),
        }
    };
    let rewritten: Vec<(String, Value)> = pairs
        .into_iter()
        .map(|(key, value)| {
            let value = match (key.as_str(), value) {
                ("id", Value::Scalar(v)) | ("parent", Value::Scalar(v)) => {
                    Value::Scalar(move_one(&v))
                }
                ("depends", Value::List(items)) => {
                    Value::List(items.iter().map(|i| move_one(i)).collect())
                }
                // created/updated came back quoted; emit them Raw so they round-trip.
                ("created", Value::Scalar(v)) | ("updated", Value::Scalar(v)) => Value::Raw(v),
                (_, other) => other,
            };
            (key, value)
        })
        .collect();
    Ok(format!("---\n{}---\n{after}", frontmatter::serialize(&rewritten)))
}
```

Create `src/rename/mod.rs` with `pub mod rewrite;` and add `mod rename;` to `src/main.rs`.

- [x] **Step 4: Run the tests to verify they pass**

Run: `cargo test --bin tasks rename::rewrite` — Expected: PASS.
Run: `just gate` — Expected: clean.

- [x] **Step 5: Reinstall, close the task, and commit**

```bash
cargo install --path .   # AGENTS.md: the `tasks` the next task uses must be this code
tasks done tasks-4107c8 "rewrite_prefix moves a task's id and its local depends/parent through the frontmatter only, preserving every byte after the closing delimiter. The created/updated pre-quote and Raw-emit pair is factored out of parse_task and serialize_task and shared."
git add src/rename/ src/format.rs src/main.rs tasks/
git commit -m "feat(rename): rewrite a task's prefix without touching its body"
```

---

### Task 9: The inventory and the snapshot observer

**Files:**
- Create: `src/rename/inventory.rs`, `src/rename/snapshot.rs`
- Modify: `Cargo.toml` (add `sha2`), `src/rename/mod.rs`

**Interfaces:**
- Produces: `Inventory { source, target, root, config_from, config_to, entries }`,
  `InventoryEntry { hex, from, to }`, `Inventory::path(source) -> PathBuf`,
  `Inventory::load/save/remove`.
- Produces: `Snapshot { invocation, registry, config, inventory, entries, named, strays }`
  and `observe(&Registry, invocation: &Invocation, inventory: Option<Inventory>) -> Result<Snapshot>`,
  where `Invocation { source: String, target: String, root: PathBuf }`.

**One signature for both regimes.** Observation happens before *and* after an inventory
exists, so the inventory is an `Option` argument and the invocation carries the names and
root the caller asked for. `Snapshot.invocation` is what R1 compares an inventory against —
without it the classifier cannot tell whether the inventory on disk belongs to this rename —
and it is what the inventory-absent rows use to decide whether the config's parsed prefix is
the source or the target. With no inventory, `entries` is empty and only `named`, `config`,
and `registry` carry information.
- Produces: `digest(bytes: &[u8]) -> String` — lowercase hex SHA-256.

The inventory lives at `~/.local/state/tasks/rename/<source>.toml`, beside claims: it is
machine state, must not appear in `git status`, and must survive `git checkout .`.

- [x] **Step 1: Write the failing test**

In `src/rename/snapshot.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observes_an_untouched_project_against_its_inventory() {
        let dir = tempfile::tempdir().unwrap();
        let project = crate::repo::Project::init(dir.path(), "dot").unwrap();
        std::fs::write(
            project.tasks_dir().join("dot-a00088.md"),
            "---\nid: dot-a00088\ntitle: T\nstatus: todo\npriority: 2\n\
created: 2026-09-01T00:00:00Z\nupdated: 2026-09-01T00:00:00Z\ndepends: []\ntags: []\n---\n",
        )
        .unwrap();

        let inventory = Inventory::build(&project, "dots").unwrap();
        assert_eq!(inventory.entries.len(), 1);
        assert_eq!(inventory.entries[0].hex, "a00088");
        assert_ne!(inventory.entries[0].from, inventory.entries[0].to);

        let registry = crate::registry::Registry::default();
        let invocation = Invocation { source: "dot".into(), target: "dots".into(), root: project.root.clone() };
        let snap = observe(&registry, &invocation, Some(inventory.clone())).unwrap();
        assert_eq!(snap.named.source, 1);
        assert_eq!(snap.named.target, 0);
        assert!(snap.strays.is_empty());
        assert!(matches!(snap.entries[0].source, FileState::Present(ref d) if *d == inventory.entries[0].from));
        assert!(matches!(snap.entries[0].dest, FileState::Absent));
    }

    #[test]
    fn a_task_file_outside_the_inventory_is_a_stray() {
        let dir = tempfile::tempdir().unwrap();
        let project = crate::repo::Project::init(dir.path(), "dot").unwrap();
        let inventory = Inventory::build(&project, "dots").unwrap();
        std::fs::write(project.tasks_dir().join("dot-ffffff.md"), "---\n").unwrap();
        let invocation = Invocation { source: "dot".into(), target: "dots".into(), root: project.root.clone() };
        let snap = observe(&crate::registry::Registry::default(), &invocation, Some(inventory)).unwrap();
        assert_eq!(snap.strays.len(), 1, "{:?}", snap.strays);
    }

    #[test]
    fn observing_without_an_inventory_still_counts_files_by_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let project = crate::repo::Project::init(dir.path(), "dot").unwrap();
        std::fs::write(project.tasks_dir().join("dot-a00088.md"), MINIMAL_TASK).unwrap();
        let invocation = Invocation { source: "dot".into(), target: "dots".into(), root: project.root.clone() };
        let snap = observe(&crate::registry::Registry::default(), &invocation, None).unwrap();
        assert!(snap.inventory.is_none());
        assert!(snap.entries.is_empty(), "no baseline means no entries");
        assert_eq!(snap.named.source, 1);
        assert_eq!(snap.named.target, 0);
        assert!(snap.strays.is_empty(), "without a baseline nothing is a stray");
    }
}
```

- [x] **Step 2: Run the tests to verify they fail**

Run: `cargo test --bin tasks rename::snapshot`
Expected: FAIL — modules do not exist.

- [x] **Step 3: Implement**

Add to `Cargo.toml` under `[dependencies]`: `sha2 = "0.10"`.

`src/rename/inventory.rs` — the record, `digest`, `build` (which computes each `to` by
running `rewrite::rewrite_prefix` over the source, and `config_to` by rewriting the config's
`prefix` line), `path`, `load`, `save`, `remove`, and `pending()` which lists every
inventory in the directory.

`src/rename/snapshot.rs` — `FileState { Absent, Present(String) }`, `EntryState`,
`RegistryState { old_key, new_key, alias }`, `ConfigState { prefix, digest }`, `Named`,
`Snapshot`, and `observe`, which scans `tasks/*.md` once, classifies each file as a source,
a destination, or a stray, and reads the config.

- [x] **Step 4: Run the tests to verify they pass**

Run: `cargo test --bin tasks rename::` — Expected: PASS.
Run: `just gate` — Expected: clean.

- [x] **Step 5: Reinstall, close the task, and commit**

```bash
cargo install --path .   # AGENTS.md: the `tasks` the next task uses must be this code
tasks done tasks-bddcab "The inventory records source, target, root, config digests and per-task from/to digests under the state directory, and observe reads the world against it -- or without it, from the config's parsed prefix and a filename scan. sha2 added for a digest stable across Rust releases."
git add Cargo.toml Cargo.lock src/rename/ tasks/
git commit -m "feat(rename): record a baseline inventory and observe against it"
```

---

### Task 10: The pure recovery classifier

**Files:**
- Create: `src/rename/classify.rs`
- Modify: `src/rename/mod.rs`

**Interfaces:**
- Consumes: `Snapshot` (Task 9).
- Produces: `Recovery { Fresh, ResumeFiles, ResumeRegistry, ResumeCleanup, Complete, Refuse(String) }`
  and `classify(&Snapshot) -> Recovery`.

Pure: no filesystem, no locks. Refusals R1–R8 are decided **before** the table, because an
earlier row that is too broad swallows a bad state the trailing row can then never see.
R1–R6 apply only when the inventory is present; R7–R8 read the registry and always apply.

- [x] **Step 1: Write the failing tests**

In `src/rename/classify.rs` — the enumeration is the point, so build snapshots directly:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_phase_boundary_resumes_rather_than_refusing() {
        // Just after P2: inventory written, nothing moved.
        let snap = fixture(Stage::InventoryOnly);
        assert!(matches!(classify(&snap), Recovery::ResumeFiles), "{:?}", classify(&snap));
        // Mid file pass.
        assert!(matches!(classify(&fixture(Stage::PartialFiles)), Recovery::ResumeFiles));
        // Files done, config not yet written.
        assert!(matches!(classify(&fixture(Stage::FilesDone)), Recovery::ResumeFiles));
        // An empty project at the same point.
        assert!(matches!(classify(&fixture(Stage::EmptyProject)), Recovery::ResumeFiles));
        assert!(matches!(classify(&fixture(Stage::ConfigWritten)), Recovery::ResumeRegistry));
        assert!(matches!(classify(&fixture(Stage::RegistryWritten)), Recovery::ResumeCleanup));
        assert!(matches!(classify(&fixture(Stage::Done)), Recovery::Complete));
        assert!(matches!(classify(&fixture(Stage::Untouched)), Recovery::Fresh));
    }

    #[test]
    fn each_refusal_fires_on_its_own() {
        for stage in [
            Stage::SourceEdited,      // R2
            Stage::ConflictingDest,   // R3
            Stage::MissingTask,       // R4
            Stage::Stray,             // R5
            Stage::ConfigEdited,      // R6
            Stage::TwoRootsRegistered,// R7
            Stage::ForeignNewKey,     // R8
        ] {
            assert!(
                matches!(classify(&fixture(stage)), Recovery::Refuse(_)),
                "{stage:?} must refuse"
            );
        }
    }

    /// The spec's table and refusal list, transcribed independently of the implementation.
    /// Written from §5.3 rather than from `classify`, so agreeing with it is evidence.
    fn expected(snap: &Snapshot) -> Recovery {
        // R1-R6 need a baseline; R7-R8 read the registry alone.
        if snap.registry.two_roots() || snap.registry.foreign_new_key(&snap.invocation.root) {
            return Recovery::Refuse("registry".into());
        }
        if let Some(inv) = &snap.inventory {
            if !inv.matches(&snap.invocation)
                || snap.entries.iter().any(|e| e.source_edited() || e.dest_conflicts() || e.missing())
                || !snap.strays.is_empty()
                || !snap.config.matches_either(inv)
            {
                return Recovery::Refuse("baseline".into());
            }
            return if snap.config.is_from(inv) && snap.registry.is_old(&snap.invocation) {
                Recovery::ResumeFiles
            } else if snap.files_done() && snap.config.is_to(inv) && snap.registry.is_old(&snap.invocation) {
                Recovery::ResumeRegistry
            } else if snap.files_done() && snap.config.is_to(inv) && snap.registry.is_new(&snap.invocation) {
                Recovery::ResumeCleanup
            } else {
                Recovery::Refuse("unresumable".into())
            };
        }
        if snap.named.target == 0
            && snap.config.prefix_is(&snap.invocation.source)
            && snap.registry.is_old(&snap.invocation)
        {
            Recovery::Fresh
        } else if snap.named.source == 0
            && snap.config.prefix_is(&snap.invocation.target)
            && snap.registry.is_new(&snap.invocation)
        {
            Recovery::Complete
        } else {
            Recovery::Refuse("unsettled".into())
        }
    }

    #[test]
    fn every_enumerated_snapshot_gets_the_verdict_the_spec_names() {
        let mut counts = std::collections::BTreeMap::new();
        for snap in enumerate_snapshots() {
            let got = classify(&snap);
            let want = expected(&snap);
            assert_eq!(
                std::mem::discriminant(&got),
                std::mem::discriminant(&want),
                "snapshot {snap:?}\n  classify -> {got:?}\n  spec     -> {want:?}"
            );
            if let Recovery::Refuse(reason) = &got {
                assert!(!reason.is_empty(), "a refusal must say what it saw: {snap:?}");
            }
            *counts.entry(format!("{got:?}").split('(').next().unwrap().to_string())
                .or_insert(0usize) += 1;
        }
        // Every verdict must be reachable, or the enumeration is not exercising the table.
        for verdict in ["Fresh", "ResumeFiles", "ResumeRegistry", "ResumeCleanup", "Complete", "Refuse"] {
            assert!(counts.get(verdict).copied().unwrap_or(0) > 0, "{verdict} unreached: {counts:?}");
        }
    }
}
```

Write `fixture(Stage)`, `expected(&Snapshot)`, and `enumerate_snapshots()` in the same test
module. `expected` is the load-bearing one: transcribe it from the spec's §5.3 table and
refusal list, **not** from the implementation, or the test proves only that `classify`
agrees with itself. The reachability assertion is what stops a classifier that returns one
verdict for everything from passing.
`enumerate_snapshots` iterates: `registry` over its three fields, `config` prefix × 3 and
digest × 3, `inventory` × 2, `named` over zero/non-zero each, `strays` × 2, and entry lists
of length 0, 1, and 2 over `source` × 3 and `dest` × 3. Include inventory-absent snapshots
carrying entries and inventory-present ones carrying none, so a predicate reaching for a
baseline that is not there is caught rather than assumed away.

- [x] **Step 2: Run the tests to verify they fail**

Run: `cargo test --bin tasks rename::classify`
Expected: FAIL — module does not exist.

- [x] **Step 3: Implement refusals first, then the table**

```rust
pub fn classify(snap: &Snapshot) -> Recovery {
    if let Some(reason) = refusal(snap) {
        return Recovery::Refuse(reason);
    }
    match snap.inventory.as_ref() {
        None => {
            if snap.named.target == 0 && snap.config.prefix_is_source() && snap.registry.is_old() {
                Recovery::Fresh
            } else if snap.named.source == 0 && snap.config.prefix_is_target() && snap.registry.is_new() {
                Recovery::Complete
            } else {
                Recovery::Refuse("no rename is pending and the project is not in a settled state".into())
            }
        }
        Some(_) => {
            if snap.config.is_from() && snap.registry.is_old() {
                Recovery::ResumeFiles
            } else if snap.files_done() && snap.config.is_to() && snap.registry.is_old() {
                Recovery::ResumeRegistry
            } else if snap.files_done() && snap.config.is_to() && snap.registry.is_new() {
                Recovery::ResumeCleanup
            } else {
                Recovery::Refuse("the world matches no resumable stage of this rename".into())
            }
        }
    }
}
```

`refusal` implements R1–R8 in order, each returning its own sentence naming what it saw.

- [x] **Step 4: Run the tests to verify they pass**

Run: `cargo test --bin tasks rename::classify` — Expected: PASS.
Run: `just gate` — Expected: clean.

- [x] **Step 5: Reinstall, close the task, and commit**

```bash
cargo install --path .   # AGENTS.md: the `tasks` the next task uses must be this code
tasks done tasks-425ef7 "classify is pure: refusals R1-R8 decided before the table, R1-R6 scoped to inventory-present, and every row naming file, config and registry state exactly. Verified by bounded enumeration against an oracle transcribed from the spec, with every verdict asserted reachable."
git add src/rename/classify.rs src/rename/mod.rs tasks/
git commit -m "feat(rename): classify recovery from a snapshot and its baseline"
```

---

### Task 11: The `rename` command and its phases

**Files:**
- Create: `src/commands/rename.rs`
- Modify: `src/rename/mod.rs` (phases), `src/cli.rs`, `src/commands/mod.rs`, `src/output.rs`,
  `src/commands/init.rs`
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: everything from Tasks 1, 6, 7, 8, 9, 10.
- Produces: `tasks rename <old> <new> [--explain]`;
  `RenameOut { prefix, previous, root, tasks, aliases, recovery, warnings }`.
- Produces: `commands::reject_pending_rename(&Project) -> Result<()>` — the freeze, called
  by `lock_and_revalidate` (Task 7) and under the registry lock by `init` and `unregister`.

**The freeze ships with the command, not after it.** A `rename` that can be interrupted but
whose interruption nothing protects is worse than no command: locks die with the process, so
between the crash and its recovery the world looks like an ordinary project under its old
name and any `add` walks into it. Task 12 keeps the broader interruption matrix and the
docs, but the freeze and the pending-name reservation are part of this commit.

Phases: P1 preflight, P2 inventory, P3 file pass, P4 config, P5 registry, P6 cleanup.
Classification precedes every fresh-operation check: after P5, `<old>` canonicalizes to
`<new>`, so `new == old` and the taken-name refusal would otherwise reject the idempotent
re-run this design promises. Claim and worktree checks gate **every** mutating path, not
just `Fresh`. `--explain` writes nothing, takes no locks, runs no authorization checks.

- [x] **Step 1: Write the failing test**

```rust
#[test]
fn rename_rewrites_the_project_and_keeps_inbound_refs_resolving() {
    let mut env = TestEnv::new();
    let dots = env.init("dot");
    let ops = env.init("ops");
    git(&dots, &["init", "-q", "-b", "main"]);
    let mine = id_of(env.json(&dots, &["add", "Mine", "-p", "2"]));
    let theirs = id_of(env.json(&ops, &["add", "Theirs", "-p", "2"]));
    env.json(&ops, &["dep", &theirs, "--on", &mine]);
    git(&dots, &["add", "-A"]);
    git(&dots, &["commit", "-qm", "seed"]);

    let v = env.json(&dots, &["rename", "dot", "dots"]);
    assert_eq!(v["prefix"], "dots");
    assert_eq!(v["previous"], "dot");
    assert_eq!(v["tasks"], 1);
    assert_eq!(v["recovery"], "fresh");
    assert_eq!(v["aliases"], serde_json::json!(["dot"]));

    let moved = format!("dots-{}", mine.split_once('-').unwrap().1);
    assert!(dots.join(format!("tasks/{moved}.md")).is_file());
    assert!(!dots.join(format!("tasks/{mine}.md")).exists());
    assert_eq!(env.json(&dots, &["show", &moved])["task"]["id"], moved);

    // The other project was not written to, and its stored ref still resolves.
    assert_eq!(env.json(&ops, &["show", &theirs])["task"]["depends"][0], mine);
    assert_eq!(env.json(&ops, &["show", &mine])["task"]["id"], moved);

    // A completed re-run reports Complete rather than tripping a fresh-operation refusal.
    let v = env.json(&dots, &["rename", "dot", "dots"]);
    assert_eq!(v["recovery"], "complete");

    // --explain writes nothing and reports the same verdict.
    let before = std::fs::read_to_string(env.home.path().join(".config/tasks/projects.toml")).unwrap();
    assert_eq!(
        env.json(&dots, &["rename", "dot", "dots", "--explain"])["recovery"],
        "complete"
    );
    assert_eq!(
        std::fs::read_to_string(env.home.path().join(".config/tasks/projects.toml")).unwrap(),
        before
    );
}

#[test]
fn an_add_waiting_through_a_rename_never_writes_the_old_prefix() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    git(&dir, &["init", "-q", "-b", "main"]);
    let seed = id_of(env.json(&dir, &["add", "Seed", "-p", "2"]));
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);
    let moved_seed = format!("dots-{}", seed.split_once('-').unwrap().1);

    // This tests revalidation, not the rename executor (covered separately above).
    // Hold the lock and establish a completed-rename fixture while the add waits.
    let lock_path = env.claim_store("dot").with_file_name("dot.lock");
    std::fs::create_dir_all(lock_path.parent().unwrap()).unwrap();
    let held = std::fs::OpenOptions::new()
        .create(true).truncate(false).write(true).open(&lock_path).unwrap();
    held.lock().unwrap();

    let mut adder = env.raw(&dir).args(["add", "Late", "-p", "2"]).spawn().unwrap();
    // Seeing its lock descriptor proves the child constructed its old-prefix context
    // and reached lock acquisition. A sleep alone could leave it not yet started.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        assert!(adder.try_wait().unwrap().is_none(), "the add exited before acquiring its lock");
        let opened = std::fs::read_dir(format!("/proc/{}/fd", adder.id())).unwrap()
            .any(|entry| {
                // Other descriptors can close while /proc is being inspected.
                std::fs::read_link(entry.unwrap().path()).is_ok_and(|path| path == lock_path)
            });
        if opened {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "the add never opened its lock");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // Only this one seed task exists. Install its renamed file, config and registry
    // directly while the lock is held; invoking rename here would wait on our lock too.
    let source = dir.join(format!("tasks/{seed}.md"));
    let text = std::fs::read_to_string(&source).unwrap();
    std::fs::write(
        dir.join(format!("tasks/{moved_seed}.md")),
        text.replacen(&format!("id: {seed}\n"), &format!("id: {moved_seed}\n"), 1),
    ).unwrap();
    std::fs::remove_file(source).unwrap();
    let config_path = dir.join("tasks/.config.toml");
    let mut config: toml::Value = std::fs::read_to_string(&config_path).unwrap().parse().unwrap();
    config["prefix"] = toml::Value::String("dots".into());
    std::fs::write(config_path, toml::to_string(&config).unwrap()).unwrap();
    let registry_path = env.home.path().join(".config/tasks/projects.toml");
    let mut registry: toml::Value = std::fs::read_to_string(&registry_path).unwrap().parse().unwrap();
    let projects = registry["projects"].as_table_mut().unwrap();
    let root = projects.remove("dot").unwrap();
    projects.insert("dots".into(), root);
    std::fs::write(&registry_path, toml::to_string(&registry).unwrap()).unwrap();
    alias_registry(&env, "dot", "dots");
    assert!(dir.join(format!("tasks/{moved_seed}.md")).is_file());

    drop(held);
    let out = adder.wait_with_output().unwrap();
    assert!(out.status.success(), "{out:?}");
    let added: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let late = added["id"].as_str().unwrap();
    assert!(late.starts_with("dots-"), "the queued add must use the refreshed prefix: {added}");
    assert_eq!(env.json(&dir, &["show", late])["task"]["title"], "Late");

    // No later rename can mask a stale write: only the queued add ran after the fixture.
    let stale: Vec<_> = std::fs::read_dir(dir.join("tasks")).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("dot-"))
        .collect();
    assert!(stale.is_empty(), "an old-prefix file survived the rename: {stale:?}");
    assert_eq!(env.json(&dir, &["show", &moved_seed])["task"]["id"], moved_seed);
    assert_eq!(env.json(&dir, &["list"])["tasks"].as_array().unwrap().len(), 2);
}

#[test]
fn an_add_waiting_through_a_pending_rename_is_refused() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    git(&dir, &["init", "-q", "-b", "main"]);
    env.json(&dir, &["add", "Seed", "-p", "2"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);

    // An interrupted rename leaves the freeze in place; a waiter that wakes into it is
    // refused rather than writing under either name.
    env.raw(&dir).env("TASKS_RENAME_STOP_AFTER", "config")
        .args(["rename", "dot", "dots"]).status().unwrap();
    let out = env.raw(&dir).args(["add", "Late", "-p", "2"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "validation");
}

#[test]
fn a_pending_rename_freezes_the_project_including_the_p4_window() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    git(&dir, &["init", "-q", "-b", "main"]);
    env.json(&dir, &["add", "T", "-p", "2"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);

    // Stop after the config write: files and config renamed, registry untouched. This is
    // the window where a lookup by resolved prefix would miss rename/dot.toml entirely.
    env.raw(&dir)
        .env("TASKS_RENAME_STOP_AFTER", "config")
        .args(["rename", "dot", "dots"])
        .status()
        .unwrap();

    assert_eq!(env.fail(&dir, &["add", "New", "-p", "2"]), "validation");
    assert_eq!(env.fail(&dir, &["unregister", "dot"]), "validation");
    assert!(env.json(&dir, &["list"])["tasks"].is_array(), "reads still work");

    // The target is reserved even though it is in no registry yet.
    let fresh = tempfile::tempdir().unwrap();
    assert_eq!(env.fail(fresh.path(), &["init", "--prefix", "dots"]), "config");

    // And the rename completes on a re-run.
    assert_eq!(env.json(&dir, &["rename", "dot", "dots"])["recovery"], "resume_registry");
    assert!(env.json(&dir, &["add", "New", "-p", "2"])["id"].as_str().unwrap().starts_with("dots-"));
}

#[test]
fn rename_refuses_a_dirty_tree_a_live_claim_and_a_second_worktree() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    git(&dir, &["init", "-q", "-b", "main"]);
    let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
    assert_eq!(env.fail(&dir, &["rename", "dot", "dots"]), "validation"); // uncommitted
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);

    as_agent(&env, &dir, "agent-a").args(["start", &id]).assert().success();
    assert_eq!(env.fail(&dir, &["rename", "dot", "dots"]), "claimed");
}
```

- [x] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli -- rename_rewrites_the_project`
Expected: FAIL — unrecognized subcommand `rename`.

- [x] **Step 3: Implement the phases and wire the command**

`src/rename/mod.rs` reads `TASKS_RENAME_STOP_AFTER` and returns early after the named
boundary — `inventory`, `file:<n>` (after the nth destination write, before its source is
removed), `files`, `config`, `registry`, `claims` — which is the only way to test
interruption without killing a process mid-write. It is production code that exists for
tests; the alternative is spawning and killing real processes, which is slower and racier.

`src/rename/mod.rs` gains `run(project, registry, source, target, explain) -> Result<RenameOut>`:
observe, classify, and then execute from the phase the verdict names. On `Fresh`, run the
§5.2 preflight refusals first. Authorization — live claims and the worktree count — runs on
every mutating verdict, and not at all under `--explain`.

`src/cli.rs` gains the subcommand with `old`, `new`, and `--explain`. `src/output.rs` gains
`RenameOut` and its `Output::Rename` variant, its `pretty` arm (the new prefix, as `init`
prints its prefix), and its `warnings_of` arm.

**The freeze**, in `src/commands/mod.rs`:

```rust
/// A rename that has begun and not finished freezes the project it names. Locks die with
/// the process that held them, so between an interruption and its recovery the world looks
/// like an ordinary project under its old name and anything could mutate it.
///
/// Discovery reads the inventories, never the registry: between P4 and P5 the config says
/// the new name while the registry still holds the old one and no alias exists, so a lookup
/// by resolved prefix finds nothing.
pub fn reject_pending_rename(project: &Project) -> Result<()> {
    for inventory in crate::rename::inventory::pending()? {
        if inventory.root == project.root
            || inventory.source == project.prefix
            || inventory.target == project.prefix
        {
            return Err(Error::Validation(format!(
                "a rename of {:?} to {:?} is unfinished; finish it with \
                 `tasks rename {} {}` or roll it back (see the design doc)",
                inventory.source, inventory.target, inventory.source, inventory.target
            )));
        }
    }
    Ok(())
}
```

**Add the call to `lock_and_revalidate`** (Task 7), immediately after the registry load —
Task 7 deliberately left it out so that task compiles on its own, since this function does
not exist until now:

```rust
        let registry = Registry::load()?;
        reject_pending_rename(&ctx.project)?;   // added here, in Task 11
```

`init` and `unregister` call it under the registry lock, and `init` additionally refuses a
prefix matching any pending `target` — until P5 the target is in no registry at all, so
nothing else reserves it.

`rename` itself must **not** be frozen by its own inventory; it passes its own invocation
through and skips the check.

- [x] **Step 4: Run the tests to verify they pass**

Run: `cargo test --test cli -- rename_` — Expected: PASS.
Run: `just gate` — Expected: clean.

- [x] **Step 5: Reinstall, close the task, and commit**

```bash
cargo install --path .   # AGENTS.md: the `tasks` the next task uses must be this code
tasks done tasks-7000aa "tasks rename runs the six phases, classifies before every fresh-operation check so a completed re-run reports Complete rather than refusing, gates authorization on every mutating path, and ships with the freeze: a pending inventory refuses mutations and reserves the target name, discovered by recorded names rather than resolved prefix. --explain classifies read-only."
git add src/rename/ src/commands/rename.rs src/commands/mod.rs src/commands/init.rs \
        src/commands/unregister.rs src/cli.rs src/output.rs tests/cli.rs tasks/
git commit -m "feat(rename): rename a project's prefix in six recoverable phases"
```

---

### Task 12: The freeze, interruption coverage, and the docs

**Files:**
- Modify: `skills/tasks/SKILL.md`, `README.md`, `docs/specs/2026-09-08-prefix-rename-design.md`
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: everything. Adds no new interface.

The freeze itself shipped with the command in Task 11. This task is the verification matrix
the spec's §8 promises and the user-facing documentation.

- [ ] **Step 1: Write the failing tests**

Stopping only after whole phases misses the boundaries where a crash actually hurts: a
destination written with its source still present, partial progress across several files,
and each of the two cleanup operations. `TASKS_RENAME_STOP_AFTER` accepts `file:<n>` for the
first, so the matrix covers every mutation boundary rather than every phase.

```rust
/// Build a three-task project inside a git repo, committed, and return (root, ids).
fn rename_fixture(env: &mut TestEnv, prefix: &str, count: usize) -> (std::path::PathBuf, Vec<String>) {
    let dir = env.init(prefix);
    git(&dir, &["init", "-q", "-b", "main"]);
    let ids: Vec<String> = (0..count)
        .map(|n| id_of(env.json(&dir, &["add", &format!("T{n}"), "-p", "2"])))
        .collect();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);
    (dir, ids)
}

#[test]
fn every_mutation_boundary_resumes_to_the_same_final_state() {
    // Each stop names the verdict the world must then classify as, and what must be true on
    // disk at that instant. Asserting only "not fresh" would pass a run that ignored the
    // hook entirely, finished everything, and reported `complete`.
    let boundaries: &[(&str, &str)] = &[
        ("inventory", "resume_files"),
        ("file:0", "resume_files"),
        ("file:1", "resume_files"),
        ("files", "resume_files"),
        ("config", "resume_registry"),
        ("registry", "resume_cleanup"),
        ("claims", "resume_cleanup"),
    ];
    for (stop, want) in boundaries {
        let mut env = TestEnv::new();
        let (dir, ids) = rename_fixture(&mut env, "dot", 3);
        let inventory = env.home.path().join(".local/state/tasks/rename/dot.toml");
        let old_claims = env.home.path().join(".local/state/tasks/claims/dot.toml");
        // Seed a claim store so its removal is observed rather than pre-existing absence.
        std::fs::create_dir_all(old_claims.parent().unwrap()).unwrap();
        std::fs::write(&old_claims, "[claims]\n").unwrap();

        let status = env.raw(&dir)
            .env("TASKS_RENAME_STOP_AFTER", stop)
            .args(["rename", "dot", "dots"])
            .status()
            .unwrap();
        assert!(status.success(), "the interrupted run must exit cleanly: {stop}");

        // The hook was honoured: the world is mid-rename, not finished.
        assert!(inventory.is_file(), "{stop}: the inventory must still be pending");
        let named = |prefix: &str| {
            std::fs::read_dir(dir.join("tasks")).unwrap().filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().starts_with(prefix))
                .count()
        };
        let config = std::fs::read_to_string(dir.join("tasks/.config.toml")).unwrap();
        match *stop {
            "inventory" => {
                assert_eq!(named("dots-"), 0, "{stop}: no file has moved yet");
                assert_eq!(named("dot-"), 3);
            }
            "file:0" => {
                // The intra-phase boundary: a destination written, its source still there.
                assert_eq!(named("dots-"), 1, "{stop}");
                assert_eq!(named("dot-"), 3, "{stop}: the source is not removed yet");
            }
            "file:1" => {
                assert_eq!(named("dots-"), 2, "{stop}: two destinations written");
                assert_eq!(named("dot-"), 2, "{stop}: only the first source removed");
                assert!(config.contains("\"dot\""), "{stop}: config not written yet");
            }
            "files" => {
                assert_eq!(named("dots-"), 3, "{stop}");
                assert_eq!(named("dot-"), 0, "{stop}");
                assert!(config.contains("\"dot\""), "{stop}: config not written yet");
            }
            "config" => assert!(config.contains("\"dots\""), "{stop}"),
            "registry" | "claims" => {
                let reg = std::fs::read_to_string(
                    env.home.path().join(".config/tasks/projects.toml")).unwrap();
                assert!(reg.contains("dots = "), "{stop}");
                assert!(reg.contains("[aliases]"), "{stop}");
                assert_eq!(old_claims.exists(), *stop == "registry", "{stop}: claim store");
            }
            _ => panic!("missing intermediate-state assertions for {stop}"),
        }

        let verdict = env.json(&dir, &["rename", "dot", "dots", "--explain"])["recovery"]
            .as_str().unwrap().to_string();
        assert_eq!(&verdict, want, "stopped after {stop}");
        // --explain changed nothing.
        assert!(inventory.is_file(), "{stop}: --explain must not clean up");

        env.json(&dir, &["rename", "dot", "dots"]);

        for id in &ids {
            let moved = format!("dots-{}", id.split_once('-').unwrap().1);
            assert!(dir.join(format!("tasks/{moved}.md")).is_file(), "{stop}: {moved}");
            assert!(!dir.join(format!("tasks/{id}.md")).exists(), "{stop}: {id} survived");
            assert_eq!(env.json(&dir, &["show", &moved])["task"]["id"], moved, "{stop}");
        }
        assert_eq!(env.json(&dir, &["show", &ids[0]])["task"]["id"],
                   format!("dots-{}", ids[0].split_once('-').unwrap().1),
                   "{stop}: the retired id still resolves");
        assert!(!inventory.exists(), "{stop}: the inventory is removed by cleanup");
        assert!(!old_claims.exists(), "{stop}: the seeded claim store is deleted, not merely absent");
        assert!(env.home.path().join(".local/state/tasks/claims/dot.lock").exists(), "{stop}: the lock is never unlinked");
    }
}

#[test]
fn an_empty_project_and_a_project_outside_git_both_rename() {
    // Empty: files_done is vacuously true at every boundary, so config and registry alone
    // must decide the verdict.
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);
    env.raw(&dir).env("TASKS_RENAME_STOP_AFTER", "inventory")
        .args(["rename", "dot", "dots"]).status().unwrap();
    assert_eq!(env.json(&dir, &["rename", "dot", "dots"])["recovery"], "resume_files");

    // Outside git: no dirty check, no undo, and recovery must still work from the baseline.
    let mut env = TestEnv::new();
    let (bare, ids) = (env.init("bare"), ());
    let _ = ids;
    let id = id_of(env.json(&bare, &["add", "T", "-p", "2"]));
    env.raw(&bare).env("TASKS_RENAME_STOP_AFTER", "file:0")
        .args(["rename", "bare", "bares"]).status().unwrap();
    let v = env.json(&bare, &["rename", "bare", "bares"]);
    assert_eq!(v["recovery"], "resume_files");
    let moved = format!("bares-{}", id.split_once('-').unwrap().1);
    assert!(bare.join(format!("tasks/{moved}.md")).is_file());
}

#[test]
fn a_live_claim_blocks_the_resume_and_the_freeze_blocks_start() {
    let mut env = TestEnv::new();
    let (dir, ids) = rename_fixture(&mut env, "dot", 1);
    env.raw(&dir).env("TASKS_RENAME_STOP_AFTER", "registry")
        .args(["rename", "dot", "dots"]).status().unwrap();
    let moved = format!("dots-{}", ids[0].split_once('-').unwrap().1);

    // `start` cannot be used to set this up: the freeze refuses every mutation while the
    // inventory is pending, which is itself worth asserting.
    assert_eq!(env.fail(&dir, &["start", &moved]), "validation");

    // Reuse the existing helper's PID/start-time/boot-ID evidence. A PID alone would
    // fall back to the TTL; this claim stays live for the test process's lifetime.
    write_claim(&env, "dots", &moved, "agent-a", true);
    let store = env.claim_store("dots");
    assert_eq!(env.json(&dir, &["show", &moved])["claim"]["live"], true);

    assert_eq!(env.fail(&dir, &["rename", "dot", "dots"]), "claimed");
    assert!(store.exists(), "the live claim store survives a refused resume");
}

#[test]
fn each_refusal_fires_end_to_end() {
    let mut env = TestEnv::new();
    let (dir, ids) = rename_fixture(&mut env, "dot", 1);
    env.raw(&dir).env("TASKS_RENAME_STOP_AFTER", "inventory")
        .args(["rename", "dot", "dots"]).status().unwrap();

    // R5: a task file the rename never knew about.
    std::fs::write(dir.join("tasks/dot-ffffff.md"), MINIMAL_TASK).unwrap();
    assert_eq!(env.fail(&dir, &["rename", "dot", "dots"]), "validation");
    std::fs::remove_file(dir.join("tasks/dot-ffffff.md")).unwrap();

    // R2: a source edited after the baseline was taken.
    let path = dir.join(format!("tasks/{}.md", ids[0]));
    let text = std::fs::read_to_string(&path).unwrap().replace("title: T0", "title: edited");
    std::fs::write(&path, text).unwrap();
    assert_eq!(env.fail(&dir, &["rename", "dot", "dots"]), "validation");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --test cli -- every_mutation_boundary an_empty_project a_live_claim_blocks_the_resume each_refusal_fires`
Expected: FAIL — `file:<n>` and `claims` stops are unrecognized; refusals not wired end to end.

- [ ] **Step 3: Extend the stop hook and close the gaps the tests find**

Add the `file:<n>` and `claims` boundaries to `TASKS_RENAME_STOP_AFTER`. Fix whatever the
matrix exposes — most likely the ordering inside P3 (write destination, *then* remove
source, never the reverse) and P6 removing the claim store before the inventory.

- [ ] **Step 4: Run the whole gate**

Run: `just gate` — Expected: clean, 12 tasks' worth of tests passing.

- [ ] **Step 5: Update the docs, reinstall, close the task, and commit**

In `skills/tasks/SKILL.md`, under the registry paragraph: renaming a prefix is
`tasks rename <old> <new>`; the retired prefix keeps resolving **for as long as the project
stays registered**, so references in other projects and in prose need no edit. Do not write
"forever" — §2 scopes the guarantee to registration, and `unregister` drops a project's
aliases with it. In `README.md`, add `rename` to the command list. Set the design doc's
status line to `implemented (<date>)`.

```bash
cargo install --path .
tasks done tasks-70c72f "Interruption matrix across every mutation boundary including intra-phase and both cleanup steps, empty-project and non-git runs, a claim taken after an interruption blocking the resume, and end-to-end refusals. Docs updated; the alias guarantee is registration-scoped, not forever."
git add src/rename/ skills/tasks/SKILL.md README.md docs/specs/2026-09-08-prefix-rename-design.md tests/cli.rs tasks/
git commit -m "test(rename): cover every mutation boundary and document the command"
```

---

## Self-review

**Spec coverage.** §2 model → Tasks 1, 2. §3 registry format → Task 1. §4 canonicalization
→ Tasks 3, 4. §5.1/5.1.1 phases and inventory → Tasks 9, 11. §5.2 preflight → Task 11.
§5.3 classifier → Task 10. §5.4 unregister/init → Tasks 6, 11 (pending-name reservation).
§5.5 byte preservation → Task 8. §5.6 undo and `--explain` → Task 11 (`--explain`), Task 12
(docs). §5.7 freeze → Task 11 (the mechanism, shipped with the command it protects),
Task 12 (its verification). §6 concurrency → Task 7. §7 surface and JSON → Task 11.
§8 testing → distributed, with the interruption matrix in Task 12. §9 limits → documented,
not implemented.

**Ordering constraint.** No task exposes a mutation whose protection lands later. The freeze
moved into Task 11 for exactly this reason: shipping `rename` first would leave a window in
which an interrupted rename is unprotected, which is worse than having no command.

**Type consistency.** `canonical_prefix`/`canonical_id`/`is_taken` (Task 1) are used under
those names in Tasks 2–5. `parse_id` (Task 3) is used in Tasks 3 and 11. `rewrite_prefix`
(Task 8) is consumed by `Inventory::build` (Task 9). `Snapshot`/`Recovery` (Tasks 9, 10) are
consumed by Task 11. `Inventory::pending` (Task 9) is consumed by Task 12.

**Known gap, deliberate.** §5.6's rollback is a documented manual procedure, not code, so no
task implements it; Task 12 links it from the skill doc. If a reviewer wants it automated
that is a new task against a new spec section.
