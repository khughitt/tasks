use super::Ctx;
use crate::error::{Error, Result};
use crate::model::{Task, TaskId};
use crate::output::{CheckOut, Finding, Output};
use crate::query::find_cycle;
use crate::resolve::{DocKind, Resolver};
use std::collections::BTreeSet;

fn finding(task: Option<&Task>, file: String, kind: &str, detail: String) -> Finding {
    Finding {
        id: task.map(|task| task.id.to_string()),
        file,
        kind: kind.into(),
        detail,
    }
}

pub fn run(ctx: Ctx) -> Result<Output> {
    let (tasks, parse_errors) = ctx.project.scan_lenient();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    for error in parse_errors {
        let file = match &error {
            Error::Parse { file, .. } => file.clone(),
            _ => "tasks/".into(),
        };
        errors.push(finding(None, file, "parse", error.to_string()));
    }

    let resolver = Resolver::new(&ctx.project, &ctx.registry);
    let local: BTreeSet<&TaskId> = tasks.iter().map(|task| &task.id).collect();
    let foreign = |id: &TaskId| -> std::result::Result<Option<Task>, String> {
        match resolver.resolve_task(id) {
            Ok(task) => Ok(task),
            Err(Error::Parse { file, detail }) => Err(format!("{file}: {detail}")),
            Err(error) => Err(error.to_string()),
        }
    };

    for task in &tasks {
        let file = format!("tasks/{}.md", task.id);
        for dependency in &task.depends {
            if dependency.prefix == ctx.project.prefix {
                if !local.contains(dependency) {
                    errors.push(finding(
                        Some(task),
                        file.clone(),
                        "dangling_dep",
                        format!("depends on missing {dependency}"),
                    ));
                }
            } else {
                match foreign(dependency) {
                    Ok(Some(_)) => {}
                    Ok(None) => warnings.push(finding(
                        Some(task),
                        file.clone(),
                        "unreachable_dep",
                        format!("depends on unreachable {dependency}"),
                    )),
                    Err(detail) => warnings.push(finding(
                        Some(task),
                        file.clone(),
                        "foreign_unparsable",
                        format!("dependency {dependency} cannot be read: {detail}"),
                    )),
                }
            }
        }
        for (kind, path) in [(DocKind::Spec, &task.spec), (DocKind::Plan, &task.plan)] {
            let Some(path) = path else { continue };
            if !ctx.project.root.join(path).is_file() {
                errors.push(finding(
                    Some(task),
                    file.clone(),
                    "doc_missing",
                    format!("{} {path} does not exist", kind.name()),
                ));
            }
        }
        if let (Some(plan), Some(step)) = (&task.plan, &task.step)
            && ctx.project.root.join(plan).is_file()
            && !resolver.step_exists(plan, step)?
        {
            errors.push(finding(
                Some(task),
                file.clone(),
                "step_missing",
                format!("heading {step:?} not found in {plan}"),
            ));
        }
    }

    let mut seen = BTreeSet::new();
    let edges = |id: &TaskId| -> Result<Option<Vec<TaskId>>> {
        if let Some(task) = tasks.iter().find(|task| &task.id == id) {
            return Ok(Some(task.depends.clone()));
        }
        Ok(foreign(id).unwrap_or(None).map(|task| task.depends))
    };
    for task in &tasks {
        match find_cycle(&task.id, &edges) {
            Ok(None) => {}
            Ok(Some(cycle)) => {
                let mut key: Vec<String> = cycle[..cycle.len() - 1]
                    .iter()
                    .map(ToString::to_string)
                    .collect();
                key.sort();
                if seen.insert(key) {
                    let lowest = cycle[..cycle.len() - 1].iter().min().unwrap();
                    let path: Vec<String> = cycle.iter().map(ToString::to_string).collect();
                    errors.push(Finding {
                        id: Some(lowest.to_string()),
                        file: format!("tasks/{lowest}.md"),
                        kind: "cycle".into(),
                        detail: path.join(" -> "),
                    });
                }
            }
            Err(Error::UnresolvableId(id)) => warnings.push(finding(
                Some(task),
                format!("tasks/{}.md", task.id),
                "cycle_unverifiable",
                format!("cannot verify acyclicity through unreachable {id}"),
            )),
            Err(error) => return Err(error),
        }
    }

    Ok(Output::Check(CheckOut { errors, warnings }))
}
