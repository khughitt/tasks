pub mod add;
pub mod init;
pub mod list;
pub mod show;

use crate::cli::{Cli, Command, FieldArgs};
use crate::error::{Error, Result};
use crate::format::{validate_body, validate_task};
use crate::model::{Size, Task, TaskId};
use crate::output::Output;
use crate::registry::Registry;
use crate::repo::Project;
use crate::resolve::{DocKind, Resolver};
use std::path::Path;

pub struct Ctx {
    pub project: Project,
    pub registry: Registry,
    pub warnings: Vec<String>,
}

pub fn open_ctx(dir: Option<&Path>) -> Result<Ctx> {
    let start = match dir {
        Some(dir) => dir.to_path_buf(),
        None => std::env::current_dir()?,
    };
    Ok(Ctx {
        project: Project::locate(&start)?,
        registry: Registry::load()?,
        warnings: Vec::new(),
    })
}

/// $TASKS_OWNER, else the current git branch, else $USER. Whatever is chosen must
/// satisfy `validate_owner`; an invalid value is an error, never sanitized.
pub fn owner_name(project: &Project) -> Result<String> {
    let name = raw_owner_name(project)?;
    crate::format::validate_owner(&name)?;
    Ok(name)
}

pub fn parse_id(s: &str) -> Result<TaskId> {
    TaskId::parse(s)
}

/// Applies the field flags present in `fields` to `task`, validating each against the repo.
pub fn apply_fields(ctx: &Ctx, task: &mut Task, fields: &FieldArgs) -> Result<()> {
    let resolver = Resolver::new(&ctx.project, &ctx.registry);
    if let Some(body) = &fields.body {
        validate_body(body)?;
        task.body = body.clone();
    }
    if let Some(priority) = fields.priority {
        if priority > 4 {
            return Err(Error::Validation("priority must be 0-4".into()));
        }
        task.priority = priority;
    }
    if let Some(size) = &fields.size {
        task.size = Some(Size::parse(size)?);
    }
    if !fields.tags.is_empty() {
        task.tags = fields.tags.clone();
    }
    if !fields.depends.is_empty() {
        let mut dependencies = Vec::new();
        for dependency in &fields.depends {
            let id = TaskId::parse(dependency)?;
            if resolver.resolve_task(&id)?.is_none() {
                return Err(Error::UnresolvableId(id.to_string()));
            }
            if !dependencies.contains(&id) {
                dependencies.push(id);
            }
        }
        task.depends = dependencies;
    }
    if let Some(spec) = &fields.spec {
        task.spec = Some(resolver.resolve_doc(DocKind::Spec, spec)?);
    }
    if let Some(plan) = &fields.plan {
        task.plan = Some(resolver.resolve_doc(DocKind::Plan, plan)?);
    }
    if let Some(step) = &fields.step {
        task.step = Some(step.clone());
    }
    match (&task.plan, &task.step) {
        (None, Some(_)) => return Err(Error::Validation("--step requires a plan".into())),
        (Some(plan), Some(step)) if !resolver.step_exists(plan, step)? => {
            return Err(Error::Validation(format!(
                "heading {step:?} not found in {plan}"
            )));
        }
        _ => {}
    }
    validate_task(task)
}

pub fn save(ctx: &Ctx, task: &mut Task) -> Result<()> {
    task.updated = crate::time::now();
    validate_task(task)?;
    ctx.project.write_task(task)
}

pub fn load(ctx: &Ctx, id: &str) -> Result<Task> {
    ctx.project.read_task(&TaskId::parse(id)?)
}

pub fn id_out(ctx: Ctx, task: &Task) -> Output {
    Output::Id(crate::output::IdOut {
        id: task.id.to_string(),
        warnings: ctx.warnings,
    })
}

fn raw_owner_name(project: &Project) -> Result<String> {
    if let Ok(owner) = std::env::var("TASKS_OWNER") {
        if !owner.is_empty() {
            return Ok(owner);
        }
    }
    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&project.root)
        .output()
    {
        if output.status.success() {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !branch.is_empty() && branch != "HEAD" {
                return Ok(branch);
            }
        }
    }
    match std::env::var("USER") {
        Ok(user) if !user.is_empty() => Ok(user),
        _ => Err(Error::Config(
            "cannot determine an owner: set TASKS_OWNER (no git branch, USER unset)".into(),
        )),
    }
}

pub fn run(cli: Cli) -> Result<Output> {
    let dir = cli.dir.as_deref();
    match cli.command {
        Command::Init { prefix } => init::run(dir, prefix),
        Command::Add {
            title,
            status,
            fields,
        } => add::run(open_ctx(dir)?, title, status, fields),
        Command::Show { id } => show::run(open_ctx(dir)?, id),
        Command::List {
            statuses,
            tags,
            owner,
            all_projects,
        } => list::list(open_ctx(dir)?, statuses, tags, owner, all_projects),
        Command::Ready { size, limit } => list::ready(open_ctx(dir)?, size, limit),
        Command::Prime => list::prime(open_ctx(dir)?),
        _ => {
            let _ctx = open_ctx(dir)?;
            Err(Error::Validation("not implemented".into()))
        }
    }
}
