use crate::error::{Error, Result};
use crate::output::{InitOut, Output};
use crate::registry::Registry;
use crate::repo::{Kind, Project};
use std::path::{Path, PathBuf};

const SKILL_REL: &str = "skills/tasks/SKILL.md";

pub fn run(
    dir: Option<&Path>,
    prefix: Option<String>,
    checkout: Option<PathBuf>,
    records: Option<PathBuf>,
    force: bool,
) -> Result<Output> {
    let root = match dir {
        Some(dir) => dir.to_path_buf(),
        None => std::env::current_dir()?,
    }
    .canonicalize()?;
    let prefix = match prefix {
        Some(prefix) => prefix,
        None => default_prefix(&root)?,
    };
    // Before the prefix names a lock file or, with `--records`, a directory.
    if !crate::model::is_valid_prefix(&prefix) {
        return Err(Error::Config(format!(
            "prefix {prefix:?} must match [a-z][a-z0-9]{{1,7}}"
        )));
    }
    // Lock order (retired-projects plan): the project lock, then the registry lock, as
    // rename takes them. `init --force` repoints the root `retire` and every writer consult.
    let _project_lock = crate::claims::MutationLock::acquire(&prefix)?;
    let _lock = Registry::lock()?;
    let mut registry = Registry::load()?;
    for inventory in crate::rename::inventory::Inventory::pending()? {
        if inventory.target == prefix {
            return Err(Error::Config(format!(
                "prefix {prefix:?} is reserved by unfinished rename {} -> {}",
                inventory.source, inventory.target
            )));
        }
    }
    super::reject_pending_operations(Some(&root), registry.canonical_prefix(&prefix))?;
    // The registry is only mutated in memory here: `save` runs after `Project::init`, so
    // an init that fails leaves the registry exactly as it was.
    if force {
        let derived = records
            .as_ref()
            .map(|records| root.join(records).join(&prefix));
        reject_repointing_a_retired_project(&registry, &prefix, &root, derived.as_deref())?;
    }
    let records = match records {
        Some(records_root) => Some(validate_records(&root, &records_root, &registry, &prefix)?),
        None => {
            reject_linked_destination(&root, &registry)?;
            None
        }
    };
    let checkout = checkout
        .map(|checkout| validate_checkout(&root, &root.join(checkout), &registry, &prefix))
        .transpose()?;
    let displaced = if force {
        registry.repoint(&prefix, &root)?
    } else {
        registry.register(&prefix, &root)?;
        None
    };
    let project = match &records {
        Some(derived) => {
            let project = Project::init_detached(derived, &prefix, &root)?;
            registry.pair_records(&prefix, derived);
            project
        }
        None => Project::init(&root, &prefix, checkout.as_deref())?,
    };
    // A plain init makes the project in-tree or linked: a pairing left from a detached
    // registration would send `locate` to the old records directory (`records_moved`).
    let unpaired = match &records {
        Some(_) => None,
        None => registry.unpair_records(&prefix),
    };
    let today = crate::time::day(&crate::time::now()).to_string();
    let previous = displaced
        .clone()
        .map(|root| registry.displaced(&prefix, root));
    let history = registry.note_root_change(&prefix, previous, &today)?;
    registry.save()?;
    let mut warnings = Vec::new();
    if let Some(previous) = displaced {
        warnings.push(format!(
            "prefix {prefix:?} was registered to {}",
            previous.display()
        ));
    }
    if let Some(dir) = unpaired {
        warnings.push(format!(
            "prefix {prefix:?} was paired with {}; the pairing is removed",
            dir.display()
        ));
    }
    warnings.extend(history);
    if !skill_installed(&project.root) {
        warnings.push(
            "no tasks skill found at user or project level; install skills/tasks (see README)"
                .into(),
        );
    }
    Ok(Output::Init(InitOut {
        prefix: project.prefix,
        root: project.root.display().to_string(),
        records: records.map(|derived| derived.display().to_string()),
        warnings,
        aliases: Vec::new(),
    }))
}

fn default_prefix(root: &Path) -> Result<String> {
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            Error::Config("cannot derive a prefix from this directory; pass --prefix".into())
        })?;
    let prefix = name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(3)
        .collect::<String>()
        .to_lowercase();
    if !crate::model::is_valid_prefix(&prefix) {
        return Err(Error::Config(format!(
            "derived prefix {prefix:?} is invalid; pass --prefix"
        )));
    }
    Ok(prefix)
}

fn skill_installed(root: &Path) -> bool {
    let mut candidates = vec![root.join(".claude").join(SKILL_REL)];
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        candidates.push(home.join(".claude").join(SKILL_REL));
        candidates.push(home.join(".agents").join(SKILL_REL));
    }
    candidates.iter().any(|path| path.is_file())
}

/// `init --force` moves the authority that `retire` and every writer consult, so it is
/// not done for a retired project: refused when the new root's config declares
/// `[retired]`, or when the currently registered root declares it. An old root that is
/// gone or holds no config does not block: repointing away from a moved checkout is what
/// `--force` is for. One whose config exists but does not parse is an error. With
/// `--records`, `records` is the records directory being paired, and a config already
/// there that declares `[retired]` refuses too.
fn reject_repointing_a_retired_project(
    registry: &Registry,
    prefix: &str,
    root: &Path,
    records: Option<&Path>,
) -> Result<()> {
    let refuse = |where_: &Path| {
        Err(Error::RetiredProject(format!(
            "{prefix} is retired (declared in {}); run `tasks unretire` there before re-pointing it",
            where_.display()
        )))
    };
    if root.join(crate::repo::CONFIG_REL).exists() && Project::open(root)?.retired.is_some() {
        return refuse(root);
    }
    if let Some(records) = records
        && records.join(crate::repo::CONFIG_REL).exists()
        && Project::open(records)?.retired.is_some()
    {
        return refuse(records);
    }
    // Only an old root that is gone, or holds no config, may be passed over unread; a
    // config that exists and does not parse is an error, never "not retired".
    if let Some(current) = registry.project_root(prefix)
        && current != root
        && crate::scope::is_reachable(registry, prefix)?
        && let Some(dir) = crate::scope::config_dir(registry, prefix)
        && Project::open(&dir)?.retired.is_some()
    {
        return refuse(&dir);
    }
    Ok(())
}

/// No command writes into a linked checkout (ops linked-checkouts §4.1), `init` included:
/// a destination in a repository that a registered project declares as its checkout is
/// refused, whichever of that repository's working trees it is in, because the
/// repository's records already live in that project's root. A detached project's checkout
/// is `records_moved`, naming the `init --records` that pairs it again (decision 6).
fn reject_linked_destination(root: &Path, registry: &Registry) -> Result<()> {
    let Some(tree) = crate::link::GitTree::of(root)? else {
        return Ok(());
    };
    let Some(main) = tree.main_checkout() else {
        return Ok(());
    };
    // A detached project's checkout: its records moved out (decision 6), and pairing the
    // checkout again is `init --records`. Read from the registry, not the scan, so a
    // records directory not on this host yet still refuses.
    for (other, other_root) in &registry.projects {
        if let Some(dir) = registry.records_dir(other)
            && crate::rename::root_identity(other_root)? == main
        {
            let records_root = dir.parent().unwrap_or(dir);
            return Err(Error::RecordsMoved(format!(
                "{other}'s records live at {}; to pair this checkout again run `tasks init \
                 --prefix {other} --records {} --force`",
                dir.display(),
                records_root.display()
            )));
        }
    }
    let owners: Vec<_> = crate::locations::scan_checkouts(registry)
        .strict()?
        .into_iter()
        .filter(|linked| linked.kind == Kind::Linked && linked.checkout == main)
        .map(|linked| linked.prefix)
        .collect();
    if owners.is_empty() {
        return Ok(());
    }
    Err(Error::Config(format!(
        "{} is in {}, the linked checkout of {}; its records live in that project's root \
         (`tasks where` names it), and nothing is written into the code tree",
        root.display(),
        main.display(),
        owners.join(" and ")
    )))
}

/// Linked-checkouts §4.1: the checkout must be the main checkout of a git repository that
/// keeps no records, outside every registered project's tree, not containing this
/// project, and declared by no other project.
fn validate_checkout(
    root: &Path,
    checkout: &Path,
    registry: &Registry,
    prefix: &str,
) -> Result<PathBuf> {
    let refuse = |why: String| {
        Err(Error::Config(format!(
            "--checkout {}: {why}",
            checkout.display()
        )))
    };
    let Ok(checkout) = checkout.canonicalize() else {
        return refuse("does not exist".into());
    };
    let Some(tree) = crate::link::GitTree::of(&checkout)? else {
        return refuse("is not in a git repository".into());
    };
    if tree.top != checkout {
        return refuse(format!(
            "is not the top level of its repository ({})",
            tree.top.display()
        ));
    }
    if !tree.is_main() {
        let main = tree
            .main_checkout()
            .map(|m| m.display().to_string())
            .unwrap_or_default();
        return refuse(format!(
            "is a linked worktree; pass its main checkout {main}"
        ));
    }
    if tree.main_checkout().as_deref() != Some(checkout.as_path()) {
        return refuse("has no main checkout (a bare repository or a separate git dir)".into());
    }
    if checkout.join(crate::repo::CONFIG_REL).exists() {
        return refuse("keeps records in-tree".into());
    }
    // Repository identity, not only containment: a worktree of this repository stored
    // anywhere (`.worktrees` linked into separate storage) is still its code tree.
    if root.starts_with(&checkout)
        || crate::link::GitTree::of(root)?.is_some_and(|here| here.common == tree.common)
    {
        return refuse("holds this project; records must live outside the code tree".into());
    }
    // A registered project in any working tree of this repository, wherever that tree is
    // stored, already gives the repository an identity: containment misses a worktree
    // stored outside the checkout, so the common git directory decides (plan review
    // round 2). A nested repository inside the checkout's tree is caught by containment.
    for (other, other_root) in &registry.projects {
        if other == prefix || !other_root.try_exists()? {
            continue;
        }
        let other_root = other_root.canonicalize()?;
        if checkout.starts_with(&other_root)
            || other_root.starts_with(&checkout)
            || crate::link::GitTree::of(&other_root)?
                .is_some_and(|theirs| theirs.common == tree.common)
        {
            return refuse(format!(
                "overlaps registered project {other} ({}): that project already keeps this \
                 repository's records",
                other_root.display()
            ));
        }
    }
    if crate::link::holds_records(registry, &tree)? {
        return refuse("keeps records in-tree".into());
    }
    let taken: Vec<_> = crate::locations::scan_checkouts(registry)
        .strict()?
        .into_iter()
        .filter(|linked| linked.prefix != prefix && linked.checkout == checkout)
        .map(|linked| linked.prefix)
        .collect();
    if !taken.is_empty() {
        return Err(Error::CheckoutConflict(format!(
            "{} is already the checkout of {}",
            checkout.display(),
            taken.join(" and ")
        )));
    }
    Ok(checkout)
}

/// Records-detached spec §5.1: `root`, the checkout running `init --records`, must be the
/// main checkout of a git repository that keeps no records in its working tree or index,
/// overlapping no other registered project, and declared by no other prefix; the records
/// directory `<records_root>/<prefix>` must lie outside its repository. Returns that
/// directory, created and canonical; a refusal creates nothing. `run` has checked that
/// `prefix` is valid, so it is one path component.
fn validate_records(
    root: &Path,
    records_root: &Path,
    registry: &Registry,
    prefix: &str,
) -> Result<PathBuf> {
    let refuse = |why: String| {
        Err(Error::Config(format!(
            "--records: {} {why}",
            root.display()
        )))
    };
    let named = root.join(records_root);
    let records_root = match named.canonicalize() {
        Ok(dir) if dir.is_dir() => dir,
        Ok(_) => {
            return Err(Error::Config(format!(
                "--records {}: is not a directory",
                named.display()
            )));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(Error::Config(format!(
                "--records {}: does not exist",
                named.display()
            )));
        }
        Err(error) => return Err(error.into()),
    };
    let Some(tree) = crate::link::GitTree::of(root)? else {
        return refuse("is not in a git repository".into());
    };
    if tree.top != root {
        return refuse(format!(
            "is not the top level of its repository ({})",
            tree.top.display()
        ));
    }
    if !tree.is_main() {
        let main = tree
            .main_checkout()
            .map(|m| m.display().to_string())
            .unwrap_or_default();
        return refuse(format!(
            "is a linked worktree; pass its main checkout {main}"
        ));
    }
    if tree.main_checkout().as_deref() != Some(root) {
        return refuse("has no main checkout (a bare repository or a separate git dir)".into());
    }
    // The derived directory need not exist yet; its nearest existing ancestor carries its
    // repository identity.
    let derived = records_root.join(prefix);
    let exists = derived.try_exists()?;
    let derived = if exists {
        derived.canonicalize()?
    } else {
        derived
    };
    let identity = if exists { &derived } else { &records_root };
    if derived.starts_with(root)
        || root.starts_with(&derived)
        || crate::link::GitTree::of(identity)?.is_some_and(|theirs| theirs.common == tree.common)
    {
        return Err(Error::Config(format!(
            "--records {}: {} is in this checkout's repository; records must live outside \
             the code tree",
            named.display(),
            derived.display()
        )));
    }
    if root.join(crate::repo::CONFIG_REL).is_file() || crate::link::config_in_index(&tree.top)? {
        return Err(Error::RecordsInTree(format!(
            "{}: tasks/.config.toml is in the working tree or the index; `git rm -r tasks` \
             (and the docs you are moving) first, then rerun; a staged deletion is allowed",
            root.display()
        )));
    }
    let mut taken: Vec<_> = crate::locations::scan_checkouts(registry)
        .strict()?
        .into_iter()
        .filter(|linked| linked.prefix != prefix && linked.checkout == root)
        .map(|linked| linked.prefix)
        .collect();
    for (other, other_root) in &registry.projects {
        if other != prefix
            && registry.records_dir(other).is_some()
            && crate::rename::root_identity(other_root)? == root
            && !taken.contains(other)
        {
            taken.push(other.clone());
        }
    }
    if !taken.is_empty() {
        return Err(Error::CheckoutConflict(format!(
            "{} is already the checkout of {}",
            root.display(),
            taken.join(" and ")
        )));
    }
    // A repository nested in, or holding, another project's tree would be reached by the
    // walk up to that project's config before the pairing; one sharing a common git
    // directory with it is the same repository (validate_checkout's overlap rule).
    for other in registry.projects.keys() {
        if other == prefix {
            continue;
        }
        let dirs = registry
            .project_root(other)
            .into_iter()
            .chain(registry.records_dir(other));
        for dir in dirs {
            if !dir.try_exists()? {
                continue;
            }
            let dir = dir.canonicalize()?;
            if root.starts_with(&dir)
                || dir.starts_with(root)
                || crate::link::GitTree::of(&dir)?
                    .is_some_and(|theirs| theirs.common == tree.common)
            {
                return refuse(format!(
                    "overlaps registered project {other} at {}",
                    dir.display()
                ));
            }
        }
    }
    std::fs::create_dir_all(&derived)?;
    Ok(derived.canonicalize()?)
}
