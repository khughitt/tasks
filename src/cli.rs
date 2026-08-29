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
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Initialize tasks/ in this repository and register it.
    Init {
        #[arg(long)]
        prefix: Option<String>,
    },
    /// Create a task.
    Add {
        title: String,
        #[arg(long, default_value = "todo")]
        status: String,
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
        #[arg(long)]
        all_projects: bool,
    },
    /// Actionable tasks: todo with all dependencies closed.
    Ready {
        #[arg(long)]
        size: Option<String>,
        #[arg(short = 'n', long)]
        limit: Option<usize>,
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
        #[command(flatten)]
        fields: FieldArgs,
    },
    /// Append a timestamped note.
    Note { id: String, text: String },
    /// Claim a task: status=doing, owner=you.
    Start { id: String },
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
    Prime,
}
