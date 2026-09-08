# Prefix rename implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename a project's prefix with one command, keeping every existing reference to
the old name resolving through a permanent alias.

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

- [ ] **Step 1: Write the failing tests**

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

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib registry::`
Expected: FAIL — `no method named canonical_prefix`, `no field aliases`.

- [ ] **Step 3: Add the field, the invariants, and the primitives**

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

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib registry::` — Expected: PASS.
Run: `just check` — Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src/registry.rs
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

- [ ] **Step 1: Write the failing test**

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
    assert_eq!(env.json(&sci, &["show", &retired])["task"]["id"], id);
    assert_eq!(env.json(&sci, &["root", &retired])["prefix"], "fam");

    // --project takes a retired name too: it is a name of the project.
    let v = env.json(&sci, &["list", "--project", "old"]);
    assert_eq!(v["tasks"][0]["id"], id, "{v}");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli -- a_retired_prefix_resolves`
Expected: FAIL — `unresolvable_id`, no project registered as "old".

- [ ] **Step 3: Follow the alias in `open_registered`**

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

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --test cli -- a_retired_prefix_resolves` — Expected: PASS.
Run: `just gate` — Expected: clean; no existing test regresses.

- [ ] **Step 5: Commit**

```bash
git add src/scope.rs tests/cli.rs
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

- [ ] **Step 1: Write the failing test**

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

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli -- a_retired_id_is_one_task`
Expected: FAIL — `task_not_found` on the first `note`.

- [ ] **Step 3: Canonicalize at the three points**

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

In `src/resolve.rs`, at the top of `resolve_task`, canonicalize the id before looking it up,
so a reference *stored* with a retired prefix still resolves.

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

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --test cli -- a_retired_id_is_one_task` — Expected: PASS.
Run: `just gate` — Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src/commands/mod.rs src/resolve.rs src/commands/dep.rs src/commands/show.rs tests/cli.rs
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

- [ ] **Step 1: Write the failing test**

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

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli -- a_checkout_still_using_a_retired`
Expected: FAIL — both commands succeed.

- [ ] **Step 3: Check the local prefix against the registry**

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

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --test cli -- a_checkout_still_using_a_retired` — Expected: PASS.
Run: `just gate` — Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src/commands/mod.rs tests/cli.rs
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

- [ ] **Step 1: Write the failing test**

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

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli -- check_nudges_a_depends`
Expected: FAIL — no `retired_prefix` warning.

- [ ] **Step 3: Raise the warning**

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

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --test cli -- check_nudges_a_depends` — Expected: PASS.
Run: `just gate` — Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src/commands/check.rs tests/cli.rs
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

- [ ] **Step 1: Write the failing test**

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

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli -- unregister_takes_the_aliases`
Expected: FAIL — `init --prefix old` succeeds.

- [ ] **Step 3: Implement**

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

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --test cli -- unregister_takes_the_aliases` — Expected: PASS.
Run: `just gate` — Expected: clean. Fix `registry.rs`'s existing
`unregister_removes_once_and_then_reports_the_prefix_is_absent` for the new return type.

- [ ] **Step 5: Commit**

```bash
git add src/registry.rs src/commands/unregister.rs src/commands/init.rs src/output.rs tests/cli.rs
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

`add` takes no mutation lock at all today (`open_ctx` sets `lock: None`), relying on
`create_task`'s exclusive create, which guards a colliding id and nothing else. And
`Registry::load` → mutate → `save` is an unserialized read-modify-write: atomic replacement
prevents a torn file, not a lost update.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn concurrent_registry_writes_do_not_lose_each_other() {
    let env = TestEnv::new();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let mut children: Vec<_> = [("aaa", a.path()), ("bbb", b.path())]
        .into_iter()
        .map(|(prefix, dir)| {
            env.raw(dir)
                .args(["init", "--prefix", prefix])
                .spawn()
                .unwrap()
        })
        .collect();
    for child in &mut children {
        assert!(child.wait().unwrap().success());
    }
    let text = std::fs::read_to_string(env.home.path().join(".config/tasks/projects.toml")).unwrap();
    assert!(text.contains("aaa = "), "{text}");
    assert!(text.contains("bbb = "), "{text}");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli -- concurrent_registry_writes`
Expected: FLAKY/FAIL — one registration is lost under contention. Run it a few times:
`cargo test --test cli -- concurrent_registry_writes --test-threads=1` repeated.

- [ ] **Step 3: Serialize the registry and lock `add`**

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

In `src/commands/mod.rs`, give `add` the mutation lock on both arms:

```rust
                Some(prefix) => {
                    let registry = Registry::load()?;
                    let project =
                        crate::scope::open_registered(&registry, &prefix, Origin::Prefix)?;
                    let lock = Some(MutationLock::acquire(&project.prefix)?);
                    Ctx { project, registry, warnings: Vec::new(), lock, claims: None, pending_claim: None }
                }
                None => {
                    let mut ctx = open_ctx(dir)?;
                    ctx.lock = Some(MutationLock::acquire(&ctx.project.prefix)?);
                    ctx
                }
```

In `open_id_write_ctx`, re-resolve after acquiring, and reacquire if identity moved:

```rust
    // A waiter resolved before the lock; the world may have moved under it. Re-resolve
    // under the lock, and if the identity changed we are holding the wrong lock.
    for _ in 0..4 {
        let registry = Registry::load()?;
        let live = registry.canonical_prefix(&ctx.project.prefix).to_string();
        if live == ctx.project.prefix {
            ctx.registry = registry;
            return Ok(ctx);
        }
        ctx.lock = None; // released before the next acquire, never held crossed
        ctx.project = crate::scope::open_registered(&registry, &live, Origin::Prefix)?;
        ctx.lock = Some(MutationLock::acquire(&ctx.project.prefix)?);
    }
    Err(Error::Io(
        "the project's identity kept changing while acquiring its lock; retry".into(),
    ))
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --test cli -- concurrent_registry_writes` — Expected: PASS, repeatedly.
Run: `just gate` — Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src/registry.rs src/commands/mod.rs src/commands/init.rs src/commands/unregister.rs tests/cli.rs
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

- [ ] **Step 1: Write the failing test**

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

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib rename::rewrite`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Implement**

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

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib rename::rewrite` — Expected: PASS.
Run: `just gate` — Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src/rename/ src/format.rs src/main.rs
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
- Produces: `Snapshot { registry, config, inventory, entries, named, strays }` and
  `observe(&Registry, root, source, target) -> Result<Snapshot>`.
- Produces: `digest(bytes: &[u8]) -> String` — lowercase hex SHA-256.

The inventory lives at `~/.local/state/tasks/rename/<source>.toml`, beside claims: it is
machine state, must not appear in `git status`, and must survive `git checkout .`.

- [ ] **Step 1: Write the failing test**

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
        let snap = observe(&registry, &project.root, &inventory).unwrap();
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
        let snap = observe(&crate::registry::Registry::default(), &project.root, &inventory).unwrap();
        assert_eq!(snap.strays.len(), 1, "{:?}", snap.strays);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib rename::snapshot`
Expected: FAIL — modules do not exist.

- [ ] **Step 3: Implement**

Add to `Cargo.toml` under `[dependencies]`: `sha2 = "0.10"`.

`src/rename/inventory.rs` — the record, `digest`, `build` (which computes each `to` by
running `rewrite::rewrite_prefix` over the source, and `config_to` by rewriting the config's
`prefix` line), `path`, `load`, `save`, `remove`, and `pending()` which lists every
inventory in the directory.

`src/rename/snapshot.rs` — `FileState { Absent, Present(String) }`, `EntryState`,
`RegistryState { old_key, new_key, alias }`, `ConfigState { prefix, digest }`, `Named`,
`Snapshot`, and `observe`, which scans `tasks/*.md` once, classifies each file as a source,
a destination, or a stray, and reads the config.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib rename::` — Expected: PASS.
Run: `just gate` — Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/rename/
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

- [ ] **Step 1: Write the failing tests**

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

    #[test]
    fn the_classifier_is_total_over_a_bounded_enumeration() {
        // Every snapshot the enumeration produces classifies; nothing panics, and anything
        // outside the table is a Refuse rather than a guess.
        let mut seen_non_refuse = 0;
        for snap in enumerate_snapshots() {
            match classify(&snap) {
                Recovery::Refuse(reason) => assert!(!reason.is_empty()),
                _ => seen_non_refuse += 1,
            }
        }
        assert!(seen_non_refuse > 0, "the enumeration must reach the table, not only refusals");
    }
}
```

Write `fixture(Stage)` and `enumerate_snapshots()` in the same test module.
`enumerate_snapshots` iterates: `registry` over its three fields, `config` prefix × 3 and
digest × 3, `inventory` × 2, `named` over zero/non-zero each, `strays` × 2, and entry lists
of length 0, 1, and 2 over `source` × 3 and `dest` × 3. Include inventory-absent snapshots
carrying entries and inventory-present ones carrying none, so a predicate reaching for a
baseline that is not there is caught rather than assumed away.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib rename::classify`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Implement refusals first, then the table**

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

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib rename::classify` — Expected: PASS.
Run: `just gate` — Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src/rename/classify.rs src/rename/mod.rs
git commit -m "feat(rename): classify recovery from a snapshot and its baseline"
```

---

### Task 11: The `rename` command and its phases

**Files:**
- Create: `src/commands/rename.rs`
- Modify: `src/rename/mod.rs` (phases), `src/cli.rs`, `src/commands/mod.rs`, `src/output.rs`
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: everything from Tasks 1, 6, 7, 8, 9, 10.
- Produces: `tasks rename <old> <new> [--explain]`;
  `RenameOut { prefix, previous, root, tasks, aliases, recovery, warnings }`.

Phases: P1 preflight, P2 inventory, P3 file pass, P4 config, P5 registry, P6 cleanup.
Classification precedes every fresh-operation check: after P5, `<old>` canonicalizes to
`<new>`, so `new == old` and the taken-name refusal would otherwise reject the idempotent
re-run this design promises. Claim and worktree checks gate **every** mutating path, not
just `Fresh`. `--explain` writes nothing, takes no locks, runs no authorization checks.

- [ ] **Step 1: Write the failing test**

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

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli -- rename_rewrites_the_project`
Expected: FAIL — unrecognized subcommand `rename`.

- [ ] **Step 3: Implement the phases and wire the command**

`src/rename/mod.rs` gains `run(project, registry, source, target, explain) -> Result<RenameOut>`:
observe, classify, and then execute from the phase the verdict names. On `Fresh`, run the
§5.2 preflight refusals first. Authorization — live claims and the worktree count — runs on
every mutating verdict, and not at all under `--explain`.

`src/cli.rs` gains the subcommand with `old`, `new`, and `--explain`. `src/output.rs` gains
`RenameOut` and its `Output::Rename` variant, its `pretty` arm (the new prefix, as `init`
prints its prefix), and its `warnings_of` arm.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --test cli -- rename_` — Expected: PASS.
Run: `just gate` — Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src/rename/ src/commands/rename.rs src/commands/mod.rs src/cli.rs src/output.rs tests/cli.rs
git commit -m "feat(rename): rename a project's prefix in six recoverable phases"
```

---

### Task 12: The freeze, interruption coverage, and the docs

**Files:**
- Modify: `src/commands/mod.rs` (the freeze check), `src/commands/init.rs`
- Modify: `skills/tasks/SKILL.md`, `README.md`, `docs/specs/2026-09-08-prefix-rename-design.md`
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `Inventory::pending()` (Task 9).
- Produces: `commands::reject_pending_rename(&Project) -> Result<()>`.

Discovery scans the pending directory and matches on the recorded `source`, `target`, and
`root` — **not** on the caller's resolved prefix. Between P4 and P5 the config says `new`
while the registry still says `old` with no alias, so a lookup by resolved prefix misses
`rename/<old>.toml` entirely and an `add` sails into a project mid-rename.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_pending_rename_freezes_the_project_including_the_p4_window() {
    let mut env = TestEnv::new();
    let dir = env.init("dot");
    git(&dir, &["init", "-q", "-b", "main"]);
    env.json(&dir, &["add", "T", "-p", "2"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);

    // Simulate the P4-P5 window: files and config renamed, registry untouched.
    interrupt_rename_after_config(&mut env, &dir, "dot", "dots");

    // Writes refuse; the freeze found the inventory by its recorded names, not by
    // resolving the caller's prefix.
    assert_eq!(env.fail(&dir, &["add", "New", "-p", "2"]), "validation");
    assert_eq!(env.fail(&dir, &["unregister", "dot"]), "validation");
    // Reads still work.
    assert!(env.json(&dir, &["list"])["tasks"].is_array());
    // The target name is reserved even though it is in no registry yet.
    let fresh = tempfile::tempdir().unwrap();
    assert_eq!(env.fail(fresh.path(), &["init", "--prefix", "dots"]), "config");

    // And the rename completes on a re-run.
    assert_eq!(env.json(&dir, &["rename", "dot", "dots"])["recovery"], "resume_registry");
}

#[test]
fn every_interruption_boundary_resumes_to_the_same_final_state() {
    for stop_after in ["inventory", "files", "config", "registry"] {
        let mut env = TestEnv::new();
        let dir = env.init("dot");
        git(&dir, &["init", "-q", "-b", "main"]);
        let id = id_of(env.json(&dir, &["add", "T", "-p", "2"]));
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-qm", "seed"]);

        interrupt_rename(&mut env, &dir, "dot", "dots", stop_after);
        env.json(&dir, &["rename", "dot", "dots"]);

        let moved = format!("dots-{}", id.split_once('-').unwrap().1);
        assert!(dir.join(format!("tasks/{moved}.md")).is_file(), "{stop_after}");
        assert_eq!(env.json(&dir, &["show", &moved])["task"]["id"], moved, "{stop_after}");
        assert!(
            !env.home.path().join(".local/state/tasks/rename/dot.toml").exists(),
            "the inventory is removed by cleanup: {stop_after}"
        );
        assert!(
            !env.home.path().join(".local/state/tasks/claims/dot.toml").exists(),
            "the old claim store is removed: {stop_after}"
        );
    }
}
```

Write `interrupt_rename` using the `TASKS_RENAME_STOP_AFTER` env var added in Step 3.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --test cli -- a_pending_rename_freezes every_interruption_boundary`
Expected: FAIL — `add` succeeds; no stop hook.

- [ ] **Step 3: Implement the freeze and the test hook**

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

Call it wherever a mutation lock is taken (`open_id_write_ctx`, `add`'s two arms) and in
`init`/`unregister` under the registry lock. `init` also refuses a prefix matching any
pending `target`.

In `src/rename/mod.rs`, read `TASKS_RENAME_STOP_AFTER` and return early after the named
phase — the only way to test interruption without killing a process mid-write.

- [ ] **Step 4: Run the tests, then the whole gate**

Run: `cargo test --test cli -- a_pending_rename_freezes every_interruption_boundary` — PASS.
Run: `just gate` — Expected: clean.

- [ ] **Step 5: Update the docs and commit**

In `skills/tasks/SKILL.md`, under the registry paragraph, add: renaming a prefix is
`tasks rename <old> <new>`; the old prefix keeps resolving forever, so references in other
projects and in prose need no edit. In `README.md`, add `rename` to the command list. Set
the design doc's status line to `implemented (<date>)`.

```bash
git add src/commands/mod.rs src/commands/init.rs src/rename/ skills/tasks/SKILL.md README.md docs/specs/2026-09-08-prefix-rename-design.md tests/cli.rs
git commit -m "feat(rename): freeze a project while its rename is unfinished"
```

---

## Self-review

**Spec coverage.** §2 model → Tasks 1, 2. §3 registry format → Task 1. §4 canonicalization
→ Tasks 3, 4. §5.1/5.1.1 phases and inventory → Tasks 9, 11. §5.2 preflight → Task 11.
§5.3 classifier → Task 10. §5.4 unregister/init → Task 6. §5.5 byte preservation → Task 8.
§5.6 undo and `--explain` → Task 11 (`--explain`), Task 12 (docs). §5.7 freeze → Task 12.
§6 concurrency → Task 7. §7 surface and JSON → Task 11. §8 testing → distributed. §9 limits
→ documented, not implemented.

**Type consistency.** `canonical_prefix`/`canonical_id`/`is_taken` (Task 1) are used under
those names in Tasks 2–5. `parse_id` (Task 3) is used in Tasks 3 and 11. `rewrite_prefix`
(Task 8) is consumed by `Inventory::build` (Task 9). `Snapshot`/`Recovery` (Tasks 9, 10) are
consumed by Task 11. `Inventory::pending` (Task 9) is consumed by Task 12.

**Known gap, deliberate.** §5.6's rollback is a documented manual procedure, not code, so no
task implements it; Task 12 links it from the skill doc. If a reviewer wants it automated
that is a new task against a new spec section.
