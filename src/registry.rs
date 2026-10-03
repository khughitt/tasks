use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::repo::atomic_write;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub projects: BTreeMap<String, PathBuf>,
    /// Retired prefix -> the live prefix it now resolves to. Never chains: every target is
    /// a live prefix, enforced on load, so resolution is always one hop.
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
    /// Group name -> member prefixes, each a live prefix, sorted and distinct
    /// (docs/specs/2026-10-03-lanes-needs-groups-design.md §6). Host-local like the rest
    /// of the registry. Not written when empty, so a registry without groups keeps its
    /// shape on save.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub groups: BTreeMap<String, Vec<String>>,
}

/// What `unregister` removed with the project.
#[derive(Debug, PartialEq)]
pub struct Unregistered {
    pub root: PathBuf,
    /// Retired prefixes that resolved to the project.
    pub aliases: Vec<String>,
    /// Groups the project was the last member of, deleted with it.
    pub emptied_groups: Vec<String>,
}

impl Registry {
    pub fn path() -> Result<PathBuf> {
        if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
            return Ok(PathBuf::from(config_home).join("tasks/projects.toml"));
        }
        if let Some(home) = std::env::var_os("HOME") {
            return Ok(PathBuf::from(home).join(".config/tasks/projects.toml"));
        }
        Err(Error::Config(
            "neither XDG_CONFIG_HOME nor HOME is set".into(),
        ))
    }

    /// Serialize the full read-modify-write; atomic replacement alone can lose updates.
    pub fn lock() -> Result<crate::claims::MutationLock> {
        crate::claims::MutationLock::acquire_at(&Self::path()?.with_file_name("projects.lock"))
    }

    pub fn load() -> Result<Registry> {
        Self::load_from(&Self::path()?)
    }

    pub fn load_from(path: &Path) -> Result<Registry> {
        // Only an absent registry is empty. `exists()` is false when an ancestor is
        // unreadable too, which would read a permission error as "no projects".
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Registry::default());
            }
            Err(error) => return Err(error.into()),
        };
        let mut registry: Registry = toml::from_str(&text)
            .map_err(|error| Error::Config(format!("{}: {error}", path.display())))?;
        for (prefix, root) in &mut registry.projects {
            if let Some(rest) = root.to_str().and_then(|root| root.strip_prefix("~/")) {
                let home = std::env::var_os("HOME").ok_or_else(|| {
                    Error::Config(format!("project {prefix} uses ~ but HOME is not set"))
                })?;
                *root = PathBuf::from(home).join(rest);
            }
        }
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
        for (name, members) in &registry.groups {
            if let Some(problem) = registry.group_name_problem(name) {
                return Err(Error::Config(format!("{}: {problem}", path.display())));
            }
            if members.is_empty() {
                return Err(Error::Config(format!(
                    "{}: group {name:?} has no members",
                    path.display()
                )));
            }
            if let Some(member) = members
                .iter()
                .find(|member| !registry.projects.contains_key(*member))
            {
                return Err(Error::Config(format!(
                    "{}: group {name:?} names {member:?}, which is not a registered project; \
                     edit [groups] in this file",
                    path.display()
                )));
            }
        }
        Ok(registry)
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::path()?)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        atomic_write(
            path,
            toml::to_string(self)
                .expect("registry serializes")
                .as_bytes(),
        )
    }

    /// Claims `prefix` for `root`. A prefix already pointing somewhere else is a conflict,
    /// never a silent takeover; the message names both ways out.
    pub fn register(&mut self, prefix: &str, root: &Path) -> Result<()> {
        self.refuse_group_name(prefix)?;
        if let Some(target) = self.aliases.get(prefix) {
            return Err(Error::Config(format!(
                "prefix {prefix:?} is retired; it resolves to {target:?}"
            )));
        }
        if let Some(existing) = self.projects.get(prefix)
            && existing != root
        {
            return Err(Error::Config(format!(
                "prefix {prefix:?} is already registered to {}; re-point it with \
                 `tasks init --prefix {prefix} --force`, or drop it with \
                 `tasks unregister {prefix}`",
                existing.display()
            )));
        }
        self.projects.insert(prefix.into(), root.into());
        Ok(())
    }

    /// Points `prefix` at `root` whatever it pointed at before, returning the displaced
    /// root when that changed anything. The deliberate override behind `init --force`.
    pub fn repoint(&mut self, prefix: &str, root: &Path) -> Result<Option<PathBuf>> {
        self.refuse_group_name(prefix)?;
        if let Some(target) = self.aliases.get(prefix) {
            return Err(Error::Config(format!(
                "prefix {prefix:?} is retired; it resolves to {target:?}"
            )));
        }
        Ok(match self.projects.insert(prefix.into(), root.into()) {
            Some(previous) if previous != root => Some(previous),
            _ => None,
        })
    }

    /// Removes a project, every alias that targeted it, and its membership in every group.
    /// A group it leaves empty is deleted with it and reported. Leaving an alias or a
    /// member behind would break the load invariants and fail every later command. Once
    /// the project is gone its ids cannot resolve anyway, alias or not.
    pub fn unregister(&mut self, prefix: &str) -> Result<Unregistered> {
        if let Some(target) = self.aliases.get(prefix) {
            return Err(Error::Config(format!(
                "{prefix:?} is a retired prefix of {target:?}; unregister {target:?} to remove the project"
            )));
        }
        let root = self
            .projects
            .remove(prefix)
            .ok_or_else(|| Error::Config(format!("no project registered as {prefix:?}")))?;
        let aliases: Vec<String> = self
            .aliases
            .iter()
            .filter(|(_, target)| target.as_str() == prefix)
            .map(|(alias, _)| alias.clone())
            .collect();
        for alias in &aliases {
            self.aliases.remove(alias);
        }
        let mut emptied_groups = Vec::new();
        self.groups.retain(|name, members| {
            members.retain(|member| member != prefix);
            if members.is_empty() {
                emptied_groups.push(name.clone());
            }
            !members.is_empty()
        });
        Ok(Unregistered {
            root,
            aliases,
            emptied_groups,
        })
    }

    /// Move the live key and flatten every retired name directly to the new key.
    pub fn rename(&mut self, source: &str, target: &str) -> Result<()> {
        if !crate::model::is_valid_prefix(target) || self.is_taken(target) {
            return Err(Error::Config(format!(
                "target prefix {target:?} is invalid or taken"
            )));
        }
        let root = self
            .projects
            .remove(source)
            .ok_or_else(|| Error::Config(format!("no project registered as {source:?}")))?;
        self.projects.insert(target.into(), root);
        for live in self.aliases.values_mut() {
            if live == source {
                *live = target.into();
            }
        }
        self.retarget_groups(source, target);
        self.aliases.insert(source.into(), target.into());
        Ok(())
    }

    /// Adopt a checkout that already has `target` in its synced files.
    pub fn adopt(&mut self, source: &str, target: &str, root: &Path) -> Result<()> {
        if source == target || !crate::model::is_valid_prefix(target) {
            return Err(Error::Config(format!("invalid adoption target {target:?}")));
        }
        if !self.projects.contains_key(source) {
            return Err(Error::Config(format!(
                "no project registered as {source:?}"
            )));
        }
        if self.aliases.contains_key(target) {
            return Err(Error::Config(format!(
                "target prefix {target:?} is retired"
            )));
        }
        if self.groups.contains_key(target) {
            return Err(Error::Config(format!(
                "target prefix {target:?} is the name of a group; remove it with \
                 `tasks group rm {target}` first"
            )));
        }
        let root = crate::rename::root_identity(root)?;
        for (prefix, registered) in &self.projects {
            if prefix == source {
                continue;
            }
            let same_root = crate::rename::root_identity(registered)? == root;
            if prefix == target && !same_root || prefix != target && same_root {
                return Err(Error::Config(format!(
                    "project {prefix:?} is registered at conflicting root {}",
                    registered.display()
                )));
            }
        }
        if self.projects.contains_key(target) {
            self.projects.remove(source);
            for live in self.aliases.values_mut() {
                if live == source {
                    *live = target.into();
                }
            }
            self.retarget_groups(source, target);
            self.aliases.insert(source.into(), target.into());
        } else {
            self.rename(source, target)?;
        }
        self.repoint(target, &root)?;
        Ok(())
    }

    pub fn project_root(&self, prefix: &str) -> Option<&Path> {
        self.projects.get(prefix).map(PathBuf::as_path)
    }

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

    /// Whether a prefix may be claimed. A live prefix, a retired one and a group's name
    /// are all taken, because a name must never mean two things.
    pub fn is_taken(&self, prefix: &str) -> bool {
        self.projects.contains_key(prefix)
            || self.aliases.contains_key(prefix)
            || self.groups.contains_key(prefix)
    }

    /// Why `name` cannot name a group, if anything. The grammar is checked first. Then the
    /// two namespaces a group name shares: a live prefix and a retired one. `load` reports
    /// a problem as `config`.
    fn group_name_problem(&self, name: &str) -> Option<String> {
        if !is_valid_group_name(name) {
            return Some(format!(
                "group name {name:?} must use lowercase letters, digits, and -, \
                 not starting with -"
            ));
        }
        if self.projects.contains_key(name) {
            return Some(format!("group name {name:?} is a registered prefix"));
        }
        if let Some(target) = self.aliases.get(name) {
            return Some(format!(
                "group name {name:?} is a retired prefix of {target:?}"
            ));
        }
        None
    }

    /// Creates or replaces group `name`. A retired prefix resolves to its live one, and
    /// each member is stored once, in prefix order. Returns the stored members.
    pub fn set_group(&mut self, name: &str, members: &[String]) -> Result<Vec<String>> {
        if let Some(problem) = self.group_name_problem(name) {
            return Err(Error::Validation(problem));
        }
        if members.is_empty() {
            return Err(Error::Validation(format!(
                "group {name:?} needs at least one member"
            )));
        }
        let mut resolved = BTreeSet::new();
        for member in members {
            let live = self.canonical_prefix(member);
            if !self.projects.contains_key(live) {
                return Err(Error::Config(format!(
                    "no project registered as {member:?}"
                )));
            }
            resolved.insert(live.to_string());
        }
        let stored: Vec<String> = resolved.into_iter().collect();
        self.groups.insert(name.into(), stored.clone());
        Ok(stored)
    }

    /// Deletes group `name`, returning the members it had. Its projects stay registered.
    pub fn remove_group(&mut self, name: &str) -> Result<Vec<String>> {
        self.groups.remove(name).ok_or_else(|| unknown_group(name))
    }

    /// `init` and `register` never claim a group's name as a prefix.
    fn refuse_group_name(&self, prefix: &str) -> Result<()> {
        if self.groups.contains_key(prefix) {
            return Err(Error::Config(format!(
                "prefix {prefix:?} is the name of a group; remove it with \
                 `tasks group rm {prefix}` or choose another prefix"
            )));
        }
        Ok(())
    }

    /// Points every group's `from` member at `to`, keeping each member once and in order.
    fn retarget_groups(&mut self, from: &str, to: &str) {
        for members in self.groups.values_mut() {
            for member in members.iter_mut() {
                if member.as_str() == from {
                    *member = to.into();
                }
            }
            members.sort();
            members.dedup();
        }
    }
}

fn unknown_group(name: &str) -> Error {
    Error::UnknownGroup(format!(
        "no group named {name:?}; `tasks groups` lists them"
    ))
}

/// The need-name grammar: non-empty, lowercase ASCII letters, digits, and `-`, not
/// starting with `-`.
fn is_valid_group_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TaskId;

    #[test]
    fn adopt_flattens_aliases_with_free_or_pre_registered_target() {
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("new");
        std::fs::create_dir(&root).unwrap();
        for partial_init in [false, true] {
            let mut registry = Registry::default();
            registry
                .register("old", &home.path().join("missing"))
                .unwrap();
            registry.aliases.insert("older".into(), "old".into());
            if partial_init {
                registry.register("new", &root.join(".")).unwrap();
            }
            registry.adopt("old", "new", &root).unwrap();
            assert_eq!(registry.projects.len(), 1);
            assert_eq!(registry.project_root("new"), Some(root.as_path()));
            assert!(registry.project_root("old").is_none());
            assert_eq!(registry.canonical_prefix("older"), "new");
            assert_eq!(registry.canonical_prefix("old"), "new");
        }
    }

    #[test]
    fn adopt_refuses_foreign_target_or_duplicate_live_root_without_mutation() {
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("new");
        std::fs::create_dir(&root).unwrap();
        for (key, path) in [
            ("new", home.path().join("foreign")),
            ("third", root.clone()),
        ] {
            let mut registry = Registry::default();
            registry
                .register("old", &home.path().join("missing"))
                .unwrap();
            registry.register(key, &path).unwrap();
            assert!(registry.adopt("old", "new", &root).is_err());
            assert!(registry.project_root("old").is_some());
            assert!(registry.aliases.is_empty());
        }
    }

    #[test]
    fn roundtrip_and_conflict() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("tasks/projects.toml");
        let mut r = Registry::load_from(&path).unwrap();
        assert!(r.projects.is_empty());
        r.register("sci", std::path::Path::new("/tmp/sci")).unwrap();
        r.register("sci", std::path::Path::new("/tmp/sci")).unwrap();
        assert!(
            r.register("sci", std::path::Path::new("/tmp/other"))
                .is_err()
        );
        r.save_to(&path).unwrap();
        let r2 = Registry::load_from(&path).unwrap();
        assert_eq!(
            r2.project_root("sci").unwrap(),
            std::path::Path::new("/tmp/sci")
        );
        assert!(r2.project_root("nope").is_none());
    }

    #[test]
    fn repoint_replaces_and_reports_the_displaced_root() {
        let mut r = Registry::default();
        assert_eq!(r.repoint("sci", Path::new("/tmp/a")).unwrap(), None);
        assert_eq!(
            r.repoint("sci", Path::new("/tmp/b")).unwrap(),
            Some(PathBuf::from("/tmp/a"))
        );
        assert_eq!(r.project_root("sci").unwrap(), Path::new("/tmp/b"));
        assert_eq!(
            r.repoint("sci", Path::new("/tmp/b")).unwrap(),
            None,
            "re-pointing at the same root displaces nothing"
        );
    }

    #[test]
    fn unregister_removes_once_and_then_reports_the_prefix_is_absent() {
        let mut r = Registry::default();
        r.register("sci", Path::new("/tmp/a")).unwrap();
        assert_eq!(
            r.unregister("sci").unwrap(),
            Unregistered {
                root: PathBuf::from("/tmp/a"),
                aliases: Vec::new(),
                emptied_groups: Vec::new(),
            }
        );
        assert!(r.project_root("sci").is_none());
        assert_eq!(r.unregister("sci").unwrap_err().kind(), "config");
        r.register("sci", Path::new("/tmp/c")).unwrap();
        assert_eq!(
            r.project_root("sci").unwrap(),
            Path::new("/tmp/c"),
            "the prefix is free again after removal"
        );
    }

    #[test]
    fn expands_leading_tilde_on_load() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("projects.toml");
        std::fs::write(&path, "[projects]\nsci = \"~/d/science\"\n").unwrap();
        unsafe {
            std::env::set_var("HOME", home.path());
        }
        let r = Registry::load_from(&path).unwrap();
        assert_eq!(
            r.project_root("sci").unwrap(),
            home.path().join("d/science")
        );
    }

    #[test]
    fn aliases_canonicalize_and_reserve_names() {
        let mut r = Registry::default();
        r.register("dots", Path::new("/tmp/dotfiles")).unwrap();
        r.aliases.insert("dot".into(), "dots".into());

        assert_eq!(r.canonical_prefix("dot"), "dots");
        assert_eq!(r.canonical_prefix("dots"), "dots");
        assert_eq!(
            r.canonical_prefix("nope"),
            "nope",
            "an unknown prefix is left alone"
        );

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

        std::fs::write(
            &path,
            "[projects]\ndots = \"/tmp/d\"\n\n[aliases]\ndot = \"gone\"\n",
        )
        .unwrap();
        let error = Registry::load_from(&path).unwrap_err();
        assert_eq!(error.kind(), "config");
        assert!(error.to_string().contains("dot"), "{error}");

        std::fs::write(
            &path,
            "[projects]\ndots = \"/tmp/d\"\n\n[aliases]\ndots = \"dots\"\n",
        )
        .unwrap();
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

    #[test]
    fn groups_round_trip_and_an_empty_table_is_not_written() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("projects.toml");
        let mut r = Registry::default();
        r.register("sci", Path::new("/tmp/a")).unwrap();
        r.save_to(&path).unwrap();
        assert!(
            !std::fs::read_to_string(&path).unwrap().contains("[groups]"),
            "a registry without groups keeps its shape"
        );
        r.groups.insert("vf".into(), vec!["sci".into()]);
        r.save_to(&path).unwrap();
        assert_eq!(Registry::load_from(&path).unwrap().groups, r.groups);
    }

    #[test]
    fn load_rejects_an_invalid_group() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("projects.toml");
        let base = "[projects]\nsci = \"/tmp/a\"\n\n[aliases]\nold = \"sci\"\n\n[groups]\n";
        for (groups, named) in [
            ("mix = [\"sci\", \"gone\"]\n", ["\"mix\"", "\"gone\""]),
            ("vf = [\"old\"]\n", ["\"vf\"", "\"old\""]),
            ("empty = []\n", ["\"empty\"", "no members"]),
            ("Bad = [\"sci\"]\n", ["\"Bad\"", "lowercase"]),
            ("\"-x\" = [\"sci\"]\n", ["\"-x\"", "lowercase"]),
            ("sci = [\"sci\"]\n", ["\"sci\"", "registered prefix"]),
            ("old = [\"sci\"]\n", ["\"old\"", "retired prefix"]),
        ] {
            std::fs::write(&path, format!("{base}{groups}")).unwrap();
            let error = Registry::load_from(&path).unwrap_err();
            assert_eq!(error.kind(), "config", "{groups}");
            for word in named {
                assert!(error.to_string().contains(word), "{groups}: {error}");
            }
        }
        std::fs::write(&path, format!("{base}data-2 = [\"sci\"]\n")).unwrap();
        assert_eq!(
            Registry::load_from(&path).unwrap().groups["data-2"],
            ["sci"],
            "digits and - are the tag grammar"
        );
    }

    #[test]
    fn set_group_resolves_aliases_stores_each_member_once_and_validates() {
        let mut r = Registry::default();
        r.register("sci", Path::new("/tmp/a")).unwrap();
        r.register("fam", Path::new("/tmp/b")).unwrap();
        r.aliases.insert("old".into(), "fam".into());

        let stored = r
            .set_group("vf", &["sci".into(), "old".into(), "sci".into()])
            .unwrap();
        assert_eq!(stored, ["fam", "sci"]);
        assert_eq!(r.groups["vf"], ["fam", "sci"]);
        assert_eq!(r.set_group("vf", &["sci".into()]).unwrap(), ["sci"]);
        assert_eq!(r.groups["vf"], ["sci"], "set replaces");

        for name in ["", "Bad", "under_score", "fam", "old"] {
            assert_eq!(
                r.set_group(name, &["sci".into()]).unwrap_err().kind(),
                "validation",
                "{name:?}"
            );
        }
        assert_eq!(
            r.set_group("vf", &[]).unwrap_err().kind(),
            "validation",
            "a group needs a member"
        );
        assert_eq!(
            r.set_group("vf", &["nope".into()]).unwrap_err().kind(),
            "config"
        );
        assert_eq!(r.groups["vf"], ["sci"], "a refused set changes nothing");
    }

    #[test]
    fn remove_group_returns_its_members_and_an_unknown_name_is_unknown_group() {
        let mut r = Registry::default();
        r.register("sci", Path::new("/tmp/a")).unwrap();
        r.set_group("vf", &["sci".into()]).unwrap();
        assert_eq!(r.remove_group("vf").unwrap(), ["sci"]);
        assert!(r.groups.is_empty());
        assert_eq!(r.remove_group("vf").unwrap_err().kind(), "unknown_group");
    }

    #[test]
    fn a_group_name_cannot_become_a_prefix() {
        let mut r = Registry::default();
        r.register("sci", Path::new("/tmp/a")).unwrap();
        r.set_group("vf", &["sci".into()]).unwrap();
        for error in [
            r.register("vf", Path::new("/tmp/b")).unwrap_err(),
            r.repoint("vf", Path::new("/tmp/b")).unwrap_err(),
        ] {
            assert_eq!(error.kind(), "config");
            assert!(error.to_string().contains("tasks group rm vf"), "{error}");
        }
        assert!(r.is_taken("vf"), "a group's name is taken");
        assert_eq!(r.rename("sci", "vf").unwrap_err().kind(), "config");
        assert!(r.project_root("vf").is_none());
        assert_eq!(r.groups["vf"], ["sci"]);
    }

    #[test]
    fn rename_and_adopt_carry_group_membership() {
        let mut r = Registry::default();
        r.register("dot", Path::new("/tmp/d")).unwrap();
        r.register("ops", Path::new("/tmp/o")).unwrap();
        r.set_group("vf", &["dot".into(), "ops".into()]).unwrap();
        r.rename("dot", "dots").unwrap();
        assert_eq!(r.groups["vf"], ["dots", "ops"]);

        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("new");
        std::fs::create_dir(&root).unwrap();
        for partial_init in [false, true] {
            let mut r = Registry::default();
            r.register("old", &home.path().join("missing")).unwrap();
            let mut members = vec!["old".to_string()];
            if partial_init {
                r.register("new", &root.join(".")).unwrap();
                members.push("new".into());
            }
            r.set_group("vf", &members).unwrap();
            r.adopt("old", "new", &root).unwrap();
            assert_eq!(r.groups["vf"], ["new"], "partial_init={partial_init}");
        }

        let mut r = Registry::default();
        r.register("old", &home.path().join("missing")).unwrap();
        r.set_group("new", &["old".into()]).unwrap();
        assert_eq!(r.adopt("old", "new", &root).unwrap_err().kind(), "config");
        assert!(
            r.project_root("old").is_some(),
            "a refused adopt changes nothing"
        );
    }

    #[test]
    fn unregister_prunes_groups_and_deletes_the_ones_it_empties() {
        let mut r = Registry::default();
        r.register("sci", Path::new("/tmp/a")).unwrap();
        r.register("fam", Path::new("/tmp/b")).unwrap();
        r.set_group("pair", &["sci".into(), "fam".into()]).unwrap();
        r.set_group("solo", &["fam".into()]).unwrap();
        let removed = r.unregister("fam").unwrap();
        assert_eq!(removed.emptied_groups, ["solo"]);
        assert_eq!(r.groups.keys().collect::<Vec<_>>(), ["pair"]);
        assert_eq!(r.groups["pair"], ["sci"]);
    }
}
