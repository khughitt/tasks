# Former roots and project resolution implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Record each project's former roots and their worktree storage in the host registry, and answer the canonical id, live prefix, or project for any id, prefix, or path through one read-only command, `tasks resolve`.

**Architecture:**

- A new module, `src/locations.rs`, holds the `[locations]` registry types, the storage probe, path normalization, and the location index that matches paths.
- `Registry` gains the table, validates it on load, carries it through rename, adopt, and unregister, and records history through one entry point, `note_root_change`. `init`, `rename`, and `rename --adopt` call it.
- `tasks resolve` classifies each input, then answers ids and prefixes from the registry, and paths from the index.

**Tech Stack:** Rust, clap, serde, toml, the existing `Registry`, `TaskId`, and `crate::time` helpers. Tests use `tests/common` `TestEnv`, plus `git` for the worktree fixture. No new dependency.

**Spec:** `docs/specs/2026-10-06-project-resolution-design.md`

## Global constraints

- `[projects]` keeps its exact shape. Eight hand readers parse it.
- `[locations]` is not written when empty, and within it a project's table is removed when it holds nothing.
- An entry's identity is the pair (`root`, `storage`). Nothing ever deletes a `former` entry except `unregister`, which removes the whole project.
- `resolve` takes no locks and writes nothing.
  - Per-item outcomes are `resolved`, `unregistered`, `ambiguous`, and `invalid`.
  - Only registry problems are errors, reported as `{"error":{"kind","detail"}}` on stderr with exit 1.
- Classification order is path → id → prefix → invalid. `resolve` never expands the bare-hex shorthand.
- `relpath` is relative to `worktree` when present, otherwise to `location`.
- JSON shapes of existing commands do not change. New warnings are the only addition to their output.
- The ops `cli.toml` row lands first: edit it in an ops worktree, then copy it byte for byte to `tools/cli.toml`. Never hand-edit `tools/cli.toml` alone.
- Run tests only through `just test-one …` and `just test-fast`, and the gate through `just check` / `just gate`. Never call `cargo test` directly.
- No machine-specific absolute paths in code, comments, or docs. Use `<Dropbox>`, `<WORK_ROOT>`, and `<ops>` in prose.

## Review focus

1. **Missing path under a symlinked parent.** A path spelled through a symlinked ancestor (`~/d/ai/x`, where `~/d` is a symlink) whose tail no longer exists must normalize through the canonical ancestor and match the former root (Task 3, `normalize_resolves_a_symlinked_ancestor_of_a_missing_tail`).
2. **Dangling storage link at a current root.** The current root's `.worktrees` points at storage that has not been moved yet (the second host between sync and storage move). `resolve` must fall back to the `storage` record and not error (Task 3, `index_uses_the_record_when_the_live_link_dangles`).
3. **`~`-spelled `[locations]` entry.** A hand-written `[locations]` entry spelled with `~/` must be refused on load with a `config` error that names the entry and says paths must be absolute, not silently ignored (Task 1, `load_rejects_invalid_locations`).
4. **Line endings and blank lines from `--stdin`.** `--stdin` input with CRLF endings or blank lines must strip the line ending, report a blank line as an `invalid` item, and keep results one-to-one with input lines (Task 4, `stdin_batch_keeps_order_and_reports_each_item`).
5. **A path inside a live worktree under current storage.** It must resolve `via: "storage"`, with `worktree` split off and `relpath` relative to it. A hook's most common input (Task 3, `storage_match_splits_the_worktree`).

---

### Task 1: The `[locations]` table in the registry

Task record: see the `--step "Task 1: …"` child of tasks-7580d2. `tasks start` it before code changes.

**Files:**
- Create: `src/locations.rs` (types and pure history operations only in this task)
- Modify: `src/main.rs` (`mod locations;`), `src/registry.rs` (field, load validation, carry on rename/adopt/unregister)
- Test: unit tests in `src/locations.rs` and `src/registry.rs`

**Interfaces:**
- Produces:
  - `locations::Locations { storage: Option<PathBuf>, former: Vec<Former> }`
  - `locations::Former { root: PathBuf, storage: Option<PathBuf>, until: String }`
  - `Locations::push_former(&mut self, Former)`
  - `Locations::merge(self, displaced: Locations) -> Locations`. The result's `storage` is always `None`.
  - `locations::Displaced { root: PathBuf, storage: Option<PathBuf> }`: a root a command moved the project off, with the storage record kept for it.
  - `Locations::is_empty(&self) -> bool`
  - `Locations::problem(&self) -> Option<String>`
  - `Registry.locations: BTreeMap<String, Locations>`
  - `Registry::adopt` now returns `Result<Displaced>`: the source's registered root before adoption, and the source table's storage record.

- [ ] **Step 1: Write the failing tests for `Locations`.** Create `src/locations.rs`:

```rust
//! Per-host location history and path resolution
//! (docs/specs/2026-10-06-project-resolution-design.md §2, §4).
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[cfg(test)]
mod tests {
    use super::*;

    fn former(root: &str, storage: Option<&str>, until: &str) -> Former {
        Former {
            root: root.into(),
            storage: storage.map(PathBuf::from),
            until: until.into(),
        }
    }

    #[test]
    fn push_former_keys_entries_by_root_and_storage() {
        let mut l = Locations::default();
        l.push_former(former("/a", Some("/s1"), "2026-01-01"));
        l.push_former(former("/b", None, "2026-02-01"));
        l.push_former(former("/a", Some("/s2"), "2026-03-01"));
        l.push_former(former("/a", Some("/s1"), "2026-04-01"));
        assert_eq!(
            l.former,
            [
                former("/b", None, "2026-02-01"),
                former("/a", Some("/s2"), "2026-03-01"),
                former("/a", Some("/s1"), "2026-04-01"),
            ],
            "the same pair moves its until forward; another storage is another entry"
        );
    }

    #[test]
    fn merge_unions_both_lists_and_copies_no_record() {
        let mut target = Locations {
            storage: Some("/target-store".into()),
            former: vec![former("/x", None, "2026-01-01")],
        };
        target.push_former(former("/shared", Some("/s"), "2026-02-01"));
        let old = Locations {
            storage: Some("/old-store".into()),
            former: vec![
                former("/shared", Some("/s"), "2026-05-01"),
                former("/y", Some("/ys"), "2026-03-01"),
            ],
        };
        let merged = target.merge(old);
        assert_eq!(merged.storage, None, "neither table's record is copied (spec §2.2)");
        assert_eq!(
            merged.former,
            [
                former("/x", None, "2026-01-01"),
                former("/y", Some("/ys"), "2026-03-01"),
                former("/shared", Some("/s"), "2026-05-01"),
            ]
        );
    }

    #[test]
    fn problem_names_each_invalid_entry() {
        for (l, word) in [
            (Locations { storage: Some("rel".into()), former: vec![] }, "absolute"),
            (Locations { storage: None, former: vec![former("~/d/ai", None, "2026-01-01")] }, "absolute"),
            (Locations { storage: None, former: vec![former("/a", Some("s"), "2026-01-01")] }, "absolute"),
            (Locations { storage: None, former: vec![former("/a", None, "2026-1-1")] }, "YYYY-MM-DD"),
            (
                Locations {
                    storage: None,
                    former: vec![former("/a", None, "2026-01-01"), former("/a", None, "2026-02-01")],
                },
                "twice",
            ),
        ] {
            let problem = l.problem().expect("invalid");
            assert!(problem.contains(word), "{problem}");
        }
        assert_eq!(Locations::default().problem(), None);
    }
}
```

Add `mod locations;` to `src/main.rs`, in alphabetical order after `mod lanes;`.

- [ ] **Step 2: Run the tests to verify they fail.**
  - Run: `just test-one locations::`
  - Expected: compile errors, because `Locations` and `Former` are not defined.

- [ ] **Step 3: Implement the types.** Put this above the tests in `src/locations.rs`:

```rust
/// One project's history on this host. `storage` is the record of where the current
/// root's `.worktrees` resolved when a command last wrote this project's entry (§2).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Locations {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub former: Vec<Former>,
}

/// A root a command moved the project off, with the storage record kept for it: the
/// fallback when the root is gone and its link cannot be read (§2.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Displaced {
    pub root: PathBuf,
    pub storage: Option<PathBuf>,
}

/// A location the project left. Identity is (`root`, `storage`): a root can recur with
/// other storage, or be current again, and no storage it used is ever dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Former {
    pub root: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<PathBuf>,
    pub until: String,
}

impl Locations {
    pub fn is_empty(&self) -> bool {
        self.storage.is_none() && self.former.is_empty()
    }

    /// Adds `entry`; an existing (root, storage) pair only moves its `until` forward.
    pub fn push_former(&mut self, entry: Former) {
        match self
            .former
            .iter_mut()
            .find(|f| f.root == entry.root && f.storage == entry.storage)
        {
            Some(existing) => {
                if entry.until > existing.until {
                    existing.until = entry.until;
                }
            }
            None => self.former.push(entry),
        }
        self.former
            .sort_by(|a, b| (&a.until, &a.root).cmp(&(&b.until, &b.root)));
    }

    /// The adoption merge (§2.2): the union of both `former` lists. The storage record
    /// starts empty: neither table's record describes the checkout's root, and
    /// `Registry::note_root_change` refreshes it from the live link. The displaced
    /// table's record travels separately, in `Displaced`.
    pub fn merge(mut self, displaced: Locations) -> Locations {
        for entry in displaced.former {
            self.push_former(entry);
        }
        self.storage = None;
        self
    }

    /// What load validation refuses (§2), or `None`.
    pub fn problem(&self) -> Option<String> {
        let absolute = |what: &str, path: &std::path::Path| {
            (!path.is_absolute()).then(|| {
                format!("{what} {} must be an absolute path", path.display())
            })
        };
        if let Some(storage) = &self.storage
            && let Some(problem) = absolute("storage", storage)
        {
            return Some(problem);
        }
        for (i, entry) in self.former.iter().enumerate() {
            if let Some(problem) = absolute("former root", &entry.root) {
                return Some(problem);
            }
            if let Some(storage) = &entry.storage
                && let Some(problem) = absolute("former storage", storage)
            {
                return Some(problem);
            }
            if entry.until.len() != 10 || crate::time::calendar_day(&entry.until).is_err() {
                return Some(format!(
                    "former root {} has until {:?}; write YYYY-MM-DD",
                    entry.root.display(),
                    entry.until
                ));
            }
            if self.former[..i]
                .iter()
                .any(|f| f.root == entry.root && f.storage == entry.storage)
            {
                return Some(format!(
                    "former root {} is listed twice with the same storage",
                    entry.root.display()
                ));
            }
        }
        None
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass.**
  - Run: `just test-one locations::`
  - Expected: PASS (3 tests).

- [ ] **Step 5: Write the failing registry tests.** Append these to `src/registry.rs` `mod tests`:

```rust
    use crate::locations::{Former, Locations};

    fn history(storage: Option<&str>, former: &[(&str, Option<&str>, &str)]) -> Locations {
        Locations {
            storage: storage.map(PathBuf::from),
            former: former
                .iter()
                .map(|(root, storage, until)| Former {
                    root: root.into(),
                    storage: storage.map(PathBuf::from),
                    until: (*until).into(),
                })
                .collect(),
        }
    }

    #[test]
    fn locations_round_trip_and_an_empty_table_is_not_written() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("projects.toml");
        let mut r = Registry::default();
        r.register("tack", Path::new("/tmp/tack")).unwrap();
        r.save_to(&path).unwrap();
        assert!(!std::fs::read_to_string(&path).unwrap().contains("locations"));
        r.locations.insert(
            "tack".into(),
            history(Some("/w/tack/.worktrees"), &[("/d/ai", Some("/w/ai/.worktrees"), "2026-09-27")]),
        );
        r.save_to(&path).unwrap();
        assert_eq!(Registry::load_from(&path).unwrap().locations, r.locations);
    }

    #[test]
    fn load_rejects_invalid_locations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("projects.toml");
        let base = "[projects]\ntack = \"/tmp/tack\"\n\n";
        for (table, named) in [
            ("[locations.gone]\nstorage = \"/s\"\n", ["\"gone\"", "not a registered project"]),
            (
                "[[locations.tack.former]]\nroot = \"~/d/ai\"\nuntil = \"2026-09-27\"\n",
                ["\"tack\"", "absolute"],
            ),
            (
                "[[locations.tack.former]]\nroot = \"/d/ai\"\nuntil = \"yesterday\"\n",
                ["\"tack\"", "YYYY-MM-DD"],
            ),
        ] {
            std::fs::write(&path, format!("{base}{table}")).unwrap();
            let error = Registry::load_from(&path).unwrap_err();
            assert_eq!(error.kind(), "config", "{table}");
            for word in named {
                assert!(error.to_string().contains(word), "{table}: {error}");
            }
        }
    }

    #[test]
    fn rename_and_unregister_carry_locations() {
        let mut r = Registry::default();
        r.register("tack", Path::new("/tmp/tack")).unwrap();
        let h = history(None, &[("/d/ai", None, "2026-09-27")]);
        r.locations.insert("tack".into(), h.clone());
        r.rename("tack", "hq").unwrap();
        assert_eq!(r.locations.get("hq"), Some(&h));
        assert!(!r.locations.contains_key("tack"));
        r.unregister("hq").unwrap();
        assert!(r.locations.is_empty());
    }

    #[test]
    fn adopt_merges_both_histories_and_returns_what_it_displaced() {
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("new");
        std::fs::create_dir(&root).unwrap();
        let old_root = home.path().join("missing");
        for partial_init in [false, true] {
            let mut r = Registry::default();
            r.register("old", &old_root).unwrap();
            r.locations.insert(
                "old".into(),
                history(Some("/w/old"), &[("/d/older", Some("/w/older"), "2026-01-01")]),
            );
            if partial_init {
                r.register("new", &root).unwrap();
                r.locations
                    .insert("new".into(), history(Some("/w/new"), &[("/d/x", None, "2026-02-01")]));
            }
            assert_eq!(
                r.adopt("old", "new", &root).unwrap(),
                crate::locations::Displaced {
                    root: old_root.clone(),
                    storage: Some(PathBuf::from("/w/old")),
                }
            );
            assert!(!r.locations.contains_key("old"));
            let merged = &r.locations["new"];
            assert_eq!(merged.storage, None, "no record is copied into the merge");
            let roots: Vec<_> = merged.former.iter().map(|f| f.root.clone()).collect();
            let expected: Vec<PathBuf> = if partial_init {
                vec!["/d/older".into(), "/d/x".into()]
            } else {
                vec!["/d/older".into()]
            };
            assert_eq!(roots, expected, "partial_init={partial_init}");
        }
    }
```

- [ ] **Step 6: Run them to verify they fail.**
  - Run: `just test-one registry::`
  - Expected: compile errors (no `locations` field), and `adopt` returns `()`.

- [ ] **Step 7: Implement in `src/registry.rs`.**

  1. Add the field after `groups`:

     ```rust
     /// Live prefix -> where the project has lived on this host
     /// (docs/specs/2026-10-06-project-resolution-design.md §2). Not written when empty.
     #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
     pub locations: BTreeMap<String, crate::locations::Locations>,
     ```

  2. In `load_from`, after the groups loop:

     ```rust
     for (prefix, history) in &registry.locations {
         if !registry.projects.contains_key(prefix) {
             return Err(Error::Config(format!(
                 "{}: [locations] names {prefix:?}, which is not a registered project; \
                  edit [locations] in this file",
                 path.display()
             )));
         }
         if let Some(problem) = history.problem() {
             return Err(Error::Config(format!(
                 "{}: locations of {prefix:?}: {problem}",
                 path.display()
             )));
         }
     }
     ```

  3. In `rename`, after `self.retarget_groups(source, target);`:

     ```rust
     if let Some(history) = self.locations.remove(source) {
         self.locations.insert(target.into(), history);
     }
     ```

  4. In `unregister`, after the groups `retain`: `self.locations.remove(prefix);`

  5. Change `adopt`'s signature to `-> Result<crate::locations::Displaced>`.
     - Immediately before the `if self.projects.contains_key(target)` branch, after
       every validation, add:

       ```rust
       let previous = self.projects[source].clone();
       let history = self.locations.get(source).cloned().unwrap_or_default();
       ```

     - Replace the final `self.repoint(target, &root)?; Ok(())` with:

       ```rust
       self.repoint(target, &root)?;
       // In the rename branch the source's table already moved to the target, and the
       // merge collapses its own pairs. In the registered-target branch it joins the
       // target's (§2.2). The source's storage record leaves with `Displaced`, never
       // as the target's record.
       self.locations.remove(source);
       let storage = history.storage.clone();
       let merged = self
           .locations
           .remove(target)
           .unwrap_or_default()
           .merge(history);
       if !merged.is_empty() {
           self.locations.insert(target.into(), merged);
       }
       Ok(crate::locations::Displaced { root: previous, storage })
       ```

     - Callers: `src/rename/adopt.rs:169` ignores the value until Task 2. Bind it there
       as `let _displaced = registry.adopt(old, new, &project.root)?;`.

- [ ] **Step 8: Run the focused tests, then the fast suite.**
  - Run: `just test-one registry::`, then `just test-one locations::`, then `just test-fast`
  - Expected: all PASS. The existing adopt tests still pass, because the return value is unused.

- [ ] **Step 9: Commit.**

```bash
git add src/locations.rs src/main.rs src/registry.rs src/rename/adopt.rs
tasks done <task-1-id> "registry [locations] table: types, load validation, carry on rename/adopt/unregister, adoption merge"
git add tasks/
git commit -m "feat(registry): record per-host location history"
```

---

### Task 2: Record history in init, rename, and adopt

**Files:**
- Modify: `src/locations.rs` (storage probe), `src/registry.rs` (`note_root_change`), `src/commands/init.rs`, `src/rename/mod.rs:246-249`, `src/rename/adopt.rs:127,168-171`
- Test: unit tests in `src/registry.rs`. Create `tests/resolve.rs`, which holds this task's CLI tests and Task 4's.

**Interfaces:**
- Consumes (Task 1): `Locations`, `Former`, `Displaced`, `push_former`, `Registry.locations`, and `Registry::adopt -> Result<Displaced>`.
- Produces:
  - `locations::StorageState { Link(PathBuf), Dangling, InTree, Absent }`
  - `locations::storage_state(root: &Path) -> std::io::Result<StorageState>`
  - `Registry::displaced(&self, prefix: &str, root: PathBuf) -> Displaced`: `root` with `prefix`'s current storage record, for `init --force` to pass on.
  - `Registry::note_root_change(&mut self, prefix: &str, previous: Option<Displaced>, today: &str) -> Result<Vec<String>>`. It returns warnings.

- [ ] **Step 1: Write the failing unit tests.** Append to `src/registry.rs` `mod tests`:

```rust
    fn link_storage(root: &Path, storage: &Path) {
        std::fs::create_dir_all(root).unwrap();
        std::fs::create_dir_all(storage).unwrap();
        std::os::unix::fs::symlink(storage, root.join(".worktrees")).unwrap();
    }

    #[test]
    fn note_root_change_records_storage_then_the_displaced_root() {
        let base = tempfile::tempdir().unwrap();
        let base = base.path().canonicalize().unwrap();
        let (a, s1, b) = (base.join("a"), base.join("w/a"), base.join("b"));
        link_storage(&a, &s1);
        std::fs::create_dir(&b).unwrap();
        let mut r = Registry::default();
        r.register("p", &a).unwrap();
        assert!(r.note_root_change("p", None, "2026-10-01").unwrap().is_empty());
        assert_eq!(r.locations["p"].storage, Some(s1.clone()));

        // The checkout and its storage move away; init --force re-points the prefix.
        std::fs::remove_file(a.join(".worktrees")).unwrap();
        std::fs::remove_dir(&a).unwrap();
        let displaced = r.repoint("p", &b).unwrap().map(|root| r.displaced("p", root));
        let warnings = r.note_root_change("p", displaced, "2026-10-02").unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        let h = &r.locations["p"];
        assert_eq!(h.storage, None, "b keeps no .worktrees link");
        assert_eq!(h.former.len(), 1);
        assert_eq!((h.former[0].root.clone(), h.former[0].storage.clone()), (a.clone(), Some(s1)));
        assert_eq!(h.former[0].until, "2026-10-02");
    }

    #[test]
    fn moving_back_keeps_the_storage_the_root_used_before() {
        let base = tempfile::tempdir().unwrap();
        let base = base.path().canonicalize().unwrap();
        let (a, s1, s2, b) = (base.join("a"), base.join("w/s1"), base.join("w/s2"), base.join("b"));
        link_storage(&a, &s1);
        std::fs::create_dir(&b).unwrap();
        let mut r = Registry::default();
        r.register("p", &a).unwrap();
        r.note_root_change("p", None, "2026-10-01").unwrap();
        let displaced = r.repoint("p", &b).unwrap().map(|root| r.displaced("p", root));
        r.note_root_change("p", displaced, "2026-10-02").unwrap();
        std::fs::remove_file(a.join(".worktrees")).unwrap();
        std::os::unix::fs::symlink(&s2, a.join(".worktrees")).unwrap();
        std::fs::create_dir_all(&s2).unwrap();
        let displaced = r.repoint("p", &a).unwrap().map(|root| r.displaced("p", root));
        r.note_root_change("p", displaced, "2026-10-03").unwrap();
        let h = &r.locations["p"];
        assert_eq!(h.storage, Some(s2));
        let pairs: Vec<_> = h.former.iter().map(|f| (f.root.clone(), f.storage.clone())).collect();
        assert_eq!(pairs, [(a, Some(s1)), (b, None)], "(a, s1) survives the move back");
    }

    #[test]
    fn a_vanished_root_without_a_record_is_kept_without_storage_and_warned() {
        let base = tempfile::tempdir().unwrap();
        let base = base.path().canonicalize().unwrap();
        let (gone, b) = (base.join("gone"), base.join("b"));
        std::fs::create_dir(&b).unwrap();
        let mut r = Registry::default();
        r.register("p", &gone).unwrap();
        let displaced = r.repoint("p", &b).unwrap().map(|root| r.displaced("p", root));
        let warnings = r.note_root_change("p", displaced, "2026-10-02").unwrap();
        assert_eq!(r.locations["p"].former[0].storage, None);
        assert!(
            warnings.iter().any(|w| w.contains("no known worktree storage") && w.contains("gone")),
            "{warnings:?}"
        );
    }

    #[test]
    fn a_dangling_link_at_a_new_root_leaves_no_record_and_the_old_storage_under_former() {
        // The second host: --adopt runs before its storage move, so the new link dangles
        // and the old root is already gone (spec §2.1, §2.2).
        let base = tempfile::tempdir().unwrap();
        let base = base.path().canonicalize().unwrap();
        let (gone, s_old, b) = (base.join("tack"), base.join("w/tack"), base.join("hq"));
        std::fs::create_dir(&b).unwrap();
        std::os::unix::fs::symlink(base.join("w/hq"), b.join(".worktrees")).unwrap();
        let mut r = Registry::default();
        r.register("p", &b).unwrap();
        let previous = crate::locations::Displaced { root: gone.clone(), storage: Some(s_old.clone()) };
        let warnings = r.note_root_change("p", Some(previous), "2026-10-07").unwrap();
        let h = &r.locations["p"];
        assert_eq!(h.storage, None, "the old record never becomes the current one");
        assert_eq!((h.former[0].root.clone(), h.former[0].storage.clone()), (gone, Some(s_old)));
        assert!(warnings.iter().any(|w| w.contains("points at missing storage")), "{warnings:?}");

        // With the root unchanged, a dangling link keeps the record it had.
        r.locations.get_mut("p").unwrap().storage = Some(base.join("w/hq"));
        r.note_root_change("p", None, "2026-10-07").unwrap();
        assert_eq!(r.locations["p"].storage, Some(base.join("w/hq")));
    }
```

- [ ] **Step 2: Run them to verify they fail.**
  - Run: `just test-one registry::`
  - Expected: compile error, because `note_root_change` is not defined.

- [ ] **Step 3: Implement the probe in `src/locations.rs`.**

```rust
/// What a root's `.worktrees` is on this host right now (§2.1, §4.2).
#[derive(Debug, Clone, PartialEq)]
pub enum StorageState {
    /// A symlink that resolves: the storage path, canonical.
    Link(PathBuf),
    /// A symlink whose target is missing.
    Dangling,
    /// A real directory inside the root, already covered by the root.
    InTree,
    /// No `.worktrees`, or no root.
    Absent,
}

pub fn storage_state(root: &std::path::Path) -> std::io::Result<StorageState> {
    let link = root.join(".worktrees");
    let meta = match std::fs::symlink_metadata(&link) {
        Ok(meta) => meta,
        Err(e) if matches!(e.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory) => {
            return Ok(StorageState::Absent);
        }
        Err(e) => return Err(e),
    };
    if !meta.file_type().is_symlink() {
        return Ok(StorageState::InTree);
    }
    match link.canonicalize() {
        Ok(real) => Ok(StorageState::Link(real)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(StorageState::Dangling),
        Err(e) => Err(e),
    }
}
```

- [ ] **Step 4: Implement `note_root_change` in `src/registry.rs`.**

```rust
    /// `root` with `prefix`'s current storage record: what `init --force` passes to
    /// `note_root_change` for the root it re-pointed away from.
    pub fn displaced(&self, prefix: &str, root: PathBuf) -> crate::locations::Displaced {
        crate::locations::Displaced {
            root,
            storage: self.locations.get(prefix).and_then(|h| h.storage.clone()),
        }
    }

    /// Records `prefix`'s location history after a command wrote its entry (spec §2.1).
    /// `previous` is the root it was registered at before, with that root's storage
    /// record, when the command may have moved it. A different root joins `former` with
    /// its storage: the live link if it still resolves, else `previous.storage`. Then the
    /// record is refreshed from the current root; it never describes another root.
    /// Returns warnings.
    pub fn note_root_change(
        &mut self,
        prefix: &str,
        previous: Option<crate::locations::Displaced>,
        today: &str,
    ) -> Result<Vec<String>> {
        use crate::locations::{Former, StorageState, storage_state};
        let root = self
            .projects
            .get(prefix)
            .cloned()
            .ok_or_else(|| Error::Config(format!("no project registered as {prefix:?}")))?;
        let mut warnings = Vec::new();
        let mut history = self.locations.remove(prefix).unwrap_or_default();
        let mut root_changed = false;
        if let Some(displaced) = previous {
            let previous = crate::rename::root_identity(&displaced.root)?;
            if previous != crate::rename::root_identity(&root)? {
                root_changed = true;
                let storage = match storage_state(&previous)? {
                    StorageState::Link(real) => Some(real),
                    StorageState::InTree => None,
                    StorageState::Dangling | StorageState::Absent => displaced.storage,
                };
                if storage.is_none() && !previous.exists() {
                    warnings.push(format!(
                        "former root {} of {prefix} has no known worktree storage; paths under \
                         its storage will not resolve (spec §2.3)",
                        previous.display()
                    ));
                }
                history.push_former(Former {
                    root: previous,
                    storage,
                    until: today.into(),
                });
            }
        }
        match storage_state(&root)? {
            StorageState::Link(real) => history.storage = Some(real),
            StorageState::InTree | StorageState::Absent => history.storage = None,
            StorageState::Dangling => {
                if root_changed {
                    history.storage = None;
                }
                warnings.push(format!(
                    "{}/.worktrees points at missing storage; the storage record for {prefix} is {}",
                    root.display(),
                    if root_changed { "empty until a later init --force there" } else { "unchanged" }
                ));
            }
        }
        if !history.is_empty() {
            self.locations.insert(prefix.into(), history);
        }
        Ok(warnings)
    }
```

In the `Dangling | Absent` arm, the displaced root's own record is the fallback. A
previous root that still exists without a link was never given a record, so it gets
`None` and no warning. The warning fires only when the root is gone and nothing is
known.

- [ ] **Step 5: Run the unit tests to verify they pass.**
  - Run: `just test-one registry::`
  - Expected: PASS.

- [ ] **Step 6: Wire the three commands.**

  - **`src/commands/init.rs`:** after `let project = Project::init(&root, &prefix)?;` and
    before `registry.save()?;`:

    ```rust
    let today = crate::time::day(&crate::time::now()).to_string();
    let previous = displaced.clone().map(|root| registry.displaced(&prefix, root));
    let history = registry.note_root_change(&prefix, previous, &today)?;
    ```

    Then, after the existing displaced-root warning, add
    `warnings.extend(history);`.

  - **`src/rename/mod.rs:247`:** replace the two lines

    ```rust
    registry.rename(&invocation.source, &invocation.target)?;
    registry.save()?;
    ```

    with

    ```rust
    registry.rename(&invocation.source, &invocation.target)?;
    let today = crate::time::day(&crate::time::now()).to_string();
    out.warnings
        .extend(registry.note_root_change(&invocation.target, None, &today)?);
    registry.save()?;
    ```

  - **`src/rename/adopt.rs`:**
    1. Make `out` mutable at `:127` (`let mut out = RenameOut {`).
    2. At `:169`, replace the `_displaced` binding from Task 1 with:

       ```rust
       let displaced = registry.adopt(old, new, &project.root)?;
       let today = crate::time::day(&crate::time::now()).to_string();
       out.warnings
           .extend(registry.note_root_change(new, Some(displaced), &today)?);
       registry.save()?;
       ```

- [ ] **Step 7: Write the failing CLI tests.** Create `tests/resolve.rs`:

```rust
mod common;
use common::TestEnv;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

fn id_of(v: serde_json::Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

fn registry(env: &TestEnv) -> toml::Value {
    let text = std::fs::read_to_string(env.home.path().join(".config/tasks/projects.toml")).unwrap();
    toml::from_str(&text).unwrap()
}

/// A checkout at `base/<name>` whose `.worktrees` links to `base/work/<name>/.worktrees`,
/// the shape the work-link tool gives a synced checkout.
fn linked_checkout(base: &Path, name: &str) -> (PathBuf, PathBuf) {
    let root = base.join(name);
    let storage = base.join(format!("work/{name}/.worktrees"));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&storage).unwrap();
    symlink(&storage, root.join(".worktrees")).unwrap();
    (root, storage)
}

/// Moves a linked checkout to `new` and relinks it as work-link would. With
/// `move_storage` false the storage stays put, so the new link dangles: the second
/// host's state between the synced move and its own storage move.
fn move_checkout(base: &Path, old: &str, new: &str, move_storage: bool) -> (PathBuf, PathBuf) {
    std::fs::rename(base.join(old), base.join(new)).unwrap();
    if move_storage {
        std::fs::rename(base.join(format!("work/{old}")), base.join(format!("work/{new}"))).unwrap();
    }
    let root = base.join(new);
    let storage = base.join(format!("work/{new}/.worktrees"));
    std::fs::remove_file(root.join(".worktrees")).unwrap();
    symlink(&storage, root.join(".worktrees")).unwrap();
    (root, storage)
}

/// The cutover on the renaming host: rename at the old root, move, init --force.
fn cutover(env: &TestEnv, base: &Path) -> (String, PathBuf, PathBuf, PathBuf, PathBuf) {
    let (old_root, old_storage) = linked_checkout(base, "tack");
    env.json(&old_root, &["init", "--prefix", "tack"]);
    let id = id_of(env.json(&old_root, &["add", "Task"]));
    env.json(&old_root, &["rename", "tack", "hq"]);
    let (root, storage) = move_checkout(base, "tack", "hq", true);
    env.json(&root, &["init", "--prefix", "hq", "--force"]);
    (id, old_root, old_storage, root, storage)
}

#[test]
fn a_cutover_records_the_former_root_with_its_storage() {
    let env = TestEnv::new();
    let base = tempfile::tempdir().unwrap();
    let base = base.path().canonicalize().unwrap();
    let (_, old_root, old_storage, _, storage) = cutover(&env, &base);
    let hq = &registry(&env)["locations"]["hq"];
    assert_eq!(hq["storage"].as_str().unwrap(), storage.to_str().unwrap());
    let former = hq["former"].as_array().unwrap();
    assert_eq!(former.len(), 1);
    assert_eq!(former[0]["root"].as_str().unwrap(), old_root.to_str().unwrap());
    assert_eq!(former[0]["storage"].as_str().unwrap(), old_storage.to_str().unwrap());
}

#[test]
fn an_init_without_a_worktrees_link_leaves_the_registry_shape_alone() {
    let mut env = TestEnv::new();
    env.init("sci");
    assert!(registry(&env).get("locations").is_none());
}
```

  Note on `cutover`: `rename` refuses a dirty `tasks/` only inside git. These fixtures
  are not git repositories, which the existing adopt fixtures rely on too. If `rename`
  refuses because of the `.worktrees` symlink or the missing git repository, stop and
  record the error in a task note before changing the fixture.

- [ ] **Step 8: Run them.**
  - Run: `just test-one --test resolve`
  - Expected: PASS, given Steps 3–6.
  - If the first test fails, the wiring in Step 6 is incomplete. Fix that, not the test.

- [ ] **Step 9: Run the fast suite and commit.**
  - Run: `just test-fast`
  - Expected: PASS. Existing init, rename, and adopt tests are unchanged, because the
    tempdir checkouts have no `.worktrees`, so no `[locations]` is written.

```bash
git add src/locations.rs src/registry.rs src/commands/init.rs src/rename/mod.rs src/rename/adopt.rs tests/resolve.rs
tasks done <task-2-id> "init, rename and adopt record storage and former roots"
git add tasks/
git commit -m "feat(registry): record former roots at init, rename and adopt"
```

---

### Task 3: Path normalization and the location index

**Files:**
- Modify: `src/locations.rs`
- Test: unit tests in `src/locations.rs`

**Interfaces:**
- Consumes (Tasks 1–2): `Registry { projects, locations }` and `storage_state`.
- Produces:
  - `locations::normalize(path: &Path, base: &Path) -> std::result::Result<PathBuf, String>`
  - `locations::Via { Root, Storage, FormerRoot, FormerStorage, Git }`: `Serialize`, snake_case, `Ord` in that order.
  - `locations::Index::build(registry: &Registry) -> Index`, whose `pub warnings: Vec<String>` field is public.
  - `Index::resolve(&self, path: &Path, warnings: &mut Vec<String>) -> Answer`, where `path` is already normalized.
  - `locations::Answer { Resolved(Match), Ambiguous(Vec<Candidate>), Unregistered }`
  - `locations::Match { prefix: String, via: Via, location: PathBuf, worktree: Option<PathBuf>, relpath: PathBuf }`
  - `locations::Candidate { prefix: String, via: Via, location: String }`: `Serialize`.

- [ ] **Step 1: Write the failing tests.** Add to `src/locations.rs` `mod tests`:

```rust
    use crate::registry::Registry;
    use std::path::Path;

    fn reg(projects: &[(&str, &Path)], history: &[(&str, Locations)]) -> Registry {
        let mut r = Registry::default();
        for (prefix, root) in projects {
            r.projects.insert((*prefix).into(), root.to_path_buf());
        }
        for (prefix, h) in history {
            r.locations.insert((*prefix).into(), h.clone());
        }
        r
    }

    fn resolved(answer: Answer) -> Match {
        match answer {
            Answer::Resolved(m) => m,
            other => panic!("expected a match, got {other:?}"),
        }
    }

    #[test]
    fn normalize_resolves_a_symlinked_ancestor_of_a_missing_tail() {
        let base = tempfile::tempdir().unwrap();
        let real = base.path().canonicalize().unwrap().join("real");
        std::fs::create_dir(&real).unwrap();
        let alias = base.path().join("alias");
        std::os::unix::fs::symlink(&real, &alias).unwrap();
        assert_eq!(normalize(&alias.join("gone/x"), Path::new("/")).unwrap(), real.join("gone/x"));
        assert_eq!(
            normalize(Path::new("gone/./x"), &alias).unwrap(),
            real.join("gone/x"),
            "relative to base, `.` dropped"
        );
        assert_eq!(normalize(&alias.join("../real/y"), Path::new("/")).unwrap(), real.join("y"));
        assert!(normalize(&alias.join("gone/../x"), Path::new("/")).unwrap_err().contains(".."));
    }

    #[test]
    fn matching_is_component_wise_and_the_deepest_location_wins() {
        let r = reg(
            &[("parent", Path::new("/p")), ("child", Path::new("/p/sub/child"))],
            &[],
        );
        let index = Index::build(&r);
        let mut w = Vec::new();
        assert!(matches!(index.resolve(Path::new("/pp/x"), &mut w), Answer::Unregistered));
        let m = resolved(index.resolve(Path::new("/p/sub/child/src/a.rs"), &mut w));
        assert_eq!((m.prefix.as_str(), m.via), ("child", Via::Root));
        assert_eq!(m.relpath, Path::new("src/a.rs"));
        let m = resolved(index.resolve(Path::new("/p/sub"), &mut w));
        assert_eq!((m.prefix.as_str(), m.relpath.as_path()), ("parent", Path::new("sub")));
    }

    #[test]
    fn current_beats_former_and_two_former_owners_are_ambiguous() {
        let gone = |root: &str| Locations {
            storage: None,
            former: vec![Former { root: root.into(), storage: None, until: "2026-01-01".into() }],
        };
        let r = reg(
            &[("new", Path::new("/x")), ("a", Path::new("/a")), ("b", Path::new("/b"))],
            &[("a", gone("/x")), ("b", gone("/y"))],
        );
        let index = Index::build(&r);
        let mut w = Vec::new();
        assert_eq!(resolved(index.resolve(Path::new("/x/f"), &mut w)).prefix, "new");
        let r = reg(&[("a", Path::new("/a")), ("b", Path::new("/b"))], &[("a", gone("/y")), ("b", gone("/y"))]);
        match Index::build(&r).resolve(Path::new("/y/f"), &mut w) {
            Answer::Ambiguous(c) => {
                let names: Vec<_> = c.iter().map(|c| c.prefix.as_str()).collect();
                assert_eq!(names, ["a", "b"]);
            }
            other => panic!("{other:?}"),
        }
        // One project as both current and former at the same path is not ambiguous.
        let r = reg(&[("a", Path::new("/y"))], &[("a", gone("/y"))]);
        assert_eq!(resolved(Index::build(&r).resolve(Path::new("/y/f"), &mut w)).via, Via::Root);
    }

    #[test]
    fn storage_match_splits_the_worktree() {
        let base = tempfile::tempdir().unwrap();
        let base = base.path().canonicalize().unwrap();
        let root = base.join("hq");
        let storage = base.join("work/hq/.worktrees");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(storage.join("wt/src")).unwrap();
        std::os::unix::fs::symlink(&storage, root.join(".worktrees")).unwrap();
        let index = Index::build(&reg(&[("hq", &root)], &[]));
        let mut w = Vec::new();
        let m = resolved(index.resolve(&storage.join("wt/src/a.rs"), &mut w));
        assert_eq!(m.via, Via::Storage);
        assert_eq!(m.worktree, Some(storage.join("wt")));
        assert_eq!(m.relpath, Path::new("src/a.rs"));
        let m = resolved(index.resolve(&storage, &mut w));
        assert_eq!((m.worktree, m.relpath), (None, PathBuf::new()));
    }

    #[test]
    fn former_storage_resolves_a_worktree_that_no_longer_exists() {
        let h = Locations {
            storage: None,
            former: vec![Former {
                root: "/d/ai".into(),
                storage: Some("/w/ai/.worktrees".into()),
                until: "2026-09-27".into(),
            }],
        };
        let index = Index::build(&reg(&[("tack", Path::new("/d/tack"))], &[("tack", h)]));
        let mut w = Vec::new();
        let m = resolved(index.resolve(Path::new("/w/ai/.worktrees/gone/f"), &mut w));
        assert_eq!((m.prefix.as_str(), m.via), ("tack", Via::FormerStorage));
        assert_eq!(m.worktree, Some(PathBuf::from("/w/ai/.worktrees/gone")));
        let m = resolved(index.resolve(Path::new("/d/ai/x"), &mut w));
        assert_eq!(m.via, Via::FormerRoot);
    }

    #[test]
    fn index_uses_the_record_when_the_live_link_dangles() {
        let base = tempfile::tempdir().unwrap();
        let base = base.path().canonicalize().unwrap();
        let root = base.join("hq");
        std::fs::create_dir(&root).unwrap();
        std::os::unix::fs::symlink(base.join("not-yet"), root.join(".worktrees")).unwrap();
        let h = Locations { storage: Some("/w/old/.worktrees".into()), former: vec![] };
        let index = Index::build(&reg(&[("hq", &root)], &[("hq", h)]));
        let mut w = Vec::new();
        let m = resolved(index.resolve(Path::new("/w/old/.worktrees/wt/f"), &mut w));
        assert_eq!(m.via, Via::Storage);
    }
```

- [ ] **Step 2: Run them to verify they fail.**
  - Run: `just test-one locations::`
  - Expected: compile errors, because `normalize`, `Index`, `Answer`, `Match`, and `Via` are missing.

- [ ] **Step 3: Implement normalization.**

```rust
/// Absolute, with the longest existing ancestor canonicalized and the rest appended
/// lexically (§4.1). `..` after a missing component cannot be resolved and is refused.
pub fn normalize(path: &Path, base: &Path) -> std::result::Result<PathBuf, String> {
    use std::path::Component;
    let absolute = if path.is_absolute() { path.to_path_buf() } else { base.join(path) };
    let mut out = PathBuf::from("/");
    let mut missing = false;
    for component in absolute.components() {
        match component {
            Component::RootDir | Component::Prefix(_) | Component::CurDir => {}
            Component::ParentDir if missing => {
                return Err(format!(
                    "{}: `..` follows a component that does not exist",
                    path.display()
                ));
            }
            // `out` is canonical while nothing is missing, so its lexical parent is real.
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(name) => {
                out.push(name);
                if !missing {
                    match out.canonicalize() {
                        Ok(real) => out = real,
                        Err(e) if matches!(
                            e.kind(),
                            std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                        ) => missing = true,
                        Err(e) => return Err(format!("{}: {e}", out.display())),
                    }
                }
            }
        }
    }
    Ok(out)
}
```

  Change the module's `use std::path::PathBuf;` to `use std::path::{Path, PathBuf};`.

- [ ] **Step 4: Implement the index.**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Via {
    Root,
    Storage,
    FormerRoot,
    FormerStorage,
    Git,
}

impl Via {
    fn current(self) -> bool {
        matches!(self, Via::Root | Via::Storage)
    }
    fn storage(self) -> bool {
        matches!(self, Via::Storage | Via::FormerStorage)
    }
}

#[derive(Debug)]
struct Location {
    prefix: String,
    via: Via,
    path: PathBuf,
}

#[derive(Debug, PartialEq)]
pub struct Match {
    pub prefix: String,
    pub via: Via,
    pub location: PathBuf,
    pub worktree: Option<PathBuf>,
    pub relpath: PathBuf,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Candidate {
    pub prefix: String,
    pub via: Via,
    pub location: String,
}

#[derive(Debug, PartialEq)]
pub enum Answer {
    Resolved(Match),
    Ambiguous(Vec<Candidate>),
    Unregistered,
}

/// Every registered project's locations on this host (§4.2), built once per call.
pub struct Index {
    locations: Vec<Location>,
    pub warnings: Vec<String>,
}

impl Index {
    pub fn build(registry: &crate::registry::Registry) -> Index {
        let mut locations = Vec::new();
        let mut warnings = Vec::new();
        let mut add = |prefix: &str, via: Via, path: &Path, warnings: &mut Vec<String>| {
            let path = normalize(path, Path::new("/")).unwrap_or_else(|problem| {
                warnings.push(format!("{prefix}: {problem}; matched as written"));
                path.to_path_buf()
            });
            locations.push(Location { prefix: prefix.into(), via, path });
        };
        for (prefix, root) in &registry.projects {
            add(prefix, Via::Root, root, &mut warnings);
            let history = registry.locations.get(prefix);
            let record = history.and_then(|h| h.storage.as_deref());
            match storage_state(root) {
                Ok(StorageState::Link(real)) => add(prefix, Via::Storage, &real, &mut warnings),
                Ok(_) => {
                    if let Some(record) = record {
                        add(prefix, Via::Storage, record, &mut warnings);
                    }
                }
                Err(e) => {
                    warnings.push(format!(
                        "{prefix}: cannot read {}/.worktrees ({e}); using its storage record",
                        root.display()
                    ));
                    if let Some(record) = record {
                        add(prefix, Via::Storage, record, &mut warnings);
                    }
                }
            }
            for former in history.map(|h| h.former.as_slice()).unwrap_or_default() {
                add(prefix, Via::FormerRoot, &former.root, &mut warnings);
                if let Some(storage) = &former.storage {
                    add(prefix, Via::FormerStorage, storage, &mut warnings);
                }
            }
        }
        Index { locations, warnings }
    }

    /// The project `path` (already normalized) belongs to (§4.3).
    pub fn resolve(&self, path: &Path, warnings: &mut Vec<String>) -> Answer {
        match self.pick(path) {
            Answer::Unregistered if path.exists() => {
                self.through_git(path, warnings).unwrap_or(Answer::Unregistered)
            }
            answer => answer,
        }
    }

    fn pick(&self, path: &Path) -> Answer {
        let found: Vec<&Location> =
            self.locations.iter().filter(|l| path.starts_with(&l.path)).collect();
        let Some(depth) = found.iter().map(|l| l.path.components().count()).max() else {
            return Answer::Unregistered;
        };
        let deepest: Vec<&Location> = found
            .into_iter()
            .filter(|l| l.path.components().count() == depth)
            .collect();
        let pool: Vec<&Location> = if deepest.iter().any(|l| l.via.current()) {
            deepest.into_iter().filter(|l| l.via.current()).collect()
        } else {
            deepest
        };
        let mut owners: Vec<&str> = pool.iter().map(|l| l.prefix.as_str()).collect();
        owners.sort();
        owners.dedup();
        if owners.len() > 1 {
            let mut candidates: Vec<Candidate> = pool
                .iter()
                .map(|l| Candidate {
                    prefix: l.prefix.clone(),
                    via: l.via,
                    location: l.path.display().to_string(),
                })
                .collect();
            candidates.sort_by(|a, b| (&a.prefix, a.via).cmp(&(&b.prefix, b.via)));
            return Answer::Ambiguous(candidates);
        }
        let best = pool.into_iter().min_by_key(|l| l.via).expect("a deepest location");
        let rest = path.strip_prefix(&best.path).expect("an ancestor").to_path_buf();
        let (worktree, relpath) = match (best.via.storage(), rest.components().next()) {
            (true, Some(first)) => (
                Some(best.path.join(first)),
                rest.strip_prefix(first).expect("its own first component").to_path_buf(),
            ),
            _ => (None, rest),
        };
        Answer::Resolved(Match {
            prefix: best.prefix.clone(),
            via: best.via,
            location: best.path.clone(),
            worktree,
            relpath,
        })
    }

    /// §4.3 step 5: map an existing path into its main checkout, keeping its offset in
    /// the worktree, and match again.
    fn through_git(&self, path: &Path, warnings: &mut Vec<String>) -> Option<Answer> {
        let dir = if path.is_dir() { path } else { path.parent()? };
        let output = match std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["rev-parse", "--path-format=absolute", "--show-toplevel", "--git-common-dir"])
            .stderr(std::process::Stdio::null())
            .output()
        {
            Ok(output) => output,
            Err(e) => {
                let warning = format!("git is unavailable ({e}); worktrees outside storage are not resolved");
                if !warnings.contains(&warning) {
                    warnings.push(warning);
                }
                return None;
            }
        };
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8(output.stdout).ok()?;
        let mut lines = text.lines();
        let top = Path::new(lines.next()?).canonicalize().ok()?;
        let common = PathBuf::from(lines.next()?);
        if common.file_name()? != ".git" {
            return None;
        }
        let main = common.parent()?.canonicalize().ok()?;
        if top == main {
            return None;
        }
        let offset = path.strip_prefix(&top).ok()?.to_path_buf();
        match self.pick(&main.join(&offset)) {
            Answer::Resolved(m) => Some(Answer::Resolved(Match {
                via: Via::Git,
                worktree: Some(top),
                relpath: offset,
                ..m
            })),
            Answer::Ambiguous(candidates) => Some(Answer::Ambiguous(candidates)),
            Answer::Unregistered => None,
        }
    }
}
```

`relpath` for a `git` match is relative to `T`, so it stays correct by the §3.2 rule
when the matched project is nested below `M`.

- [ ] **Step 5: Add the git-fallback unit test.** Append to `mod tests`:

```rust
    fn git(dir: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@e")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@e")
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
    }

    #[test]
    fn git_fallback_keeps_the_offset_into_a_nested_project() {
        let base = tempfile::tempdir().unwrap();
        let base = base.path().canonicalize().unwrap();
        let main = base.join("m");
        std::fs::create_dir_all(main.join("sub/nested")).unwrap();
        std::fs::write(main.join("sub/nested/f"), "").unwrap();
        git(&main, &["init", "-q"]);
        git(&main, &["add", "-A"]);
        git(&main, &["commit", "-qm", "init"]);
        let top = base.join("t");
        git(&main, &["worktree", "add", "-q", top.to_str().unwrap()]);
        std::fs::write(top.join("sub/nested/x"), "").unwrap();
        let mut w = Vec::new();
        for parent in [true, false] {
            let mut projects = vec![("nested", main.join("sub/nested"))];
            if parent {
                projects.push(("parent", main.clone()));
            }
            let pairs: Vec<(&str, &Path)> = projects.iter().map(|(p, r)| (*p, r.as_path())).collect();
            let index = Index::build(&reg(&pairs, &[]));
            let m = resolved(index.resolve(&top.join("sub/nested/x"), &mut w));
            assert_eq!(m.prefix, "nested", "parent={parent}");
            assert_eq!(m.via, Via::Git);
            assert_eq!(m.location, main.join("sub/nested"));
            assert_eq!(m.worktree, Some(top.clone()));
            assert_eq!(m.relpath, Path::new("sub/nested/x"));
        }
    }
```

  With `parent` registered, the path `T/sub/nested/x` does not match `M`, because `T`
  sits outside it. So the fallback runs, and the depth rule picks `nested`.

- [ ] **Step 6: Run the tests to verify they pass.**
  - Run: `just test-one locations::`
  - Expected: PASS (all tests in the module).

- [ ] **Step 7: Run clippy and the fast suite, then commit.**
  - Run: `just check`, then `just test-fast`
  - Expected: PASS. Fix any clippy findings in `src/locations.rs`.

```bash
git add src/locations.rs
tasks done <task-3-id> "path normalization and the location index with the git fallback"
git add tasks/
git commit -m "feat(locations): match paths against current and former locations"
```

---

### Task 4: `tasks resolve`

**Files:**
- Modify, in a locked ops worktree: ops `cli.toml`. Copy it to `tools/cli.toml`.
- Create: `src/commands/resolver.rs`. The name avoids the existing `crate::resolve`, which links specs and plans.
- Modify: `src/cli.rs`, `src/commands/mod.rs` (module and dispatch), `src/output.rs` (payload, pretty, warnings), `src/main.rs` (width-error exemption)
- Test: `tests/resolve.rs`

**Interfaces:**
- Consumes (Task 3): `normalize`, `Index::build`, `Index::resolve`, `Answer`, `Match`, `Candidate`, `Via`.
- Consumes (registry): `Registry::canonical_prefix`, `canonical_id`, and `project_root`; and `TaskId::parse` and `model::is_valid_prefix`.
- Produces:
  - `Output::Resolve(ResolveOut)`, where `ResolveOut { results: Vec<ResolveRow>, warnings: Vec<String> }`.
  - The JSON is the spec §3.2 contract.

- [ ] **Step 1: Land the vocabulary row in ops first.**
  1. File the ops piece:
     `tasks add "Add tasks resolve to cli.toml" --project ops -p 1 --size xs --complexity low --process direct --source tasks-7580d2`.
  2. Then: `cd <ops> && work-link --ensure .worktrees && git worktree add .worktrees/tasks-resolve-row && git worktree lock --reason "on WORK_ROOT storage (host: $(uname -n))" .worktrees/tasks-resolve-row`
  3. Run `just setup` there if the justfile defines it.
  4. Add after the `root` row:

     ```toml
     [[cli.tasks.commands]]
     path = ["resolve"]
     summary = "The project, canonical id, or live prefix for an id, prefix, or path, through renames and moves"
     args = [{ name = "inputs", value = "string", variadic = true }]
     options = [
       { names = ["--stdin"], value = "none" },
     ]
     ```

  5. Run `just test-one test_cli` in the ops worktree. Expected: PASS.
  6. Copy it: `cp <ops worktree>/cli.toml .worktrees/tasks-7580d2/tools/cli.toml`.

  Leave the ops change uncommitted. Task 6 publishes and commits it.

- [ ] **Step 2: Write the failing CLI tests.** Append to `tests/resolve.rs`:

```rust
fn resolve(env: &TestEnv, dir: &Path, inputs: &[&str]) -> Vec<serde_json::Value> {
    let mut args = vec!["resolve"];
    args.extend_from_slice(inputs);
    env.json(dir, &args)["results"].as_array().unwrap().clone()
}

#[test]
fn every_id_and_path_form_resolves_through_a_cutover() {
    let env = TestEnv::new();
    let base = tempfile::tempdir().unwrap();
    let base = base.path().canonicalize().unwrap();
    let (id, old_root, old_storage, root, _) = cutover(&env, &base);
    let hex = id.rsplit_once('-').unwrap().1;
    let gone = old_storage.join("gone/src/a.rs");
    let results = resolve(
        &env,
        &base,
        &[&id, "tack", old_root.join("tasks").to_str().unwrap(), gone.to_str().unwrap()],
    );
    assert_eq!(results[0]["id"], format!("hq-{hex}"));
    assert_eq!(results[0]["alias"], "tack");
    assert_eq!(results[0]["root"], root.to_str().unwrap());
    assert_eq!(results[1]["kind"], "prefix");
    assert_eq!(results[1]["prefix"], "hq");
    assert_eq!(results[2]["via"], "former_root");
    assert_eq!(results[2]["relpath"], "tasks");
    assert_eq!(results[3]["via"], "former_storage");
    assert_eq!(results[3]["worktree"], old_storage.join("gone").to_str().unwrap());
    assert_eq!(results[3]["relpath"], "src/a.rs");
    for r in &results {
        assert_eq!(r["status"], "resolved", "{r}");
        assert_eq!(r["prefix"], "hq", "{r}");
    }
}

/// The second host: its own paths, a synced checkout already renamed, then --adopt.
/// `storage_first` moves this host's storage before adopting. tack's runbook adopts
/// first, which leaves the new link dangling.
fn second_host(
    base: &Path,
    pre_register_target: bool,
    storage_first: bool,
) -> (TestEnv, String, PathBuf, PathBuf, serde_json::Value) {
    let env = TestEnv::new();
    let (old_root, old_storage) = linked_checkout(base, "tack");
    env.json(&old_root, &["init", "--prefix", "tack"]);
    let old_id = id_of(env.json(&old_root, &["add", "Task"]));
    // What sync delivers: the files renamed by the first host.
    let new_id = old_id.replacen("tack-", "hq-", 1);
    let task = old_root.join(format!("tasks/{old_id}.md"));
    let text = std::fs::read_to_string(&task).unwrap();
    std::fs::write(old_root.join(format!("tasks/{new_id}.md")), text.replacen(&old_id, &new_id, 1)).unwrap();
    std::fs::remove_file(task).unwrap();
    let config = old_root.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, text.replace("prefix = \"tack\"", "prefix = \"hq\"")).unwrap();
    let (root, _) = move_checkout(base, "tack", "hq", storage_first);
    if pre_register_target {
        env.json(&root, &["init", "--prefix", "hq", "--force"]);
        let path = env.home.path().join(".config/tasks/projects.toml");
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str("\n[[locations.hq.former]]\nroot = \"/elsewhere\"\nuntil = \"2026-01-01\"\n");
        std::fs::write(&path, text).unwrap();
    }
    let out = env.json(&root, &["rename", "tack", "hq", "--adopt"]);
    (env, old_id, old_root, old_storage, out)
}

#[test]
fn a_host_that_adopts_gets_the_same_answers_with_its_own_storage() {
    for (pre_register_target, storage_first) in [(false, true), (true, true), (false, false), (true, false)] {
        let base = tempfile::tempdir().unwrap();
        let base = base.path().canonicalize().unwrap();
        let (env, old_id, old_root, old_storage, out) =
            second_host(&base, pre_register_target, storage_first);
        let gone = old_storage.join("wt/f");
        let results = resolve(
            &env,
            &base,
            &[&old_id, old_root.join("x").to_str().unwrap(), gone.to_str().unwrap()],
        );
        assert_eq!(results[0]["id"], old_id.replacen("tack-", "hq-", 1));
        assert_eq!(results[1]["via"], "former_root");
        assert_eq!(results[2]["via"], "former_storage", "pre={pre_register_target}");
        let former = registry(&env)["locations"]["hq"]["former"].as_array().unwrap().clone();
        let roots: Vec<_> = former.iter().map(|f| f["root"].as_str().unwrap().to_string()).collect();
        if pre_register_target {
            assert!(roots.contains(&"/elsewhere".to_string()), "{roots:?}");
        }
        assert!(roots.contains(&old_root.to_str().unwrap().to_string()), "{roots:?}");
        let old = former
            .iter()
            .find(|f| f["root"].as_str() == old_root.to_str())
            .unwrap();
        assert_eq!(old["storage"].as_str(), old_storage.to_str(), "the old storage lives under former");
        let hq = &registry(&env)["locations"]["hq"];
        if storage_first {
            assert_eq!(hq["storage"].as_str(), base.join("work/hq/.worktrees").to_str());
        } else {
            assert!(hq.get("storage").is_none(), "a dangling link leaves no current record: {hq}");
            let warnings = out["warnings"].as_array().unwrap();
            assert!(
                warnings.iter().any(|w| w.as_str().unwrap().contains("points at missing storage")),
                "{warnings:?}"
            );
        }
    }
}

#[test]
fn adoption_without_a_storage_record_keeps_the_root_and_warns() {
    let env = TestEnv::new();
    let base = tempfile::tempdir().unwrap();
    let base = base.path().canonicalize().unwrap();
    // A synced checkout already renamed to hq, and a registry that knew tack only at an
    // old root that is gone, with no storage record: the host skipped the pre-move step.
    let (root, _) = linked_checkout(&base, "hq");
    std::fs::create_dir_all(root.join("tasks")).unwrap();
    std::fs::write(root.join("tasks/.config.toml"), "prefix = \"hq\"\n").unwrap();
    let gone = base.join("tack");
    let path = env.home.path().join(".config/tasks/projects.toml");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, format!("[projects]\ntack = {:?}\n", gone.to_str().unwrap())).unwrap();
    let out = env.json(&root, &["rename", "tack", "hq", "--adopt"]);
    let warnings = out["warnings"].as_array().unwrap();
    assert!(
        warnings.iter().any(|w| w.as_str().unwrap().contains("no known worktree storage")),
        "{warnings:?}"
    );
    let former = &registry(&env)["locations"]["hq"]["former"][0];
    assert_eq!(former["root"].as_str().unwrap(), gone.to_str().unwrap());
    assert!(former.get("storage").is_none());
}

#[test]
fn stdin_batch_keeps_order_and_reports_each_item() {
    let mut env = TestEnv::new();
    let dir = env.init("abcdef");
    let out = env
        .cmd(&dir)
        .args(["resolve", "--stdin"])
        .write_stdin("abcdef\r\n4f2a9c\n\nzz-123456\nbeef00\n/nowhere/at/all\nnot an id\n")
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let rows = v["results"].as_array().unwrap();
    let got: Vec<(&str, &str, &str)> = rows
        .iter()
        .map(|r| (r["input"].as_str().unwrap(), r["kind"].as_str().unwrap(), r["status"].as_str().unwrap()))
        .collect();
    assert_eq!(
        got,
        [
            ("abcdef", "prefix", "resolved"),
            ("4f2a9c", "invalid", "invalid"),
            ("", "invalid", "invalid"),
            ("zz-123456", "id", "unregistered"),
            ("beef00", "prefix", "unregistered"),
            ("/nowhere/at/all", "path", "unregistered"),
            ("not an id", "invalid", "invalid"),
        ]
    );
    assert_eq!(rows[3]["prefix"], "zz");
}

#[test]
fn a_path_in_an_outside_worktree_resolves_through_git_with_its_offset() {
    let env = TestEnv::new();
    let base = tempfile::tempdir().unwrap();
    let base = base.path().canonicalize().unwrap();
    let main = base.join("m");
    std::fs::create_dir_all(main.join("sub/nested")).unwrap();
    env.json(&main.join("sub/nested"), &["init", "--prefix", "nested"]);
    env.json(&main, &["init", "--prefix", "parent"]);
    git(&main, &["init", "-q"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-qm", "init"]);
    let top = base.join("t");
    git(&main, &["worktree", "add", "-q", top.to_str().unwrap()]);
    std::fs::write(top.join("sub/nested/x"), "").unwrap();
    let r = &resolve(&env, &base, &[top.join("sub/nested/x").to_str().unwrap()])[0];
    assert_eq!(r["prefix"], "nested");
    assert_eq!(r["via"], "git");
    assert_eq!(r["location"], main.join("sub/nested").to_str().unwrap());
    assert_eq!(r["worktree"], top.to_str().unwrap());
    assert_eq!(r["relpath"], "sub/nested/x");
}

#[test]
fn a_malformed_locations_table_is_a_config_error() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let path = env.home.path().join(".config/tasks/projects.toml");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("\n[locations.gone]\nstorage = \"/s\"\n");
    std::fs::write(&path, text).unwrap();
    let out = env.cmd(&dir).args(["resolve", "sci"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["error"]["kind"], "config");
}

fn git(dir: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@e")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@e")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {out:?}");
}
```

  - If `init` of `parent` at `M` refuses because a project already exists below it,
    record that in a note and register `parent` with a hand-written registry line, as
    `adoption_without_a_storage_record…` does.
  - If the hand-written `tasks/.config.toml` in `adoption_without_a_storage_record…`
    lacks a key the loader requires, write the file `tasks init --prefix hq` produces
    in a scratch directory instead, and note the change.

- [ ] **Step 3: Run them to verify they fail.**
  - Run: `just test-one --test resolve`
  - Expected: the new tests fail with a clap usage error (exit 2), because `resolve` is
    not a subcommand. Task 2's tests still pass.

- [ ] **Step 4: Add the command to `src/cli.rs`, after `Root`.**

```rust
    /// The project, canonical id, or live prefix for an id, prefix, or path, through
    /// renames and moves.
    Resolve {
        /// Ids, prefixes, or paths.
        #[arg(required_unless_present = "stdin", conflicts_with = "stdin")]
        inputs: Vec<String>,
        /// Read one input per line from stdin.
        #[arg(long)]
        stdin: bool,
    },
```

  - **`src/main.rs`:** add `| cli::Command::Resolve { .. }` to the width-error exemption
    list after `Root`.
  - **`src/commands/mod.rs`:** add `mod resolver;` beside the other modules, and add the
    dispatch arm after `Root`:
    `Command::Resolve { inputs, stdin } => resolver::run(inputs, stdin, dir),`

- [ ] **Step 5: Add the payload to `src/output.rs`.**

```rust
#[derive(Serialize)]
pub struct ResolveOut {
    pub results: Vec<ResolveRow>,
    pub warnings: Vec<String>,
}

/// One input's answer (spec §3.2). Fields that do not apply are omitted.
#[derive(Serialize, Default)]
pub struct ResolveRow {
    pub input: String,
    pub kind: &'static str,
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via: Option<crate::locations::Via>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relpath: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<crate::locations::Candidate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}
```

  - Add `Resolve(ResolveOut),` to `Output` after `Root(RootOut),`.
  - Add `Output::Resolve(o) => o.warnings.clone(),` to `warnings_of`.
  - In `pretty`, add:

```rust
        Output::Resolve(o) => o
            .results
            .iter()
            .map(|r| {
                let answer = r.id.as_deref().or(r.prefix.as_deref()).unwrap_or("");
                let via = r
                    .via
                    .map(|v| serde_json::to_value(v).unwrap().as_str().unwrap().to_string())
                    .unwrap_or_default();
                [r.input.as_str(), r.status, answer, via.as_str()]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join("  ")
            })
            .collect::<Vec<_>>()
            .join("\n"),
```

- [ ] **Step 6: Implement `src/commands/resolver.rs`.**

```rust
//! `tasks resolve` (docs/specs/2026-10-06-project-resolution-design.md §3, §4).
use crate::error::Result;
use crate::locations::{Answer, Index, normalize};
use crate::model::{TaskId, is_valid_prefix};
use crate::output::{Output, ResolveOut, ResolveRow};
use crate::registry::Registry;
use std::io::BufRead;
use std::path::{Path, PathBuf};

pub fn run(inputs: Vec<String>, stdin: bool, dir: Option<&Path>) -> Result<Output> {
    let registry = Registry::load()?;
    let base = super::start_dir(dir)?;
    let inputs = if stdin {
        std::io::stdin()
            .lock()
            .lines()
            .map(|line| line.map(|l| l.trim_end_matches('\r').to_string()))
            .collect::<std::io::Result<Vec<_>>>()?
    } else {
        inputs
    };
    let index = Index::build(&registry);
    let mut warnings = index.warnings.clone();
    let results = inputs
        .into_iter()
        .map(|input| row(&registry, &index, &base, input, &mut warnings))
        .collect();
    Ok(Output::Resolve(ResolveOut { results, warnings }))
}

/// §3.1: path, then id, then prefix; never the bare-hex shorthand.
fn row(
    registry: &Registry,
    index: &Index,
    base: &Path,
    input: String,
    warnings: &mut Vec<String>,
) -> ResolveRow {
    if input.contains('/') || input.starts_with('.') || input.starts_with('~') {
        return path_row(registry, index, base, input, warnings);
    }
    if let Ok(id) = TaskId::parse(&input) {
        let live = registry.canonical_prefix(&id.prefix).to_string();
        return match registry.project_root(&live) {
            Some(root) => ResolveRow {
                id: Some(registry.canonical_id(&id).to_string()),
                alias: (live != id.prefix).then(|| id.prefix.clone()),
                prefix: Some(live),
                root: Some(root.display().to_string()),
                ..row_of(input, "id", "resolved")
            },
            None => ResolveRow {
                prefix: Some(id.prefix.clone()),
                ..row_of(input, "id", "unregistered")
            },
        };
    }
    if is_valid_prefix(&input) {
        let live = registry.canonical_prefix(&input).to_string();
        return match registry.project_root(&live) {
            Some(root) => ResolveRow {
                alias: (live != input).then(|| input.clone()),
                prefix: Some(live),
                root: Some(root.display().to_string()),
                ..row_of(input, "prefix", "resolved")
            },
            None => ResolveRow {
                prefix: Some(input.clone()),
                ..row_of(input, "prefix", "unregistered")
            },
        };
    }
    ResolveRow {
        detail: Some("expected an id <prefix>-<hex6>, a prefix, or a path".into()),
        ..row_of(input, "invalid", "invalid")
    }
}

fn path_row(
    registry: &Registry,
    index: &Index,
    base: &Path,
    input: String,
    warnings: &mut Vec<String>,
) -> ResolveRow {
    let expanded = match expand_home(&input) {
        Ok(path) => path,
        Err(detail) => return ResolveRow { detail: Some(detail), ..row_of(input, "path", "invalid") },
    };
    let path = match normalize(&expanded, base) {
        Ok(path) => path,
        Err(detail) => return ResolveRow { detail: Some(detail), ..row_of(input, "path", "invalid") },
    };
    match index.resolve(&path, warnings) {
        Answer::Resolved(m) => ResolveRow {
            root: registry.project_root(&m.prefix).map(|r| r.display().to_string()),
            prefix: Some(m.prefix),
            via: Some(m.via),
            location: Some(m.location.display().to_string()),
            worktree: m.worktree.map(|w| w.display().to_string()),
            relpath: Some(m.relpath.display().to_string()),
            ..row_of(input, "path", "resolved")
        },
        Answer::Ambiguous(candidates) => ResolveRow { candidates, ..row_of(input, "path", "ambiguous") },
        Answer::Unregistered => row_of(input, "path", "unregistered"),
    }
}

fn expand_home(input: &str) -> std::result::Result<PathBuf, String> {
    let Some(rest) = input.strip_prefix('~') else {
        return Ok(PathBuf::from(input));
    };
    if !(rest.is_empty() || rest.starts_with('/')) {
        return Err(format!("{input}: only ~ and ~/ are expanded"));
    }
    let home = std::env::var_os("HOME").ok_or_else(|| format!("{input}: HOME is not set"))?;
    Ok(PathBuf::from(home).join(rest.trim_start_matches('/')))
}

fn row_of(input: String, kind: &'static str, status: &'static str) -> ResolveRow {
    ResolveRow { input, kind, status, ..ResolveRow::default() }
}
```

- [ ] **Step 7: Run the tests.**
  - Run: `just test-one --test resolve`, then `just test-one surface`
  - Expected: PASS.
  - If the surface test reports a difference, fix the ops row, run its `test_cli`, and
    copy it again. Never edit `tools/cli.toml` alone.

- [ ] **Step 8: Run the gate pieces, then commit.**
  - Run: `just check`, then `just test-fast`
  - Expected: PASS.

```bash
git add src/cli.rs src/main.rs src/commands/mod.rs src/commands/resolver.rs src/output.rs tools/cli.toml tests/resolve.rs
tasks done <task-4-id> "tasks resolve: ids, prefixes and paths through renames and moves"
git add tasks/
git commit -m "feat(resolve): add tasks resolve"
```

---

### Task 5: Documentation and the consumers' contract

**Files:**
- Modify:
  - `README.md` (the prefix rename section; a new "Resolving ids and paths" section)
  - `skills/tasks/SKILL.md` ("Prefix renames and recovery")
  - `AGENTS.md` (Layout: `locations.rs`, `commands/resolver.rs`)
  - `docs/specs/2026-10-06-project-resolution-design.md` (status line)
- Test: `tasks check` and `just check`. This task is documentation only.

- [ ] **Step 1: README.**
  - Under the prefix rename section, add a subsection "Location history". It covers:
    - what `[locations]` records and when (spec §2.1, the table);
    - the pre-move step on every other host, `tasks -C <root> init --prefix <p> --force`;
    - the hand-edit form for backfilling a former root, with the TOML from spec §6 using
      `<Dropbox>` and `<WORK_ROOT>` placeholders, and the note that paths must be
      absolute;
    - that an older `tasks` binary writing the registry drops the table.
  - Add a section "Resolving ids and paths" with:
    - the command;
    - the classification order;
    - one example of each status in JSON, from spec §3.2;
    - the field rule for `worktree` and `relpath`;
    - that only registry problems exit 1.

- [ ] **Step 2: Skill.**
  - In `skills/tasks/SKILL.md`, under "Prefix renames and recovery", add a paragraph:
    - `tasks resolve <id|prefix|path>...` (or `--stdin`) is how a tool maps any id or
      path to its project through renames and moves;
    - never parse `projects.toml` by hand;
    - before moving a checkout, run `tasks init --prefix <p> --force` there on every host
      so its storage is recorded.
  - Keep it to five lines.

- [ ] **Step 3: AGENTS.md Layout.**
  - Add `locations.rs` (registry location history, path normalization and the location
    index behind `resolve`) to the `src/` list.
  - Add `resolver.rs` under `src/commands/` in the existing style.

- [ ] **Step 4: Spec status.**
  - Set the status line to `Status: implemented 2026-MM-DD (tasks-7580d2)`, using the
    landing date.

- [ ] **Step 5: Record the contract on the consumers.**

```bash
tasks note tasks-9b0a2e "tasks-7580d2 landed src/locations.rs: where's link step takes registered checkouts from Index and adds checkout as a location kind there; where never consults former roots (spec 2026-10-06-project-resolution §5)"
tasks note obs-ff4e76 "mechanism landed: tasks resolve --stdin (tasks docs/specs/2026-10-06-project-resolution-design.md §3); one call per run over distinct ids and paths"
tasks note flows-44890e "mechanism landed: tasks resolve maps alias ids and prefixes to the live prefix (tasks spec 2026-10-06-project-resolution §3.3)"
tasks note tack-8b7a28 "tasks-7580d2 landed; runbook adds tasks -C <root> init --prefix tack --force on every host before the move (tasks spec 2026-10-06-project-resolution §2.3)"
```

- [ ] **Step 6: Check and commit.**
  - Run: `tasks check`, then `just check`
  - Expected: clean.

```bash
git add README.md skills/tasks/SKILL.md AGENTS.md docs/specs/2026-10-06-project-resolution-design.md
tasks done <task-5-id> "documented location history and resolve; consumers noted"
git add tasks/
git commit -m "docs(tasks): document location history and tasks resolve"
```

---

### Task 6: Integrate, publish the vocabulary, and roll out on this host

This task changes live host state: the installed binary, the registry, and vendored
copies in other repositories. Each step that changes host state is announced before it
runs. The `ai` backfill edits the user's registry, so ask before writing it.

- [ ] **Step 1: Final review and integration.**
  1. Run the whole-branch review.
  2. Run `just gate` in `.worktrees/tasks-7580d2`.
  3. Merge into `main` locally. This is a personal checkout, so a local merge is the
     default.

- [ ] **Step 2: Publish the vocabulary.**
  1. From the ops worktree, run `just vendor-cli --force`.
  2. Run ops `just test-one test_cli`, `just test-fast`, and `just check`.
  3. Commit the ops source as `feat: add tasks resolve to cli inventory`, and commit
     each clean vendored copy in its owning repository.
  4. Close the ops task filed in Task 4.

  Publish before committing the ops source, because ops pre-commit refuses while
  copies are stale.

- [ ] **Step 3: Install.**
  1. Run `cargo install --path .` from the main checkout.
  2. Run `tasks resolve tasks`. Expected: `resolved`.

- [ ] **Step 4: Record storage for live projects.**
  1. In the tack checkout, run `tasks init --prefix tack --force`.
  2. Read back `[locations.tack]`. Expected: `storage` is tack's storage path.
  3. Run `git status --porcelain` in the tack checkout. Expected: empty. A re-init that
     rewrote `tasks/.config.toml` would trip tack's `rename-cutover` clean-tree
     precondition. If it is not empty, stop and report the diff; do not commit it.

- [ ] **Step 5: Backfill `ai` (ask first).**
  1. Show the user the exact `[[locations.tack.former]]` entry from spec §6, with this
     host's paths.
  2. On a yes, add it to the registry.
  3. Then run `tasks resolve ai ~/d/ai/x <WORK_ROOT>/ai/.worktrees/w/f`. Expected: all
     three resolve to `tack`.
- [ ] **Step 6: Hand the other host over, explicitly.** This session cannot reach the
  other host. Its steps are the user's, in this order, because an older binary drops
  `[locations]` on any registry write (spec §2), and the pre-move record must exist
  before the synced move arrives (spec §2.3):
  1. Install the current binary there: `cargo install --path .` in its synced tasks
     checkout, once `main` carries this work.
  2. In its tack checkout, run `tasks init --prefix tack --force`, and check that
     `git status --porcelain` is empty.
  3. Hand-add its `ai` former root, with that host's own paths, per the README.

  Add `tasks note tack-8b7a28 "other host: install current tasks, then init --prefix
  tack --force in the tack checkout, before the cutover's move (tasks-7580d2 Task 6)"`.
  Then park nothing: these steps belong to the cutover's runbook, which tack owns.

- [ ] **Step 7: Close.**
  1. Remove the ops worktree after its branch merged and `tt-report` ran.
  2. Remove the tasks worktree the same way.
  3. Run `tasks done tasks-7580d2 "<what landed>"`. The close message names the other
     host's three steps from Step 6 as the user's, install first.
