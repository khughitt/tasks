//! A write from a copy that is behind another checkout's refuses (record-home spec §3).

use crate::model::TaskId;
use crate::repo::{Project, SiblingCopy};
use std::path::{Path, PathBuf};

/// The sibling copy a write must not leave behind.
pub struct Newer {
    pub root: PathBuf,
    /// Its `updated` stamp.
    pub theirs: String,
    /// The stamps are equal but the bytes differ: a same-second fork (§3.1).
    pub same_stamp: bool,
    /// Every other sibling that also refuses, named after `root`.
    pub others: Vec<PathBuf>,
}

/// What a refusal tells the caller to do (§3.2). The caller chooses it, because only the
/// caller can read the claim store and its own identity.
pub enum Remedy<'a> {
    /// Rule 1: another session works in the newer checkout, as `(task id, session)` pairs.
    Occupied(Vec<(String, String)>),
    /// Rule 2: the same stamp with different bytes.
    Merge,
    /// Rule 3: a linked worktree behind the main checkout; the arguments for the alternative.
    Handoff(&'a [String]),
    /// Rule 4: this invocation's arguments, rerun there.
    Rerun(&'a [String]),
    /// A feedback recurrence where rule 4 would apply: `-C` would change the reporter.
    New,
}

/// Compares the copy this command loaded (its `loaded` stamp and `raw` bytes) with every
/// other worktree's copy of `id`. `Some` proves the write must refuse: a strictly newer
/// stamp, or else the same stamp with different bytes. The warnings name what could not
/// be compared: an unreadable copy, or a failure to list the worktrees.
pub fn compare(
    project: &Project,
    id: &TaskId,
    loaded: &str,
    raw: &str,
) -> (Option<Newer>, Vec<String>) {
    let copies = match project.sibling_task_copies(id) {
        Ok(Some(copies)) => copies,
        Ok(None) => return (None, Vec::new()),
        Err(error) => {
            return (
                None,
                vec![format!(
                    "could not check other checkouts for a newer copy of {id} ({error})"
                )],
            );
        }
    };
    let mut warnings = Vec::new();
    let mut newer: Vec<(PathBuf, String)> = Vec::new();
    let mut forked: Vec<PathBuf> = Vec::new();
    for copy in copies {
        match copy {
            SiblingCopy::Found { root, updated, .. } if updated.as_str() > loaded => {
                newer.push((root, updated));
            }
            SiblingCopy::Found {
                root,
                updated,
                raw: theirs,
            } if updated == loaded && theirs != raw => forked.push(root),
            SiblingCopy::Found { .. } => {}
            SiblingCopy::Unreadable { root, detail } => warnings.push(format!(
                "tasks/{id}.md in {} could not be read ({detail}); whether that copy has \
                 diverged from this one is unknown",
                root.display()
            )),
        }
    }
    // Newest first, ties in path order, so the message is stable.
    newer.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    forked.sort();
    let found = if !newer.is_empty() {
        let (root, theirs) = newer.remove(0);
        Some(Newer {
            root,
            theirs,
            same_stamp: false,
            others: newer.into_iter().map(|(root, _)| root).collect(),
        })
    } else if !forked.is_empty() {
        let root = forked.remove(0);
        Some(Newer {
            root,
            theirs: loaded.to_string(),
            same_stamp: true,
            others: forked,
        })
    } else {
        None
    };
    (found, warnings)
}

/// The `stale_copy` detail (§3.2).
pub fn refusal(id: &TaskId, newer: &Newer, ours: &str, remedy: Remedy) -> String {
    let root = newer.root.display();
    let head = if newer.same_stamp {
        format!(
            "tasks/{id}.md in {root} has the same stamp as this copy ({ours}) but different \
             content, so both were written in the same second"
        )
    } else {
        format!(
            "tasks/{id}.md in {root} is newer than this copy ({} there, {ours} here)",
            newer.theirs
        )
    };
    let also = if newer.others.is_empty() {
        String::new()
    } else {
        let others: Vec<String> = newer
            .others
            .iter()
            .map(|root| root.display().to_string())
            .collect();
        format!(" (also newer in: {})", others.join(", "))
    };
    let next = match remedy {
        Remedy::Occupied(work) => {
            let work: Vec<String> = work
                .iter()
                .map(|(task, session)| format!("{task} (session {session})"))
                .collect();
            format!(
                "{root} is where another session works: {}. Wait for that branch to merge, \
                 or merge its copy of tasks/{id}.md into this checkout, then rerun here",
                work.join(", ")
            )
        }
        Remedy::Merge => {
            format!("Merge that copy of tasks/{id}.md into this checkout, then rerun here")
        }
        Remedy::Handoff(args) => format!(
            "Commit tasks/{id}.md in {root} if it has changes, merge it into this branch, then \
             rerun here; the merge may conflict where both copies changed. Or, to write in \
             the main checkout instead: {}{}",
            rerun_line(&newer.root, args),
            stdin_hint(args)
        ),
        Remedy::Rerun(args) => format!(
            "Run it there: {}{}",
            rerun_line(&newer.root, args),
            stdin_hint(args)
        ),
        Remedy::New => "Rerun with --new to file a separate entry".to_string(),
    };
    format!("{head}{also}; nothing was written. {next}")
}

/// `-` is how every command here names stdin, which a printed line cannot carry.
fn stdin_hint(args: &[String]) -> &'static str {
    if args.iter().any(|arg| arg == "-") {
        "; supply the same input on stdin"
    } else {
        ""
    }
}

/// `tasks -C <root> <args>`, with any `-C` the invocation carried removed, quoted so the line
/// pastes into a POSIX shell. `-C` only chooses the project and the process keeps its
/// working directory, so a relative path in `args` resolves the same way in the retry.
pub fn rerun_line(root: &Path, args: &[String]) -> String {
    let mut words = vec![
        "tasks".to_string(),
        "-C".to_string(),
        quote(&root.display().to_string()),
    ];
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if arg == "--" {
            words.push(quote(arg));
            words.extend(rest.by_ref().map(|arg| quote(arg)));
            break;
        }
        if arg == "-C" {
            rest.next();
            continue;
        }
        if arg.starts_with("-C") {
            continue;
        }
        words.push(quote(arg));
    }
    words.join(" ")
}

fn quote(word: &str) -> String {
    let safe = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c));
    if safe {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| word.to_string()).collect()
    }

    fn newer(same_stamp: bool, others: &[&str]) -> Newer {
        Newer {
            root: PathBuf::from("/wt"),
            theirs: "2026-09-07T10:00:00Z".into(),
            same_stamp,
            others: others.iter().map(PathBuf::from).collect(),
        }
    }

    fn id() -> TaskId {
        TaskId::parse("sci-1a2b3c").unwrap()
    }

    #[test]
    fn rerun_line_quotes_only_what_a_shell_would_split() {
        let line = rerun_line(
            Path::new("/w t"),
            &args(&["note", "sci-1", "it's done", "--pretty", ""]),
        );
        assert_eq!(
            line,
            r"tasks -C '/w t' note sci-1 'it'\''s done' --pretty ''"
        );
    }

    #[test]
    fn rerun_line_drops_every_spelling_of_an_earlier_dash_c() {
        for given in [
            &["-C", "/old", "note", "sci-1", "x"][..],
            &["-C/old", "note", "sci-1", "x"],
            &["-C=/old", "note", "sci-1", "x"],
        ] {
            assert_eq!(
                rerun_line(Path::new("/new"), &args(given)),
                "tasks -C /new note sci-1 x"
            );
        }
    }

    #[test]
    fn rerun_line_keeps_everything_after_a_double_dash() {
        assert_eq!(
            rerun_line(Path::new("/new"), &args(&["note", "sci-1", "--", "-C"])),
            "tasks -C /new note sci-1 -- -C"
        );
    }

    #[test]
    fn non_ascii_is_quoted() {
        assert_eq!(quote("naïve"), "'naïve'");
    }

    #[test]
    fn the_retry_remedy_names_the_command_and_stdin() {
        let given = args(&["attach", "sci-1a2b3c", "-", "--name", "x"]);
        assert_eq!(
            refusal(
                &id(),
                &newer(false, &["/b"]),
                "2026-09-05T09:00:00Z",
                Remedy::Rerun(&given)
            ),
            "tasks/sci-1a2b3c.md in /wt is newer than this copy (2026-09-07T10:00:00Z there, \
             2026-09-05T09:00:00Z here) (also newer in: /b); nothing was written. Run it \
             there: tasks -C /wt attach sci-1a2b3c - --name x; supply the same input on stdin"
        );
    }

    #[test]
    fn the_handoff_remedy_leads_with_the_merge() {
        let given = args(&["start", "sci-1a2b3c"]);
        assert_eq!(
            refusal(
                &id(),
                &newer(false, &[]),
                "2026-09-05T09:00:00Z",
                Remedy::Handoff(&given)
            ),
            "tasks/sci-1a2b3c.md in /wt is newer than this copy (2026-09-07T10:00:00Z there, \
             2026-09-05T09:00:00Z here); nothing was written. Commit tasks/sci-1a2b3c.md in \
             /wt if it has changes, merge it into this branch, then rerun here; the merge may \
             conflict where both copies changed. Or, to write in the main checkout instead: \
             tasks -C /wt start sci-1a2b3c"
        );
    }

    #[test]
    fn the_occupied_remedy_names_the_work_and_offers_no_retry() {
        let text = refusal(
            &id(),
            &newer(false, &[]),
            "2026-09-05T09:00:00Z",
            Remedy::Occupied(vec![("sci-999999".into(), "agent-b".into())]),
        );
        assert!(
            text.ends_with(
                "nothing was written. /wt is where another session works: sci-999999 \
                 (session agent-b). Wait for that branch to merge, or merge its copy of \
                 tasks/sci-1a2b3c.md into this checkout, then rerun here"
            ),
            "{text}"
        );
        assert!(!text.contains("tasks -C"), "{text}");
    }

    #[test]
    fn the_same_stamp_remedy_is_the_merge_alone() {
        assert_eq!(
            refusal(
                &id(),
                &newer(true, &[]),
                "2026-09-05T09:00:00Z",
                Remedy::Merge
            ),
            "tasks/sci-1a2b3c.md in /wt has the same stamp as this copy \
             (2026-09-05T09:00:00Z) but different content, so both were written in the same \
             second; nothing was written. Merge that copy of tasks/sci-1a2b3c.md into this \
             checkout, then rerun here"
        );
    }

    #[test]
    fn the_feedback_remedy_offers_new() {
        assert!(
            refusal(
                &id(),
                &newer(false, &[]),
                "2026-09-05T09:00:00Z",
                Remedy::New
            )
            .ends_with("nothing was written. Rerun with --new to file a separate entry")
        );
    }
}
