//! Missing-record inspection hints; discovered files never become automatic read authority.
use crate::error::Error;
use crate::model::TaskId;
use crate::recovery::{Scan, SearchUnavailable};

pub(super) fn inspection_error(id: &TaskId, scan: &Scan) -> Error {
    let mut parts = Vec::new();
    match scan.unavailable {
        Some(SearchUnavailable::Detached) => parts
            .push("detached projects use one shared records home; no worktree search".to_string()),
        Some(SearchUnavailable::NoGit) => parts.push(
            "worktree search unavailable: records outside Git or Git unavailable".to_string(),
        ),
        None if !scan.complete() => parts.push("worktree search incomplete".into()),
        None if scan.matches.is_empty() => {
            parts.push("no matching record file found in inspected eligible worktrees".into())
        }
        None if scan.matches.len() > 1 => parts.push("record-file location is ambiguous".into()),
        None => {}
    }
    for gap in scan.gaps.iter().take(8) {
        let root = gap
            .root
            .as_ref()
            .map(|p| format!("{}: ", p.display()))
            .unwrap_or_default();
        parts.push(format!("{root}{}", gap.detail));
    }
    if scan.gaps.len() > 8 {
        parts.push(format!(
            "{} further unresolved locations omitted",
            scan.gaps.len() - 8
        ));
    }
    for found in scan.matches.iter().take(8) {
        parts.push(format!("record file observed in {}", found.root.display()));
        parts.push(crate::stale::rerun_line(
            &found.root,
            &["show".into(), id.to_string()],
        ));
    }
    if scan.matches.len() > 8 {
        parts.push(format!(
            "{} further matches omitted",
            scan.matches.len() - 8
        ));
    }
    if !scan.matches.is_empty() {
        parts.push("location hints only; explicit retry revalidates the record".into());
    }
    Error::TaskNotFoundNext {
        id: id.to_string(),
        next: parts.join("; "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recovery::{Candidate, Gap};
    #[test]
    fn display_is_capped_without_hiding_ambiguity_or_omitted_locations() {
        let id = TaskId::parse("sci-000001").unwrap();
        let scan = Scan {
            matches: (0..10)
                .map(|n| Candidate {
                    root: format!("/candidate-{n}").into(),
                    kind: crate::repo::Kind::InTree,
                    checkout: format!("/candidate-{n}").into(),
                })
                .collect(),
            gaps: (0..11)
                .map(|n| Gap {
                    root: Some(format!("/gap-{n}").into()),
                    detail: "unreadable".into(),
                })
                .collect(),
            ..Scan::default()
        };
        let text = inspection_error(&id, &scan).to_string();
        assert!(text.contains("incomplete"));
        assert_eq!(text.matches("show sci-000001").count(), 8);
        assert!(text.contains("2 further matches omitted"));
        assert!(text.contains("3 further unresolved locations omitted"));
        assert!(!text.contains("/candidate-8") && !text.contains("/gap-8"));
    }
}
