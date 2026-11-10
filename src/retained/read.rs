//! Invocation-owned Git snapshots for retention validation; ordinary readers stay live.
use crate::{
    error::{Error, Result},
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
#[derive(Default)]
struct SessionState {
    aliases: BTreeMap<PathBuf, RepoKey>,
    repositories: BTreeMap<RepoKey, Arc<Snapshot>>,
    lineages: BTreeSet<String>,
}
type RepoKey = (PathBuf, PathBuf, PathBuf);
struct Snapshot {
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
    let out = raw(&root(p)?, &["merge-base", "--is-ancestor", older, newer])?;
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
    let output = raw(&snapshot.root, &args)?;
    let bytes_read = if args.starts_with(&["cat-file", "blob"]) {
        output.stdout.len() as u64
    } else {
        0
    };
    Ok(ReadOutput { output, bytes_read })
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
        s.lock()?.lineages.insert(key);
    }
    Ok(())
}
#[cfg(test)]
mod tests;
