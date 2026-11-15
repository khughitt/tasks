use super::input::HookInput;
use crate::link::{GitTree, project_of_tree};
use crate::registry::Registry;
use crate::repo::{CONFIG_REL, Kind, Project};
use crate::scope::{Origin, open_registered};
use std::path::Path;

/// Match registered locations in the main tree to their location in this worktree.
/// Ordinary `where` intentionally has a narrower in-tree discovery contract.
fn without_local_config(
    registry: &Registry,
    tree: &GitTree,
    cwd: &Path,
) -> Result<Option<Project>, String> {
    let mut locations = Vec::new();
    if let Some(main) = tree.main_checkout() {
        for (prefix, root) in &registry.projects {
            let root = crate::rename::root_identity(root)
                .map_err(|_| "registered checkout could not be verified")?;
            let Ok(relative) = root.strip_prefix(&main) else {
                continue;
            };
            if !cwd.starts_with(tree.top.join(relative)) {
                continue;
            }
            // A nested registered repository belongs to its own Git tree.
            if !relative.as_os_str().is_empty()
                && GitTree::of(&root)
                    .map_err(|_| "registered code tree could not be verified")?
                    .as_ref()
                    .is_none_or(|t| t.common != tree.common)
            {
                continue;
            }
            locations.push((relative.components().count(), prefix));
        }
    }
    let nearest = locations.iter().map(|(depth, _)| *depth).max();
    let candidates: Vec<_> = locations
        .iter()
        .filter(|(depth, _)| Some(*depth) == nearest)
        .map(|(_, prefix)| prefix.as_str())
        .collect();
    if candidates.len() > 1 {
        return Err("registered code-tree location is ambiguous".into());
    }
    let linked = project_of_tree(registry, tree)
        .map_err(|_| "registered code-tree pairing or link could not be established")?;
    if let Some(project) = linked {
        if candidates
            .first()
            .is_some_and(|prefix| *prefix != project.prefix)
        {
            return Err("registered code-tree and link matches disagree".into());
        }
        return Ok(Some(project));
    }
    candidates
        .first()
        .map(|prefix| {
            open_registered(registry, prefix, Origin::Prefix)
                .map_err(|_| "registered authority config or pairing is invalid".into())
        })
        .transpose()
}

fn authority(event: &HookInput) -> Result<Option<Project>, String> {
    let cwd = event.event["cwd"]
        .as_str()
        .filter(|s| !s.contains('\0') && s.len() <= 8192 && Path::new(s).is_absolute())
        .ok_or("event.cwd is not an absolute directory")?;
    let cwd = Path::new(cwd)
        .canonicalize()
        .map_err(|_| "event.cwd is missing or unreadable")?;
    if !cwd.is_dir() {
        return Err("event.cwd is not a directory".into());
    }
    let tree = GitTree::of(&cwd).map_err(|_| "code-tree discovery failed")?;
    let stop = tree.as_ref().map(|t| t.top.as_path());
    let mut local = None;
    for dir in cwd.ancestors() {
        match dir.join(CONFIG_REL).symlink_metadata() {
            Ok(_) => {
                local = Some(
                    Project::open(dir)
                        .map_err(|_| "local tasks config is malformed or unreadable")?,
                );
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("local tasks config could not be discovered".into()),
        }
        if Some(dir) == stop {
            break;
        }
    }
    let registry = Registry::load().map_err(|_| "project registry is unreadable")?;
    let Some(local) = local else {
        return tree
            .map(|t| without_local_config(&registry, &t, &cwd))
            .unwrap_or(Ok(None));
    };
    if registry.project_root(&local.prefix).is_none() {
        return Err(format!("unregistered prefix {}", local.prefix));
    }
    let registered = open_registered(&registry, &local.prefix, Origin::Prefix)
        .map_err(|_| "registered authority config or pairing is invalid")?;
    if local.kind == Kind::InTree && registry.records_dir(&local.prefix).is_some() {
        // A historical config identifies a prefix, but cannot establish its current authority.
        let main = tree
            .as_ref()
            .and_then(GitTree::main_checkout)
            .ok_or("historical config has no verifiable code tree")?;
        let mut matches = Vec::new();
        for (prefix, root) in &registry.projects {
            if registry.records_dir(prefix).is_some()
                && crate::rename::root_identity(root)
                    .map_err(|_| "registered checkout could not be verified")?
                    == main
            {
                matches.push(prefix);
            }
        }
        if matches.len() != 1
            || matches[0] != &local.prefix
            || registered.kind != Kind::Detached
            || registered.checkout != main
        {
            return Err("historical config does not match one registered detached pairing".into());
        }
    }
    Ok(Some(registered))
}

pub(super) fn notice(event: &HookInput) -> Result<Option<String>, String> {
    if event.event["nativeEvent"] != "SessionStart" || event.event["kind"] != "session-start" {
        return Err("unsupported event; repair the SessionStart subscriber registration".into());
    }
    let project = match authority(event) {
        Ok(Some(project)) => project,
        Ok(None) => return Ok(None),
        Err(detail) => {
            let detail = detail.split_whitespace().collect::<Vec<_>>().join(" ");
            return Ok(Some(format!(
                "tasks: cannot tell which project this session is in ({detail}); run `tasks where`\n"
            )));
        }
    };
    let tasks = project
        .scan()
        .map_err(|_| "registered task records could not be scanned")?;
    let halt = tasks
        .iter()
        .filter(|t| t.status.is_open() && t.tags.iter().any(|tag| tag == "halt"))
        .min_by(|a, b| a.created.cmp(&b.created).then_with(|| a.id.cmp(&b.id)));
    Ok(halt.map(|halt|format!("halt: {} is under stop-work halt {} ({}); start it, or override with `tasks start <id> --force --reason \"<why>\"`\n",project.prefix,halt.id,single_line(&halt.title))))
}

fn single_line(title: &str) -> String {
    title
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '\u{2028}' | '\u{2029}') {
                c.escape_default().collect::<String>()
            } else {
                c.to_string()
            }
        })
        .collect()
}
