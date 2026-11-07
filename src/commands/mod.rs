pub mod add;
pub mod archive_notes;
pub mod attach;
pub mod attachments;
pub mod blockers;
pub mod check;
pub mod claims;
pub mod commit;
mod context;
pub mod dep;
pub mod doc;
pub mod docs;
pub mod edit;
pub mod export;
pub mod feedback;
pub mod goals;
pub mod graph;
pub mod group;
pub mod init;
pub mod lanes;
pub mod list;
mod migrate;
pub(crate) mod move_;
mod mutate;
mod occupants;
mod ownership;
pub mod park;
pub mod parked;
pub mod projects;
pub mod quiet;
pub mod release;
pub mod rename;
mod resolver;
pub mod retained;
pub mod retire;
pub mod root;
pub mod sample;
pub mod show;
pub mod status;
pub mod tags;
pub mod tree;
pub mod unregister;
pub mod where_;

pub use context::*;
pub use mutate::*;
pub use occupants::*;
pub use ownership::*;

use crate::cli::{Cli, Command, GroupAction, ScopeArgs};
use crate::config::HostConfig;
use crate::error::{Error, Result};
use crate::model::Status;
use crate::output::Output;
use std::path::Path;
use std::sync::Arc;

/// The `--no-fetch` switch of the read commands that may consult a document provider.
fn fetch(no_fetch: bool) -> crate::resolve::Fetch {
    if no_fetch {
        crate::resolve::Fetch::Disabled
    } else {
        crate::resolve::Fetch::Allowed
    }
}

fn dispatch(cli: Cli, host: Option<Arc<HostConfig>>) -> Result<Output> {
    if let Command::Attachments { paths } = cli.command {
        return attachments::run(cli.dir.as_deref(), paths);
    }
    let host =
        host.ok_or_else(|| Error::Config("ordinary invocation has no host configuration".into()))?;
    let dir = cli.dir.as_deref();
    match cli.command {
        Command::Retained { action } => retained::dispatch(dir, action, host.clone()),
        Command::Attachments { paths } => attachments::run(dir, paths),
        Command::Init {
            prefix,
            checkout,
            records,
            force,
        } => init::run(dir, prefix, checkout, records, force),
        Command::Where { path } => where_::run(path, dir),
        Command::Rename {
            old,
            new,
            explain,
            adopt,
        } => rename::run(dir, old, new, explain, adopt),
        Command::Move { id, to, explain } => move_::run(dir, &id, &to, explain, host.clone()),
        Command::Migrate {
            list,
            to,
            draft,
            explain,
        } => migrate::run(dir, list, to, draft, explain, host.clone()),
        Command::Unregister { prefix } => unregister::run(prefix),
        Command::Retire { successor, reason } => {
            let project = open_ctx(dir, host.clone())?.project;
            let prefix = project.prefix.clone();
            let retired = retire::retire(project, successor, reason)?;
            Ok(Output::Retire(crate::output::RetireOut {
                prefix,
                retired: Some(retired),
                warnings: Vec::new(),
            }))
        }
        Command::Unretire => {
            let project = open_ctx(dir, host.clone())?.project;
            let prefix = project.prefix.clone();
            retire::unretire(project)?;
            Ok(Output::Retire(crate::output::RetireOut {
                prefix,
                retired: None,
                warnings: Vec::new(),
            }))
        }
        Command::Projects {
            sort,
            reverse,
            closed,
            paths,
        } => projects::run(dir, &sort, reverse, closed, paths),
        Command::Group { action } => match action {
            GroupAction::Set { name, prefixes } => group::set(name, prefixes),
            GroupAction::Rm { name } => group::rm(name),
        },
        Command::Groups => group::list(dir),
        Command::Root { id } => root::run(id, dir),
        Command::Resolve { inputs, stdin } => resolver::run(inputs, stdin, dir),
        Command::Add {
            title,
            status,
            project,
            origin_session,
            fields,
        } => {
            // An explicit target replaces the lookup, and `add` validates
            // every field against whichever project it is handed.
            let ctx = open_project_write_ctx(dir, project, host.clone())?;
            add::run(ctx, title, status, origin_session, fields)
        }
        Command::Show { id, no_fetch } => {
            show::run(open_id_ctx(dir, &id, host.clone())?, id, fetch(no_fetch))
        }
        Command::Doc {
            id,
            plan,
            spec: _,
            handle,
            no_fetch,
        } => doc::run(
            open_id_ctx(dir, &id, host.clone())?,
            id,
            match handle {
                Some(handle) => doc::DocTarget::Ref(handle),
                None if plan => doc::DocTarget::Plan,
                None => doc::DocTarget::Spec,
            },
            fetch(no_fetch),
        ),
        Command::Export {
            id,
            name,
            at,
            output,
            verify: _,
        } => export::run(
            open_id_ctx(dir, &id, host.clone())?,
            id,
            name,
            at,
            match output {
                Some(path) if path == Path::new("-") => export::ExportTarget::Stdout,
                Some(path) => export::ExportTarget::Path(path),
                None => export::ExportTarget::Verify,
            },
        ),
        Command::Docs {
            kinds,
            search,
            scope,
        } => {
            // Validated before the scan, so an empty phrase never reads a project.
            let text = search
                .as_deref()
                .map(crate::filter::TextQuery::parse)
                .transpose()?;
            docs::run(open_read_ctx(dir, &scope, host.clone())?, &kinds, text)
        }
        Command::List { list: args } => list::list(
            open_filtered_read_ctx(
                dir,
                &args.scope,
                args.filter.parent.as_deref(),
                args.filter.under.as_deref(),
                host.clone(),
            )?,
            args,
            None,
        ),
        Command::Search { query, list: args } => {
            // Validated before the scan, so an empty query never reads a project.
            let text = crate::filter::TextQuery::parse(&query)?;
            list::list(
                open_filtered_read_ctx(
                    dir,
                    &args.scope,
                    args.filter.parent.as_deref(),
                    args.filter.under.as_deref(),
                    host.clone(),
                )?,
                args,
                Some(text),
            )
        }
        Command::Ready {
            filter,
            limit,
            max_complexity,
            without,
            scope,
        } => list::ready(
            open_filtered_read_ctx(
                dir,
                &scope,
                filter.parent.as_deref(),
                filter.under.as_deref(),
                host.clone(),
            )?,
            filter,
            limit,
            max_complexity,
            without.without,
        ),
        Command::Next {
            max_complexity,
            without,
            under,
            no_fetch,
            scope,
        } => list::next(
            open_filtered_read_ctx(dir, &scope, None, under.as_deref(), host.clone())?,
            max_complexity,
            without.without,
            under,
            fetch(no_fetch),
        ),
        Command::Sample {
            limit,
            older_than,
            seed,
            scope,
        } => sample::sample(
            open_read_ctx(dir, &scope, host.clone())?,
            limit,
            older_than,
            seed,
        ),
        Command::Edit { id, args } => {
            edit::run(open_id_write_ctx(dir, &id, host.clone())?, id, args)
        }
        Command::Prime {
            scope,
            closed,
            summary,
            without,
        } => {
            let full = if summary {
                Some(crate::prime_summary::full_argv(
                    &std::env::current_exe()?,
                    &std::path::absolute(start_dir(dir)?)?,
                    &scope,
                    closed,
                    &without.without,
                )?)
            } else {
                None
            };
            list::prime(
                open_read_ctx(dir, &scope, host.clone())?,
                closed,
                without.without,
                full,
            )
        }
        Command::Lanes {
            without,
            max_complexity,
            scope,
        } => lanes::run(
            open_read_ctx(dir, &scope, host.clone())?,
            without,
            max_complexity,
        ),
        Command::Note {
            id,
            text,
            stdin,
            stamp,
            reviewed,
            no_reviewed,
            awaiting_user,
        } => {
            let texts = if stdin {
                status::read_notes(std::io::stdin().lock())?
            } else {
                vec![text.expect("clap requires text unless --stdin")]
            };
            status::note(
                open_id_write_ctx(dir, &id, host.clone())?,
                id,
                texts,
                stamp,
                crate::review::Review::from_flags(reviewed, no_reviewed),
                crate::review::Awaiting::from_flags(awaiting_user, false),
            )
        }
        Command::Attach {
            id,
            source,
            clipboard,
            multipart,
            name,
            caption,
        } => attach::attach(
            open_id_write_ctx(dir, &id, host.clone())?,
            id,
            attach::Source::from_args(source, clipboard)?,
            multipart,
            name,
            caption,
        ),
        Command::ArchiveNotes { id, expect } => {
            let ctx = if expect.is_some() {
                open_id_write_ctx(dir, &id, host.clone())?
            } else {
                open_id_ctx(dir, &id, host.clone())?
            };
            archive_notes::run(ctx, id, expect)
        }
        Command::Detach { id, name, why } => {
            attach::detach(open_id_write_ctx(dir, &id, host.clone())?, id, name, why)
        }
        Command::Start { id, force, reason } => status::start(
            open_id_write_ctx(dir, &id, host.clone())?,
            id,
            force,
            reason,
        ),
        Command::Park {
            id,
            next_step,
            waiting_on,
            reason,
            complexity,
            needs,
            minutes,
        } => park::run(
            open_id_write_ctx(dir, &id, host.clone())?,
            id,
            next_step,
            waiting_on,
            reason,
            complexity,
            needs,
            minutes,
        ),
        Command::Done { id, message, force } => status::close(
            open_id_write_ctx(dir, &id, host.clone())?,
            id,
            Status::Done,
            message,
            force,
        ),
        Command::Drop { id, message } => status::close(
            open_id_write_ctx(dir, &id, host.clone())?,
            id,
            Status::Dropped,
            message,
            false,
        ),
        Command::Release { id, why, adopting } => {
            let (ctx, adopting) = release::open(dir, &id, adopting.as_deref(), host.clone())?;
            release::run(ctx, adopting, id, why)
        }
        Command::Block { id, message } => {
            status::block(open_id_write_ctx(dir, &id, host.clone())?, id, message)
        }
        Command::Unblock { id } => status::unblock(open_id_write_ctx(dir, &id, host.clone())?, id),
        Command::Shelve {
            id,
            wake,
            wake_on,
            defer,
        } => status::shelve(
            open_id_write_ctx(dir, &id, host.clone())?,
            id,
            wake,
            wake_on,
            defer,
        ),
        Command::Unshelve { id } => {
            status::unshelve(open_id_write_ctx(dir, &id, host.clone())?, id)
        }
        Command::Dep { id, on, rm } => {
            dep::run(open_id_write_ctx(dir, &id, host.clone())?, id, on, rm)
        }
        Command::Blockers(args) => {
            blockers::run(open_read_ctx(dir, &args.scope, host.clone())?, args)
        }
        Command::Graph { format, all } => graph::run(open_ctx(dir, host.clone())?, format, all),
        Command::Check { growth } => check::run(open_ctx(dir, host.clone())?, growth),
        Command::Commit {
            targets,
            message,
            project,
        } => commit::open_and_run(dir, targets, message, project, host.clone()),
        Command::Tree { id, all, scope } => tree::run(
            open_id_read_ctx(dir, &scope, id.as_deref(), host.clone())?,
            id,
            all,
        ),
        Command::Tags { statuses, scope } => {
            tags::run(open_read_ctx(dir, &scope, host.clone())?, statuses)
        }
        Command::Claims { all_projects: _ } => claims::run(),
        Command::Quiet {
            limit,
            project,
            all_projects: _,
            group,
        } => {
            let scope = ScopeArgs {
                all_projects: project.is_none() && group.is_none(),
                project,
                group,
            };
            quiet::run(open_read_ctx(dir, &scope, host.clone())?, limit)
        }
        Command::Goals {
            project,
            all_projects: _,
            group,
            closed,
        } => {
            let scope = ScopeArgs {
                all_projects: project.is_none() && group.is_none(),
                project,
                group,
            };
            goals::run(open_read_ctx(dir, &scope, host.clone())?, closed)
        }
        Command::Feedback {
            project,
            summary,
            category,
            body,
            recur,
            new,
            origin,
            origin_session,
        } => {
            let body = body.unwrap_or_default();
            feedback::validate(&summary, &category, &body)?;
            let attribution = Attribution::parse(origin.as_deref(), origin_session.as_deref())?;
            let (ctx, reporter) = feedback::open(dir, &project, host.clone())?;
            feedback::run(
                ctx,
                reporter,
                project,
                summary,
                category,
                body,
                recur,
                new,
                attribution,
            )
        }
    }
}

pub fn run(cli: Cli, host: Option<Arc<HostConfig>>) -> Result<Output> {
    crate::history::begin_metrics();
    let started = std::time::Instant::now();
    crate::written::begin(cli.command.commit_verb());
    let result = dispatch(cli, host);
    // A command that failed after writing leaves its files uncommitted; `prime` lists
    // them. A successful one must have reached `finish`: anything left is a write path
    // that skipped it, which the debug build refuses so the test suite catches it.
    let left = crate::written::take();
    if result.is_ok() && left.verb.is_some() {
        debug_assert!(
            left.roots.is_empty(),
            "written but never committed: {:?}",
            left.roots
        );
    }
    if std::env::var("TASKS_RETAINED_TEST_METRICS").as_deref() == Ok("1") {
        let mut metrics = crate::retained::verify::Metrics::default().finish();
        metrics.wall_ns = started.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        eprintln!(
            "retained test command metrics: {}",
            serde_json::to_string(&metrics).expect("metrics serialize")
        );
    }
    result
}
