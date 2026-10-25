//! `tasks docs`: the working documents under each project's records, with the tasks that
//! link them (records-home design §5.5).
//!
//! A project's working documents are the Markdown files directly under its spec and plan
//! directories and `docs/notes`, read from its records directory: the project root for an
//! in-tree project, the overlay for a linked one, `<records>/<prefix>` for a detached
//! one. Reference documents a detached project keeps in its code tree are not working
//! documents and are not listed. Listing never fetches, and never reads a task's links
//! beyond comparing the stored value with the document's path.

use super::ReadCtx;
use crate::error::{Error, Result};
use crate::filter::TextQuery;
use crate::model::Task;
use crate::output::{DocLink, DocRow, DocsOut, Output};
use crate::repo::Project;
use crate::scope::Scope;
use std::collections::BTreeMap;
use std::path::Path;

/// Where briefs, handoffs and other notes live; specs and plans follow the project's
/// configured directories.
const NOTES_DIR: &str = "docs/notes";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Spec,
    Plan,
    Brief,
    Handoff,
    Note,
}

impl Kind {
    pub const ALL: [Kind; 5] = [
        Kind::Spec,
        Kind::Plan,
        Kind::Brief,
        Kind::Handoff,
        Kind::Note,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Spec => "spec",
            Kind::Plan => "plan",
            Kind::Brief => "brief",
            Kind::Handoff => "handoff",
            Kind::Note => "note",
        }
    }

    pub fn parse(value: &str) -> Result<Kind> {
        Kind::ALL
            .into_iter()
            .find(|kind| kind.as_str() == value)
            .ok_or_else(|| {
                Error::Validation(format!(
                    "unknown document kind {value:?}; expected spec, plan, brief, handoff, or note"
                ))
            })
    }

    /// A file in the notes directory takes its kind from its name.
    fn of_note(file: &str) -> Kind {
        if file.ends_with("-brief.md") {
            Kind::Brief
        } else if file.ends_with("-handoff.md") {
            Kind::Handoff
        } else {
            Kind::Note
        }
    }
}

pub fn run(ctx: ReadCtx, kinds: &[String], text: Option<TextQuery>) -> Result<Output> {
    let kinds = kinds
        .iter()
        .map(|kind| Kind::parse(kind))
        .collect::<Result<Vec<_>>>()?;
    let mut warnings = ctx.warnings;
    let mut docs = Vec::new();
    for (project, mut tasks) in ctx.scope.scan_each()? {
        tasks.sort_by(|a, b| a.id.cmp(&b.id));
        for (path, kind) in documents(project, &mut warnings)? {
            if !kinds.is_empty() && !kinds.contains(&kind) {
                continue;
            }
            let file = project.records.join(&path);
            let stem = file
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(&path)
                .to_string();
            let bytes = std::fs::read(&file).map_err(io_at(&file))?;
            let (title, body) = match String::from_utf8(bytes) {
                Ok(body) => (title_of(&body).unwrap_or(stem), Some(body)),
                Err(_) => {
                    let fate = if text.is_some() {
                        "not listed, since --search cannot match it"
                    } else {
                        "listed under its file name"
                    };
                    warnings.push(format!(
                        "{}: {path} is not UTF-8 text; {fate}",
                        project.prefix
                    ));
                    (stem, None)
                }
            };
            if let Some(text) = &text
                && !body
                    .as_deref()
                    .is_some_and(|body| text.matches_text(&title, body))
            {
                continue;
            }
            docs.push(DocRow {
                project: project.prefix.clone(),
                kind: kind.as_str(),
                tasks: links(&tasks, &path),
                file: file.to_string_lossy().into_owned(),
                path,
                title,
            });
        }
    }
    Ok(Output::Docs(DocsOut {
        docs,
        warnings,
        wide: matches!(ctx.scope, Scope::All { .. }),
    }))
}

/// Every working document of `project`, path -> kind, in path order. Directories are read
/// flat, as bare spec and plan names are searched. A path under two directories (a spec
/// directory that is also a plan directory) takes the first kind: spec, plan, then notes.
fn documents(project: &Project, warnings: &mut Vec<String>) -> Result<BTreeMap<String, Kind>> {
    let mut found = BTreeMap::new();
    let dirs = project
        .spec_dirs
        .iter()
        .map(|dir| (dir.as_str(), Some(Kind::Spec)))
        .chain(
            project
                .plan_dirs
                .iter()
                .map(|dir| (dir.as_str(), Some(Kind::Plan))),
        )
        .chain([(NOTES_DIR, None)]);
    for (dir, kind) in dirs {
        let (files, dangling) = markdown_files(&project.records.join(dir))?;
        for file in files {
            let kind = kind.unwrap_or_else(|| Kind::of_note(&file));
            found.entry(format!("{dir}/{file}")).or_insert(kind);
        }
        for file in dangling {
            let line = format!(
                "{}: {dir}/{file} is a symlink to nothing; not listed",
                project.prefix
            );
            if !warnings.contains(&line) {
                warnings.push(line);
            }
        }
    }
    Ok(found)
}

/// The names of the regular `.md` files directly in `dir`, and of the `.md` symlinks there
/// whose target is missing; none when `dir` is not a directory. A name that is not UTF-8
/// cannot be a stored link and is skipped, as are directories and special files.
fn markdown_files(dir: &Path) -> Result<(Vec<String>, Vec<String>)> {
    let (mut files, mut dangling) = (Vec::new(), Vec::new());
    if !dir.is_dir() {
        return Ok((files, dangling));
    }
    for entry in std::fs::read_dir(dir).map_err(io_at(dir))? {
        let path = entry.map_err(io_at(dir))?.path();
        let Some(file) = path.file_name().and_then(|file| file.to_str()) else {
            continue;
        };
        if !file.ends_with(".md") {
            continue;
        }
        if path.is_file() {
            files.push(file.to_string());
        } else if path.is_symlink() && !path.exists() {
            dangling.push(file.to_string());
        }
    }
    Ok((files, dangling))
}

/// An io error that names the path it happened at, so a failure in one of many projects
/// says which file to fix.
fn io_at(path: &Path) -> impl Fn(std::io::Error) -> Error + '_ {
    move |error| Error::Io(format!("{}: {error}", path.display()))
}

/// The first level-one ATX heading's text.
fn title_of(body: &str) -> Option<String> {
    body.lines()
        .find_map(|line| line.strip_prefix("# "))
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty())
}

/// The tasks whose `spec`, `plan` or a `--ref` entry stores exactly `path`, in the order
/// of `tasks` (sorted by id), then spec, plan and refs in entry order.
fn links(tasks: &[Task], path: &str) -> Vec<DocLink> {
    let mut links = Vec::new();
    for task in tasks {
        let link = |via, handle| DocLink {
            id: task.id.to_string(),
            status: task.status.as_str().to_string(),
            via,
            handle,
        };
        if task.spec.as_deref() == Some(path) {
            links.push(link("spec", None));
        }
        if task.plan.as_deref() == Some(path) {
            links.push(link("plan", None));
        }
        for entry in task.refs.iter().filter(|entry| entry.reference == path) {
            links.push(link("ref", Some(entry.handle.clone())));
        }
    }
    links
}
