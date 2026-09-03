use crate::model::{Task, TaskId};

pub const SIMILAR_THRESHOLD: f64 = 0.6;

/// Lowercase ASCII-alphanumeric tokens of three or more characters, in order.
pub fn tokens(title: &str) -> Vec<String> {
    title
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| word.len() >= 3)
        .map(str::to_string)
        .collect()
}

pub fn jaccard(a: &[String], b: &[String]) -> f64 {
    let a: std::collections::BTreeSet<&String> = a.iter().collect();
    let b: std::collections::BTreeSet<&String> = b.iter().collect();
    let union = a.union(&b).count();
    if union == 0 {
        return 0.0;
    }
    a.intersection(&b).count() as f64 / union as f64
}

#[derive(Debug, PartialEq)]
pub enum Match {
    Exact(TaskId),
    /// Candidates requiring a reporter choice, best first, ties to the older task.
    Ambiguous(Vec<(TaskId, String)>),
    None,
}

/// `candidates` are already filtered to open tasks tagged `feedback`. `summary` must have
/// at least one token; the caller rejects empty ones, since `[] == []` would otherwise
/// make every wordless summary an exact match for every other.
pub fn find(summary: &str, candidates: &[Task]) -> Match {
    let wanted = tokens(summary);
    assert!(
        !wanted.is_empty(),
        "caller validates that the summary has tokens"
    );
    let mut exact: Vec<&Task> = Vec::new();
    let mut ambiguous: Vec<(f64, &Task)> = Vec::new();
    for task in candidates {
        let have = tokens(&task.title);
        if have == wanted {
            exact.push(task);
        }
        let score = jaccard(&wanted, &have);
        if score >= SIMILAR_THRESHOLD {
            ambiguous.push((score, task));
        }
    }
    // Exactly one exact match recurs. Two or more, which `--new` makes possible, are a
    // choice the reporter has to make: they fall through to the ambiguous list, where
    // their score of 1.0 sorts them first.
    if exact.len() == 1 {
        return Match::Exact(exact[0].id.clone());
    }
    if ambiguous.is_empty() {
        return Match::None;
    }
    ambiguous.sort_by(|(x, a), (y, b)| {
        y.partial_cmp(x)
            .unwrap()
            .then_with(|| a.created.cmp(&b.created))
            .then_with(|| a.id.cmp(&b.id))
    });
    Match::Ambiguous(
        ambiguous
            .into_iter()
            .map(|(_, task)| (task.id.clone(), task.title.clone()))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_drop_case_punctuation_and_short_words() {
        assert_eq!(
            tokens("Check rejects a MISSING spec!"),
            ["check", "rejects", "missing", "spec"]
        );
        assert_eq!(tokens("a b cd"), Vec::<String>::new());
    }

    fn feedback_task(id: &str, title: &str) -> Task {
        Task {
            id: TaskId::parse(id).unwrap(),
            title: title.into(),
            status: crate::model::Status::Idea,
            priority: 2,
            size: None,
            owner: None,
            created: "2026-09-03T00:00:00Z".into(),
            updated: "2026-09-03T00:00:00Z".into(),
            depends: vec![],
            parent: None,
            tags: vec!["feedback".into()],
            spec: None,
            plan: None,
            step: None,
            body: String::new(),
            notes: vec![],
        }
    }

    #[test]
    fn exact_versus_similar_versus_none() {
        let spec = feedback_task("tasks-000001", "check rejects missing spec");
        assert_eq!(
            find("Check rejects MISSING spec!", std::slice::from_ref(&spec)),
            Match::Exact(spec.id.clone())
        );
        match find("check rejects missing plan", std::slice::from_ref(&spec)) {
            Match::Ambiguous(candidates) => assert_eq!(candidates[0].0, spec.id),
            other => panic!("{other:?}"),
        }
        let twin = feedback_task("tasks-000002", "Check rejects missing spec");
        match find("check rejects missing spec", &[spec.clone(), twin]) {
            Match::Ambiguous(candidates) => assert_eq!(candidates.len(), 2),
            other => panic!("two exact matches must not recur on their own: {other:?}"),
        }
        assert_eq!(
            find("prime output is delightful", std::slice::from_ref(&spec)),
            Match::None
        );
        let a: Vec<String> = tokens("check rejects missing spec");
        let b: Vec<String> = tokens("check rejects missing plan");
        assert!(
            (jaccard(&a, &b) - 3.0 / 5.0).abs() < 1e-9,
            "three shared tokens of five: exactly the inclusive threshold"
        );
        let c: Vec<String> = tokens("check rejects a missing plan file");
        assert!(jaccard(&a, &c) < SIMILAR_THRESHOLD, "three of six is below");
    }
}
