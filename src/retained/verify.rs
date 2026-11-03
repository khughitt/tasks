use super::{basis, model::*};
use crate::{error::Result, repo::Project};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, Metadata, OpenOptions},
    io::Read,
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Debug, Default, Clone, Serialize)]
pub struct Metrics {
    pub wall_ns: u64,
    pub hash_ns: u64,
    pub bytes_hashed: u64,
    pub history_bytes_read: u64,
}
#[derive(Debug, Clone)]
pub struct ManifestBytes {
    pub bytes: Vec<u8>,
    pub sha256: String,
}
#[derive(Debug)]
pub struct Observation {
    pub record: RetentionRecord,
    pub manifest: Option<ManifestBytes>,
    pub git: Option<String>,
}
pub fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn timed_hash(bytes: &[u8], metrics: &mut Metrics) -> String {
    let start = Instant::now();
    let digest = sha(bytes);
    metrics.bytes_hashed += bytes.len() as u64;
    metrics.hash_ns += start.elapsed().as_nanos().min(u64::MAX as u128) as u64;
    digest
}
fn stamp(m: &Metadata) -> (u64, u64, u64, i64, i64, i64, i64, u32) {
    (
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
        m.mode(),
    )
}
fn open(path: &Path, directory: bool) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(
            libc::O_NOFOLLOW | libc::O_NONBLOCK | if directory { libc::O_DIRECTORY } else { 0 },
        )
        .open(path)?;
    let m = file.metadata()?;
    if (directory && !m.is_dir()) || (!directory && !m.is_file()) || m.mode() & 0o444 == 0 {
        return Err(invalid("unreadable or unsupported retained input"));
    }
    Ok(file)
}
fn stable(path: &Path, file: &File, before: &Metadata) -> Result<()> {
    if stamp(before) != stamp(&file.metadata()?)
        || stamp(before) != stamp(&std::fs::symlink_metadata(path)?)
    {
        return Err(invalid("retained input changed during inspection"));
    }
    Ok(())
}
fn hash_stream(reader: &mut impl Read, metrics: &mut Metrics) -> Result<String> {
    let start = Instant::now();
    let mut hash = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
        metrics.bytes_hashed += n as u64;
    }
    metrics.hash_ns += start.elapsed().as_nanos().min(u64::MAX as u128) as u64;
    Ok(format!("{:x}", hash.finalize()))
}
fn hash_file(path: &Path, metrics: &mut Metrics) -> Result<String> {
    let mut f = open(path, false)?;
    let before = f.metadata()?;
    let digest = hash_stream(&mut f, metrics)?;
    stable(path, &f, &before)?;
    Ok(digest)
}
fn walk(
    dir: &Path,
    prefix: &str,
    entries: &mut Vec<ManifestEntry>,
    metrics: &mut Metrics,
) -> Result<()> {
    let handle = open(dir, true)?;
    let before = handle.metadata()?;
    // Held directory descriptors prevent a replaced ancestor from redirecting enumeration.
    let held = PathBuf::from(format!("/proc/self/fd/{}", handle.as_raw_fd()));
    let mut names = std::fs::read_dir(&held)?
        .map(|e| e.map(|e| e.file_name()))
        .collect::<std::io::Result<Vec<_>>>()?;
    names.sort();
    for name in names {
        let n = name
            .to_str()
            .ok_or_else(|| invalid("manifest member name is not UTF-8"))?;
        let path = if prefix.is_empty() {
            n.to_string()
        } else {
            format!("{prefix}/{n}")
        };
        if !member_path(&path) {
            return Err(invalid("unsafe manifest member"));
        }
        let physical = held.join(&name);
        let meta = std::fs::symlink_metadata(&physical)?;
        if meta.is_symlink() {
            let target = std::fs::read_link(&physical)?
                .into_os_string()
                .into_string()
                .map_err(|_| invalid("symlink target is not UTF-8"))?;
            if stamp(&meta) != stamp(&std::fs::symlink_metadata(&physical)?) {
                return Err(invalid("symlink changed during inspection"));
            }
            entries.push(ManifestEntry::Symlink { path, target });
        } else if meta.is_dir() {
            entries.push(ManifestEntry::Directory { path: path.clone() });
            walk(&physical, &path, entries, metrics)?;
        } else if meta.is_file() {
            entries.push(ManifestEntry::File {
                path,
                sha256: hash_file(&physical, metrics)?,
            });
        } else {
            return Err(invalid("unsupported special retained member"));
        }
    }
    if stamp(&before) != stamp(&handle.metadata()?)
        || stamp(&before) != stamp(&std::fs::symlink_metadata(dir)?)
    {
        return Err(invalid("retained directory changed during enumeration"));
    }
    Ok(())
}
pub fn manifest(root: &Path, metrics: &mut Metrics) -> Result<ManifestBytes> {
    let mut entries = Vec::new();
    walk(root, "", &mut entries, metrics)?;
    entries.sort_by(|a, b| a.path().cmp(b.path()));
    let bytes = serde_json::to_vec(&Manifest {
        version: 1,
        entries,
    })
    .map_err(invalid)?;
    let sha256 = timed_hash(&bytes, metrics);
    Ok(ManifestBytes { bytes, sha256 })
}
pub fn current_path(project: &Project, path: &str) -> Result<PathBuf> {
    if let Some(s) = path.strip_prefix("$WORK_ROOT/") {
        let root = std::env::var_os("WORK_ROOT")
            .ok_or_else(|| invalid("WORK_ROOT is unavailable on this host"))?;
        Ok(PathBuf::from(root).join(s))
    } else if path.starts_with(".worktrees/") {
        Ok(project.checkout.join(path))
    } else {
        Ok(PathBuf::from(path))
    }
}
fn read_file(path: &Path, cap: u64) -> Result<Vec<u8>> {
    let mut file = open(path, false)?;
    let before = file.metadata()?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(cap.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > cap {
        return Err(invalid("retained metadata exceeds attachment cap"));
    }
    stable(path, &file, &before)?;
    Ok(bytes)
}
fn artifact_location(
    project: &Project,
    locator: &ArtifactLocator,
    metrics: &mut Metrics,
) -> Result<(Project, Basis, String, String)> {
    let declared = match locator {
        ArtifactLocator::Git { basis, .. }
        | ArtifactLocator::Attachment {
            basis: Some(basis), ..
        } => &basis.project,
        _ => return Err(invalid("directory locator has no file payload")),
    };
    let resolved = if declared == &project.prefix {
        project.clone()
    } else {
        crate::scope::open_registered(
            &crate::registry::Registry::load()?,
            declared,
            crate::scope::Origin::Prefix,
        )?
    };
    let project = &resolved;
    let (b, path, expected) = match locator {
        ArtifactLocator::Git {
            basis,
            path,
            sha256,
        } => (basis, path.clone(), sha256),
        ArtifactLocator::Attachment {
            task,
            name,
            basis: Some(basis),
            sha256,
        } => {
            let id = crate::model::TaskId::parse(task)?;
            basis::validate_locator(project, &id, basis)?;
            let pinned = basis::read_exact(project, &id, basis)?;
            metrics.history_bytes_read += pinned.history_bytes_read;
            if crate::attachments::ledger(&pinned.task).get(name)
                != Some(&crate::attachments::Ledger::Attached)
            {
                return Err(invalid("preserved attachment lacks a live ledger entry"));
            }
            (
                basis,
                format!("{}/tasks/files/{task}/{name}", basis.records_subdir)
                    .trim_start_matches("./")
                    .to_string(),
                sha256,
            )
        }
        ArtifactLocator::Directory { .. } | ArtifactLocator::Attachment { basis: None, .. } => {
            return Err(invalid("artifact needs an explicit pinned file locator"));
        }
    };
    b.validate()?;
    if b.project != project.prefix {
        return Err(invalid("artifact requires its declared repository pairing"));
    }
    let id = crate::model::TaskId::parse(
        Path::new(&b.task_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| invalid("artifact task path"))?,
    )?;
    basis::validate_locator(project, &id, b)?;
    Ok((resolved, b.clone(), path, expected.clone()))
}
pub fn artifact(
    project: &Project,
    locator: &ArtifactLocator,
    metrics: &mut Metrics,
) -> Result<Vec<u8>> {
    let (p, b, path, expected) = artifact_location(project, locator, metrics)?;
    let bytes = basis::blob(
        &p,
        &b.commit,
        &path,
        Some(p.attachments_max_bytes),
        &mut metrics.history_bytes_read,
    )?;
    if timed_hash(&bytes, metrics) != expected {
        return Err(invalid("preserved artifact digest mismatch"));
    }
    Ok(bytes)
}
fn payload_digest(
    project: &Project,
    locator: &ArtifactLocator,
    metrics: &mut Metrics,
) -> Result<String> {
    let (p, b, path, expected) = artifact_location(project, locator, metrics)?;
    let mode = basis::git(&p, &["ls-tree", "-z", &b.commit, "--", &path])?;
    if !(mode.starts_with(b"100644 blob ") || mode.starts_with(b"100755 blob ")) {
        return Err(invalid("preserved payload is not a regular Git blob"));
    }
    let object = format!("{}:{path}", b.commit);
    let mut child = crate::git::at(&basis::root(&p)?)
        .args(["cat-file", "blob", &object])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let result = hash_stream(&mut child.stdout.take().expect("piped stdout"), metrics);
    if result.is_err() {
        let _ = child.kill();
    }
    let output = child.wait_with_output()?;
    let observed = result?;
    if !output.status.success() || observed != expected {
        return Err(invalid("preserved payload unavailable or digest mismatch"));
    }
    Ok(observed)
}
fn parsed(bytes: &[u8]) -> Result<Manifest> {
    let manifest = decode_manifest(std::str::from_utf8(bytes).map_err(invalid)?)?;
    let mut paths = std::collections::BTreeMap::new();
    for entry in &manifest.entries {
        if paths.insert(entry.path(), entry).is_some() {
            return Err(invalid("duplicate manifest member path"));
        }
    }
    for entry in &manifest.entries {
        let mut parent = Path::new(entry.path()).parent();
        while let Some(p) = parent {
            if let Some(found) = paths.get(p.to_str().ok_or_else(|| invalid("manifest path"))?)
                && !matches!(found, ManifestEntry::Directory { .. })
            {
                return Err(invalid("manifest member has a non-directory ancestor"));
            }
            parent = p.parent();
        }
    }
    Ok(manifest)
}

fn previous(
    project: &Project,
    r: &RetentionRecord,
    locator: &ArtifactLocator,
    metrics: &mut Metrics,
) -> Result<()> {
    let expected = r
        .sha256
        .value()
        .ok_or_else(|| invalid("previous digest missing"))?;
    match locator {
        ArtifactLocator::Directory {
            host,
            path,
            manifest: loc,
        } => {
            if *host != crate::claims::hostname() {
                return Err(invalid("preserved member root is on another host"));
            }
            let old = artifact(project, loc, metrics)?;
            if sha(&old) != expected {
                return Err(invalid("preserved manifest differs from previous digest"));
            }
            let root = current_path(project, path)?;
            let observed = manifest(&root, metrics)?;
            if parsed(&old)? != parsed(&observed.bytes)? {
                return Err(invalid("preserved tree lacks a prior member version"));
            }
            Ok(())
        }
        _ => {
            if payload_digest(project, locator, metrics)? != expected {
                return Err(invalid("preserved bytes differ from previous digest"));
            }
            Ok(())
        }
    }
}
fn unknown(mut r: RetentionRecord, detail: impl std::fmt::Display) -> Observation {
    r.version = Some(1);
    r.state = "kept".into();
    r.verification = "unknown".into();
    if r.kind != "worktree" {
        r.sha256 = FieldString::Null;
    }
    r.verification_action = Some(format!(
        "Restore and inspect exact retained bytes: {detail}"
    ));
    Observation {
        record: r,
        manifest: None,
        git: None,
    }
}
pub fn inspect(
    project: &Project,
    record: &RetentionRecord,
    preserved: Option<&ArtifactLocator>,
    metrics: &mut Metrics,
) -> Result<Observation> {
    record.validate()?;
    if record.host != crate::claims::hostname() {
        return Err(invalid("inspection is restricted to the executing host"));
    }
    if record.state != "kept" {
        return Err(invalid("only kept identities can be inspected"));
    }
    if let Some(name) = record.manifest.value()
        && let Ok(bytes) = read_file(&current_path(project, name)?, project.attachments_max_bytes)
        && let Ok(v) = strict(&bytes)
        && v.get("version").and_then(|v| v.as_u64()) != Some(1)
    {
        return Err(invalid(
            "unsupported historical manifest format; preserve and migrate it explicitly",
        ));
    }
    let attempt = (|| -> Result<Observation> {
        let path = current_path(project, &record.path)?;
        let mut r = record.clone();
        r.version = Some(1);
        if r.kind == "worktree" {
            if !std::fs::symlink_metadata(&path)?.is_dir() {
                return Err(invalid("worktree is not a directory"));
            }
            let top = crate::git::at(&path)
                .args(["rev-parse", "--show-toplevel"])
                .output()?;
            if !top.status.success()
                || Path::new(std::str::from_utf8(&top.stdout).map_err(invalid)?.trim())
                    .canonicalize()?
                    != path.canonicalize()?
            {
                return Err(invalid("retained path is not the Git worktree root"));
            }
            let status = crate::git::at(&path)
                .args(["status", "--ignored", "--porcelain=v1", "-z"])
                .output()?;
            if !status.status.success() {
                return Err(invalid("worktree status unavailable"));
            }
            r.verification = "verified".into();
            r.verification_action = None;
            return Ok(Observation {
                record: r,
                manifest: None,
                git: Some(String::from_utf8(status.stdout).map_err(invalid)?),
            });
        }
        let meta = std::fs::symlink_metadata(&path)?;
        if r.kind == "scratch" && !meta.is_dir() {
            return Err(invalid("scratch identity must be a directory"));
        }
        let mut generated = None;
        let digest = if meta.is_dir() {
            let m = manifest(&path, metrics)?;
            let mut candidate = m;
            if let Some(oldpath) = record.manifest.value() {
                let old = read_file(
                    &current_path(project, oldpath)?,
                    project.attachments_max_bytes,
                )?;
                let oldparsed = parsed(&old)?;
                if record.sha256.value() != Some(timed_hash(&old, metrics).as_str()) {
                    return Err(invalid("previous manifest digest mismatch"));
                }
                if oldparsed == parsed(&candidate.bytes)? {
                    candidate = ManifestBytes {
                        sha256: sha(&old),
                        bytes: old,
                    };
                }
            }
            let digest = candidate.sha256.clone();
            generated = Some(candidate);
            digest
        } else {
            hash_file(&path, metrics)?
        };
        if record.verification == "verified" && record.sha256.value() != Some(digest.as_str()) {
            previous(
                project,
                record,
                preserved
                    .ok_or_else(|| invalid("changed payload requires preserved prior bytes"))?,
                metrics,
            )?;
        }
        r.verification = "verified".into();
        r.sha256 = FieldString::Text(digest);
        r.verification_action = None;
        Ok(Observation {
            record: r,
            manifest: generated,
            git: None,
        })
    })();
    Ok(attempt.unwrap_or_else(|e| unknown(record.clone(), e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    fn record(path: &Path, kind: &str) -> RetentionRecord {
        RetentionRecord {
            version: Some(1),
            host: crate::claims::hostname(),
            kind: kind.into(),
            path: path.to_str().unwrap().into(),
            state: "kept".into(),
            verification: "unknown".into(),
            purpose: "Review".into(),
            next_owner: "task:sci-a00001".into(),
            cleanup_trigger: "Review completes".into(),
            sha256: if kind == "worktree" {
                FieldString::Absent
            } else {
                FieldString::Null
            },
            manifest: FieldString::Absent,
            handoff: None,
            verification_action: Some("Inspect".into()),
        }
    }
    fn project(root: &Path) -> Project {
        std::fs::create_dir_all(root.join("tasks")).unwrap();
        std::fs::write(root.join("tasks/.config.toml"), "prefix = \"sci\"\n").unwrap();
        Project::open(root).unwrap()
    }

    fn isolated(test: &str) -> bool {
        if std::env::var_os("TASKS_VERIFY_FIXTURE_CHILD").is_some() {
            return false;
        }
        let home = tempfile::tempdir().unwrap();
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("HOME", home.path())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("TASKS_VERIFY_FIXTURE_CHILD", "1")
            .args(["--exact", test, "--nocapture"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        true
    }
    fn git(dir: &Path, args: &[&str]) -> Vec<u8> {
        let out = crate::git::at(dir)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("HOME", dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "fixture")
            .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
            .env("GIT_COMMITTER_NAME", "fixture")
            .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        out.stdout
    }
    fn repo(root: &Path) {
        git(root, &["init", "-qb", "main"]);
        assert_eq!(
            std::str::from_utf8(&git(root, &["rev-parse", "--show-toplevel"]))
                .unwrap()
                .trim(),
            root.to_str().unwrap()
        );
        assert!(
            Path::new(
                std::str::from_utf8(&git(root, &["rev-parse", "--absolute-git-dir"]))
                    .unwrap()
                    .trim()
            )
            .starts_with(root)
        );
    }
    #[test]
    fn retained_verify_worktree() {
        if isolated("retained::verify::tests::retained_verify_worktree") {
            return;
        }
        let t = tempfile::tempdir().unwrap();
        let p = project(t.path());
        repo(t.path());
        std::fs::write(t.path().join(".gitignore"), "ignored\n").unwrap();
        std::fs::write(t.path().join("ignored"), "data").unwrap();
        let o = inspect(
            &p,
            &record(t.path(), "worktree"),
            None,
            &mut Metrics::default(),
        )
        .unwrap();
        assert_eq!(o.record.verification, "verified");
        assert_eq!(o.record.sha256, FieldString::Absent);
        assert!(o.git.unwrap().contains("!! ignored"));
    }
    #[test]
    fn retained_verify_preserved_payload() {
        if isolated("retained::verify::tests::retained_verify_preserved_payload") {
            return;
        }
        let t = tempfile::tempdir().unwrap();
        let p = project(t.path());
        repo(t.path());
        let path = t.path().join("report");
        std::fs::write(&path, "old").unwrap();
        git(t.path(), &["add", "."]);
        git(t.path(), &["commit", "-qm", "old"]);
        let commit = String::from_utf8(git(t.path(), &["rev-parse", "HEAD"]))
            .unwrap()
            .trim()
            .to_string();
        let b = Basis {
            project: "sci".into(),
            layout: "in-tree".into(),
            records_subdir: ".".into(),
            commit: commit.clone(),
            task_path: "tasks/sci-a00001.md".into(),
            retained_ref: "refs/heads/main".into(),
        };
        let old = inspect(
            &p,
            &record(&path, "evidence"),
            None,
            &mut Metrics::default(),
        )
        .unwrap();
        let loc = ArtifactLocator::Git {
            basis: b,
            path: "report".into(),
            sha256: old.record.sha256.value().unwrap().into(),
        };
        std::fs::write(&path, "new").unwrap();
        git(t.path(), &["add", "report"]);
        git(t.path(), &["commit", "-qm", "new"]);
        let new = inspect(&p, &old.record, Some(&loc), &mut Metrics::default()).unwrap();
        assert_eq!(new.record.verification, "verified");
        assert_ne!(new.record.sha256, old.record.sha256);
        assert_eq!(artifact(&p, &loc, &mut Metrics::default()).unwrap(), b"old");
    }
    #[test]
    fn retained_verify_exact_manifest() {
        let t = tempfile::tempdir().unwrap();
        let p = project(t.path());
        let root = t.path().join("tree");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("report"), "data").unwrap();
        let first = manifest(&root, &mut Metrics::default()).unwrap();
        let mut r = record(&root, "scratch");
        let bytes = serde_json::to_vec_pretty(&parsed(&first.bytes).unwrap()).unwrap();
        let mp = t.path().join("prior-manifest.json");
        std::fs::write(&mp, &bytes).unwrap();
        r.verification = "verified".into();
        r.verification_action = None;
        r.sha256 = FieldString::Text(sha(&bytes));
        r.manifest = FieldString::Text(mp.to_str().unwrap().into());
        let mut metrics = Metrics::default();
        let o = inspect(&p, &r, None, &mut metrics).unwrap();
        assert_eq!(o.record.verification, "verified");
        assert_eq!(o.manifest.unwrap().bytes, bytes);
        assert!(metrics.hash_ns > 0);
        assert_eq!(metrics.wall_ns, 0);
    }
    #[test]
    fn retained_verify_special_unreadable() {
        use std::os::unix::fs::PermissionsExt;
        let t = tempfile::tempdir().unwrap();
        let p = project(t.path());
        let fifo = t.path().join("fifo");
        let c = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        assert_eq!(
            inspect(
                &p,
                &record(&fifo, "evidence"),
                None,
                &mut Metrics::default()
            )
            .unwrap()
            .record
            .verification,
            "unknown"
        );
        assert!(manifest(t.path(), &mut Metrics::default()).is_err());
        std::fs::remove_file(fifo).unwrap();
        let path = t.path().join("unreadable");
        std::fs::write(&path, "secret").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        assert_eq!(
            inspect(
                &p,
                &record(&path, "evidence"),
                None,
                &mut Metrics::default()
            )
            .unwrap()
            .record
            .verification,
            "unknown"
        );
    }
    #[test]
    fn retained_verify_unstable() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("input");
        std::fs::write(&path, "before").unwrap();
        let f = open(&path, false).unwrap();
        let before = f.metadata().unwrap();
        stable(&path, &f, &before).unwrap();
        std::fs::write(&path, "after").unwrap();
        assert!(stable(&path, &f, &before).is_err());
        std::fs::remove_file(&path).unwrap();
        std::fs::write(&path, "before").unwrap();
        assert!(stable(&path, &f, &before).is_err());
    }
    #[test]
    fn retained_verify_directory_versions() {
        if isolated("retained::verify::tests::retained_verify_directory_versions") {
            return;
        }
        let t = tempfile::tempdir().unwrap();
        let p = project(t.path());
        repo(t.path());
        let root = t.path().join("tree");
        let prior = t.path().join("prior");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&prior).unwrap();
        std::fs::write(root.join("report"), "old").unwrap();
        std::fs::write(prior.join("report"), "old").unwrap();
        let first = manifest(&root, &mut Metrics::default()).unwrap();
        let mp = t.path().join("old-manifest.json");
        std::fs::write(&mp, &first.bytes).unwrap();
        git(t.path(), &["add", "."]);
        git(t.path(), &["commit", "-qm", "preserved"]);
        let commit = String::from_utf8(git(t.path(), &["rev-parse", "HEAD"]))
            .unwrap()
            .trim()
            .to_string();
        let b = Basis {
            project: "sci".into(),
            layout: "in-tree".into(),
            records_subdir: ".".into(),
            commit,
            task_path: "tasks/sci-a00001.md".into(),
            retained_ref: "refs/heads/main".into(),
        };
        let mut r = record(&root, "scratch");
        r.verification = "verified".into();
        r.verification_action = None;
        r.sha256 = FieldString::Text(first.sha256.clone());
        r.manifest = FieldString::Text(mp.to_str().unwrap().into());
        let loc = ArtifactLocator::Directory {
            host: crate::claims::hostname(),
            path: prior.to_str().unwrap().into(),
            manifest: Box::new(ArtifactLocator::Git {
                basis: b,
                path: "old-manifest.json".into(),
                sha256: first.sha256,
            }),
        };
        std::fs::write(root.join("report"), "new").unwrap();
        assert_eq!(
            inspect(&p, &r, None, &mut Metrics::default())
                .unwrap()
                .record
                .verification,
            "unknown"
        );
        let o = inspect(&p, &r, Some(&loc), &mut Metrics::default()).unwrap();
        assert_eq!(o.record.verification, "verified");
        assert_ne!(o.manifest.unwrap().bytes, first.bytes);
        std::fs::remove_file(prior.join("report")).unwrap();
        assert_eq!(
            inspect(&p, &r, Some(&loc), &mut Metrics::default())
                .unwrap()
                .record
                .verification,
            "unknown"
        );
        std::fs::write(&mp, b"{\"version\":2,\"entries\":[]}").unwrap();
        assert!(inspect(&p, &r, None, &mut Metrics::default()).is_err());
    }
    #[test]
    fn retained_verify_complete_manifest() {
        let t = tempfile::tempdir().unwrap();
        std::fs::write(t.path().join(".hidden"), "data").unwrap();
        std::fs::create_dir(t.path().join("empty")).unwrap();
        std::os::unix::fs::symlink("../../outside", t.path().join("link")).unwrap();
        let m = manifest(t.path(), &mut Metrics::default()).unwrap();
        let parsed = decode_manifest(std::str::from_utf8(&m.bytes).unwrap()).unwrap();
        assert_eq!(parsed.entries.len(), 3);
        assert!(
            matches!(&parsed.entries[2],ManifestEntry::Symlink{target,..} if target=="../../outside")
        );
        assert_eq!(m.sha256, format!("{:x}", Sha256::digest(&m.bytes)));
    }
    #[test]
    fn retained_verify_repeated_bytes() {
        let t = tempfile::tempdir().unwrap();
        std::fs::write(t.path().join("file"), "payload").unwrap();
        let mut m = Metrics::default();
        manifest(t.path(), &mut m).unwrap();
        let once = m.bytes_hashed;
        manifest(t.path(), &mut m).unwrap();
        assert_eq!(m.bytes_hashed, once * 2);
        assert!(once >= 7);
    }
    #[test]
    fn retained_verify_unknown() {
        let t = tempfile::tempdir().unwrap();
        let p = project(t.path());
        let r = record(&t.path().join("missing"), "evidence");
        let o = inspect(&p, &r, None, &mut Metrics::default()).unwrap();
        assert_eq!(o.record.verification, "unknown");
        assert_eq!(o.record.sha256, FieldString::Null);
        std::fs::write(t.path().join("file"), "new").unwrap();
        let o = inspect(
            &p,
            &record(&t.path().join("file"), "evidence"),
            None,
            &mut Metrics::default(),
        )
        .unwrap();
        assert_eq!(o.record.verification, "verified");
    }
    #[test]
    fn retained_verify_replaced_versions() {
        let t = tempfile::tempdir().unwrap();
        let p = project(t.path());
        let path = t.path().join("report");
        std::fs::write(&path, "old").unwrap();
        let old = inspect(
            &p,
            &record(&path, "evidence"),
            None,
            &mut Metrics::default(),
        )
        .unwrap();
        std::fs::write(&path, "new").unwrap();
        let next = inspect(&p, &old.record, None, &mut Metrics::default()).unwrap();
        assert_eq!(next.record.verification, "unknown");
        assert_ne!(old.record.sha256, FieldString::Null);
    }
    #[test]
    fn retained_verify_manifest_paths() {
        assert!(parsed(br#"{"version":1,"entries":[{"type":"directory","path":"same"},{"type":"directory","path":"same"}]}"#).is_err());
        assert!(
            decode_manifest(r#"{"version":1,"entries":[{"type":"directory","path":"../escape"}]}"#)
                .is_err()
        );
    }
}
