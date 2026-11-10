//! Invocation-owned Git snapshots for retention validation; ordinary readers stay live.
use crate::{
    error::{Error, Result},
    model::Task,
    repo::Project,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    os::unix::process::ExitStatusExt,
    path::PathBuf,
    process::{Command, ExitStatus, Output},
    sync::{Arc, Mutex, MutexGuard},
};

#[derive(Clone, Default)]
pub(crate) struct Session(Arc<Mutex<SessionState>>);
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Session")
    }
}
struct SessionState {
    aliases: BTreeMap<PathBuf, RepoKey>,
    repositories: BTreeMap<RepoKey, Arc<Snapshot>>,
    lineages: BTreeSet<String>,
    max_entries: usize,
    max_bytes: usize,
    used_bytes: usize,
    queries: BTreeMap<QueryKey, Output>,
    tasks: BTreeMap<TaskKey, Task>,
}
type RepoKey = (PathBuf, PathBuf, PathBuf);
type QueryKey = (RepoKey, Vec<String>);
type TaskKey = (RepoKey, String, String, u64);
impl Default for SessionState {
    fn default() -> Self {
        Self {
            aliases: BTreeMap::new(),
            repositories: BTreeMap::new(),
            lineages: BTreeSet::new(),
            queries: BTreeMap::new(),
            tasks: BTreeMap::new(),
            max_entries: 2048,
            max_bytes: 32 * 1024 * 1024,
            used_bytes: 0,
        }
    }
}
impl SessionState {
    fn admit(&mut self, charge: usize) -> bool {
        if self.queries.len() + self.tasks.len() + self.lineages.len() >= self.max_entries {
            return false;
        }
        let Some(total) = self.used_bytes.checked_add(charge) else {
            return false;
        };
        if total > self.max_bytes {
            return false;
        }
        self.used_bytes = total;
        true
    }
}
struct Snapshot {
    identity: RepoKey,
    root: PathBuf,
    head: String,
    symbolic_head: Option<String>,
    branches: Vec<(String, String)>,
}
pub(crate) struct ReadOutput {
    pub output: Output,
    pub bytes_read: u64,
}
fn invalid(message: impl std::fmt::Display) -> Error {
    Error::RetainedHistoryBlocked(message.to_string())
}
impl Session {
    fn lock(&self) -> Result<MutexGuard<'_, SessionState>> {
        self.0
            .lock()
            .map_err(|_| invalid("validation read session lock poisoned"))
    }
    fn snapshot(&self, p: &Project) -> Result<Arc<Snapshot>> {
        {
            let state = self.lock()?;
            if let Some(key) = state.aliases.get(&p.records) {
                return Ok(state.repositories[key].clone());
            }
        }
        let discovery = crate::git::at(&p.records)
            .args([
                "rev-parse",
                "--path-format=absolute",
                "--show-toplevel",
                "--absolute-git-dir",
                "--git-common-dir",
            ])
            .output()?;
        let paths = text(checked(discovery)?)?
            .lines()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        if paths.len() != 3 {
            return Err(invalid("invalid Git repository identity"));
        }
        let key = (paths[0].clone(), paths[1].clone(), paths[2].clone());
        {
            let mut state = self.lock()?;
            if let Some(found) = state.repositories.get(&key).cloned() {
                state.aliases.insert(p.records.clone(), key);
                return Ok(found);
            }
        }
        let root = paths[0].clone();
        let head = text(checked(raw(
            &root,
            &["rev-parse", "--verify", "HEAD^{commit}"],
        )?)?)?
        .trim()
        .to_string();
        let symbolic = raw(&root, &["symbolic-ref", "-q", "HEAD"])?;
        let symbolic_head = match symbolic.status.code() {
            Some(0) => Some(text(symbolic.stdout)?.trim().to_string()),
            Some(1) => None,
            _ => return Err(invalid(String::from_utf8_lossy(&symbolic.stderr))),
        };
        let refs = text(checked(raw(
            &root,
            &[
                "for-each-ref",
                "--format=%(refname) %(objectname)",
                "refs/heads/",
            ],
        )?)?)?;
        let branches = refs
            .lines()
            .map(|line| {
                let (name, oid) = line
                    .split_once(' ')
                    .ok_or_else(|| invalid("invalid branch snapshot"))?;
                if !oid_is_full(oid) {
                    return Err(invalid("invalid branch object ID"));
                }
                Ok((name.to_string(), oid.to_string()))
            })
            .collect::<Result<Vec<_>>>()?;
        if !oid_is_full(&head) {
            return Err(invalid("invalid HEAD object ID"));
        }
        let snapshot = Arc::new(Snapshot {
            identity: key.clone(),
            root,
            head,
            symbolic_head,
            branches,
        });
        let mut state = self.lock()?;
        let snapshot = state
            .repositories
            .entry(key.clone())
            .or_insert(snapshot)
            .clone();
        state.aliases.insert(p.records.clone(), key);
        Ok(snapshot)
    }
}
fn text(bytes: Vec<u8>) -> Result<String> {
    String::from_utf8(bytes).map_err(invalid)
}
fn checked(out: Output) -> Result<Vec<u8>> {
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(invalid(String::from_utf8_lossy(&out.stderr)))
    }
}
fn oid_is_full(s: &str) -> bool {
    s.len() == 40
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn raw(root: &std::path::Path, args: &[&str]) -> Result<Output> {
    Ok(crate::git::at(root)
        .arg("--literal-pathspecs")
        .args(args)
        .output()?)
}
fn reply(code: i32, stdout: String) -> ReadOutput {
    ReadOutput {
        output: Output {
            status: ExitStatus::from_raw(code << 8),
            stdout: stdout.into_bytes(),
            stderr: Vec::new(),
        },
        bytes_read: 0,
    }
}
impl Snapshot {
    fn resolve(&self, revision: &str) -> Result<String> {
        let revision = revision.strip_suffix("^{commit}").unwrap_or(revision);
        if revision == "HEAD" {
            return Ok(self.head.clone());
        }
        if revision.starts_with("refs/heads/") {
            return self.branches.iter().find(|(name, _)| name == revision).map(|(_, id)| id.clone()).ok_or_else(|| invalid(format!("Retained ref {revision} is not an existing named branch; recover that exact ref")));
        }
        if oid_is_full(revision) {
            return Ok(revision.to_string());
        }
        Err(invalid(format!(
            "unsupported snapshot revision {revision:?}"
        )))
    }
}
pub(crate) fn start(project: &Project) -> Result<Project> {
    let mut p = project.clone();
    p.retention_reads = Some(Session::default());
    p.retention_reads
        .as_ref()
        .expect("installed session")
        .snapshot(&p)?;
    Ok(p)
}
pub(crate) fn inherit(from: &Project, mut peer: Project) -> Result<Project> {
    peer.retention_reads = from.retention_reads.clone();
    if let Some(session) = &peer.retention_reads {
        session.snapshot(&peer)?;
    }
    Ok(peer)
}
pub(crate) fn root(p: &Project) -> Result<PathBuf> {
    if let Some(session) = &p.retention_reads {
        return Ok(session.snapshot(p)?.root.clone());
    }
    crate::record_commit::working_tree(&p.records).map_err(Error::Io)?.ok_or_else(|| Error::RetainedPublicationUnsupported("Retained state needs a Git-backed records basis; use the manual immutable-snapshot recovery route for non-Git records".into()))
}
pub(crate) fn command(p: &Project, args: &[&str]) -> Result<Command> {
    let mut command = crate::git::at(&root(p)?);
    command.arg("--literal-pathspecs").args(args);
    Ok(command)
}
pub(crate) fn ancestor(p: &Project, older: &str, newer: &str) -> Result<bool> {
    let args = ["merge-base", "--is-ancestor", older, newer];
    let out = if let Some(session) = &p.retention_reads {
        let snapshot = session.snapshot(p)?;
        session.query(&snapshot, &args)?.output
    } else {
        raw(&root(p)?, &args)?
    };
    match out.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(invalid(format!(
            "Git ancestry failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ))),
    }
}
pub(crate) fn run(p: &Project, args: &[&str]) -> Result<ReadOutput> {
    let Some(session) = &p.retention_reads else {
        let output = raw(&root(p)?, args)?;
        let bytes_read = if args.starts_with(&["cat-file", "blob"]) {
            output.stdout.len() as u64
        } else {
            0
        };
        return Ok(ReadOutput { output, bytes_read });
    };
    let snapshot = session.snapshot(p)?;
    match args {
        ["rev-parse", "--verify", "--end-of-options", rev] => {
            if oid_is_full(rev.strip_suffix("^{commit}").unwrap_or(rev)) {
                return Ok(ReadOutput {
                    output: raw(&snapshot.root, args)?,
                    bytes_read: 0,
                });
            }
            return Ok(reply(0, format!("{}\n", snapshot.resolve(rev)?)));
        }
        ["symbolic-ref", "-q", "HEAD"] => {
            return Ok(match &snapshot.symbolic_head {
                Some(name) => reply(0, format!("{name}\n")),
                None => reply(1, String::new()),
            });
        }
        ["show-ref", "--verify", rest @ ..] => {
            let name = rest.last().ok_or_else(|| invalid("missing ref name"))?;
            let found = snapshot.branches.iter().find(|(n, _)| n == name);
            return Ok(match found {
                Some((name, oid)) => reply(
                    0,
                    if rest.contains(&"--quiet") {
                        String::new()
                    } else {
                        format!("{oid} {name}\n")
                    },
                ),
                None => {
                    let quiet = rest.contains(&"--quiet");
                    let mut result = reply(if quiet { 1 } else { 128 }, String::new());
                    if !quiet {
                        result.output.stderr =
                            format!("fatal: '{name}' - not a valid ref\n").into_bytes();
                    }
                    result
                }
            });
        }
        [
            "for-each-ref",
            "--contains",
            commit,
            "--format=%(refname)",
            "refs/heads/",
        ] => {
            let commit = snapshot.resolve(commit)?;
            let mut result = String::new();
            for (name, tip) in &snapshot.branches {
                if ancestor(p, &commit, tip)? {
                    result.push_str(name);
                    result.push('\n');
                }
            }
            return Ok(reply(0, result));
        }
        ["merge-base", "--is-ancestor", older, newer] => {
            return Ok(reply(
                if ancestor(p, &snapshot.resolve(older)?, &snapshot.resolve(newer)?)? {
                    0
                } else {
                    1
                },
                String::new(),
            ));
        }
        _ => {}
    }
    let mut normalized = args.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    match args {
        ["ls-tree", "-z", revision, "--", _] => normalized[2] = snapshot.resolve(revision)?,
        [
            "log",
            "--first-parent",
            "--reverse",
            "--format=%H",
            revision,
            "--",
            _,
        ] => normalized[4] = snapshot.resolve(revision)?,
        ["diff", "--quiet", "HEAD", "--", ..] => normalized[2] = snapshot.head.clone(),
        _ => {}
    }
    let args = normalized.iter().map(String::as_str).collect::<Vec<_>>();
    session.query(&snapshot, &args)
}
fn lineage_key(p: &Project) -> Result<String> {
    serde_json::to_string(&(&p.records, &p.prefix, p.kind.as_str(), &p.retention_lineage))
        .map_err(invalid)
}
pub(crate) fn lineage_cached(p: &Project) -> Result<bool> {
    match &p.retention_reads {
        Some(s) => Ok(s.lock()?.lineages.contains(&lineage_key(p)?)),
        None => Ok(false),
    }
}
pub(crate) fn remember_lineage(p: &Project) -> Result<()> {
    if let Some(s) = &p.retention_reads {
        let key = lineage_key(p)?;
        let mut state = s.lock()?;
        if !state.lineages.contains(&key) && state.admit(key.len().saturating_add(64)) {
            state.lineages.insert(key);
        }
    }
    Ok(())
}
fn cacheable(args: &[&str]) -> bool {
    match args {
        ["ls-tree", "-z", oid, "--", _] => oid_is_full(oid),
        ["cat-file", "blob" | "-s", object] => oid_is_full(object.split(':').next().unwrap_or("")),
        ["merge-base", "--is-ancestor", a, b] => oid_is_full(a) && oid_is_full(b),
        [
            "log",
            "--first-parent",
            "--reverse",
            "--format=%H",
            oid,
            "--",
            _,
        ] => oid_is_full(oid),
        _ => false,
    }
}
impl Session {
    fn query(&self, snapshot: &Snapshot, args: &[&str]) -> Result<ReadOutput> {
        let allowed = cacheable(args);
        let key = (
            snapshot.identity.clone(),
            args.iter().map(|s| s.to_string()).collect(),
        );
        if allowed && let Some(output) = self.lock()?.queries.get(&key).cloned() {
            return Ok(ReadOutput {
                output,
                bytes_read: 0,
            });
        }
        let output = raw(&snapshot.root, args)?;
        let bytes_read = if args.starts_with(&["cat-file", "blob"]) {
            output.stdout.len() as u64
        } else {
            0
        };
        let complete = output.status.success()
            || (args.starts_with(&["merge-base", "--is-ancestor"])
                && output.status.code() == Some(1));
        if allowed && complete {
            let charge = serde_json::to_vec(&key)
                .map_err(invalid)?
                .len()
                .saturating_add(output.stdout.len())
                .saturating_add(output.stderr.len())
                .saturating_add(64);
            let mut state = self.lock()?;
            if !state.queries.contains_key(&key) && state.admit(charge) {
                state.queries.insert(key, output.clone());
            }
        }
        Ok(ReadOutput { output, bytes_read })
    }
    fn task_key(&self, p: &Project, b: &super::model::Basis) -> Result<TaskKey> {
        Ok((
            self.snapshot(p)?.identity.clone(),
            b.commit.clone(),
            b.task_path.clone(),
            p.attachments_max_bytes,
        ))
    }
}
pub(crate) fn task_cached(p: &Project, b: &super::model::Basis) -> Result<Option<Task>> {
    let Some(session) = &p.retention_reads else {
        return Ok(None);
    };
    if !oid_is_full(&b.commit) {
        return Ok(None);
    }
    let key = session.task_key(p, b)?;
    Ok(session.lock()?.tasks.get(&key).cloned())
}
pub(crate) fn remember_task(p: &Project, b: &super::model::Basis, task: &Task) -> Result<()> {
    let Some(session) = &p.retention_reads else {
        return Ok(());
    };
    if !oid_is_full(&b.commit) {
        return Ok(());
    }
    let key = session.task_key(p, b)?;
    let charge = serde_json::to_vec(&key)
        .map_err(invalid)?
        .len()
        .saturating_add(serde_json::to_vec(task).map_err(invalid)?.len())
        .saturating_add(64);
    let mut state = session.lock()?;
    if !state.tasks.contains_key(&key) && state.admit(charge) {
        state.tasks.insert(key, task.clone());
    }
    Ok(())
}
#[cfg(test)]
mod tests;
