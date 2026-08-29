use crate::model::{Status, Task, TaskId};
use std::cmp::Ordering;

/// `lookup` returns Some(closed?) for a reachable dependency, None if unreachable.
pub fn is_ready(task: &Task, lookup: &dyn Fn(&TaskId) -> Option<bool>) -> bool {
    task.status == Status::Todo && task.depends.iter().all(|d| lookup(d) == Some(true))
}

pub fn sort_list(tasks: &mut [Task]) {
    tasks.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| b.updated.cmp(&a.updated))
            .then_with(|| a.id.cmp(&b.id))
    });
}

pub fn sort_ready(tasks: &mut [Task]) {
    tasks.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| match (a.size, b.size) {
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            })
            .then_with(|| a.created.cmp(&b.created))
            .then_with(|| a.id.cmp(&b.id))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn t(id: &str, status: Status, priority: u8, size: Option<Size>, deps: &[&str]) -> Task {
        Task {
            id: TaskId::parse(id).unwrap(),
            title: id.into(),
            status,
            priority,
            size,
            owner: None,
            created: format!("2026-08-29T00:00:0{}Z", priority),
            updated: "2026-08-29T00:00:00Z".into(),
            depends: deps.iter().map(|d| TaskId::parse(d).unwrap()).collect(),
            tags: vec![],
            spec: None,
            plan: None,
            step: None,
            body: String::new(),
            notes: vec![],
        }
    }

    #[test]
    fn ready_requires_todo_and_closed_deps() {
        let a = t(
            "xx-000001",
            Status::Todo,
            2,
            None,
            &["xx-000002", "yy-000003"],
        );
        let closed_all = |_: &TaskId| Some(true);
        let open_one = |id: &TaskId| Some(id.hex != "000002");
        let unreachable = |id: &TaskId| if id.prefix == "yy" { None } else { Some(true) };
        assert!(is_ready(&a, &closed_all));
        assert!(!is_ready(&a, &open_one));
        assert!(!is_ready(&a, &unreachable));
        let idea = t("xx-000009", Status::Idea, 0, None, &[]);
        assert!(!is_ready(&idea, &closed_all));
        let doing = t("xx-000008", Status::Doing, 0, None, &[]);
        assert!(!is_ready(&doing, &closed_all));
    }

    #[test]
    fn ready_sort_order() {
        let mut v = vec![
            t("xx-000001", Status::Todo, 2, None, &[]),
            t("xx-000002", Status::Todo, 1, Some(Size::L), &[]),
            t("xx-000003", Status::Todo, 1, Some(Size::Xs), &[]),
            t("xx-000004", Status::Todo, 1, None, &[]),
        ];
        sort_ready(&mut v);
        let ids: Vec<String> = v.iter().map(|t| t.id.hex.clone()).collect();
        assert_eq!(ids, ["000003", "000002", "000004", "000001"]);
    }
}
