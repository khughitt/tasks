use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::repo::Project;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;

pub struct Revision {
    toplevel: PathBuf,
    commit: String,
}

impl Revision {
    pub fn commit(&self) -> &str {
        &self.commit
    }

    pub fn resolve(records: &Path, rev: &str) -> Result<Self> {
        let toplevel = crate::record_commit::working_tree(records)
            .map_err(Error::Validation)?
            .ok_or_else(|| Error::Validation("records are outside a Git repository".into()))?;
        let output = crate::git::at(&toplevel)
            .arg("--literal-pathspecs")
            .args([
                "rev-parse",
                "--verify",
                "--end-of-options",
                &format!("{rev}^{{commit}}"),
            ])
            .output()?;
        if !output.status.success() {
            return Err(Error::Validation(format!(
                "cannot resolve records revision {rev:?}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let commit = String::from_utf8(output.stdout)
            .map_err(|error| Error::Validation(error.to_string()))?
            .trim()
            .to_string();
        Ok(Self { toplevel, commit })
    }

    pub fn read(&self, rel: &Path, max: u64) -> Result<Option<Vec<u8>>> {
        if rel.as_os_str().is_empty()
            || rel
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(Error::Validation(
                "revision paths must be relative and stay inside the repository".into(),
            ));
        }
        let output = crate::git::at(&self.toplevel)
            .arg("--literal-pathspecs")
            .args(["ls-tree", "-z", &self.commit, "--"])
            .arg(rel)
            .output()?;
        if !output.status.success() {
            return Err(Error::Validation(format!(
                "cannot read revision tree: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        if output.stdout.is_empty() {
            return Ok(None);
        }
        let metadata = output.stdout.split(|byte| *byte == b'\t').next().unwrap();
        let metadata =
            std::str::from_utf8(metadata).map_err(|error| Error::Validation(error.to_string()))?;
        let fields: Vec<_> = metadata.split_whitespace().collect();
        if fields.len() != 3 || fields[1] != "blob" {
            return Err(Error::Validation(format!(
                "{} is not a blob at {}",
                rel.display(),
                self.commit
            )));
        }
        let mut child = crate::git::at(&self.toplevel)
            .arg("--literal-pathspecs")
            .args(["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let result = (|| {
            writeln!(child.stdin.take().unwrap(), "{}", fields[2])?;
            let mut reader = BufReader::new(child.stdout.take().unwrap());
            let mut header = String::new();
            reader.read_line(&mut header)?;
            let fields: Vec<_> = header.split_whitespace().collect();
            let size = fields
                .get(2)
                .and_then(|size| size.parse::<u64>().ok())
                .filter(|_| fields.get(1) == Some(&"blob"))
                .ok_or_else(|| Error::Validation("invalid Git blob response".into()))?;
            if size > max {
                return Err(Error::Validation(format!(
                    "{} exceeds {max} bytes at {}",
                    rel.display(),
                    self.commit
                )));
            }
            let mut bytes = Vec::new();
            reader.by_ref().take(size).read_to_end(&mut bytes)?;
            if bytes.len() as u64 != size {
                return Err(Error::Validation("truncated Git blob response".into()));
            }
            let mut newline = [0];
            reader.read_exact(&mut newline)?;
            if newline != *b"\n" {
                return Err(Error::Validation("invalid Git blob terminator".into()));
            }
            Ok(bytes)
        })();
        if result.is_err() {
            let _ = child.kill();
        }
        let status = child.wait()?;
        let bytes = result?;
        if !status.success() {
            return Err(Error::Validation("git cat-file failed".into()));
        }
        Ok(Some(bytes))
    }

    pub fn task_at(&self, project: &Project, id: &TaskId) -> Result<Option<Task>> {
        let records = project.records.strip_prefix(&self.toplevel).map_err(|_| {
            Error::Validation("project records are outside this revision's repository".into())
        })?;
        let path = records.join(format!("tasks/{id}.md"));
        let Some(bytes) = self.read(&path, super::RECOVERY_MAX_BYTES)? else {
            return Ok(None);
        };
        let file = path.display().to_string();
        let text = std::str::from_utf8(&bytes).map_err(|error| Error::Parse {
            file: file.clone(),
            detail: error.to_string(),
        })?;
        let physical = crate::format::parse_physical(text, &file)?;
        if physical.id() != id || id.prefix != project.prefix {
            return Err(Error::Parse {
                file,
                detail: format!(
                    "id field {} does not match requested task {id} in project {}",
                    physical.id(),
                    project.prefix
                ),
            });
        }
        crate::history::load_logical(physical, |archive| {
            let path = records.join(format!("tasks/files/{id}/{}", archive.file_name()));
            self.read(&path, project.attachments_max_bytes)?
                .ok_or_else(|| {
                    Error::Validation(format!(
                        "missing archive {} at {}",
                        path.display(),
                        self.commit
                    ))
                })
        })
        .map(Some)
    }
}

pub fn commit_cause(commit_records: bool, warnings: &[String]) -> Option<&'static str> {
    if !commit_records {
        return Some("commit_disabled");
    }
    warnings.iter().find_map(|warning| {
        ["commit_deferred", "commit_failed", "commit_unavailable"]
            .into_iter()
            .find(|cause| {
                warning
                    .strip_prefix(cause)
                    .is_some_and(|tail| tail.starts_with(':'))
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::process::Command;

    fn git(root: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .current_dir(root)
            .args(args)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("HOME", root)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    fn isolated_test(name: &str) -> bool {
        if std::env::var_os("TASKS_REVISION_TEST").is_some() {
            return false;
        }
        let home = tempfile::tempdir().unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("HOME", home.path())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("TASKS_REVISION_TEST", "1")
            .args([
                "--exact",
                &format!("multipart::git_objects::tests::{name}"),
                "--nocapture",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        true
    }

    #[test]
    fn revision_task_loads_archives_from_the_same_commit() {
        if isolated_test("revision_task_loads_archives_from_the_same_commit") {
            return;
        }
        use sha2::{Digest, Sha256};
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q", "-b", "main"]);
        let canonical = root.canonicalize().unwrap();
        assert_eq!(
            Path::new(git(root, &["rev-parse", "--show-toplevel"]).trim()),
            canonical
        );
        assert!(
            Path::new(git(root, &["rev-parse", "--absolute-git-dir"]).trim())
                .starts_with(&canonical)
        );
        let records = root.join("project");
        std::fs::create_dir_all(records.join("tasks/files/tst-000001")).unwrap();
        std::fs::write(records.join("tasks/.config.toml"), "prefix = \"tst\"\n").unwrap();
        let archive = b"## Notes\n\n- 2026-10-01T00:00:00Z (fixture): historical evidence\n";
        let sha = format!("{:x}", Sha256::digest(archive));
        let archive_path = records.join(format!("tasks/files/tst-000001/notes-{sha}.notes"));
        std::fs::write(&archive_path, archive).unwrap();
        let raw = format!(
            "---\nid: tst-000001\ntitle: Historical\nstatus: todo\npriority: 2\ncreated: 2026-10-01T00:00:00Z\nupdated: 2026-10-02T00:00:00Z\ndepends: []\ntags: []\nnote_archives: [\"v1:{sha}:1\"]\n---\n\n## Notes\n\n- 2026-10-02T00:00:00Z (fixture): attached: notes-{sha}.notes ({} bytes)\n",
            archive.len()
        );
        std::fs::write(records.join("tasks/tst-000001.md"), &raw).unwrap();
        git(root, &["add", "."]);
        git(
            root,
            &[
                "-c",
                "core.hooksPath=/dev/null",
                "commit",
                "-q",
                "-m",
                "archive",
            ],
        );
        let revision = Revision::resolve(&records, "HEAD").unwrap();
        let project = Project::open(&records).unwrap();
        std::fs::remove_file(archive_path).unwrap();
        let id = TaskId::parse("tst-000001").unwrap();
        let task = revision.task_at(&project, &id).unwrap().unwrap();
        assert_eq!(task.notes.len(), 2);
        assert_eq!(task.notes[0].text, "historical evidence");
        assert!(
            revision
                .task_at(&project, &TaskId::parse("tst-000002").unwrap())
                .unwrap()
                .is_none()
        );
        // The reader must reject a record stored under another task's name.
        let mismatched = root.join("project/tasks/tst-000002.md");
        std::fs::write(
            mismatched,
            raw.replace(&format!("note_archives: [\"v1:{sha}:1\"]\n"), ""),
        )
        .unwrap();
        git(root, &["add", "project/tasks/tst-000002.md"]);
        git(
            root,
            &[
                "-c",
                "core.hooksPath=/dev/null",
                "commit",
                "-q",
                "-m",
                "mismatched",
            ],
        );
        let revision = Revision::resolve(&records, "HEAD").unwrap();
        assert!(
            revision
                .task_at(&project, &TaskId::parse("tst-000002").unwrap())
                .is_err()
        );
    }

    #[test]
    fn revision_reads_blobs_and_absent_paths() {
        if isolated_test("revision_reads_blobs_and_absent_paths") {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q", "-b", "main"]);
        assert_eq!(
            Path::new(git(root, &["rev-parse", "--show-toplevel"]).trim()),
            root.canonicalize().unwrap()
        );
        assert!(
            Path::new(git(root, &["rev-parse", "--absolute-git-dir"]).trim()).starts_with(root)
        );
        std::fs::write(root.join("a*.txt"), "old\n").unwrap();
        std::fs::write(root.join("ab.txt"), "foreign\n").unwrap();
        git(root, &["add", "."]);
        git(
            root,
            &[
                "-c",
                "core.hooksPath=/dev/null",
                "commit",
                "-q",
                "-m",
                "initial",
            ],
        );
        let revision = Revision::resolve(root, "HEAD").unwrap();
        std::fs::write(root.join("a*.txt"), "new\n").unwrap();
        assert_eq!(
            revision.read(Path::new("a*.txt"), 4).unwrap(),
            Some(b"old\n".to_vec())
        );
        assert_eq!(revision.read(Path::new("missing"), 4).unwrap(), None);
        assert!(revision.read(Path::new("a*.txt"), 3).is_err());
        assert!(revision.read(Path::new("../outside"), 4).is_err());
        assert!(Revision::resolve(root, "missing").is_err());
    }

    #[test]
    fn commit_cause_maps_each_warning() {
        assert_eq!(commit_cause(false, &[]), Some("commit_disabled"));
        assert_eq!(commit_cause(true, &[]), None);
        assert_eq!(commit_cause(true, &["unrelated".into()]), None);
        for cause in ["commit_deferred", "commit_failed", "commit_unavailable"] {
            assert_eq!(
                commit_cause(true, &[format!("{cause}: reason")]),
                Some(cause)
            );
        }
    }
}
