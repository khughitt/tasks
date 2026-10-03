//! Shared resources a task's work uses, drawn from the vocabulary its project declares in
//! `[needs]` (docs/specs/2026-10-03-lanes-needs-groups-design.md §4).

use crate::error::{Error, Result};
use crate::model::Task;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// The tag grammar the spec gives need names (§4.1): lowercase ASCII letters, digits, and
/// `-`, never starting with `-`, so a name cannot read as a flag. Shared with the
/// vocabulary keys, the record field, and the `--need` flags.
pub fn validate_name(name: &str) -> Result<()> {
    let well_formed = !name.is_empty()
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if well_formed {
        Ok(())
    } else {
        Err(Error::Validation(format!(
            "need {name:?} must be lowercase letters, digits, and '-', not starting with '-'"
        )))
    }
}

/// One `[needs.<name>]` table (spec §4.1). `meaning` is one required line; `exclusive`
/// says the resource serves one session at a time.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NeedDecl {
    pub meaning: String,
    #[serde(default)]
    pub exclusive: bool,
}

/// A project's declared needs, by name. Committed with the project like `[tags]`.
pub type Vocabulary = BTreeMap<String, NeedDecl>;

fn declared_list(vocab: &Vocabulary) -> String {
    if vocab.is_empty() {
        "none are declared".into()
    } else {
        format!(
            "declared: {}",
            vocab.keys().cloned().collect::<Vec<_>>().join(", ")
        )
    }
}

/// Every name must be declared (spec §4.2, on write). The first that is not fails
/// `unknown_need`, naming what the project does declare.
pub fn require_declared(vocab: &Vocabulary, names: &[String]) -> Result<()> {
    match names.iter().find(|name| !vocab.contains_key(*name)) {
        None => Ok(()),
        Some(name) => Err(Error::UnknownNeed(format!(
            "need {name:?} is not declared in {}'s [needs]; {}",
            crate::repo::CONFIG_REL,
            declared_list(vocab)
        ))),
    }
}

/// The needs a claim holds (spec §4.4): those its own project declares exclusive. An
/// undeclared name is ignored. Sorted and deduped.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "exclusive holds (slice 2) are its first caller")
)]
pub fn exclusive_of(vocab: &Vocabulary, needs: &[String]) -> Vec<String> {
    needs
        .iter()
        .filter(|need| vocab.get(*need).is_some_and(|decl| decl.exclusive))
        .cloned()
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect()
}

/// The variable a session sets for the needs it cannot meet (spec §4.3).
pub const WITHOUT_ENV: &str = "TASKS_WITHOUT";

/// Each in-scope project's vocabulary, by prefix. A task's needs mean what its own
/// project declares, so a view looks a task's vocabulary up here rather than in the
/// union of the scope's.
pub type Vocabularies<'a> = HashMap<&'a str, &'a Vocabulary>;

/// The vocabulary of the project `task` belongs to. Every task a view filters was scanned
/// from a project in scope, and `Project::scan` refuses a record whose prefix is not its
/// project's, so a miss is a bug in the caller, not a state to tolerate.
pub fn vocabulary_of<'a>(vocabs: &Vocabularies<'a>, task: &Task) -> &'a Vocabulary {
    vocabs
        .get(task.id.prefix.as_str())
        .copied()
        .unwrap_or_else(|| {
            panic!(
                "{}: project {} is not in the vocabularies in scope",
                task.id, task.id.prefix
            )
        })
}

/// The needs a session cannot meet: the union of `--without` and `TASKS_WITHOUT`. A task
/// is hidden from `ready`, `next`, and `prime` when it needs one of them that its own
/// project declares.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Without {
    names: BTreeSet<String>,
}

impl Without {
    /// `flag` is strict: a name no vocabulary in scope declares is a typo, refused with
    /// `unknown_need`. `env` is lenient: set once per host for every project, so any name
    /// is accepted. Entries are comma-separated, trimmed, and empty ones dropped; an
    /// empty variable contributes nothing. Strictness ends here: `hides` treats every
    /// name alike, hiding only in the projects that declare it.
    pub fn resolve(
        flag: &[String],
        env: Option<&str>,
        vocabs: &Vocabularies<'_>,
    ) -> Result<Without> {
        let mut names = BTreeSet::new();
        for name in flag {
            if !vocabs.values().any(|vocab| vocab.contains_key(name)) {
                return Err(Error::UnknownNeed(format!(
                    "--without {name:?}: no project in scope declares this need in {}'s [needs]",
                    crate::repo::CONFIG_REL
                )));
            }
            names.insert(name.clone());
        }
        for name in env
            .unwrap_or("")
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            names.insert(name.to_string());
        }
        Ok(Without { names })
    }

    /// `resolve` with the variable read from the process environment.
    pub fn from_env(flag: &[String], vocabs: &Vocabularies<'_>) -> Result<Without> {
        let env = match std::env::var(WITHOUT_ENV) {
            Ok(value) => Some(value),
            Err(std::env::VarError::NotPresent) => None,
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err(Error::Validation(format!(
                    "{WITHOUT_ENV} is not valid UTF-8"
                )));
            }
        };
        Self::resolve(flag, env.as_deref(), vocabs)
    }

    /// `vocab` is the vocabulary of `task`'s own project. A name that project does not
    /// declare hides nothing there (spec §4.3), even when another project in scope
    /// declares it.
    pub fn hides(&self, task: &Task, vocab: &Vocabulary) -> bool {
        task.needs
            .iter()
            .any(|need| self.names.contains(need) && vocab.contains_key(need))
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }

    /// Drops the hidden tasks, each judged by its own project's vocabulary, and returns
    /// how many went.
    pub fn retain(&self, tasks: &mut Vec<Task>, vocabs: &Vocabularies<'_>) -> usize {
        let before = tasks.len();
        tasks.retain(|task| !self.hides(task, vocabulary_of(vocabs, task)));
        before - tasks.len()
    }

    /// One warning per view, like the complexity cutoff's: nothing is hidden silently.
    pub fn warning(&self, hidden: usize) -> Option<String> {
        (hidden > 0).then(|| {
            format!(
                "without {}: {hidden} task(s) hidden",
                self.names().collect::<Vec<_>>().join(", ")
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_tag_grammar() {
        for good in ["quiet", "gpu-0", "2d", "needs-owner"] {
            assert!(validate_name(good).is_ok(), "{good}");
        }
        for bad in ["", "Quiet", "-quiet", "a_b", "a b", "qüiet", "quiet\n"] {
            let error = validate_name(bad).unwrap_err();
            assert_eq!(error.kind(), "validation", "{bad:?}");
        }
    }

    fn vocab(entries: &[(&str, bool)]) -> Vocabulary {
        entries
            .iter()
            .map(|(name, exclusive)| {
                (
                    (*name).to_string(),
                    NeedDecl {
                        meaning: format!("{name} meaning"),
                        exclusive: *exclusive,
                    },
                )
            })
            .collect()
    }

    #[test]
    fn require_declared_names_the_first_undeclared_need_and_the_vocabulary() {
        let declared = vocab(&[("quiet", true), ("owner", false)]);
        assert!(require_declared(&declared, &["quiet".into(), "owner".into()]).is_ok());
        assert!(require_declared(&declared, &[]).is_ok());
        let error = require_declared(&declared, &["quiet".into(), "gpu".into()]).unwrap_err();
        assert_eq!(error.kind(), "unknown_need");
        let detail = error.to_string();
        assert!(detail.contains("\"gpu\""), "{detail}");
        assert!(detail.contains("declared: owner, quiet"), "{detail}");
        let error = require_declared(&Vocabulary::new(), &["quiet".into()]).unwrap_err();
        assert!(error.to_string().contains("none are declared"), "{error}");
    }

    #[test]
    fn exclusive_of_keeps_declared_exclusive_needs_sorted_once() {
        let declared = vocab(&[("quiet", true), ("gpu", true), ("owner", false)]);
        let needs: Vec<String> = ["quiet", "owner", "gpu", "quiet", "undeclared"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(exclusive_of(&declared, &needs), ["gpu", "quiet"]);
        assert!(exclusive_of(&declared, &["owner".into()]).is_empty());
    }

    fn task_needing(id: &str, needs: &str) -> Task {
        crate::format::parse_task(
            &format!(
                "---\nid: {id}\ntitle: T\nstatus: todo\npriority: 2\nneeds: [{needs}]\n\
                 created: 2026-10-03T00:00:00Z\nupdated: 2026-10-03T00:00:00Z\n\
                 depends: []\ntags: []\n---\n"
            ),
            "x",
        )
        .unwrap()
    }

    fn names(without: &Without) -> Vec<&str> {
        without.names().collect()
    }

    #[test]
    fn the_variable_is_trimmed_lenient_and_joins_the_strict_flag() {
        let declared = vocab(&[("quiet", true), ("owner", false)]);
        let sci = Vocabularies::from([("sci", &declared)]);
        let without = Without::resolve(&[], Some(",quiet, "), &sci).unwrap();
        assert_eq!(
            names(&without),
            ["quiet"],
            "whitespace and empty entries drop"
        );
        let without = Without::resolve(&[], Some(""), &sci).unwrap();
        assert!(without.is_empty(), "an empty variable contributes nothing");
        assert!(Without::resolve(&[], None, &sci).unwrap().is_empty());
        let without = Without::resolve(&[], Some("gpu"), &sci).unwrap();
        assert_eq!(
            names(&without),
            ["gpu"],
            "an undeclared variable name is no error"
        );
        let without = Without::resolve(&["owner".into()], Some("quiet"), &sci).unwrap();
        assert_eq!(
            names(&without),
            ["owner", "quiet"],
            "the flag adds to the variable"
        );

        let error = Without::resolve(&["gpu".into()], Some("quiet"), &sci).unwrap_err();
        assert_eq!(error.kind(), "unknown_need", "the flag is strict");
        let none = Vocabulary::new();
        let both = Vocabularies::from([("fam", &none), ("sci", &declared)]);
        assert!(
            Without::resolve(&["quiet".into()], None, &both).is_ok(),
            "one project in scope declaring it is enough"
        );
        let fam = Vocabularies::from([("fam", &none)]);
        let error = Without::resolve(&["quiet".into()], None, &fam).unwrap_err();
        assert_eq!(error.kind(), "unknown_need");
    }

    #[test]
    fn without_hides_a_task_needing_any_withheld_name_and_counts_it() {
        let declared = vocab(&[("quiet", true), ("owner", false)]);
        let sci = Vocabularies::from([("sci", &declared)]);
        let without = Without::resolve(&["quiet".into()], None, &sci).unwrap();
        assert!(without.hides(&task_needing("sci-000001", "owner, quiet"), &declared));
        assert!(!without.hides(&task_needing("sci-000001", "owner"), &declared));
        let mut tasks = vec![
            task_needing("sci-000001", "quiet"),
            task_needing("sci-000002", "owner"),
        ];
        assert_eq!(without.retain(&mut tasks, &sci), 1);
        assert_eq!(tasks[0].needs, ["owner"]);
        assert_eq!(
            without.warning(1).as_deref(),
            Some("without quiet: 1 task(s) hidden")
        );
        assert_eq!(without.warning(0), None);
    }

    #[test]
    fn without_hides_only_where_the_owning_project_declares_the_name() {
        // sci declares quiet; fam declares nothing but keeps a record naming quiet. The
        // union of the two vocabularies declares quiet, so a union check would hide both.
        let declared = vocab(&[("quiet", true)]);
        let none = Vocabulary::new();
        let scope = Vocabularies::from([("sci", &declared), ("fam", &none)]);
        let needy = task_needing("sci-000001", "quiet");
        let stale = task_needing("fam-000001", "quiet");
        for (source, without) in [
            (
                "variable",
                Without::resolve(&[], Some("quiet"), &scope).unwrap(),
            ),
            (
                "flag",
                Without::resolve(&["quiet".into()], None, &scope).unwrap(),
            ),
        ] {
            assert!(without.hides(&needy, &declared), "{source}");
            assert!(
                !without.hides(&stale, &none),
                "{source}: fam never declared quiet"
            );
            assert!(
                std::ptr::eq(vocabulary_of(&scope, &stale), &none),
                "{source}: the task's own project supplies the vocabulary"
            );
            let mut tasks = vec![needy.clone(), stale.clone()];
            assert_eq!(without.retain(&mut tasks, &scope), 1, "{source}");
            assert_eq!(tasks.len(), 1, "{source}");
            assert_eq!(tasks[0].id, stale.id, "{source}");
        }
    }

    #[test]
    #[should_panic(expected = "not in the vocabularies in scope")]
    fn a_task_from_outside_the_scope_is_a_caller_bug() {
        let declared = vocab(&[("quiet", true)]);
        let scope = Vocabularies::from([("sci", &declared)]);
        let without = Without::resolve(&[], Some("quiet"), &scope).unwrap();
        let _ = without.retain(&mut vec![task_needing("fam-000001", "quiet")], &scope);
    }
}
