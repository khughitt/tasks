use crate::error::{Error, Result};
use crate::model::{Size, Status, Task, TaskId};
use std::cmp::Ordering;
use std::collections::HashSet;

/// Depth-first search for a cycle reachable from `start`.
pub fn find_cycle(
    start: &TaskId,
    edges: &dyn Fn(&TaskId) -> Result<Option<Vec<TaskId>>>,
) -> Result<Option<Vec<TaskId>>> {
    fn visit(
        node: &TaskId,
        edges: &dyn Fn(&TaskId) -> Result<Option<Vec<TaskId>>>,
        path: &mut Vec<TaskId>,
        done: &mut HashSet<TaskId>,
    ) -> Result<Option<Vec<TaskId>>> {
        if let Some(position) = path.iter().position(|item| item == node) {
            let mut cycle = path[position..].to_vec();
            cycle.push(node.clone());
            return Ok(Some(cycle));
        }
        if done.contains(node) {
            return Ok(None);
        }
        let dependencies = edges(node)?.ok_or_else(|| Error::UnresolvableId(node.to_string()))?;
        path.push(node.clone());
        for dependency in &dependencies {
            if let Some(cycle) = visit(dependency, edges, path, done)? {
                return Ok(Some(cycle));
            }
        }
        path.pop();
        done.insert(node.clone());
        Ok(None)
    }

    visit(start, edges, &mut Vec::new(), &mut HashSet::new())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphFormat {
    Mermaid,
    Dot,
}

impl GraphFormat {
    pub fn parse(value: &str) -> Result<GraphFormat> {
        match value {
            "mermaid" => Ok(GraphFormat::Mermaid),
            "dot" => Ok(GraphFormat::Dot),
            other => Err(Error::Validation(format!(
                "unknown graph format {other:?}; use mermaid or dot"
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            GraphFormat::Mermaid => "mermaid",
            GraphFormat::Dot => "dot",
        }
    }
}

fn node_label(task: &Task) -> String {
    let size = task.size.map(Size::as_str).unwrap_or("-");
    format!(
        "{} P{} {} {}: {}",
        task.id,
        task.priority,
        size,
        task.status.as_str(),
        task.title
    )
}

fn mermaid_escape(value: &str) -> String {
    value.replace('#', "#35;").replace('"', "#quot;")
}

fn dot_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Renders dependencies in work-flow direction: dependency to dependent.
pub fn render_graph(tasks: &[Task], format: GraphFormat) -> String {
    let shown: HashSet<&TaskId> = tasks.iter().map(|task| &task.id).collect();
    let mut rendered = String::new();
    match format {
        GraphFormat::Mermaid => {
            rendered.push_str("graph TD\n");
            for task in tasks {
                rendered.push_str(&format!(
                    "  {}[\"{}\"]\n",
                    task.id.to_string().replace('-', "_"),
                    mermaid_escape(&node_label(task))
                ));
            }
            for task in tasks {
                for dependency in task.depends.iter().filter(|id| shown.contains(id)) {
                    rendered.push_str(&format!(
                        "  {} --> {}\n",
                        dependency.to_string().replace('-', "_"),
                        task.id.to_string().replace('-', "_")
                    ));
                }
            }
        }
        GraphFormat::Dot => {
            rendered.push_str("digraph tasks {\n  rankdir=TB;\n");
            for task in tasks {
                rendered.push_str(&format!(
                    "  \"{}\" [label=\"{}\"];\n",
                    task.id,
                    dot_escape(&node_label(task))
                ));
            }
            for task in tasks {
                for dependency in task.depends.iter().filter(|id| shown.contains(id)) {
                    rendered.push_str(&format!("  \"{}\" -> \"{}\";\n", dependency, task.id));
                }
            }
            rendered.push_str("}\n");
        }
    }
    rendered
}

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

    fn edges_from<'a>(
        map: &'a [(&'a str, &'a [&'a str])],
    ) -> impl Fn(&TaskId) -> crate::error::Result<Option<Vec<TaskId>>> + 'a {
        move |id: &TaskId| {
            Ok(map
                .iter()
                .find(|(key, _)| *key == id.to_string())
                .map(|(_, deps)| deps.iter().map(|dep| TaskId::parse(dep).unwrap()).collect()))
        }
    }

    #[test]
    fn finds_cycles_and_reports_unreachable() {
        let id = |s: &str| TaskId::parse(s).unwrap();
        let acyclic = [("xx-000001", &["xx-000002"][..]), ("xx-000002", &[][..])];
        assert_eq!(
            find_cycle(&id("xx-000001"), &edges_from(&acyclic)).unwrap(),
            None
        );
        let cyclic = [
            ("xx-000001", &["yy-000002"][..]),
            ("yy-000002", &["xx-000001"][..]),
        ];
        let cycle = find_cycle(&id("xx-000001"), &edges_from(&cyclic))
            .unwrap()
            .unwrap();
        assert_eq!(
            cycle
                .iter()
                .map(|item| item.to_string())
                .collect::<Vec<_>>(),
            ["xx-000001", "yy-000002", "xx-000001"]
        );
        let dangling = [("xx-000001", &["zz-000009"][..])];
        assert!(matches!(
            find_cycle(&id("xx-000001"), &edges_from(&dangling)),
            Err(crate::error::Error::UnresolvableId(_))
        ));
    }

    #[test]
    fn renders_mermaid_and_dot() {
        let a = t("xx-000001", Status::Todo, 1, Some(Size::S), &["xx-000002"]);
        let b = t("xx-000002", Status::Doing, 2, None, &[]);
        let mermaid = render_graph(&[a.clone(), b.clone()], GraphFormat::Mermaid);
        assert!(mermaid.starts_with("graph TD\n"));
        assert!(mermaid.contains("xx_000001[\"xx-000001 P1 s todo: xx-000001\"]"));
        assert!(mermaid.contains("xx_000002 --> xx_000001\n"), "{mermaid}");
        let dot = render_graph(&[a, b], GraphFormat::Dot);
        assert!(dot.starts_with("digraph tasks {\n"));
        assert!(dot.contains("\"xx-000002\" -> \"xx-000001\";"), "{dot}");
    }

    #[test]
    fn escapes_hostile_titles() {
        let mut a = t("xx-000001", Status::Todo, 2, None, &[]);
        a.title = "say \"hi\" #1 \\ ] done".into();
        let mermaid = render_graph(std::slice::from_ref(&a), GraphFormat::Mermaid);
        assert!(
            mermaid.contains("[\"xx-000001 P2 - todo: say #quot;hi#quot; #35;1 \\ ] done\"]"),
            "{mermaid}"
        );
        let dot = render_graph(std::slice::from_ref(&a), GraphFormat::Dot);
        assert!(
            dot.contains("[label=\"xx-000001 P2 - todo: say \\\"hi\\\" #1 \\\\ ] done\"]"),
            "{dot}"
        );
    }
}
