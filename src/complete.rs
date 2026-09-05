//! Completion candidates for the `CompleteEnv` protocol.
//!
//! Nothing here is reachable from a command path, and nothing here returns `Result`.
//! Completion runs on every TAB with no error channel — the bash stub discards
//! `COMPREPLY` when the completer exits non-zero, and a write to stderr corrupts the
//! prompt — so every failure yields an empty candidate list instead. This is the
//! deliberate exception to the repo's fail-early rule recorded in
//! `docs/specs/2026-09-05-shell-completions-design.md`.

use std::ffi::OsStr;
use std::path::PathBuf;

use clap::CommandFactory;
use clap_complete::CompletionCandidate;

use crate::cli::Cli;
use crate::model::{Size, Status, Task, TaskId};
use crate::registry::Registry;
use crate::repo::Project;
use crate::scope::Origin;

fn plain(
    values: impl IntoIterator<Item = impl Into<std::ffi::OsString>>,
) -> Vec<CompletionCandidate> {
    values.into_iter().map(CompletionCandidate::new).collect()
}

/// Every status. `edit --status`, `list --status`, `tags --status`.
pub fn statuses() -> Vec<CompletionCandidate> {
    plain(Status::ALL.iter().map(|status| status.as_str()))
}

/// The two `add` accepts; it rejects the rest with a validation error.
pub fn add_statuses() -> Vec<CompletionCandidate> {
    plain([Status::Idea.as_str(), Status::Todo.as_str()])
}

pub fn sizes() -> Vec<CompletionCandidate> {
    plain(Size::ALL.iter().map(|size| size.as_str()))
}

/// The keys `query::SortKey::parse` accepts.
pub fn sorts() -> Vec<CompletionCandidate> {
    plain(["priority", "updated", "created"])
}

/// The modes `style::ColorMode::resolve` accepts.
pub fn colors() -> Vec<CompletionCandidate> {
    plain(["auto", "always", "never"])
}

pub fn categories() -> Vec<CompletionCandidate> {
    plain(crate::commands::feedback::CATEGORIES)
}

/// Registered prefixes, in registry order. An unreadable registry offers nothing.
pub fn prefixes() -> Vec<CompletionCandidate> {
    let Ok(registry) = Registry::load() else {
        return Vec::new();
    };
    plain(registry.projects.keys().cloned().collect::<Vec<_>>())
}

/// Subcommands whose first positional is a task id. `add`'s is a title, so it has no
/// subject; guessing one there would read `tasks add fam-000001 --parent <TAB>` as an
/// invocation against `fam`.
const ID_FIRST: [&str; 11] = [
    "show", "root", "tree", "edit", "note", "start", "done", "drop", "block", "unblock", "dep",
];

/// What the words on the command line say about the invocation. Only the parts the
/// candidate scopes need; everything else is discarded.
#[derive(Debug, Default, PartialEq)]
pub struct Line {
    pub subcommand: Option<String>,
    pub dir: Option<PathBuf>,
    pub project: Option<String>,
    pub subject: Option<TaskId>,
    pub all_projects: bool,
}

/// The user's command line, from this process's own argv.
///
/// The transport is `tasks -- tasks -C dir show tasks-`: everything after the first `--`
/// is what the shell had, `words[0]` being the binary name. `_CLAP_COMPLETE_INDEX` names
/// the word under the cursor, which is the fragment being completed rather than context,
/// so it is blanked instead of removed — every other word keeps its own index.
pub fn line() -> Line {
    let argv: Vec<String> = std::env::args().collect();
    let Some(dashes) = argv.iter().skip(1).position(|word| word == "--") else {
        return Line::default();
    };
    let words: Vec<String> = argv[dashes + 2..].to_vec();
    let cursor: usize = std::env::var("_CLAP_COMPLETE_INDEX")
        .ok()
        .and_then(|index| index.parse().ok())
        .unwrap_or(usize::MAX);
    let mut refs: Vec<Option<&str>> = words.iter().map(|word| Some(word.as_str())).collect();
    if cursor < refs.len() {
        refs[cursor] = None;
    }
    walk(&refs)
}

/// Classify each word the way clap will. Value-taking is read off the built `Command`
/// rather than a hardcoded list, so a new flag cannot desynchronize this walk.
fn walk(words: &[Option<&str>]) -> Line {
    let root = Cli::command();
    let mut line = Line::default();
    let mut sub: Option<clap::Command> = None;
    let mut positionals: Vec<&str> = Vec::new();
    let mut escaped = false;
    let mut index = 1; // words[0] is the binary name

    while index < words.len() {
        let Some(word) = words[index] else {
            // The cursor. Never context: not a subcommand, not a positional.
            if sub.is_none() {
                return Line::default();
            }
            index += 1;
            continue;
        };
        index += 1;

        if !escaped && word == "--" {
            escaped = true;
            continue;
        }
        if escaped || !word.starts_with('-') || word == "-" {
            match &sub {
                Some(_) => positionals.push(word),
                None => match root.get_subcommands().find(|c| c.get_name() == word) {
                    Some(found) => {
                        line.subcommand = Some(word.to_string());
                        sub = Some(found.clone());
                    }
                    None => return Line::default(),
                },
            }
            continue;
        }

        let (arg, attached) = match word.strip_prefix("--") {
            Some(rest) => match rest.split_once('=') {
                Some((name, value)) => (find_long(&root, sub.as_ref(), name), Some(value)),
                None => (find_long(&root, sub.as_ref(), rest), None),
            },
            None => {
                let rest = &word[1..];
                let Some(short) = rest.chars().next() else {
                    continue;
                };
                let tail = &rest[short.len_utf8()..];
                let attached = (!tail.is_empty()).then(|| tail.strip_prefix('=').unwrap_or(tail));
                (find_short(&root, sub.as_ref(), short), attached)
            }
        };
        let Some(arg) = arg else { continue };

        let mut values: Vec<&str> = Vec::new();
        if arg.get_action().takes_values() {
            match attached {
                Some(value) => values.push(value),
                None => {
                    let max = arg.get_num_args().map_or(1, |range| range.max_values());
                    while values.len() < max && index < words.len() {
                        let Some(next) = words[index] else {
                            // The option's value is the word being completed. That is the
                            // ordinary case for every option-valued completion, not
                            // ambiguity: stop consuming, and keep everything the walk has
                            // already learned. `-C <cursor>` records no directory and
                            // falls back to the process's own, which is what the spec asks.
                            break;
                        };
                        if next.starts_with('-') && next != "-" {
                            break;
                        }
                        values.push(next);
                        index += 1;
                    }
                }
            }
        }
        record(&mut line, arg, &values);
    }

    if let Some(name) = &line.subcommand
        && ID_FIRST.contains(&name.as_str())
        && let Some(first) = positionals.first()
    {
        line.subject = TaskId::parse(first).ok();
    }
    line
}

/// The subcommand's arg, else the root's — where the globals `-C`, `--pretty`, `--color`
/// are declared.
fn find_long<'a>(
    root: &'a clap::Command,
    sub: Option<&'a clap::Command>,
    name: &str,
) -> Option<&'a clap::Arg> {
    let matches = |arg: &&clap::Arg| arg.get_long() == Some(name);
    sub.and_then(|sub| sub.get_arguments().find(matches))
        .or_else(|| root.get_arguments().find(matches))
}

fn find_short<'a>(
    root: &'a clap::Command,
    sub: Option<&'a clap::Command>,
    short: char,
) -> Option<&'a clap::Arg> {
    let matches = |arg: &&clap::Arg| arg.get_short() == Some(short);
    sub.and_then(|sub| sub.get_arguments().find(matches))
        .or_else(|| root.get_arguments().find(matches))
}

fn record(line: &mut Line, arg: &clap::Arg, values: &[&str]) {
    match arg.get_id().as_str() {
        "dir" => {
            if let Some(value) = values.last() {
                line.dir = Some(PathBuf::from(value));
            }
        }
        "project" => {
            if let Some(value) = values.last() {
                line.project = Some((*value).to_string());
            }
        }
        "all_projects" => line.all_projects = true,
        _ => {}
    }
}

/// The project at the effective directory, if there is one.
fn open_local(line: &Line) -> Option<Project> {
    let dir = match &line.dir {
        Some(dir) => dir.clone(),
        None => std::env::current_dir().ok()?,
    };
    Project::locate(&dir).ok()
}

/// A registered project, if the registry knows it and it is reachable.
fn open_prefix(registry: &Registry, prefix: &str) -> Option<Project> {
    let root = registry.project_root(prefix)?;
    if !crate::scope::is_reachable(root).unwrap_or(false) {
        return None;
    }
    crate::scope::open_registered(registry, prefix, Origin::Prefix).ok()
}

/// The prefix the user has typed ahead of `-`, when it is a well-formed one.
fn typed_prefix(current: &str) -> Option<&str> {
    let (prefix, _) = current.split_once('-')?;
    crate::model::is_valid_prefix(prefix).then_some(prefix)
}

/// Local first, foreign by typed prefix — the write-side rule in §6 of
/// `docs/specs/2026-08-29-tasks-design.md`. A prefix equal to `base`'s completes from
/// `base`, so a worktree beats the registered root of the same project.
fn local_or_foreign(registry: &Registry, base: Option<Project>, current: &str) -> Vec<Task> {
    if let Some(prefix) = typed_prefix(current)
        && base.as_ref().is_none_or(|project| project.prefix != prefix)
        && let Some(foreign) = open_prefix(registry, prefix)
    {
        return foreign.scan().unwrap_or_default();
    }
    base.map(|project| project.scan().unwrap_or_default())
        .unwrap_or_default()
}

/// An id, described when its task can be read. `dep --rm` offers dependencies that may
/// be unreachable, so a missing task drops the description, never the candidate.
fn described(id: &str, task: Option<&Task>) -> CompletionCandidate {
    let help = task.map(|task| {
        clap::builder::StyledStr::from(format!("{}  {}", task.status.as_str(), task.title))
    });
    CompletionCandidate::new(id).help(help)
}

/// Filter to the typed fragment, open tasks first, each group by id.
fn candidates(tasks: Vec<Task>, current: &str) -> Vec<CompletionCandidate> {
    let mut matching: Vec<Task> = tasks
        .into_iter()
        .filter(|task| task.id.to_string().starts_with(current))
        .collect();
    matching.sort_by(|a, b| {
        b.status
            .is_open()
            .cmp(&a.status.is_open())
            .then_with(|| a.id.cmp(&b.id))
    });
    matching
        .iter()
        .map(|task| described(&task.id.to_string(), Some(task)))
        .collect()
}

/// `show`, `root`, and every id-taking write.
pub fn id_directed(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    let line = line();
    let registry = Registry::load().unwrap_or_default();
    candidates(
        local_or_foreign(&registry, open_local(&line), current),
        current,
    )
}

/// The project a new or edited task lands in: `--project`, else the subject id's project,
/// else the effective directory's. A subject whose prefix matches the local project keeps
/// the local checkout, as the write path does.
fn destination(registry: &Registry, line: &Line) -> Option<Project> {
    if let Some(prefix) = &line.project {
        return open_prefix(registry, prefix);
    }
    let local = open_local(line);
    let Some(subject) = &line.subject else {
        return local;
    };
    if local
        .as_ref()
        .is_some_and(|project| project.prefix == subject.prefix)
    {
        return local;
    }
    open_prefix(registry, &subject.prefix)
}

/// `tree <id>` and `list --parent`: the scope the command itself scans, which
/// `--all-projects` widens to the registry (`list.rs` validates the parent against it).
pub fn scoped(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    let line = line();
    let registry = Registry::load().unwrap_or_default();
    let mut tasks = Vec::new();
    if line.all_projects {
        for prefix in registry.projects.keys() {
            if let Some(project) = open_prefix(&registry, prefix) {
                tasks.extend(project.scan().unwrap_or_default());
            }
        }
    } else if let Some(project) = open_local(&line) {
        tasks.extend(project.scan().unwrap_or_default());
    }
    candidates(tasks, current)
}

/// `--parent`: a parent must live in the same project as its child.
pub fn destination_ids(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    let line = line();
    let registry = Registry::load().unwrap_or_default();
    let tasks = destination(&registry, &line)
        .map(|project| project.scan().unwrap_or_default())
        .unwrap_or_default();
    candidates(tasks, current)
}

/// `--depends` and `dep --on`: whatever `Resolver` can reach *from the destination*.
/// `apply_fields` and `dep::run` both build their `Resolver` on the project being written
/// to, so a worktree's own ids are the ones `add --project` would reject.
pub fn resolvable(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    let line = line();
    let registry = Registry::load().unwrap_or_default();
    let base = destination(&registry, &line);
    candidates(local_or_foreign(&registry, base, current), current)
}

/// `dep --rm`: only what the task already depends on. Removal does not resolve ids, so an
/// unreachable dependency stays a candidate and simply loses its description.
pub fn dependencies(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    let line = line();
    let registry = Registry::load().unwrap_or_default();
    let (Some(subject), Some(project)) = (&line.subject, destination(&registry, &line)) else {
        return Vec::new();
    };
    let Ok(task) = project.read_task(subject) else {
        return Vec::new();
    };
    let resolver = crate::resolve::Resolver::new(&project, &registry);
    let mut resolved: Vec<(TaskId, Option<Task>)> = task
        .depends
        .iter()
        .filter(|id| id.to_string().starts_with(current))
        .map(|id| (id.clone(), resolver.resolve_task(id).ok().flatten()))
        .collect();
    // Open dependencies before closed-or-unresolvable, each group by id ascending —
    // the same ordering `candidates()` applies, with an unresolvable dependency (no
    // status to be "open" with) sorted alongside the closed ones.
    resolved.sort_by(|(a_id, a_task), (b_id, b_task)| {
        let a_open = a_task.as_ref().is_some_and(|task| task.status.is_open());
        let b_open = b_task.as_ref().is_some_and(|task| task.status.is_open());
        b_open.cmp(&a_open).then_with(|| a_id.cmp(b_id))
    });
    resolved
        .into_iter()
        .map(|(id, task)| described(&id.to_string(), task.as_ref()))
        .collect()
}

/// `feedback --recur`: open, `feedback`-tagged tasks in the project registered as
/// `tasks`, never the local directory — `feedback::locate_target` resolves the same way,
/// so a worktree of the upstream must not suggest records it has not pushed.
pub fn upstream_feedback(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    let registry = Registry::load().unwrap_or_default();
    let Some(project) = open_prefix(&registry, crate::commands::feedback::TARGET_PREFIX) else {
        return Vec::new();
    };
    let tasks = project
        .scan()
        .unwrap_or_default()
        .into_iter()
        .filter(crate::commands::feedback::is_open_feedback)
        .collect();
    candidates(tasks, current)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The words a shell sends, with `None` marking the cursor.
    fn walked(words: &[&str], cursor: usize) -> Line {
        let mut words: Vec<Option<&str>> = words.iter().map(|word| Some(*word)).collect();
        if cursor < words.len() {
            words[cursor] = None;
        }
        walk(&words)
    }

    #[test]
    fn the_subject_is_the_first_positional_of_an_id_taking_subcommand() {
        let line = walked(&["tasks", "edit", "sci-000001", "--parent", ""], 4);
        assert_eq!(line.subcommand.as_deref(), Some("edit"));
        assert_eq!(
            line.subject.map(|id| id.to_string()),
            Some("sci-000001".into())
        );
    }

    #[test]
    fn adds_title_is_not_a_subject_even_when_it_parses_as_an_id() {
        let line = walked(&["tasks", "add", "fam-000001", "--parent", ""], 4);
        assert_eq!(line.subcommand.as_deref(), Some("add"));
        assert_eq!(line.subject, None, "add's first positional is a title");
    }

    #[test]
    fn option_values_are_not_positionals() {
        let line = walked(
            &[
                "tasks",
                "edit",
                "--body",
                "fam-000001",
                "sci-abcdef",
                "--parent",
                "",
            ],
            6,
        );
        assert_eq!(
            line.subject.map(|id| id.to_string()),
            Some("sci-abcdef".into())
        );
    }

    #[test]
    fn a_multi_value_option_consumes_every_following_word() {
        let line = walked(
            &[
                "tasks",
                "dep",
                "sci-000001",
                "--on",
                "fam-1",
                "fam-2",
                "--rm",
                "",
            ],
            7,
        );
        assert_eq!(
            line.subject.map(|id| id.to_string()),
            Some("sci-000001".into())
        );
    }

    #[test]
    fn dir_is_read_in_every_form_and_the_last_one_wins() {
        for words in [
            vec!["tasks", "-C", "/w", "show", ""],
            vec!["tasks", "-C/w", "show", ""],
            vec!["tasks", "-C=/w", "show", ""],
        ] {
            let cursor = words.len() - 1;
            assert_eq!(
                walked(&words, cursor).dir,
                Some(PathBuf::from("/w")),
                "{words:?}"
            );
        }
        let line = walked(&["tasks", "-C", "/a", "-C", "/b", "show", ""], 6);
        assert_eq!(line.dir, Some(PathBuf::from("/b")));
    }

    #[test]
    fn project_and_all_projects_are_recorded() {
        let line = walked(
            &["tasks", "add", "T", "--project", "fam", "--parent", ""],
            6,
        );
        assert_eq!(line.project.as_deref(), Some("fam"));
        let line = walked(&["tasks", "add", "T", "--project=fam", "--parent", ""], 5);
        assert_eq!(line.project.as_deref(), Some("fam"));
        let line = walked(&["tasks", "list", "--all-projects", "--parent", ""], 4);
        assert!(line.all_projects);
        assert!(!walked(&["tasks", "list", "--parent", ""], 3).all_projects);
    }

    #[test]
    fn ambiguity_yields_no_context() {
        // an unrecognized subcommand
        assert_eq!(walked(&["tasks", "frobnicate", ""], 2).subcommand, None);
        // the subcommand itself is under the cursor
        assert_eq!(walked(&["tasks", ""], 1).subcommand, None);
        // -C's value is the word being completed
        assert_eq!(walked(&["tasks", "-C", ""], 2).dir, None);
        // a first positional that is not an id
        assert_eq!(
            walked(&["tasks", "show", "nonsense", "--"], 3).subject,
            None
        );
    }

    #[test]
    fn a_user_typed_double_dash_makes_the_rest_positional() {
        let line = walked(&["tasks", "note", "--", "sci-000001", "text"], 4);
        assert_eq!(
            line.subject.map(|id| id.to_string()),
            Some("sci-000001".into())
        );
    }
}
