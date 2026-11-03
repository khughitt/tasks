use super::model::*;
use crate::{
    error::{Error, Result},
    model::{Status, Task},
    repo::Project,
};
use std::collections::BTreeMap;

pub struct PendingOperation {
    pub note: OperationNote,
}
pub fn pending(task: &Task) -> Result<Option<PendingOperation>> {
    let mut phases = BTreeMap::<(String, String), OperationNote>::new();
    for note in &task.notes {
        if let Some(raw) = note.text.strip_prefix("retention operation: ") {
            let n = decode_operation(raw)?;
            let key = (n.key.coordinator.clone(), n.key.operation_id.clone());
            validate_transition(phases.get(&key), &n)?;
            phases.insert(key, n);
        }
    }
    let mut live = phases
        .into_values()
        .filter(|n| !matches!(n.phase.as_str(), "completed" | "reconciled"));
    let first = live.next();
    if live.next().is_some() {
        return Err(Error::RetainedHistoryBlocked(
            "Competing retained operation intents; recover exact operation history".into(),
        ));
    }
    Ok(first.map(|note| PendingOperation { note }))
}
fn pending_here(project: &Project, task: &Task) -> Result<Option<PendingOperation>> {
    let Some(p) = pending(task)? else {
        return Ok(None);
    };
    if p.note.phase == "reconcile_ready" && p.note.key.coordinator != task.id.to_string() {
        let ctx =
            crate::commands::open_id_unlocked_ctx(Some(&project.records), &p.note.key.coordinator)?;
        let id = crate::model::TaskId::parse(&p.note.key.coordinator)?;
        let coordinator = super::basis::read(&ctx.project, &id, None)?;
        for n in &coordinator.task.notes {
            if let Some(raw) = n.text.strip_prefix("retention operation: ") {
                let terminal = decode_operation(raw)?;
                if terminal.key == p.note.key
                    && terminal.request == p.note.request
                    && terminal.phase == "reconciled"
                {
                    return Ok(None);
                }
            }
        }
    }
    Ok(Some(p))
}
fn reserved(text: &str) -> bool {
    [
        "retained:",
        "retention operation:",
        "retention checkpoint:",
        "attached:",
        "detached:",
    ]
    .iter()
    .any(|p| text.starts_with(p))
}
pub fn protected(name: &str) -> bool {
    (name.starts_with("retained-op-") && name.ends_with(".request.json"))
        || ((name.starts_with("retained-manifest-") || name.starts_with("retained-checkpoint-"))
            && name.ends_with(".json"))
}
pub enum WriteAction<'a> {
    Archive,
    Detach(&'a str),
    Identity,
    Retire,
}
pub fn permission(project: &Project, task: &Task, action: WriteAction<'_>) -> Result<()> {
    if let WriteAction::Detach(name) = action
        && protected(name)
    {
        return Err(Error::RetainedHistoryBlocked(format!(
            "{name} is protected retained evidence; preserve its immutable locator"
        )));
    }
    if matches!(action, WriteAction::Identity) {
        let mut artifacts = crate::attachments::ledger(task)
            .keys()
            .any(|n| protected(n));
        if crate::attachments::check_storage(project, &task.id)? {
            for entry in std::fs::read_dir(crate::attachments::task_dir(project, &task.id))? {
                let entry = entry?;
                if entry.file_name().to_str().is_some_and(protected) {
                    artifacts = true;
                }
            }
        }
        if artifacts
            || task
                .notes
                .iter()
                .any(|n| n.text.starts_with("retention operation: "))
        {
            return Err(Error::RetainedHistoryBlocked(format!(
                "{} carries immutable retention locators; ID-changing transformations require an explicit locator-preservation design",
                task.id
            )));
        }
    }
    if let Some(p) = pending_here(project, task)? {
        return Err(Error::RetainedHistoryBlocked(format!(
            "{} is reserved by {}/{} at {}; resume or reconcile that operation before mutation",
            task.id, p.note.key.coordinator, p.note.key.operation_id, p.note.phase
        )));
    }
    Ok(())
}
pub fn project_permission(project: &Project, identity: bool) -> Result<()> {
    for task in project.scan()? {
        permission(
            project,
            &task,
            if identity {
                WriteAction::Identity
            } else {
                WriteAction::Retire
            },
        )?;
    }
    Ok(())
}
pub fn check_write(
    project: &Project,
    old: &Task,
    new: &Task,
    permission: WritePermission,
) -> Result<()> {
    #[cfg(test)]
    if let WritePermission::Retained(ref authority) = permission {
        return authority.validate(old, new);
    }
    if old
        .notes
        .iter()
        .any(|n| n.text.starts_with("retention operation: "))
        && !new.notes.starts_with(&old.notes)
    {
        return Err(invalid(
            "immutable retained operation history cannot be rewritten",
        ));
    }
    if new.notes.starts_with(&old.notes) {
        for n in &new.notes[old.notes.len()..] {
            if n.text.starts_with("retention operation:")
                || n.text
                    .strip_prefix("retention checkpoint:")
                    .is_some_and(|s| s.trim_start().starts_with('{'))
            {
                return Err(invalid(
                    "machine retention envelopes require the owned retained publication path",
                ));
            }
            if let Some((crate::attachments::Ledger::Detached, name)) =
                crate::attachments::parse_ledger_note(&n.text)
                && protected(name)
            {
                return Err(invalid(
                    "protected retained evidence cannot be detached by a generic note",
                ));
            }
        }
    }
    if pending_here(project, old)?.is_none() {
        return Ok(());
    }
    let mut allowed = old.clone();
    allowed.updated = new.updated.clone();
    allowed.notes = new.notes.clone();
    if matches!(permission, WritePermission::Claim)
        && new.status == Status::Doing
        && old.status.is_open()
    {
        let at = new.started.as_deref().unwrap_or(&new.updated);
        crate::time::parse(at)?;
        crate::commands::apply_status_effects(&mut allowed, Status::Doing, at, None)?;
        allowed.owner = Some(crate::commands::owner_name(project)?);
    }
    if matches!(permission, WritePermission::Park) {
        allowed.complexity = new.complexity;
    }
    if allowed != *new
        || !new.notes.starts_with(&old.notes)
        || new.notes[old.notes.len()..]
            .iter()
            .any(|n| reserved(&n.text))
    {
        return Err(Error::RetainedHistoryBlocked(format!(
            "{} has a pending retained operation; only unrelated appended notes, start/claim refresh and park may commute",
            old.id
        )));
    }
    Ok(())
}

#[cfg(test)]
pub struct Authority {
    key: OperationKey,
}
pub enum WritePermission {
    Ordinary,
    Claim,
    Park,
    #[cfg(test)]
    Retained(Authority),
}
#[cfg(test)]
impl Authority {
    fn validate(&self, old: &Task, new: &Task) -> Result<()> {
        if !new.notes.starts_with(&old.notes) {
            return Err(invalid("publication cannot rewrite prior history"));
        }
        if let Some(p) = pending(old)?
            && p.note.key != self.key
        {
            return Err(invalid("publication lacks the task reservation"));
        }
        for n in &new.notes[old.notes.len()..] {
            if let Some(raw) = n.text.strip_prefix("retention operation: ")
                && decode_operation(raw)?.key != self.key
            {
                return Err(invalid("publication carries a foreign operation"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub struct OperationCtx {
    pub key: OperationKey,
    pub tasks: BTreeMap<crate::model::TaskId, super::basis::PinnedTask>,
    pub metrics: super::verify::Metrics,
    contexts: BTreeMap<String, crate::commands::Ctx>,
    locks: BTreeMap<std::path::PathBuf, crate::record_commit::CommitLock>,
}
#[cfg(test)]
pub struct NewArtifact {
    pub name: String,
    pub bytes: Vec<u8>,
}
#[cfg(test)]
pub struct GeneratedNote {
    pub at: String,
    pub text: String,
}
#[cfg(test)]
pub struct Published {
    pub publication: Publication,
    pub sources: Vec<Source>,
}
#[cfg(test)]
pub fn open(
    dir: Option<&std::path::Path>,
    ids: &[String],
    key: &OperationKey,
) -> Result<OperationCtx> {
    key.validate()?;
    let mut contexts = BTreeMap::new();
    let mut resolved = std::collections::BTreeSet::new();
    for raw in ids {
        let ctx = crate::commands::open_id_unlocked_ctx(dir, raw)?;
        let id = crate::commands::parse_id(&ctx.registry, &ctx.shorthand, raw)?;
        if id.to_string() != *raw {
            return Err(invalid("operation participants require canonical task ids"));
        }
        if let Some(existing) = contexts.get(&id.prefix) {
            let existing: &crate::commands::Ctx = existing;
            if existing.project.records != ctx.project.records {
                return Err(invalid(
                    "same-prefix participants have different record pairings",
                ));
            }
        } else {
            contexts.insert(id.prefix.clone(), ctx);
        }
        resolved.insert(id);
    }
    for ctx in contexts.values_mut() {
        ctx.lock = Some(crate::claims::MutationLock::acquire(&ctx.project.prefix)?);
    }
    for ctx in contexts.values_mut() {
        crate::commands::revalidate_retained_ctx(ctx)?;
        if !ctx.project.commit_records {
            return Err(Error::RetainedPublicationUnsupported("Retained publication requires Git records with [git] commit = true; use the manual immutable-publication route".into()));
        }
    }
    let mut repositories = BTreeMap::new();
    for ctx in contexts.values() {
        let common = crate::record_commit::common_dir(&ctx.project.records)
            .map_err(Error::RetainedPublicationUnsupported)?;
        repositories
            .entry(common)
            .or_insert_with(|| ctx.project.records.clone());
    }
    let mut locks = BTreeMap::new();
    for (common, records) in repositories {
        let lock =
            crate::record_commit::lock(&records).map_err(Error::RetainedPublicationUnsupported)?;
        let warnings = lock.commit_paths(&records, &[], "chore(tasks): retained preflight");
        if !warnings.is_empty() {
            return Err(Error::RetainedHistoryBlocked(warnings.join("; ")));
        }
        locks.insert(common, lock);
    }
    let mut tasks = BTreeMap::new();
    for id in resolved {
        let ctx = contexts.get_mut(&id.prefix).expect("resolved prefix");
        crate::commands::retained_owner(ctx, &id)?;
        let mut pinned = super::basis::read(&ctx.project, &id, Some("HEAD"))?;
        let working = crate::commands::load(ctx, &id.to_string())?;
        if let Some(p) = pending(&working)?
            && p.note.key != *key
        {
            return Err(invalid("task is reserved by another retained operation"));
        }
        if working != pinned.task {
            let mut expected = pinned.task.clone();
            expected.updated = working.updated.clone();
            expected.notes = working.notes.clone();
            if expected != working
                || !working.notes.starts_with(&pinned.task.notes)
                || !working.notes[pinned.task.notes.len()..].iter().any(|n| {
                    n.text
                        .strip_prefix("retention operation: ")
                        .and_then(|raw| decode_operation(raw).ok())
                        .is_some_and(|n| n.key == *key)
                })
            {
                return Err(Error::RetainedStaleBasis("Unpublished task changes are outside this operation; publish or recover their exact preimage first".into()));
            }
            for n in &working.notes[pinned.task.notes.len()..] {
                if let Some(raw) = n.text.strip_prefix("retention operation: ")
                    && decode_operation(raw)?.key != *key
                {
                    return Err(invalid("foreign unpublished operation"));
                }
            }
            pinned.task = working;
        }
        tasks.insert(id, pinned);
    }
    let metrics = super::verify::Metrics {
        history_bytes_read: tasks.values().map(|p| p.history_bytes_read).sum(),
        ..Default::default()
    };
    Ok(OperationCtx {
        key: key.clone(),
        tasks,
        contexts,
        locks,
        metrics,
    })
}
#[cfg(test)]
impl OperationCtx {
    pub fn project(&self, id: &crate::model::TaskId) -> &Project {
        &self
            .contexts
            .get(&id.prefix)
            .expect("operation participant prefix")
            .project
    }
}
#[cfg(test)]
fn stop(boundary: &str) {
    if std::env::var("TASKS_RETAINED_STOP_AFTER").as_deref() == Ok(boundary) {
        std::process::exit(86)
    }
}
#[cfg(test)]
pub fn batch(
    ctx: &mut OperationCtx,
    id: &crate::model::TaskId,
    artifacts: &[NewArtifact],
    notes: &[GeneratedNote],
) -> Result<Published> {
    let expected = &ctx
        .tasks
        .get(id)
        .ok_or_else(|| invalid("publication names a nonparticipant"))?
        .task;
    let context = ctx
        .contexts
        .get_mut(&id.prefix)
        .expect("participant context");
    crate::commands::retained_owner(context, id)?;
    let input = &ctx.tasks.get(id).expect("participant").basis;
    let original = super::basis::read_exact(&context.project, id, input)?;
    ctx.metrics.history_bytes_read += original.history_bytes_read;
    let current = super::basis::read(&context.project, id, Some("HEAD"))?;
    ctx.metrics.history_bytes_read += current.history_bytes_read;
    if current.task != original.task || current.basis.retained_ref != input.retained_ref {
        return Err(Error::RetainedStaleBasis(
            "Committed task or publishing branch changed between phases".into(),
        ));
    }
    for (name, state) in crate::attachments::ledger(&original.task) {
        if state == crate::attachments::Ledger::Attached {
            let path = format!("{}/tasks/files/{id}/{name}", input.records_subdir);
            let bytes = super::basis::blob(
                &context.project,
                &input.commit,
                path.trim_start_matches("./"),
                Some(context.project.attachments_max_bytes),
                &mut ctx.metrics.history_bytes_read,
            )?;
            if std::fs::read(crate::attachments::task_dir(&context.project, id).join(&name))?
                != bytes
            {
                return Err(Error::RetainedStaleBasis(format!(
                    "Required attachment {name} differs from its committed preimage"
                )));
            }
        }
    }
    let mut task = context.project.read_task(id)?;
    if task != *expected {
        return Err(Error::RetainedStaleBasis(
            "Task changed between retained phases; retry after recovering the exact preimage"
                .into(),
        ));
    }
    let mut sources = Vec::new();
    if notes.is_empty() {
        return Err(invalid("publication requires generated notes"));
    }
    let reused = task
        .notes
        .windows(notes.len())
        .enumerate()
        .rev()
        .find(|(_, window)| {
            window
                .iter()
                .zip(notes)
                .all(|(old, new)| old.at == new.at && old.text == new.text)
        })
        .map(|(i, _)| i);
    let phase = notes
        .iter()
        .rev()
        .find_map(|n| n.text.strip_prefix("retention operation: "))
        .ok_or_else(|| invalid("every publication requires its phase receipt"))?;
    let operation = decode_operation(phase)?;
    if operation.key != ctx.key {
        return Err(invalid("publication key mismatch"));
    }
    if reused.is_none() {
        let previous = task.notes.iter().rev().find_map(|n| {
            n.text
                .strip_prefix("retention operation: ")
                .and_then(|s| decode_operation(s).ok())
                .filter(|n| n.key == ctx.key)
        });
        validate_transition(previous.as_ref(), &operation)?;
    }

    for artifact in artifacts {
        crate::attachments::validate_name(&artifact.name)?;
        if artifact.bytes.len() as u64 > context.project.attachments_max_bytes {
            return Err(Error::AttachmentTooLarge("Retained artifact exceeds the configured cap; preserve raw bytes and use the labelled portable fallback before retry".into()));
        }
        if crate::attachments::ledger(&task).get(&artifact.name)
            == Some(&crate::attachments::Ledger::Detached)
        {
            return Err(invalid(
                "retained artifact name has a detached history; choose a fresh name before publishing",
            ));
        }
        crate::attachments::check_storage(&context.project, id)?;
        crate::attachments::ensure_task_dir(&context.project, id)?;
        let path = crate::attachments::task_dir(&context.project, id).join(&artifact.name);
        match crate::attachments::entry_state(&path)? {
            crate::attachments::EntryState::Absent => {
                crate::attachments::write_new(&path, &artifact.bytes)?
            }
            crate::attachments::EntryState::File(_) => {
                if std::fs::read(&path)? != artifact.bytes {
                    return Err(invalid(
                        "immutable retained artifact name already holds different bytes",
                    ));
                }
            }
            crate::attachments::EntryState::Unsafe(detail) => {
                return Err(Error::AttachmentUnsafe(detail));
            }
        }
        crate::written::record(&context.project.records, &path);
        let mut parent = path.parent();
        while let Some(dir) = parent {
            std::fs::File::open(dir)?.sync_all()?;
            if dir == context.project.records {
                break;
            }
            parent = dir.parent();
        }
        if reused.is_some()
            && crate::attachments::ledger(&task).get(&artifact.name)
                != Some(&crate::attachments::Ledger::Attached)
        {
            return Err(invalid(
                "a phase receipt cannot substitute for its missing attachment ledger",
            ));
        }
        match crate::attachments::ledger(&task).get(&artifact.name) {
            Some(crate::attachments::Ledger::Attached) => {}
            Some(crate::attachments::Ledger::Detached) => {
                return Err(invalid(
                    "retained artifact name was detached; preserve that history and use a new immutable name",
                ));
            }
            None => {
                crate::commands::append_note(
                    &mut task,
                    &crate::commands::owner_name(&context.project)?,
                    &crate::attachments::attached_note(
                        &artifact.name,
                        artifact.bytes.len() as u64,
                        None,
                    ),
                )?;
            }
        }
    }
    stop("artifact");
    let start = reused.unwrap_or(task.notes.len());
    if reused.is_none() {
        for n in notes {
            crate::time::parse(&n.at)?;
            crate::commands::append_note(
                &mut task,
                &crate::commands::owner_name(&context.project)?,
                &n.text,
            )?;
            task.notes.last_mut().expect("new note").at = n.at.clone();
        }
        let max = notes
            .iter()
            .map(|n| n.at.as_str())
            .chain(std::iter::once(task.updated.as_str()))
            .max()
            .expect("updated");
        task.updated = crate::time::after(max)?;
    }
    crate::format::validate_task(&task)?;
    let (_, problems) = crate::attachments::audit_task(&context.project, &task)?;
    if !problems.is_empty() {
        return Err(Error::RetainedHistoryBlocked(format!(
            "Retained publication attachment audit refused: {problems:?}"
        )));
    }
    context.project.write_task_permitted(
        &context.registry,
        &task,
        || Ok(()),
        WritePermission::Retained(Authority {
            key: ctx.key.clone(),
        }),
    )?;
    stop("save");
    let written = crate::written::take();
    if written
        .roots
        .keys()
        .any(|root| root != &context.project.records)
    {
        return Err(invalid(
            "publication encountered written paths belonging to another phase/project",
        ));
    }
    let paths = written
        .roots
        .get(&context.project.records)
        .map(|p| p.iter().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let common = crate::record_commit::common_dir(&context.project.records)
        .map_err(Error::RetainedPublicationUnsupported)?;
    let warnings = ctx
        .locks
        .get(&common)
        .ok_or_else(|| invalid("publication repository changed under held lock"))?
        .commit_paths(
            &context.project.records,
            &paths,
            &format!("chore(tasks): retained {} {}", id, operation.phase),
        );
    if !warnings.is_empty() {
        return Err(Error::RetainedHistoryBlocked(format!(
            "Operation {}/{} publication incomplete at {}: {}",
            ctx.key.coordinator,
            ctx.key.operation_id,
            operation.phase,
            warnings.join("; ")
        )));
    }
    stop("commit");
    let pinned = super::basis::read(&context.project, id, None)?;
    ctx.metrics.history_bytes_read += pinned.history_bytes_read;
    if pinned.task != task {
        return Err(Error::RetainedHistoryBlocked("Committed retained record differs from intended publication; preserve exact evidence and recover".into()));
    }
    for artifact in artifacts {
        let path = format!(
            "{}/tasks/files/{id}/{}",
            pinned.basis.records_subdir, artifact.name
        );
        let bytes = super::basis::blob(
            &context.project,
            &pinned.basis.commit,
            path.trim_start_matches("./"),
            Some(context.project.attachments_max_bytes),
            &mut ctx.metrics.history_bytes_read,
        )?;
        if bytes != artifact.bytes {
            return Err(invalid(
                "committed retained artifact differs from exact intended bytes",
            ));
        }
    }
    for i in start..start + notes.len() {
        sources.push(Source {
            basis: pinned.basis.clone(),
            ordinal: i as u64 + 1,
        });
    }
    let publication = Publication {
        task: id.to_string(),
        phase: operation.phase,
        basis: pinned.basis.clone(),
    };
    ctx.tasks.insert(id.clone(), pinned);
    Ok(Published {
        publication,
        sources,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    fn fixture(test: &str) -> Option<(PathBuf, crate::model::TaskId)> {
        if let Ok(dir) = std::env::var("TASKS_PUBLICATION_FIXTURE") {
            return Some((
                dir.into(),
                crate::model::TaskId::parse(&std::env::var("TASKS_PUBLICATION_ID").unwrap())
                    .unwrap(),
            ));
        }
        let f = crate::common::retained::RetainedEnv::new("in-tree");
        f.json(&["start", &f.id]);
        let binary = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("tasks");
        use std::os::unix::fs::PermissionsExt;
        let quote = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
        let hook = f.dir.join(".git/hooks/pre-commit");
        std::fs::write(
            &hook,
            format!(
                "#!/bin/sh\nexec {} -C {} check >/dev/null\n",
                quote(binary.to_str().unwrap()),
                quote(f.dir.to_str().unwrap())
            ),
        )
        .unwrap();
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
        let extra = tempfile::tempdir().unwrap();
        let second = if test.ends_with("same_git_lock") {
            crate::common::git(extra.path(), &["init", "-qb", "main"]);
            f.env.json(
                extra.path(),
                &[
                    "init",
                    "--prefix",
                    "fam",
                    "--records",
                    f.dir.to_str().unwrap(),
                ],
            );
            let config = f.dir.join("fam/tasks/.config.toml");
            let mut value: toml::Value =
                toml::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
            value.as_table_mut().unwrap().insert(
                "git".into(),
                toml::Value::Table(toml::map::Map::from_iter([(
                    "commit".into(),
                    toml::Value::Boolean(true),
                )])),
            );
            std::fs::write(config, toml::to_string(&value).unwrap()).unwrap();
            crate::common::git(&f.dir, &["add", "fam/tasks/.config.toml"]);
            crate::common::git(&f.dir, &["commit", "-qm", "second records configuration"]);
            let id = f.json(&["add", "Second participant", "--project", "fam"])["id"]
                .as_str()
                .unwrap()
                .to_string();
            f.json(&["start", &id]);
            Some(id)
        } else {
            None
        };
        let command = || {
            let mut c = std::process::Command::new(std::env::current_exe().unwrap());
            c.env_clear()
                .envs(crate::common::isolated_env(f.env.home.path()))
                .env("TASKS_SESSION", "retained-fixture")
                .env("TASKS_PUBLICATION_FIXTURE", &f.dir)
                .env("TASKS_PUBLICATION_ID", &f.id)
                .args(["--exact", test, "--nocapture"]);
            if let Some(second) = &second {
                c.env("TASKS_PUBLICATION_SECOND", second);
            }
            c
        };
        if second.is_some() {
            let mut children = vec![
                command()
                    .env("TASKS_PUBLICATION_REVERSE", "0")
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .unwrap(),
                command()
                    .env("TASKS_PUBLICATION_REVERSE", "1")
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .unwrap(),
            ];
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            while children.iter_mut().any(|c| c.try_wait().unwrap().is_none()) {
                if std::time::Instant::now() > deadline {
                    for c in &mut children {
                        let _ = c.kill();
                        let _ = c.wait();
                    }
                    panic!("opposite-order retained locking timed out");
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            for c in children {
                let out = c.wait_with_output().unwrap();
                assert!(out.status.success(), "{out:?}");
            }
            return None;
        }
        if let Some(boundary) = test.rsplit_once("_crash_").map(|p| p.1) {
            let out = command()
                .env("TASKS_RETAINED_STOP_AFTER", boundary)
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(86), "{out:?}");
        }
        let out = command().output().unwrap();
        assert!(out.status.success(), "{out:?}");
        None
    }
    fn example(id: &crate::model::TaskId) -> (OperationKey, NewArtifact, GeneratedNote) {
        let key = OperationKey {
            coordinator: id.to_string(),
            operation_id: "a".repeat(32),
        };
        let name = format!("retained-op-{}.request.json", key.operation_id);
        let bytes = b"{}".to_vec();
        let p = Project::open(std::path::Path::new(
            &std::env::var("TASKS_PUBLICATION_FIXTURE").unwrap(),
        ))
        .unwrap();
        let existing = p
            .read_task(id)
            .unwrap()
            .notes
            .into_iter()
            .rev()
            .find_map(|n| {
                n.text
                    .strip_prefix("retention operation: ")
                    .and_then(|s| decode_operation(s).ok())
            });
        let note = if let Some(note) = existing {
            note
        } else {
            let input = super::super::basis::read(&p, id, None).unwrap().basis;
            OperationNote {
                version: 1,
                key: key.clone(),
                phase: "intent".into(),
                request: ArtifactLocator::Attachment {
                    task: id.to_string(),
                    name: name.clone(),
                    basis: None,
                    sha256: super::super::verify::sha(&bytes),
                },
                input_bases: vec![Input {
                    task: id.to_string(),
                    basis: input,
                }],
                host: crate::claims::hostname(),
                at: "2026-11-03T00:00:00Z".into(),
                observer: None,
                outputs: PhaseOutputs::Intent(Empty {}),
                diagnostics: vec![],
            }
        };
        (
            key,
            NewArtifact { name, bytes },
            GeneratedNote {
                at: note.at.clone(),
                text: format!(
                    "retention operation: {}",
                    serde_json::to_string(&note).unwrap()
                ),
            },
        )
    }
    #[test]
    fn retained_publish_orphan_request() {
        let Some((dir, id)) = fixture("retained::publish::tests::retained_publish_orphan_request")
        else {
            return;
        };
        let (key, a, n) = example(&id);
        let p = Project::open(&dir).unwrap();
        crate::attachments::ensure_task_dir(&p, &id).unwrap();
        let path = crate::attachments::task_dir(&p, &id).join(&a.name);
        std::fs::write(&path, &a.bytes).unwrap();
        let mut ctx = open(Some(&dir), &[id.to_string()], &key).unwrap();
        assert_eq!(ctx.project(&id).prefix, id.prefix);
        let result = batch(&mut ctx, &id, &[a], &[n]).unwrap();
        assert_eq!(result.publication.task, id.to_string());
        assert_eq!(result.sources.len(), 1);
        assert!(
            !super::super::basis::git(
                &p,
                &[
                    "ls-files",
                    "--error-unmatch",
                    path.strip_prefix(&dir).unwrap().to_str().unwrap()
                ]
            )
            .unwrap()
            .is_empty()
        );
        assert!(crate::written::take().roots.is_empty());
    }
    fn publish_case(test: &str, case: &str) {
        let Some((dir, id)) = fixture(test) else {
            return;
        };
        let (key, mut a, n) = example(&id);
        let p = Project::open(&dir).unwrap();
        if case == "foreign_index" {
            std::fs::write(dir.join("foreign"), "foreign").unwrap();
            crate::common::git(&dir, &["add", "foreign"]);
            assert!(open(Some(&dir), &[id.to_string()], &key).is_err());
            assert!(
                !p.read_task(&id)
                    .unwrap()
                    .notes
                    .iter()
                    .any(|n| n.text.starts_with("retention operation: "))
            );
            return;
        }
        if case == "different" {
            crate::attachments::ensure_task_dir(&p, &id).unwrap();
            let path = crate::attachments::task_dir(&p, &id).join(&a.name);
            std::fs::write(&path, "different").unwrap();
            let mut ctx = open(Some(&dir), &[id.to_string()], &key).unwrap();
            assert!(batch(&mut ctx, &id, &[a], &[n]).is_err());
            assert_eq!(std::fs::read(path).unwrap(), b"different");
            return;
        }
        if case == "missing_ledger" {
            crate::attachments::ensure_task_dir(&p, &id).unwrap();
            std::fs::write(
                crate::attachments::task_dir(&p, &id).join(&a.name),
                &a.bytes,
            )
            .unwrap();
            let path = p.task_path(&id);
            let mut raw = std::fs::read_to_string(&path).unwrap();
            raw.push_str(&format!("- {} (fixture): {}\n", n.at, n.text));
            std::fs::write(path, raw).unwrap();
            let mut ctx = open(Some(&dir), &[id.to_string()], &key).unwrap();
            assert!(batch(&mut ctx, &id, &[a], &[n]).is_err());
            return;
        }
        if case == "manifest" {
            a.name = "retained-manifest-a.json".into();
            crate::attachments::ensure_task_dir(&p, &id).unwrap();
            std::fs::write(
                crate::attachments::task_dir(&p, &id).join(&a.name),
                &a.bytes,
            )
            .unwrap();
        }
        if case == "warning" {
            std::fs::write(dir.join(".git/hooks/pre-commit"), "#!/bin/sh\nexit 1\n").unwrap();
        }
        let mut ctx = open(Some(&dir), &[id.to_string()], &key).unwrap();
        assert_eq!(ctx.contexts.len(), 1);
        assert_eq!(ctx.locks.len(), 1);
        let before = super::super::basis::resolve(&p, "HEAD").unwrap();
        let result = batch(&mut ctx, &id, &[a], &[n]);
        if case == "warning" {
            assert!(result.is_err());
            assert_eq!(before, super::super::basis::resolve(&p, "HEAD").unwrap());
            assert!(pending(&p.read_task(&id).unwrap()).unwrap().is_some());
        } else {
            let result = result.unwrap();
            assert_eq!(result.publication.phase, "intent");
            assert_eq!(
                pending(&p.read_task(&id).unwrap())
                    .unwrap()
                    .unwrap()
                    .note
                    .key,
                key
            );
            assert!(crate::written::take().roots.is_empty());
        }
    }
    #[test]
    fn retained_publish_orphan_manifest() {
        publish_case(
            "retained::publish::tests::retained_publish_orphan_manifest",
            "manifest",
        )
    }
    #[test]
    fn retained_publish_foreign_index() {
        publish_case(
            "retained::publish::tests::retained_publish_foreign_index",
            "foreign_index",
        )
    }
    #[test]
    fn retained_publication_warning() {
        publish_case(
            "retained::publish::tests::retained_publication_warning",
            "warning",
        )
    }
    #[test]
    fn retained_publish_crash_artifact() {
        publish_case(
            "retained::publish::tests::retained_publish_crash_artifact",
            "crash",
        )
    }
    #[test]
    fn retained_publish_crash_save() {
        publish_case(
            "retained::publish::tests::retained_publish_crash_save",
            "crash",
        )
    }
    #[test]
    fn retained_publish_crash_commit() {
        publish_case(
            "retained::publish::tests::retained_publish_crash_commit",
            "crash",
        )
    }
    #[test]
    fn retained_publish_same_git_lock() {
        let Some((dir, id)) = fixture("retained::publish::tests::retained_publish_same_git_lock")
        else {
            return;
        };
        let key = OperationKey {
            coordinator: id.to_string(),
            operation_id: "a".repeat(32),
        };
        let second = std::env::var("TASKS_PUBLICATION_SECOND").unwrap();
        let mut ids = vec![id.to_string(), second.clone()];
        if std::env::var("TASKS_PUBLICATION_REVERSE").as_deref() == Ok("1") {
            ids.reverse();
        }
        let mut ctx = open(Some(&dir), &ids, &key).unwrap();
        let (_, a, n) = example(&id);
        assert_eq!(ctx.contexts.len(), 2);
        assert_eq!(ctx.locks.len(), 1);
        let path = dir.join("fam/unrelated.txt");
        if !path.exists() {
            std::fs::write(&path, "unrelated").unwrap();
        }
        batch(&mut ctx, &id, &[a], &[n]).unwrap();
        let out = crate::git::at(&dir)
            .args(["ls-files", "--error-unmatch", "fam/unrelated.txt"])
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "unrelated");
        assert!(
            ctx.tasks
                .contains_key(&crate::model::TaskId::parse(&second).unwrap())
        );
    }
    #[test]
    fn retained_publish_different_orphan() {
        publish_case(
            "retained::publish::tests::retained_publish_different_orphan",
            "different",
        )
    }
    #[test]
    fn retained_publish_receipt_without_ledger() {
        publish_case(
            "retained::publish::tests::retained_publish_receipt_without_ledger",
            "missing_ledger",
        )
    }
}
