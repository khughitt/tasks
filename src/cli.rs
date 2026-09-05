use clap::{Args, Parser, Subcommand};
use clap_complete::ArgValueCandidates;
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
    /// Add a tag (repeatable). On `edit` this appends; see `--rm-tag` and `--no-tags`.
    #[arg(long = "tag")]
    pub tags: Vec<String>,
    #[arg(long = "depends")]
    pub depends: Vec<String>,
    #[arg(long)]
    pub spec: Option<String>,
    #[arg(long)]
    pub plan: Option<String>,
    #[arg(long)]
    pub step: Option<String>,
    /// Make this task part of another task (same project).
    #[arg(long)]
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
    Projects,
    /// The registered root of the project an id belongs to.
    Root { id: String },
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
    Show { id: String },
    /// List tasks (open by default).
    List {
        #[arg(long = "status", add = ArgValueCandidates::new(crate::complete::statuses))]
        statuses: Vec<String>,
        #[arg(long = "tag")]
        tags: Vec<String>,
        #[arg(long)]
        owner: Option<String>,
        /// Only direct children of this task.
        #[arg(long)]
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
        id: String,
        #[command(flatten)]
        args: EditArgs,
    },
    /// Append a timestamped note.
    Note { id: String, text: String },
    /// Claim a task: status=doing, owner=you.
    Start {
        id: String,
        /// Take over a claim another live session holds.
        #[arg(long)]
        force: bool,
    },
    /// Close a task as done.
    Done {
        id: String,
        message: Option<String>,
        #[arg(long)]
        force: bool,
    },
    /// Close a task as dropped.
    Drop { id: String, message: Option<String> },
    /// Mark a task blocked.
    Block { id: String, message: Option<String> },
    /// Return a blocked task to todo.
    Unblock { id: String },
    /// Add or remove dependencies.
    Dep {
        id: String,
        #[arg(long = "on", conflicts_with = "rm", required_unless_present = "rm", num_args = 1..)]
        on: Vec<String>,
        #[arg(long = "rm", num_args = 1..)]
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
        #[arg(long, conflicts_with = "new")]
        recur: Option<String>,
        /// Create a new entry even if a similar one exists.
        #[arg(long)]
        new: bool,
    },
    /// The task hierarchy as nested nodes (open work only unless --all).
    Tree {
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
