use crate::error::{Error, Result};
use crate::output::{Counts, Output, ProjectRow, ProjectsOut};
use crate::registry::Registry;
use crate::scope::{Origin, is_reachable, open_registered, registry_warnings};
use std::path::Path;

/// The registry as rows. Missing roots/configs are unreachable rows; malformed reachable
/// configs still error because a listed project must agree with its registered prefix.
/// How the rows are ordered. Deliberately not `query::SortKey`: that enum ranks tasks by
/// priority and their own timestamps, and answers a date-column question projects never ask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectSort {
    Prefix,
    Size,
    Activity,
}

impl ProjectSort {
    pub fn parse(value: &str) -> Result<ProjectSort> {
        match value {
            "prefix" => Ok(ProjectSort::Prefix),
            "size" => Ok(ProjectSort::Size),
            "activity" => Ok(ProjectSort::Activity),
            other => Err(Error::Validation(format!(
                "unknown sort key {other:?}; expected prefix, size, or activity"
            ))),
        }
    }
}

/// Rows that lack the chosen key sink to the bottom in prefix order, whatever the
/// direction: an unreachable project has no size to be the smallest of, and `--reverse`
/// must not promote absent data to the top. Under `prefix` no row lacks the key, so the
/// registry listing keeps its alphabetical order, broken entries included.
fn sort_rows(rows: &mut Vec<ProjectRow>, key: ProjectSort, reverse: bool) {
    let ranked = |row: &ProjectRow| match key {
        ProjectSort::Prefix => true,
        ProjectSort::Size => row.total.is_some(),
        ProjectSort::Activity => row.last_activity.is_some(),
    };
    let (mut ranked, mut absent): (Vec<ProjectRow>, Vec<ProjectRow>) =
        rows.drain(..).partition(ranked);
    ranked.sort_by(|a, b| {
        let ordering = match key {
            ProjectSort::Prefix => a.prefix.cmp(&b.prefix),
            ProjectSort::Size => b.total.cmp(&a.total).then_with(|| a.prefix.cmp(&b.prefix)),
            ProjectSort::Activity => b
                .last_activity
                .cmp(&a.last_activity)
                .then_with(|| a.prefix.cmp(&b.prefix)),
        };
        if reverse {
            ordering.reverse()
        } else {
            ordering
        }
    });
    absent.sort_by(|a, b| a.prefix.cmp(&b.prefix));
    rows.extend(ranked);
    rows.extend(absent);
}

pub fn run(
    dir: Option<&Path>,
    sort: Option<&str>,
    reverse: bool,
    closed: bool,
    paths: bool,
) -> Result<Output> {
    let key = sort
        .map(ProjectSort::parse)
        .transpose()?
        .unwrap_or(ProjectSort::Prefix);
    let registry = Registry::load()?;
    let warnings = registry_warnings(&registry, &super::start_dir(dir)?)?;
    let mut rows = Vec::new();
    for (prefix, root) in &registry.projects {
        let reachable = is_reachable(root)?;
        let (counts, total, last_activity) = if reachable {
            let project = open_registered(&registry, prefix, Origin::Prefix)?;
            let tasks = project.scan()?;
            // Timestamps are fixed-width UTC with no fractional part, so the lexical max
            // is the chronological one.
            let last = tasks.iter().map(|task| task.updated.clone()).max();
            (Some(Counts::of(&tasks)), Some(tasks.len()), last)
        } else {
            (None, None, None)
        };
        rows.push(ProjectRow {
            prefix: prefix.clone(),
            root: root.display().to_string(),
            reachable,
            counts,
            total,
            last_activity,
        });
    }
    sort_rows(&mut rows, key, reverse);
    Ok(Output::Projects(ProjectsOut {
        projects: rows,
        warnings,
        closed,
        paths,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::Counts;

    fn row(prefix: &str, total: Option<usize>, activity: Option<&str>) -> ProjectRow {
        ProjectRow {
            prefix: prefix.into(),
            root: format!("/tmp/{prefix}"),
            reachable: total.is_some(),
            counts: total.map(|_| Counts::default()),
            total,
            last_activity: activity.map(str::to_string),
        }
    }

    fn order(rows: &[ProjectRow]) -> Vec<&str> {
        rows.iter().map(|row| row.prefix.as_str()).collect()
    }

    #[test]
    fn size_and_activity_lead_with_the_largest_and_the_newest() {
        let mut rows = vec![
            row("a", Some(1), Some("2026-09-01T00:00:00Z")),
            row("b", Some(9), Some("2026-08-01T00:00:00Z")),
            row("c", Some(5), Some("2026-09-06T00:00:00Z")),
        ];
        sort_rows(&mut rows, ProjectSort::Size, false);
        assert_eq!(order(&rows), ["b", "c", "a"]);
        sort_rows(&mut rows, ProjectSort::Activity, false);
        assert_eq!(order(&rows), ["c", "a", "b"]);
        sort_rows(&mut rows, ProjectSort::Prefix, false);
        assert_eq!(order(&rows), ["a", "b", "c"]);
    }

    #[test]
    fn ties_break_on_prefix_so_a_run_is_reproducible() {
        let mut rows = vec![
            row("z", Some(4), Some("2026-09-01T00:00:00Z")),
            row("a", Some(4), Some("2026-09-01T00:00:00Z")),
        ];
        sort_rows(&mut rows, ProjectSort::Size, false);
        assert_eq!(order(&rows), ["a", "z"]);
        // reversing reverses the key, and the tie-break with it
        sort_rows(&mut rows, ProjectSort::Size, true);
        assert_eq!(order(&rows), ["z", "a"]);
    }

    #[test]
    fn a_row_without_the_key_sinks_under_every_direction() {
        let mut rows = vec![
            row("gone", None, None),
            row("empty", Some(0), None),
            row("live", Some(3), Some("2026-09-01T00:00:00Z")),
        ];
        for reverse in [false, true] {
            for key in [ProjectSort::Size, ProjectSort::Activity] {
                sort_rows(&mut rows, key, reverse);
                assert_eq!(
                    rows.last().unwrap().prefix,
                    "gone",
                    "unreachable row moved under {key:?} reverse={reverse}"
                );
            }
        }
        // no activity is no key either, so an empty project sinks beside the broken one -
        // in prefix order, because neither can be ranked by recency
        sort_rows(&mut rows, ProjectSort::Activity, false);
        assert_eq!(order(&rows), ["live", "empty", "gone"]);
    }

    #[test]
    fn prefix_order_ranks_every_row_including_the_unreachable_one() {
        let mut rows = vec![
            row("live", Some(3), Some("2026-09-01T00:00:00Z")),
            row("gone", None, None),
            row("empty", Some(0), None),
        ];
        // a broken entry still has a name, so the registry listing stays alphabetical
        sort_rows(&mut rows, ProjectSort::Prefix, false);
        assert_eq!(order(&rows), ["empty", "gone", "live"]);
    }

    #[test]
    fn parse_names_the_keys_it_accepts() {
        assert!(ProjectSort::parse("size").is_ok());
        let error = ProjectSort::parse("updated").unwrap_err();
        assert_eq!(error.kind(), "validation");
        assert!(
            error.to_string().contains("prefix, size, or activity"),
            "{error}"
        );
    }
}
