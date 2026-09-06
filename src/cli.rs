use clap::{Args, Parser, Subcommand};
use clap_complete::{ArgValueCandidates, ArgValueCompleter};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "tasks",
    version,
    about = "File-based task tracker for projects and agents"
)]
pub struct Cli {
    /// Run as if started in this directory.
    #[arg(short = 'C', global = true, value_name = "DIR")]
    pub dir: Option<PathBuf>,
    /// Human-readable output instead of JSON (also TASKS_FORMAT=pretty).
    #[arg(long, global = true)]
    pub pretty: bool,
    /// Color pretty output: auto (when the stream is a terminal), always, or never.
    /// Also TASKS_COLOR. Off unless asked for; never applies to JSON.
    #[arg(
        long,
        global = true,
        value_name = "WHEN",
        add = ArgValueCandidates::new(crate::complete::colors)
    )]
    pub color: Option<String>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Args, Debug, Default, Clone)]
pub struct FieldArgs {
    #[arg(short = 'b', long)]
    pub body: Option<String>,
    #[arg(short = 'p', long)]
    pub priority: Option<u8>,
    #[arg(long, add = ArgValueCandidates::new(crate::complete::sizes))]
    pub size: Option<String>,
    /// Mark as safe to run beside other tasks marked parallel. On `edit` this sets the
    /// flag; see `--no-parallel` to clear it.
    #[arg(long)]
    pub parallel: bool,
    /// Add a tag (repeatable). On `edit` this appends; see `--rm-tag` and `--no-tags`.
    #[arg(long = "tag")]
    pub tags: Vec<String>,
    #[arg(long = "depends", add = ArgValueCompleter::new(crate::complete::resolvable))]
    pub depends: Vec<String>,
    #[arg(long)]
    pub spec: Option<String>,
    #[arg(long)]
    pub plan: Option<String>,
    #[arg(long)]
    pub step: Option<String>,
    /// Make this task part of another task (same project).
    #[arg(long, add = ArgValueCompleter::new(crate::complete::destination_ids))]
    pub parent: Option<String>,
}

/// The flags `edit` adds to the shared field flags.
#[derive(Args, Debug)]
pub struct EditArgs {
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long, add = ArgValueCandidates::new(crate::complete::statuses))]
    pub status: Option<String>,
    #[arg(long)]
    pub force: bool,
    /// Detach from the parent.
    #[arg(long, conflicts_with = "parent")]
    pub no_parent: bool,
    /// Clear the parallel marker.
    #[arg(long, conflicts_with = "parallel")]
    pub no_parallel: bool,
    /// Remove a tag (repeatable); `--tag` adds one.
    #[arg(long = "rm-tag", value_name = "TAG", conflicts_with = "no_tags")]
    pub rm_tags: Vec<String>,
    /// Clear every tag; with `--tag`, replaces the list wholesale.
    #[arg(long)]
    pub no_tags: bool,
    #[command(flatten)]
    pub fields: FieldArgs,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Initialize tasks/ in this repository and register it.
    Init {
        #[arg(long)]
        prefix: Option<String>,
        /// Re-point the prefix at this directory even if it is registered elsewhere.
        #[arg(long)]
        force: bool,
    },
    /// Remove a prefix from the registry. Project files are left untouched.
    Unregister {
        #[arg(add = ArgValueCandidates::new(crate::complete::prefixes))]
        prefix: String,
    },
    /// The registry: every project, whether it is reachable, and its status counts.
    Projects {
        /// Order: prefix (default), size (most tasks first), or activity (most recent
        /// first). Unreachable projects stay last whatever the order.
        #[arg(long, add = ArgValueCandidates::new(crate::complete::project_sorts))]
        sort: Option<String>,
        /// Reverse the chosen order.
        #[arg(long)]
        reverse: bool,
        /// Also show the done and dropped columns.
        #[arg(long)]
        closed: bool,
        /// Show each project's registered root as a final column.
        #[arg(long)]
        paths: bool,
    },
    /// The registered root of the project an id belongs to.
    Root {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
    },
    /// Create a task.
    Add {
        title: String,
        #[arg(
            long,
            default_value = "todo",
            add = ArgValueCandidates::new(crate::complete::add_statuses)
        )]
        status: String,
        /// Create it in this registered project instead of the current one; needs no
        /// local project. Every field is validated against that project.
        #[arg(long, add = ArgValueCandidates::new(crate::complete::prefixes))]
        project: Option<String>,
        #[command(flatten)]
        fields: FieldArgs,
    },
    /// Show one task with resolved links and dependencies.
    Show {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
    },
    /// List tasks (open by default).
    List {
        #[arg(long = "status", add = ArgValueCandidates::new(crate::complete::statuses))]
        statuses: Vec<String>,
        #[arg(long = "tag")]
        tags: Vec<String>,
        #[arg(long)]
        owner: Option<String>,
        /// Only direct children of this task.
        #[arg(long, add = ArgValueCompleter::new(crate::complete::scoped))]
        parent: Option<String>,
        /// Order: priority (default: priority, then last activity), updated, or created
        /// (most recent first). Pretty rows show the date sorted on, else last activity.
        #[arg(long, add = ArgValueCandidates::new(crate::complete::sorts))]
        sort: Option<String>,
        /// Reverse the chosen order.
        #[arg(long)]
        reverse: bool,
        #[arg(long)]
        all_projects: bool,
    },
    /// Actionable tasks: todo with all dependencies closed.
    Ready {
        #[arg(long, add = ArgValueCandidates::new(crate::complete::sizes))]
        size: Option<String>,
        /// Only tasks marked safe to run beside each other.
        #[arg(long)]
        parallel: bool,
        #[arg(short = 'n', long)]
        limit: Option<usize>,
        /// Every reachable registered project; needs no local project.
        #[arg(long)]
        all_projects: bool,
    },
    /// The first ready task, in the show shape; null when nothing is ready.
    Next {
        /// Every reachable registered project; needs no local project.
        #[arg(long)]
        all_projects: bool,
    },
    /// Edit fields, or open the task in $EDITOR when no field flags are given.
    Edit {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
        #[command(flatten)]
        args: EditArgs,
    },
    /// Append a timestamped note.
    Note {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
        text: String,
    },
    /// Claim a task: status=doing, owner=you.
    Start {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
        /// Take over a claim another live session holds.
        #[arg(long)]
        force: bool,
    },
    /// Close a task as done.
    Done {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
        message: Option<String>,
        #[arg(long)]
        force: bool,
    },
    /// Close a task as dropped.
    Drop {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
        message: Option<String>,
    },
    /// Mark a task blocked.
    Block {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
        message: Option<String>,
    },
    /// Return a blocked task to todo.
    Unblock {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
    },
    /// Add or remove dependencies.
    Dep {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
        #[arg(
            long = "on",
            conflicts_with = "rm",
            required_unless_present = "rm",
            num_args = 1..,
            add = ArgValueCompleter::new(crate::complete::resolvable)
        )]
        on: Vec<String>,
        #[arg(long = "rm", num_args = 1.., add = ArgValueCompleter::new(crate::complete::dependencies))]
        rm: Vec<String>,
    },
    /// Dependency graph as mermaid or dot.
    Graph {
        #[arg(long, default_value = "mermaid")]
        format: String,
        #[arg(long)]
        all: bool,
    },
    /// Validate every task file.
    Check,
    /// Session context for agents.
    Prime {
        /// Every reachable registered project; needs no local project.
        #[arg(long)]
        all_projects: bool,
        /// Also show the done and dropped counts.
        #[arg(long)]
        closed: bool,
    },
    /// File feedback about the tasks tool itself into the upstream tasks project.
    Feedback {
        summary: String,
        /// friction | gap | idea | positive
        #[arg(long, add = ArgValueCandidates::new(crate::complete::categories))]
        category: String,
        #[arg(short = 'b', long)]
        body: Option<String>,
        /// Append to this open feedback task instead of matching titles.
        #[arg(
            long,
            conflicts_with = "new",
            add = ArgValueCompleter::new(crate::complete::upstream_feedback)
        )]
        recur: Option<String>,
        /// Create a new entry even if a similar one exists.
        #[arg(long)]
        new: bool,
    },
    /// The task hierarchy as nested nodes (open work only unless --all).
    Tree {
        #[arg(add = ArgValueCompleter::new(crate::complete::scoped))]
        id: Option<String>,
        #[arg(long)]
        all: bool,
        /// Every reachable registered project, one forest each; needs no local project.
        #[arg(long, conflicts_with = "id")]
        all_projects: bool,
    },
    /// Tag frequencies (open tasks unless --status), per project.
    Tags {
        #[arg(long = "status", add = ArgValueCandidates::new(crate::complete::statuses))]
        statuses: Vec<String>,
        /// Every reachable registered project; needs no local project.
        #[arg(long)]
        all_projects: bool,
    },
}
