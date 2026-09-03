use super::Ctx;
use crate::error::{Error, Result};
use crate::format::{validate_body, validate_line};
use crate::model::{Status, Task};
use crate::output::{FeedbackOut, Output};
use crate::registry::Registry;
use crate::repo::{CONFIG_REL, Project};

pub const TARGET_PREFIX: &str = "tasks";
pub const CATEGORIES: [&str; 4] = ["friction", "gap", "idea", "positive"];

/// The upstream project: the registry entry whose prefix is `tasks`.
pub fn locate_target(registry: &Registry) -> Result<Project> {
    let Some(root) = registry.project_root(TARGET_PREFIX) else {
        return Err(Error::Config(format!(
            "no project registered as {TARGET_PREFIX:?}; clone the upstream tasks repository and run `tasks init` there"
        )));
    };
    if !root.join(CONFIG_REL).is_file() {
        return Err(Error::Config(format!(
            "project {TARGET_PREFIX:?} at {} has no {CONFIG_REL}; run `tasks init` there",
            root.display()
        )));
    }
    let project = Project::open(root)?;
    if project.prefix != TARGET_PREFIX {
        return Err(Error::Config(format!(
            "registry maps {TARGET_PREFIX:?} to {}, whose prefix is {:?}; fix the registry",
            root.display(),
            project.prefix
        )));
    }
    Ok(project)
}

pub fn run(
    mut ctx: Ctx,
    summary: String,
    category: String,
    body: Option<String>,
) -> Result<Output> {
    if !CATEGORIES.contains(&category.as_str()) {
        return Err(Error::Validation(format!(
            "category must be one of {}",
            CATEGORIES.join(", ")
        )));
    }
    validate_line("summary", &summary)?;
    let body = body.unwrap_or_default();
    validate_body(&body)?;
    let target = locate_target(&ctx.registry)?;
    let from = format!("from:{}", ctx.project.prefix);

    let task = create(&target, summary, body, &category, &from)?;
    let path = target.task_path(&task.id);
    ctx.warnings.push(format!(
        "filed as uncommitted {} in {}; a maintainer there reviews and commits it",
        path.display(),
        target.root.display()
    ));
    Ok(Output::Feedback(FeedbackOut {
        id: task.id.to_string(),
        action: "created".into(),
        path: path.display().to_string(),
        warnings: ctx.warnings,
    }))
}

fn create(
    target: &Project,
    summary: String,
    body: String,
    category: &str,
    from: &str,
) -> Result<Task> {
    let now = crate::time::now();
    let mut task = Task {
        id: target.new_id()?,
        title: summary,
        status: Status::Idea,
        priority: 2,
        size: None,
        owner: None,
        created: now.clone(),
        updated: now,
        depends: vec![],
        parent: None,
        tags: vec!["feedback".into(), category.into(), from.into()],
        spec: None,
        plan: None,
        step: None,
        body,
        notes: vec![],
    };
    super::create(target, &mut task)?;
    Ok(task)
}
