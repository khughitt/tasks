use super::snapshot::{Snapshot, SnapshotSource};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Output;

#[derive(Debug, Deserialize)]
pub struct PublicIdentity {
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub purpose: String,
    pub prefix: Option<String>,
}

#[derive(Debug)]
pub struct HeadMeta {
    pub branch: String,
    pub commit: String,
}

#[derive(Debug)]
pub struct HeadIdentity {
    pub head: HeadMeta,
    pub found: Result<Option<(String, PublicIdentity)>>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Freshness {
    Current,
    Behind,
    Unknown,
}

pub fn parse_identity(text: &str, shown: &str) -> Result<PublicIdentity> {
    let invalid = |detail| Error::IdentityInvalid(format!("{shown}: {detail}"));
    let identity: PublicIdentity =
        toml::from_str(text).map_err(|error| invalid(error.to_string()))?;
    for (field, value) in [("name", &identity.name), ("purpose", &identity.purpose)] {
        crate::format::validate_line(field, value).map_err(|error| invalid(error.to_string()))?;
    }
    for alias in &identity.aliases {
        crate::format::validate_line("alias", alias).map_err(|error| invalid(error.to_string()))?;
    }
    Ok(identity)
}

pub fn read_head(root: &Path) -> Result<HeadIdentity> {
    read_head_with(root, &mut |root, args| {
        crate::git::at(root).args(args).output()
    })
}

pub(crate) fn read_head_with(
    root: &Path,
    git: &mut dyn FnMut(&Path, &[&str]) -> std::io::Result<Output>,
) -> Result<HeadIdentity> {
    let branch = git(root, &["symbolic-ref", "-q", "--short", "HEAD"])?;
    if !branch.status.success() {
        return Err(Error::RootDetached(format!(
            "{}: cannot read the checked-out branch: {}",
            root.display(),
            String::from_utf8_lossy(&branch.stderr).trim()
        )));
    }
    let branch = output_text(branch)?;
    let commit = output_text(git(root, &["rev-parse", "--verify", "HEAD"])?)?;
    let found = (|| {
        let object = format!("{commit}:./identity.toml");
        let blob = git(root, &["rev-parse", "--verify", "-q", &object])?;
        if blob.status.code() == Some(1) {
            return Ok(None);
        }
        let blob = output_text(blob)?;
        let content = git(root, &["cat-file", "blob", &blob])?;
        let shown = format!("{} at {commit}", root.join("identity.toml").display());
        if !content.status.success() {
            return Err(Error::Io(format!(
                "{shown}: git cat-file: {}",
                String::from_utf8_lossy(&content.stderr).trim()
            )));
        }
        let text = String::from_utf8(content.stdout)
            .map_err(|error| Error::IdentityInvalid(format!("{shown}: {error}")))?;
        Ok(Some((blob, parse_identity(&text, &shown)?)))
    })();
    Ok(HeadIdentity {
        head: HeadMeta { branch, commit },
        found,
    })
}

fn output_text(output: Output) -> Result<String> {
    if !output.status.success() {
        return Err(Error::Io(format!(
            "git: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    String::from_utf8(output.stdout)
        .map(|text| text.trim().to_owned())
        .map_err(|error| Error::Io(error.to_string()))
}

pub fn freshness(root: &Path, snapshot: Option<&Snapshot>) -> (Freshness, Option<String>) {
    freshness_with(root, snapshot, &mut |root, args| {
        crate::git::at(root).args(args).output()
    })
}

pub(crate) fn freshness_with(
    root: &Path,
    snapshot: Option<&Snapshot>,
    git: &mut dyn FnMut(&Path, &[&str]) -> std::io::Result<Output>,
) -> (Freshness, Option<String>) {
    let Some(snapshot) = snapshot else {
        return (Freshness::Unknown, None);
    };
    let current = (|| {
        let output = git(
            root,
            &["rev-parse", "--verify", "-q", "HEAD:./identity.toml"],
        )?;
        if output.status.code() == Some(1) {
            return Ok(None);
        }
        output_text(output).map(Some)
    })();
    match (current, &snapshot.source) {
        (Ok(Some(current)), SnapshotSource::Head { blob, .. }) if &current == blob => {
            (Freshness::Current, None)
        }
        (Ok(Some(_)), _) => (Freshness::Behind, None),
        (Ok(None), SnapshotSource::Seeded) => (Freshness::Current, None),
        (Ok(None), SnapshotSource::Head { .. }) => (Freshness::Unknown, None),
        (Err(error), _) => (
            Freshness::Unknown,
            Some(format!("{}: {error}", root.display())),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    struct Repository {
        dir: TempDir,
        home: TempDir,
    }

    impl Repository {
        fn new() -> Self {
            let repo = Self {
                dir: tempfile::tempdir().unwrap(),
                home: tempfile::tempdir().unwrap(),
            };
            repo.git(repo.root(), &["init", "--initial-branch=main"]);
            assert_eq!(
                repo.git(repo.root(), &["rev-parse", "--show-toplevel"]),
                repo.root().to_str().unwrap()
            );
            assert!(
                Path::new(&repo.git(repo.root(), &["rev-parse", "--absolute-git-dir"]))
                    .starts_with(repo.root())
            );
            repo.git(repo.root(), &["commit", "--allow-empty", "-m", "initial"]);
            repo
        }

        fn root(&self) -> &Path {
            self.dir.path()
        }

        fn output(&self, root: &Path, args: &[&str]) -> std::io::Result<Output> {
            crate::git::at(root)
                .env_clear()
                .env("PATH", std::env::var_os("PATH").unwrap())
                .env("HOME", self.home.path())
                .env("GIT_CONFIG_GLOBAL", self.home.path().join("gitconfig"))
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_AUTHOR_NAME", "Test")
                .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
                .env("GIT_COMMITTER_NAME", "Test")
                .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
                .env("LC_ALL", "C")
                .args(args)
                .output()
        }

        fn git(&self, root: &Path, args: &[&str]) -> String {
            let out = self.output(root, args).unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8(out.stdout).unwrap().trim().to_owned()
        }

        fn commit_identity(&self, root: &Path, text: &str) -> String {
            fs::write(root.join("identity.toml"), text).unwrap();
            self.git(root, &["add", "identity.toml"]);
            self.git(root, &["commit", "-m", "identity"]);
            self.git(root, &["rev-parse", "HEAD"])
        }

        fn read(&self, root: &Path) -> HeadIdentity {
            read_head_with(root, &mut |root, args| self.output(root, args)).unwrap()
        }

        fn freshness(
            &self,
            root: &Path,
            snapshot: Option<&Snapshot>,
        ) -> (Freshness, Option<String>) {
            freshness_with(root, snapshot, &mut |root, args| self.output(root, args))
        }
    }

    fn identity(name: &str) -> String {
        format!("name = '{name}'\npurpose = 'Keeps identities'\n")
    }

    #[test]
    fn parse_ignores_kit_keys() {
        let text = format!(
            "{}\nprefix = 'tasks'\n[kit]\nversion = 1\n",
            include_str!("../../identity.toml")
        );
        let parsed = parse_identity(&text, "identity.toml").unwrap();
        assert_eq!(parsed.name, "tasks");
        assert!(parsed.aliases.is_empty());
        assert_eq!(
            parsed.purpose,
            "The file-based task tracker every project here uses"
        );
        // The fixture's prefix appears inside [documents], so it is ignored too.
        assert_eq!(parsed.prefix, None);
        let parsed = parse_identity(
            "name = 'N'\npurpose = 'P'\nprefix = 'p'\naliases = ['A']",
            "seed",
        )
        .unwrap();
        assert_eq!(parsed.prefix.as_deref(), Some("p"));
        assert_eq!(parsed.aliases, ["A"]);
    }

    #[test]
    fn parse_refuses_a_non_string_name() {
        let error = parse_identity("name = 1\npurpose = 'P'", "source@commit").unwrap_err();
        assert_eq!(error.kind(), "identity_invalid");
        assert!(error.to_string().contains("source@commit"));
    }

    #[test]
    fn parse_refuses_invalid_lines_and_types() {
        for text in [
            "name = ''\npurpose = 'P'",
            "name = 'N'\npurpose = 1",
            "name = 'N'\npurpose = 'P'\naliases = [1]",
            "name = 'N'\npurpose = 'P'\nprefix = 1",
            "name = \"N\\nN\"\npurpose = 'P'",
            "name = 'N'\npurpose = \"P\\nP\"",
            "name = 'N'\npurpose = 'P'\naliases = [\"A\\nA\"]",
            "not toml",
        ] {
            assert_eq!(
                parse_identity(text, "input").unwrap_err().kind(),
                "identity_invalid"
            );
        }
    }

    #[test]
    fn read_head_reads_the_commit_not_the_working_tree() {
        let repo = Repository::new();
        let commit = repo.commit_identity(repo.root(), &identity("A"));
        fs::write(repo.root().join("identity.toml"), identity("B")).unwrap();
        let result = repo.read(repo.root());
        assert_eq!(result.head.branch, "main");
        assert_eq!(result.head.commit, commit);
        assert_eq!(result.found.unwrap().unwrap().1.name, "A");
    }

    #[test]
    fn read_head_reads_a_subdirectory_root() {
        let repo = Repository::new();
        let root = repo.root().join("nexc");
        fs::create_dir(&root).unwrap();
        repo.commit_identity(&root, &identity("Nested"));
        assert_eq!(repo.read(&root).found.unwrap().unwrap().1.name, "Nested");
    }

    #[test]
    fn read_head_without_identity_is_found_none() {
        let repo = Repository::new();
        assert!(repo.read(repo.root()).found.unwrap().is_none());
    }

    #[test]
    fn read_head_pins_the_captured_commit() {
        let repo = Repository::new();
        let commit = repo.commit_identity(repo.root(), &identity("A"));
        let result = read_head_with(repo.root(), &mut |root, args| {
            let out = repo.output(root, args)?;
            if args == ["rev-parse", "--verify", "HEAD"] {
                repo.commit_identity(root, &identity("C"));
            }
            Ok(out)
        })
        .unwrap();
        assert_eq!(result.head.commit, commit);
        assert_eq!(result.found.unwrap().unwrap().1.name, "A");
    }

    #[test]
    fn read_head_keeps_branch_and_commit_on_invalid_identity() {
        let repo = Repository::new();
        let commit = repo.commit_identity(repo.root(), "name = 1\npurpose = 'P'");
        let result = repo.read(repo.root());
        assert_eq!(result.head.branch, "main");
        assert_eq!(result.head.commit, commit);
        let error = result.found.unwrap_err();
        assert_eq!(error.kind(), "identity_invalid");
        assert!(error.to_string().contains(&commit));
    }

    #[test]
    fn read_head_keeps_metadata_when_blob_read_fails() {
        let repo = Repository::new();
        let commit = repo.commit_identity(repo.root(), &identity("A"));
        let result = read_head_with(repo.root(), &mut |root, args| {
            if args.first() == Some(&"cat-file") {
                return Err(std::io::Error::other("blob read failed"));
            }
            repo.output(root, args)
        })
        .unwrap();
        assert_eq!(result.head.branch, "main");
        assert_eq!(result.head.commit, commit);
        assert!(
            result
                .found
                .unwrap_err()
                .to_string()
                .contains("blob read failed")
        );
    }

    #[test]
    fn read_head_refuses_detached() {
        let repo = Repository::new();
        repo.git(repo.root(), &["checkout", "--detach"]);
        let error =
            read_head_with(repo.root(), &mut |root, args| repo.output(root, args)).unwrap_err();
        assert_eq!(error.kind(), "root_detached");
    }

    #[test]
    fn freshness_rules() {
        let repo = Repository::new();
        let mut snapshot = Snapshot {
            name: "N".into(),
            aliases: vec![],
            purpose: "P".into(),
            source: SnapshotSource::Seeded,
            refreshed: "now".into(),
        };
        assert_eq!(
            repo.freshness(repo.root(), Some(&snapshot)),
            (Freshness::Current, None)
        );
        assert_eq!(
            repo.freshness(repo.root(), None),
            (Freshness::Unknown, None)
        );
        let missing = repo.root().join("missing");
        let (state, warning) = repo.freshness(&missing, Some(&snapshot));
        assert_eq!(state, Freshness::Unknown);
        assert!(warning.is_some());
        let commit = repo.commit_identity(repo.root(), &identity("A"));
        assert_eq!(
            repo.freshness(repo.root(), Some(&snapshot)),
            (Freshness::Behind, None)
        );
        let blob = repo.git(repo.root(), &["rev-parse", "HEAD:identity.toml"]);
        snapshot.source = SnapshotSource::Head { commit, blob };
        assert_eq!(
            repo.freshness(repo.root(), Some(&snapshot)),
            (Freshness::Current, None)
        );
        repo.commit_identity(repo.root(), &identity("B"));
        assert_eq!(
            repo.freshness(repo.root(), Some(&snapshot)),
            (Freshness::Behind, None)
        );
        repo.git(repo.root(), &["rm", "identity.toml"]);
        repo.git(repo.root(), &["commit", "-m", "remove identity"]);
        assert_eq!(
            repo.freshness(repo.root(), Some(&snapshot)),
            (Freshness::Unknown, None)
        );
    }

    #[test]
    fn freshness_uses_the_supplied_runner_once() {
        let repo = Repository::new();
        let snapshot = Snapshot {
            name: "N".into(),
            aliases: vec![],
            purpose: "P".into(),
            source: SnapshotSource::Seeded,
            refreshed: "now".into(),
        };
        let mut calls = 0;
        let mut git = |root: &Path, args: &[&str]| {
            calls += 1;
            assert_eq!(root, repo.root());
            assert_eq!(
                args,
                ["rev-parse", "--verify", "-q", "HEAD:./identity.toml"]
            );
            repo.output(root, args)
        };
        assert_eq!(
            freshness_with(repo.root(), None, &mut git),
            (Freshness::Unknown, None)
        );
        assert_eq!(
            freshness_with(repo.root(), Some(&snapshot), &mut git),
            (Freshness::Current, None)
        );
        assert_eq!(calls, 1);
    }
}
