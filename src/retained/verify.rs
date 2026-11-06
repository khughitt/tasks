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
    #[serde(skip)]
    owners: Vec<(crate::model::TaskId, String)>,
    #[serde(skip)]
    pulse_at: Option<Instant>,
}
impl Metrics {
    pub fn track(&mut self, owners: Vec<(crate::model::TaskId, String)>) {
        self.owners = owners;
        self.pulse_at = Some(Instant::now());
    }
    pub fn finish(&self) -> Self {
        let (bytes_hashed, hash_ns) = crate::history::hash_work();
        Self {
            wall_ns: self.wall_ns,
            hash_ns,
            bytes_hashed,
            history_bytes_read: crate::history::metadata_bytes_read(),
            ..Self::default()
        }
    }
    fn pulse(&mut self) -> Result<()> {
        if self.owners.is_empty()
            || self
                .pulse_at
                .is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(60))
        {
            return Ok(());
        }
        for (id, session) in &self.owners {
            let update = (|| -> Result<()> {
                let mut store = crate::claims::ClaimStore::load(&id.prefix)?;
                let mut claim = store
                    .get(id)
                    .cloned()
                    .ok_or_else(|| invalid("claim disappeared during verification"))?;
                if claim.session != *session || crate::claims::liveness(&claim).is_stale() {
                    return Err(invalid("claim changed or expired during verification"));
                }
                claim.seen = crate::time::now();
                store.insert(id, claim);
                store.save()?;
                Ok(())
            })();
            update.map_err(|e|crate::error::Error::RetainedOwnership(format!("Retained claim heartbeat on {id} failed: {e}; re-establish ownership before retry")))?;
        }
        self.pulse_at = Some(Instant::now());
        Ok(())
    }
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
    pub preservations: Vec<ArtifactLocator>,
}
pub fn sha(bytes: &[u8]) -> String {
    crate::history::sha256(bytes)
}
pub fn hash_bytes(bytes: &[u8], metrics: &mut Metrics) -> String {
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
    let before = metrics.bytes_hashed;
    let result = (|| {
        let mut hash = Sha256::new();
        let mut buf = [0u8; 65536];
        loop {
            metrics.pulse()?;
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hash.update(&buf[..n]);
            metrics.bytes_hashed += n as u64;
        }
        Ok(format!("{:x}", hash.finalize()))
    })();
    let ns = start.elapsed().as_nanos().min(u64::MAX as u128) as u64;
    metrics.hash_ns += ns;
    crate::history::record_hash(metrics.bytes_hashed - before, ns);
    result
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
    snapshots: &mut std::collections::BTreeMap<String, Metadata>,
    metrics: &mut Metrics,
) -> Result<()> {
    let handle = open(dir, true)?;
    let before = handle.metadata()?;
    snapshots.insert(prefix.to_string(), before.clone());
    // Held directory descriptors prevent a replaced ancestor from redirecting enumeration.
    let held = PathBuf::from(format!("/proc/self/fd/{}", handle.as_raw_fd()));
    let mut names = Vec::new();
    for entry in std::fs::read_dir(&held)? {
        metrics.pulse()?;
        names.push(entry?.file_name());
    }
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
        snapshots.insert(path.clone(), meta.clone());
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
            walk(&physical, &path, entries, snapshots, metrics)?;
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
// Recheck every member after the entire hash pass; parent mtimes do not cover file edits.
fn recheck(
    dir: &Path,
    prefix: &str,
    snapshots: &std::collections::BTreeMap<String, Metadata>,
    metrics: &mut Metrics,
) -> Result<()> {
    let handle = open(dir, true)?;
    let before = snapshots
        .get(prefix)
        .ok_or_else(|| invalid("directory inventory changed"))?;
    stable(dir, &handle, before)?;
    let held = PathBuf::from(format!("/proc/self/fd/{}", handle.as_raw_fd()));
    for entry in std::fs::read_dir(&held)? {
        metrics.pulse()?;
        let entry = entry?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| invalid("member name changed"))?;
        let key = if prefix.is_empty() {
            name.into()
        } else {
            format!("{prefix}/{name}")
        };
        let expected = snapshots
            .get(&key)
            .ok_or_else(|| invalid("new retained member during inspection"))?;
        let path = held.join(name);
        let actual = std::fs::symlink_metadata(&path)?;
        if stamp(expected) != stamp(&actual) {
            return Err(invalid("retained member changed during inventory"));
        }
        if actual.is_dir() {
            recheck(&path, &key, snapshots, metrics)?;
        }
    }
    stable(dir, &handle, before)
}
pub fn manifest(root: &Path, metrics: &mut Metrics) -> Result<ManifestBytes> {
    manifest_observed(root, metrics, || {})
}
fn manifest_observed(
    root: &Path,
    metrics: &mut Metrics,
    after: impl FnOnce(),
) -> Result<ManifestBytes> {
    let mut entries = Vec::new();
    let mut snapshots = std::collections::BTreeMap::new();
    walk(root, "", &mut entries, &mut snapshots, metrics)?;
    after();
    recheck(root, "", &snapshots, metrics)?;
    entries.sort_by(|a, b| a.path().cmp(b.path()));
    let bytes = serde_json::to_vec(&Manifest {
        version: 1,
        entries,
    })
    .map_err(invalid)?;
    let sha256 = hash_bytes(&bytes, metrics);
    Ok(ManifestBytes { bytes, sha256 })
}
pub fn current_path(project: &Project, path: &str) -> Result<PathBuf> {
    if let Some(s) = path.strip_prefix("$WORK_ROOT/") {
        let root = std::env::var_os("WORK_ROOT")
            .ok_or_else(|| invalid("WORK_ROOT is unavailable on this host"))?;
        Ok(PathBuf::from(root).join(s))
    } else if path.starts_with(".worktrees/") {
        let main = if project.kind == crate::repo::Kind::InTree {
            project
                .main_checkout_root()?
                .unwrap_or_else(|| project.checkout.clone())
        } else {
            project.checkout.clone()
        };
        Ok(main.join(path))
    } else {
        Ok(PathBuf::from(path))
    }
}
pub fn read_local(path: &Path, cap: u64) -> Result<Vec<u8>> {
    let mut file = open(path, false)?;
    let before = file.metadata()?;
    let mut bytes = Vec::new();
    let read = Read::by_ref(&mut file)
        .take(cap.saturating_add(1))
        .read_to_end(&mut bytes);
    crate::history::record_metadata_read(bytes.len());
    read?;
    if bytes.len() as u64 > cap {
        return Err(invalid("retained metadata exceeds attachment cap"));
    }
    stable(path, &file, &before)?;
    Ok(bytes)
}
fn artifact_location(
    resolver: &super::proof::Resolver<'_>,
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
    let resolved = resolver.prefix(project, declared)?;
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
    if !super::lineage::owns_prefix(project, &b.project) {
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
    artifact_with(
        &super::proof::Resolver::new(project),
        project,
        locator,
        metrics,
    )
}
pub(super) fn artifact_with(
    resolver: &super::proof::Resolver<'_>,
    project: &Project,
    locator: &ArtifactLocator,
    metrics: &mut Metrics,
) -> Result<Vec<u8>> {
    let (p, b, path, expected) = artifact_location(resolver, project, locator, metrics)?;
    let bytes = basis::blob(
        &p,
        &b.commit,
        &path,
        Some(p.attachments_max_bytes),
        &mut metrics.history_bytes_read,
    )?;
    if hash_bytes(&bytes, metrics) != expected {
        return Err(invalid("preserved artifact digest mismatch"));
    }
    Ok(bytes)
}
fn payload_digest(
    project: &Project,
    locator: &ArtifactLocator,
    metrics: &mut Metrics,
) -> Result<String> {
    let (p, b, path, expected) = artifact_location(
        &super::proof::Resolver::new(project),
        project,
        locator,
        metrics,
    )?;
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
    let before = metrics.bytes_hashed;
    let result = hash_stream(&mut child.stdout.take().expect("piped stdout"), metrics);
    crate::history::record_metadata_read((metrics.bytes_hashed - before) as usize);
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
    let directory = r.kind == "scratch" || r.manifest.value().is_some();
    if directory != matches!(locator, ArtifactLocator::Directory { .. }) {
        return Err(invalid(
            "prior directories require a complete member root; a manifest alone is not a payload version",
        ));
    }
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
            if hash_bytes(&old, metrics) != expected {
                return Err(invalid("preserved manifest differs from previous digest"));
            }
            let root = current_path(project, path)?;
            let observed = manifest(&root, metrics)?;
            let expected = parsed(&old)?;
            let actual = parsed(&observed.bytes)?;
            if expected
                .entries
                .iter()
                .any(|wanted| !actual.entries.contains(wanted))
            {
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
pub fn unknown(mut r: RetentionRecord, detail: impl std::fmt::Display) -> Observation {
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
        preservations: Vec::new(),
    }
}
pub fn preflight(
    project: &Project,
    _record: &RetentionRecord,
    manifest: Option<&Path>,
) -> Result<()> {
    if let Some(path) = manifest
        && let Ok(bytes) = read_local(path, project.attachments_max_bytes)
        && let Ok(v) = strict(&bytes)
        && v.get("version").and_then(|v| v.as_u64()) != Some(1)
    {
        return Err(invalid(
            "unsupported historical manifest format; preserve and migrate it explicitly",
        ));
    }
    Ok(())
}
pub fn inspect(
    project: &Project,
    record: &RetentionRecord,
    preserved: Option<&ArtifactLocator>,
    manifest: Option<&Path>,
    metrics: &mut Metrics,
) -> Result<Observation> {
    record.validate()?;
    if record.host != crate::claims::hostname() {
        return Err(invalid("inspection is restricted to the executing host"));
    }
    if record.state != "kept" {
        return Err(invalid("only kept identities can be inspected"));
    }
    preflight(project, record, manifest)?;
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
                preservations: Vec::new(),
            });
        }
        let meta = std::fs::symlink_metadata(&path)?;
        if r.kind == "scratch" && !meta.is_dir() {
            return Err(invalid("scratch identity must be a directory"));
        }
        let mut generated = None;
        let digest = if meta.is_dir() {
            let m = self::manifest(&path, metrics)?;
            let mut candidate = m;
            if let Some(oldpath) = manifest {
                let old = read_local(oldpath, project.attachments_max_bytes)?;
                let oldparsed = parsed(&old)?;
                let old_hash = hash_bytes(&old, metrics);
                if record.sha256.value() != Some(old_hash.as_str()) {
                    return Err(invalid("previous manifest digest mismatch"));
                }
                if oldparsed == parsed(&candidate.bytes)? {
                    candidate = ManifestBytes {
                        sha256: old_hash,
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
        let previous_directory = record.kind == "scratch" || record.manifest.value().is_some();
        if record.verification == "verified"
            && (record.sha256.value() != Some(digest.as_str())
                || meta.is_dir() != previous_directory)
        {
            previous(
                project,
                record,
                preserved
                    .ok_or_else(|| invalid("changed payload requires preserved prior bytes"))?,
                metrics,
            )?;
        }
        if !meta.is_dir() {
            r.manifest = FieldString::Absent;
        }
        r.verification = "verified".into();
        r.sha256 = FieldString::Text(digest);
        r.verification_action = None;
        Ok(Observation {
            record: r,
            manifest: generated,
            git: None,
            preservations: Vec::new(),
        })
    })();
    match attempt {
        Err(e @ crate::error::Error::RetainedOwnership(_)) => Err(e),
        Err(e) => Ok(unknown(record.clone(), e)),
        Ok(observation) => Ok(observation),
    }
}

pub fn version_key(locator: &ArtifactLocator) -> Result<(bool, String)> {
    match locator {
        ArtifactLocator::Git { sha256, .. } | ArtifactLocator::Attachment { sha256, .. } => {
            Ok((false, sha256.clone()))
        }
        ArtifactLocator::Directory { manifest, .. } => {
            let (_, digest) = version_key(manifest)?;
            Ok((true, digest))
        }
    }
}
pub fn preserved(
    project: &Project,
    locator: &ArtifactLocator,
    metrics: &mut Metrics,
) -> Result<()> {
    match locator {
        ArtifactLocator::Directory {
            host,
            path,
            manifest: locator,
        } => {
            if *host != crate::claims::hostname() {
                return Err(invalid("prior member root is on another host"));
            }
            let saved = artifact(project, locator, metrics)?;
            let wanted = parsed(&saved)?;
            let actual = manifest(&current_path(project, path)?, metrics)?;
            let actual = parsed(&actual.bytes)?;
            if wanted
                .entries
                .iter()
                .any(|entry| !actual.entries.contains(entry))
            {
                return Err(invalid(
                    "prior member root lacks a required immutable version",
                ));
            }
            Ok(())
        }
        _ => payload_digest(project, locator, metrics).map(|_| ()),
    }
}
fn references(
    project: &Project,
    pinned: &basis::PinnedTask,
    identity: &Identity,
    metrics: &mut Metrics,
    active: &mut std::collections::BTreeSet<(String, String, Identity)>,
) -> Result<std::collections::BTreeMap<(bool, String), ArtifactLocator>> {
    let key = (
        pinned.task.id.to_string(),
        pinned.basis.commit.clone(),
        identity.clone(),
    );
    if !active.insert(key.clone()) {
        return Err(invalid("cycle in preserved ownership evidence"));
    }
    let result = (|| {
        let mut versions = std::collections::BTreeMap::new();
        let start = pinned
            .task
            .notes
            .iter()
            .rposition(|n| {
                n.text
                    .strip_prefix("retained: ")
                    .and_then(|r| decode_record(r).ok())
                    .is_some_and(|r| r.identity() == *identity && r.state == "released")
            })
            .map_or(0, |i| i + 1);
        for (offset, note) in pinned.task.notes[start..].iter().enumerate() {
            metrics.pulse()?;
            let Some(raw) = note.text.strip_prefix("retention operation: ") else {
                continue;
            };
            let operation = decode_operation(raw)?;
            let input = super::lineage::participant(project, &operation, &pinned.task.id)?.clone();
            match operation.outputs {
                PhaseOutputs::Receiver(output) => {
                    for mapping in output.mapping.iter().filter(|m| m.receiver == *identity) {
                        let source = operation
                            .input_bases
                            .iter()
                            .find(|i| i.task == operation.key.coordinator)
                            .ok_or_else(|| {
                                invalid("receiver phase lacks its exact sender input")
                            })?;
                        let source_id = crate::model::TaskId::parse(&source.task)?;
                        let source_project =
                            if super::lineage::owns_prefix(project, &source.basis.project) {
                                project.clone()
                            } else {
                                crate::scope::open_registered(
                                    &crate::registry::Registry::load()?,
                                    &source.basis.project,
                                    crate::scope::Origin::Id(&source_id),
                                )?
                            };
                        let source = basis::read_exact(&source_project, &source_id, &source.basis)?;
                        metrics.history_bytes_read += source.history_bytes_read;
                        versions.extend(references(
                            &source_project,
                            &source,
                            &mapping.sender,
                            metrics,
                            active,
                        )?);
                    }
                }
                PhaseOutputs::Items(output) => {
                    if output.artifacts.is_empty() {
                        continue;
                    }
                    let publication = basis::publication_basis(
                        project,
                        &pinned.task.id,
                        &input.basis,
                        &pinned.task.notes[..=start + offset],
                        &mut metrics.history_bytes_read,
                    )?;
                    let loc = operation.request.pinned(&input.task, &publication)?;
                    let bytes = artifact(project, &loc, metrics)?;
                    let request: CheckpointRequest = decode_request(&bytes)?;
                    request.validate()?;
                    for previous in request
                        .preserved_previous
                        .iter()
                        .filter(|p| p.identity == *identity)
                    {
                        for candidate in &output.artifacts {
                            let candidate_key = version_key(candidate)?;
                            let derived = matches!((candidate,&previous.artifact),(ArtifactLocator::Directory{host,path,..},ArtifactLocator::Directory{host:expected_host,path:expected_path,..}) if host==expected_host&&path==expected_path)
                                && versions.contains_key(&candidate_key);
                            if *candidate == previous.artifact || derived {
                                versions.insert(candidate_key, candidate.clone());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(versions)
    })();
    active.remove(&key);
    result
}
pub fn inspect_history(
    project: &Project,
    pinned: &basis::PinnedTask,
    record: &RetentionRecord,
    provided: Option<&ArtifactLocator>,
    metrics: &mut Metrics,
) -> Result<Observation> {
    let known = super::state::replay(project, &pinned.task, &pinned.basis, Some(&record.host))?
        .items
        .iter()
        .any(|i| {
            i.identity == record.identity() && i.record.as_ref().is_some_and(|r| r.state == "kept")
        });
    let required = if known {
        super::state::verification_requirement(&pinned.task, record)
    } else {
        record.clone()
    };
    let mut versions = if known && record.kind != "worktree" {
        references(
            project,
            pinned,
            &record.identity(),
            metrics,
            &mut std::collections::BTreeSet::new(),
        )?
    } else {
        std::collections::BTreeMap::new()
    };
    let manifest = required
        .manifest
        .value()
        .map(|_| super::lineage::manifest_path(project, pinned, &required))
        .transpose()?;
    let mut observation = inspect(project, &required, provided, manifest.as_deref(), metrics)?;
    let mut proved = std::collections::BTreeSet::new();
    let mut used = Vec::new();
    if let Some(locator) = provided {
        let key = version_key(locator)?;
        let current = required.sha256.value().map(|sha| {
            (
                required.kind == "scratch" || required.manifest.value().is_some(),
                sha.to_string(),
            )
        });
        if record.kind == "scratch" && !key.0 {
            return Err(invalid(
                "scratch preservation needs a complete directory member root",
            ));
        }
        if known
            && required.verification == "verified"
            && current.as_ref() != Some(&key)
            && !versions.contains_key(&key)
        {
            return Err(invalid(
                "preservation does not match a current or referenced prior version",
            ));
        }
        match preserved(project, locator, metrics) {
            Ok(()) => {
                versions.insert(key.clone(), locator.clone());
                proved.insert(key);
                used.push(locator.clone());
            }
            Err(e @ crate::error::Error::RetainedOwnership(_)) => return Err(e),
            Err(e) => observation = unknown(record.clone(), e),
        }
    }
    let current = if observation.record.verification == "verified" {
        observation
            .record
            .sha256
            .value()
            .map(|sha| (observation.manifest.is_some(), sha.to_string()))
    } else {
        None
    };
    for (key, locator) in versions {
        if current.as_ref() == Some(&key) || proved.contains(&key) {
            continue;
        }
        let mut checked = preserved(project, &locator, metrics);
        if checked.is_err()
            && let (
                ArtifactLocator::Directory { manifest, .. },
                Some(ArtifactLocator::Directory { host, path, .. }),
            ) = (&locator, provided)
        {
            let rebound = ArtifactLocator::Directory {
                host: host.clone(),
                path: path.clone(),
                manifest: manifest.clone(),
            };
            checked = preserved(project, &rebound, metrics);
            if checked.is_ok() {
                used.push(rebound);
            }
        }
        match checked {
            Ok(()) => {}
            Err(e @ crate::error::Error::RetainedOwnership(_)) => return Err(e),
            Err(e) => observation = unknown(record.clone(), e),
        }
    }
    observation.preservations = used;
    Ok(observation)
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
            (&record(t.path(), "worktree"))
                .manifest
                .value()
                .map(Path::new),
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
            (&record(&path, "evidence")).manifest.value().map(Path::new),
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
        let new = inspect(
            &p,
            &old.record,
            Some(&loc),
            (&old.record).manifest.value().map(Path::new),
            &mut Metrics::default(),
        )
        .unwrap();
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
        let o = inspect(
            &p,
            &r,
            None,
            (&r).manifest.value().map(Path::new),
            &mut metrics,
        )
        .unwrap();
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
                (&record(&fifo, "evidence")).manifest.value().map(Path::new),
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
                (&record(&path, "evidence")).manifest.value().map(Path::new),
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
            inspect(
                &p,
                &r,
                None,
                (&r).manifest.value().map(Path::new),
                &mut Metrics::default()
            )
            .unwrap()
            .record
            .verification,
            "unknown"
        );
        let bare = match &loc {
            ArtifactLocator::Directory { manifest, .. } => manifest.as_ref(),
            _ => unreachable!(),
        };
        assert_eq!(
            inspect(
                &p,
                &r,
                Some(bare),
                (&r).manifest.value().map(Path::new),
                &mut Metrics::default()
            )
            .unwrap()
            .record
            .verification,
            "unknown",
            "manifest bytes alone do not preserve directory members"
        );
        std::fs::write(prior.join("additional-report"), "later version").unwrap();
        let o = inspect(
            &p,
            &r,
            Some(&loc),
            (&r).manifest.value().map(Path::new),
            &mut Metrics::default(),
        )
        .unwrap();
        assert_eq!(o.record.verification, "verified");
        assert_ne!(o.manifest.unwrap().bytes, first.bytes);
        std::fs::remove_file(prior.join("report")).unwrap();
        assert_eq!(
            inspect(
                &p,
                &r,
                Some(&loc),
                (&r).manifest.value().map(Path::new),
                &mut Metrics::default()
            )
            .unwrap()
            .record
            .verification,
            "unknown"
        );
        std::fs::write(&mp, b"{\"version\":2,\"entries\":[]}").unwrap();
        assert!(
            inspect(
                &p,
                &r,
                None,
                (&r).manifest.value().map(Path::new),
                &mut Metrics::default()
            )
            .is_err()
        );
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
    fn retained_verify_command_hash_totals_include_direct_and_partial_work() {
        let before = Metrics::default().finish();
        sha(b"abc");
        let mut local = Metrics::default();
        hash_bytes(b"def", &mut local);
        struct Interrupted(bool);
        impl Read for Interrupted {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if self.0 {
                    return Err(std::io::Error::other("interrupted fixture"));
                }
                self.0 = true;
                buf[..3].copy_from_slice(b"ghi");
                Ok(3)
            }
        }
        assert!(hash_stream(&mut Interrupted(false), &mut local).is_err());
        let after = Metrics::default().finish();
        assert_eq!(after.bytes_hashed - before.bytes_hashed, 9);
        assert_eq!(local.bytes_hashed, 6);
        assert!(after.hash_ns > before.hash_ns);
    }
    #[test]
    fn retained_verify_unknown() {
        let t = tempfile::tempdir().unwrap();
        let p = project(t.path());
        let r = record(&t.path().join("missing"), "evidence");
        let o = inspect(
            &p,
            &r,
            None,
            (&r).manifest.value().map(Path::new),
            &mut Metrics::default(),
        )
        .unwrap();
        assert_eq!(o.record.verification, "unknown");
        assert_eq!(o.record.sha256, FieldString::Null);
        std::fs::write(t.path().join("file"), "new").unwrap();
        let o = inspect(
            &p,
            &record(&t.path().join("file"), "evidence"),
            None,
            (&record(&t.path().join("file"), "evidence"))
                .manifest
                .value()
                .map(Path::new),
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
            (&record(&path, "evidence")).manifest.value().map(Path::new),
            &mut Metrics::default(),
        )
        .unwrap();
        std::fs::write(&path, "new").unwrap();
        let next = inspect(
            &p,
            &old.record,
            None,
            (&old.record).manifest.value().map(Path::new),
            &mut Metrics::default(),
        )
        .unwrap();
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
    #[test]
    fn retained_verify_claim_heartbeat() {
        if isolated("retained::verify::tests::retained_verify_claim_heartbeat") {
            return;
        }
        let _lock = crate::claims::MutationLock::acquire("sci").unwrap();
        let id = crate::model::TaskId::parse("sci-a00001").unwrap();
        let mut claim:crate::claims::Claim=serde_json::from_value(serde_json::json!({"owner":"tester","session":"owned","host":crate::claims::hostname(),"worktree":"/fixture","started":crate::time::now(),"seen":crate::time::now()})).unwrap();
        let mut store = crate::claims::ClaimStore::load("sci").unwrap();
        store.insert(&id, claim.clone());
        store.save().unwrap();
        let mut metrics = Metrics::default();
        metrics.track(vec![(id.clone(), "owned".into())]);
        metrics.pulse_at = None;
        metrics.pulse().unwrap();
        assert_eq!(
            crate::claims::ClaimStore::load("sci")
                .unwrap()
                .get(&id)
                .unwrap()
                .session,
            "owned"
        );
        claim.seen = "2020-01-01T00:00:00Z".into();
        store.insert(&id, claim);
        store.save().unwrap();
        metrics.pulse_at = None;
        assert!(matches!(
            metrics.pulse(),
            Err(crate::error::Error::RetainedOwnership(_))
        ));
        assert_eq!(
            crate::claims::ClaimStore::load("sci")
                .unwrap()
                .get(&id)
                .unwrap()
                .seen,
            "2020-01-01T00:00:00Z"
        );
        let serialized = serde_json::to_value(metrics.finish()).unwrap();
        assert_eq!(serialized.as_object().unwrap().len(), 4);
    }
    #[test]
    fn retained_verify_relative_main_checkout() {
        if isolated("retained::verify::tests::retained_verify_relative_main_checkout") {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let p = project(root.path());
        repo(root.path());
        std::fs::write(root.path().join(".gitignore"), ".worktrees/\n").unwrap();
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-qm", "project"]);
        let tree = root.path().join(".worktrees/feature");
        git(
            root.path(),
            &["worktree", "add", "-qb", "feature", tree.to_str().unwrap()],
        );
        crate::common::git_fixture::assert_worktree(root.path(), root.path(), &tree);
        let branch = Project::open(&tree).unwrap();
        let mut record = record(&tree, "worktree");
        record.path = ".worktrees/feature".into();
        let observation = inspect(
            &branch,
            &record,
            None,
            (&record).manifest.value().map(Path::new),
            &mut Metrics::default(),
        )
        .unwrap();
        assert_eq!(observation.record.verification, "verified");
        assert_eq!(observation.record.path, ".worktrees/feature");
        assert_eq!(
            current_path(&branch, &record.path)
                .unwrap()
                .canonicalize()
                .unwrap(),
            tree.canonicalize().unwrap()
        );
        assert_eq!(
            current_path(&p, &record.path)
                .unwrap()
                .canonicalize()
                .unwrap(),
            tree.canonicalize().unwrap()
        );
    }
    #[test]
    fn retained_verify_directory_to_file() {
        if isolated("retained::verify::tests::retained_verify_directory_to_file") {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let p = project(root.path());
        repo(root.path());
        let tree = root.path().join("tree");
        let prior = root.path().join("prior");
        std::fs::create_dir(&tree).unwrap();
        std::fs::create_dir(&prior).unwrap();
        std::fs::write(tree.join("report"), "old").unwrap();
        std::fs::write(prior.join("report"), "old").unwrap();
        let m = manifest(&tree, &mut Metrics::default()).unwrap();
        let path = root.path().join("manifest.json");
        std::fs::write(&path, &m.bytes).unwrap();
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-qm", "prior version"]);
        let commit = String::from_utf8(git(root.path(), &["rev-parse", "HEAD"]))
            .unwrap()
            .trim()
            .to_string();
        let mut r = record(&tree, "evidence");
        r.verification = "verified".into();
        r.verification_action = None;
        r.sha256 = FieldString::Text(m.sha256.clone());
        r.manifest = FieldString::Text(path.to_str().unwrap().into());
        let prior = ArtifactLocator::Directory {
            host: crate::claims::hostname(),
            path: prior.to_str().unwrap().into(),
            manifest: Box::new(ArtifactLocator::Git {
                basis: Basis {
                    project: "sci".into(),
                    layout: "in-tree".into(),
                    records_subdir: ".".into(),
                    commit,
                    task_path: "tasks/sci-a00001.md".into(),
                    retained_ref: "refs/heads/main".into(),
                },
                path: "manifest.json".into(),
                sha256: m.sha256,
            }),
        };
        std::fs::remove_dir_all(&tree).unwrap();
        std::fs::write(&tree, &m.bytes).unwrap();
        assert_eq!(
            inspect(
                &p,
                &r,
                None,
                (&r).manifest.value().map(Path::new),
                &mut Metrics::default()
            )
            .unwrap()
            .record
            .verification,
            "unknown",
            "equal file/manifest bytes do not establish the prior directory payload"
        );
        let observed = inspect(
            &p,
            &r,
            Some(&prior),
            (&r).manifest.value().map(Path::new),
            &mut Metrics::default(),
        )
        .unwrap();
        assert_eq!(observed.record.verification, "verified");
        assert_eq!(observed.record.manifest, FieldString::Absent);
        assert!(observed.manifest.is_none());
    }
    #[test]
    fn retained_verify_directory_late_member_change() {
        let root = tempfile::tempdir().unwrap();
        let early = root.path().join("a");
        std::fs::write(&early, "old").unwrap();
        std::fs::write(root.path().join("z"), "last").unwrap();
        assert!(
            manifest_observed(root.path(), &mut Metrics::default(), || {
                std::fs::write(&early, "new").unwrap();
            })
            .is_err(),
            "a member changed after hashing but before the inventory was complete"
        );
    }
    #[test]
    fn retained_verify_locator_task_project() {
        if isolated("retained::verify::tests::retained_verify_locator_task_project") {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let p = project(root.path());
        repo(root.path());
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-qm", "project"]);
        let commit = String::from_utf8(git(root.path(), &["rev-parse", "HEAD"]))
            .unwrap()
            .trim()
            .to_string();
        let id = crate::model::TaskId::parse("fam-a00001").unwrap();
        let basis = Basis {
            project: "sci".into(),
            layout: "in-tree".into(),
            records_subdir: ".".into(),
            task_path: "tasks/fam-a00001.md".into(),
            commit,
            retained_ref: "refs/heads/main".into(),
        };
        assert!(
            basis::validate_locator(&p, &id, &basis).is_err(),
            "a task locator must use its actual project prefix"
        );
    }
    #[test]
    fn retained_verify_locator_requires_an_existing_named_branch() {
        if isolated(
            "retained::verify::tests::retained_verify_locator_requires_an_existing_named_branch",
        ) {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let p = project(root.path());
        repo(root.path());
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-qm", "project"]);
        let commit = String::from_utf8(git(root.path(), &["rev-parse", "HEAD"]))
            .unwrap()
            .trim()
            .to_string();
        let id = crate::model::TaskId::parse("sci-a00001").unwrap();
        let mut b = Basis {
            project: "sci".into(),
            layout: "in-tree".into(),
            records_subdir: ".".into(),
            task_path: "tasks/sci-a00001.md".into(),
            commit,
            retained_ref: "refs/heads/main".into(),
        };
        assert!(basis::validate_locator(&p, &id, &b).is_ok());
        for expression in [
            "refs/heads/main^{commit}",
            "refs/heads/main~0",
            "refs/heads/absent",
        ] {
            b.retained_ref = expression.into();
            assert!(
                basis::validate_locator(&p, &id, &b).is_err(),
                "{expression} is not an existing named branch"
            );
        }
    }
}
