use clap::{Args, Parser, Subcommand};
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
    #[arg(long, global = true, value_name = "WHEN")]
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
    #[arg(long)]
    pub size: Option<String>,
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
    Unregister { prefix: String },
    /// The registry: every project, whether it is reachable, and its status counts.
    Projects,
    /// The registered root of the project an id belongs to.
    Root { id: String },
    /// Create a task.
    Add {
        title: String,
        #[arg(long, default_value = "todo")]
        status: String,
        /// Create it in this registered project instead of the current one; needs no
        /// local project. Every field is validated against that project.
        #[arg(long)]
        project: Option<String>,
        #[command(flatten)]
        fields: FieldArgs,
    },
    /// Show one task with resolved links and dependencies.
    Show { id: String },
    /// List tasks (open by default).
    List {
        #[arg(long = "status")]
        statuses: Vec<String>,
        #[arg(long = "tag")]
        tags: Vec<String>,
        #[arg(long)]
        owner: Option<String>,
        /// Only direct children of this task.
        #[arg(long)]
        parent: Option<String>,
        #[arg(long)]
        all_projects: bool,
    },
    /// Actionable tasks: todo with all dependencies closed.
    Ready {
        #[arg(long)]
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
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        force: bool,
        /// Detach from the parent.
        #[arg(long, conflicts_with = "parent")]
        no_parent: bool,
        #[command(flatten)]
        fields: FieldArgs,
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
        #[arg(long)]
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
        #[arg(long = "status")]
        statuses: Vec<String>,
        /// Every reachable registered project; needs no local project.
        #[arg(long)]
        all_projects: bool,
    },
}
