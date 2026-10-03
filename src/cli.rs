use clap::{Args, Parser, Subcommand};
use clap_complete::{ArgValueCandidates, ArgValueCompleter};
use std::path::PathBuf;

use crate::complete::ValueSet;

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
    /// JSON output (the default; the explicit form of TASKS_FORMAT=json).
    #[arg(long, global = true, conflicts_with = "pretty")]
    pub json: bool,
    /// Human-readable output instead of JSON (also TASKS_FORMAT=pretty).
    #[arg(long, global = true)]
    pub pretty: bool,
    /// Color pretty output: auto (when the stream is a terminal), always, or never.
    /// Also TASKS_COLOR. Off unless asked for; never applies to JSON.
    #[arg(
        long,
        global = true,
        value_name = "WHEN",
        add = ArgValueCandidates::new(crate::complete::colors),
        add = ValueSet,
        value_parser = ValueSet
    )]
    pub color: Option<String>,
    #[command(subcommand)]
    pub command: Command,
}

/// The four read scopes: the current project (no flag), one named registered project,
/// the members of a project group, or every reachable one. `--project` opens the registered root through the
/// same path `add --project` uses, so a worktree of that prefix does not displace it.
#[derive(Args, Debug)]
pub struct ScopeArgs {
    /// Read this registered project instead of the current one; needs no local project.
    #[arg(
        long,
        conflicts_with = "all_projects",
        add = ArgValueCandidates::new(crate::complete::prefixes)
    )]
    pub project: Option<String>,
    /// Every reachable registered project; needs no local project.
    #[arg(long)]
    pub all_projects: bool,
    /// The members of this project group (`tasks groups`); needs no local project.
    #[arg(
        long,
        value_name = "NAME",
        conflicts_with_all = ["project", "all_projects"],
        add = ArgValueCandidates::new(crate::complete::groups)
    )]
    pub group: Option<String>,
}

/// Selection over record fields, shared by `list` and `ready`. Repeats of one flag widen
/// (any of them); different flags narrow (all of them); `--tag` is all-of.
#[derive(Args, Debug, Default, Clone)]
pub struct FilterArgs {
    /// Only tasks of this priority (repeatable). Repeats of one filter widen; different
    /// filters narrow.
    #[arg(
        short = 'p',
        long = "priority",
        value_name = "N",
        value_parser = clap::value_parser!(u8).range(0..=4),
        add = ArgValueCandidates::new(crate::complete::priorities),
        add = ValueSet
    )]
    pub priorities: Vec<u8>,
    /// Only tasks of this size (repeatable); `none` selects unsized tasks.
    #[arg(
        long = "size",
        value_name = "SIZE",
        add = ArgValueCandidates::new(crate::complete::filter_sizes),
        add = ValueSet,
        value_parser = ValueSet
    )]
    pub sizes: Vec<String>,
    /// Only tasks at this effective complexity (repeatable); `none` selects unassessed
    /// tasks. A selection, not the session cutoff.
    #[arg(
        long = "complexity",
        value_name = "LEVEL",
        add = ArgValueCandidates::new(crate::complete::filter_complexities),
        add = ValueSet,
        value_parser = ValueSet
    )]
    pub complexities: Vec<String>,
    /// Only tasks with this process (repeatable); `none` selects unassessed tasks.
    #[arg(
        long = "process",
        value_name = "PROCESS",
        add = ArgValueCandidates::new(crate::complete::filter_processes),
        add = ValueSet,
        value_parser = ValueSet
    )]
    pub processes: Vec<String>,
    /// Filter by tag (repeatable); a task must carry every one.
    #[arg(long = "tag", value_name = "TAG")]
    pub tags: Vec<String>,
    /// Filter by need (repeatable); a task must need every one.
    #[arg(long = "need", value_name = "NEED")]
    pub needs: Vec<String>,
    /// Only tasks owned by this value.
    #[arg(long)]
    pub owner: Option<String>,
    /// Only tasks whose source is exactly this reference; matched byte for byte,
    /// never interpreted, so it answers "what came from here" for any origin.
    #[arg(long)]
    pub source: Option<String>,
    /// Only direct children of this task.
    #[arg(long, value_name = "REF", add = ArgValueCompleter::new(crate::complete::scoped))]
    pub parent: Option<String>,
    /// Only descendants of this task, at any depth (`--parent` is direct children only).
    #[arg(long, value_name = "REF", add = ArgValueCompleter::new(crate::complete::scoped))]
    pub under: Option<String>,
    /// Only tasks marked safe to run beside each other.
    #[arg(long)]
    pub parallel: bool,
}

/// The needs this session cannot meet, shared by the pickers (lanes-needs spec §4.3).
#[derive(Args, Debug, Default, Clone)]
pub struct WithoutArgs {
    /// Hide tasks that need this (repeatable), in the projects that declare it; adds to
    /// TASKS_WITHOUT. Refused when no project in scope declares it.
    #[arg(long = "without", value_name = "NEED")]
    pub without: Vec<String>,
}

#[derive(Args, Debug, Default, Clone)]
pub struct FieldArgs {
    #[arg(short = 'b', long)]
    pub body: Option<String>,
    /// Urgency, 0 (most urgent) to 4.
    #[arg(
        short = 'p',
        long,
        value_parser = clap::value_parser!(u8).range(0..=4),
        add = ArgValueCandidates::new(crate::complete::priorities),
        add = ValueSet
    )]
    pub priority: Option<u8>,
    /// Effort: xs, s, m, l, or xl.
    #[arg(
        long,
        add = ArgValueCandidates::new(crate::complete::sizes),
        add = ValueSet,
        value_parser = ValueSet
    )]
    pub size: Option<String>,
    /// The judgment the task demands: low, mid, or high. Absent is unassessed.
    #[arg(
        long,
        add = ArgValueCandidates::new(crate::complete::complexities),
        add = ValueSet,
        value_parser = ValueSet
    )]
    pub complexity: Option<String>,
    /// The chosen workflow: direct or planned. Absent is unassessed.
    #[arg(
        long,
        add = ArgValueCandidates::new(crate::complete::processes),
        add = ValueSet,
        value_parser = ValueSet
    )]
    pub process: Option<String>,
    /// Mark as safe to run beside other tasks marked parallel. On `edit` this sets the
    /// flag; see `--no-parallel` to clear it.
    #[arg(long)]
    pub parallel: bool,
    /// Mark this goal as a lane: an effort meant to proceed alongside other lanes. On
    /// `edit` this sets the flag; see `--no-lane` to clear it.
    #[arg(long)]
    pub lane: bool,
    /// Make this a recurrence: `<n>d` or `<n>w`, measured from each completion.
    #[arg(long, value_name = "AGE", add = ArgValueCandidates::new(crate::complete::intervals))]
    pub every: Option<String>,
    /// Hide this task until a date: `YYYY-MM-DD`, or `<n>d`/`<n>w` from today.
    #[arg(long, value_name = "WHEN", add = ArgValueCandidates::new(crate::complete::defer_dates))]
    pub defer: Option<String>,
    /// Add a tag (repeatable). On `edit` this appends; see `--rm-tag` and `--no-tags`.
    #[arg(long = "tag")]
    pub tags: Vec<String>,
    /// Need a shared resource declared in `[needs]` (repeatable). On `edit` this
    /// appends; see `--rm-need` and `--no-needs`.
    #[arg(long = "need", value_name = "NEED")]
    pub needs: Vec<String>,
    /// Depend on another task (repeatable). On `edit` this appends; see `--no-depends`
    /// and `dep --rm`.
    #[arg(long = "depends", value_name = "REF", add = ArgValueCompleter::new(crate::complete::resolvable))]
    pub depends: Vec<String>,
    #[arg(long)]
    pub spec: Option<String>,
    #[arg(long)]
    pub plan: Option<String>,
    #[arg(long)]
    pub step: Option<String>,
    /// Make this task part of another task (same project).
    #[arg(long, value_name = "REF", add = ArgValueCompleter::new(crate::complete::destination_ids))]
    pub parent: Option<String>,
    /// Where the task came from: an opaque, single-line reference such as a URL or a
    /// message id. Never interpreted. On `edit` this replaces; see `--no-source`.
    #[arg(long)]
    pub source: Option<String>,
    /// The harness and model filing the task, `<harness>/<model>` or the harness alone.
    /// On `add` this overrides `TASKS_AGENT`; on `edit` it replaces the stamp. Never
    /// interpreted; see `--no-agent`.
    #[arg(long)]
    pub agent: Option<String>,
}

/// The flags `edit` adds to the shared field flags.
#[derive(Args, Debug)]
pub struct EditArgs {
    #[arg(long)]
    pub title: Option<String>,
    /// Move to a status: idea, todo, doing, blocked, done, or dropped. Shelved is
    /// entered with `shelve`, never here.
    /// Status, defer, and needs change in separate operations.
    #[arg(
        long,
        conflicts_with_all = ["defer", "needs", "rm_needs", "no_needs"],
        add = ArgValueCandidates::new(crate::complete::edit_statuses),
        add = ValueSet,
        value_parser = ValueSet
    )]
    pub status: Option<String>,
    #[arg(long)]
    pub force: bool,
    /// With --force and --need: why the need is added while another session holds it.
    /// Noted on the task, and on the holder when it is in this project.
    #[arg(long)]
    pub reason: Option<String>,
    /// Detach from the parent.
    #[arg(long, conflicts_with = "parent")]
    pub no_parent: bool,
    /// Clear the parallel marker.
    #[arg(long, conflicts_with = "parallel")]
    pub no_parallel: bool,
    /// Clear the lane marker.
    #[arg(long, conflicts_with = "lane")]
    pub no_lane: bool,
    /// Stop the recurrence, clearing both the cadence and its anchor.
    #[arg(long, conflicts_with = "every")]
    pub no_every: bool,
    /// Clear the deferral.
    #[arg(long, conflicts_with = "defer")]
    pub no_defer: bool,
    /// Clear the source.
    #[arg(long, conflicts_with = "source")]
    pub no_source: bool,
    /// Clear the agent stamp.
    #[arg(long, conflicts_with = "agent")]
    pub no_agent: bool,
    /// Clear the spec link.
    #[arg(long, conflicts_with = "spec")]
    pub no_spec: bool,
    /// Clear the plan link. A step needs a plan, so a linked step goes with `--no-step`.
    #[arg(long, conflicts_with = "plan")]
    pub no_plan: bool,
    /// Clear the plan-step link.
    #[arg(long, conflicts_with = "step")]
    pub no_step: bool,
    /// Clear the complexity rating (back to unassessed).
    #[arg(long, conflicts_with = "complexity")]
    pub no_complexity: bool,
    /// Clear the process decision (back to unassessed).
    #[arg(long, conflicts_with = "process")]
    pub no_process: bool,
    /// Replace the model stamp recorded at completion; see `--no-model`.
    #[arg(long)]
    pub model: Option<String>,
    /// Clear the model stamp.
    #[arg(long, conflicts_with = "model")]
    pub no_model: bool,
    /// Remove a tag (repeatable); `--tag` adds one.
    #[arg(long = "rm-tag", value_name = "TAG", conflicts_with = "no_tags")]
    pub rm_tags: Vec<String>,
    /// Clear every tag; with `--tag`, replaces the list wholesale.
    #[arg(long)]
    pub no_tags: bool,
    /// Remove a need (repeatable); `--need` adds one.
    #[arg(long = "rm-need", value_name = "NEED", conflicts_with = "no_needs")]
    pub rm_needs: Vec<String>,
    /// Clear every need; with `--need`, replaces the list wholesale.
    #[arg(long)]
    pub no_needs: bool,
    /// Clear every dependency; with `--depends`, replaces the list wholesale.
    #[arg(long)]
    pub no_depends: bool,
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
    /// Rename a registered project's prefix and retain its retired names as aliases.
    Rename {
        /// The prefix as registered today.
        #[arg(value_name = "REF", add = ArgValueCandidates::new(crate::complete::prefixes))]
        old: String,
        /// The prefix to use from now on.
        #[arg(value_name = "REF")]
        new: String,
        /// Explain recovery without locks or writes.
        #[arg(long)]
        explain: bool,
        /// Adopt a rename already present in this checkout from another host.
        #[arg(long)]
        adopt: bool,
    },
    /// Remove a prefix from the registry. Project files are left untouched.
    Unregister {
        #[arg(add = ArgValueCandidates::new(crate::complete::prefixes))]
        prefix: String,
    },
    /// The registry: every project, whether it is reachable, and its status counts.
    Projects {
        /// Order: prefix, size (most tasks first), or activity (most recent first).
        /// Unreachable projects stay last whatever the order.
        #[arg(
            long,
            default_value = "prefix",
            add = ArgValueCandidates::new(crate::complete::project_sorts),
            add = ValueSet,
            value_parser = ValueSet
        )]
        sort: String,
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
    /// Named sets of registered projects, read together with --group.
    Group {
        #[command(subcommand)]
        action: GroupAction,
    },
    /// Every group with its members and whether each is reachable.
    Groups,
    /// The registered root of the project an id belongs to.
    Root {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
    },
    /// Create a task.
    Add {
        title: String,
        /// Start as todo or as an idea.
        #[arg(
            long,
            default_value = "todo",
            add = ArgValueCandidates::new(crate::complete::add_statuses),
            add = ValueSet,
            value_parser = ValueSet
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
    #[command(
        after_help = "Examples:\n  tasks list --sort updated\n  tasks list --status todo --tag cli\n  tasks list -p 0 -p 1 --size s --size xs"
    )]
    List {
        /// Filter by status (repeatable): idea, todo, doing, blocked, shelved, done, or dropped.
        #[arg(
            long = "status",
            value_name = "STATUS",
            add = ArgValueCandidates::new(crate::complete::statuses),
            add = ValueSet,
            value_parser = ValueSet
        )]
        statuses: Vec<String>,
        #[command(flatten)]
        filter: FilterArgs,
        /// Order: priority (then last activity), updated, or created (most recent
        /// first). Pretty rows show the date sorted on, else last activity.
        #[arg(
            long,
            default_value = "priority",
            add = ArgValueCandidates::new(crate::complete::sorts),
            add = ValueSet,
            value_parser = ValueSet
        )]
        sort: String,
        /// Reverse the chosen order.
        #[arg(long)]
        reverse: bool,
        /// Only parked tasks, most recently parked first.
        #[arg(long, conflicts_with_all = ["sort", "reverse"])]
        parked: bool,
        /// Only tasks with a cadence, soonest due first, at any status.
        #[arg(long, conflicts_with_all = ["sort", "reverse", "parked"])]
        periodic: bool,
        /// Only deferred tasks, soonest date first; due ones lead.
        #[arg(long, conflicts_with_all = ["sort", "reverse", "parked", "periodic"])]
        deferred: bool,

        #[command(flatten)]
        scope: ScopeArgs,
    },
    /// Actionable tasks: todo or due recurrences with all dependencies closed.
    Ready {
        #[command(flatten)]
        filter: FilterArgs,
        /// At most this many.
        #[arg(short = 'n', long, value_name = "N")]
        limit: Option<usize>,
        /// Hide tasks rated above this level and unassessed tasks; overrides
        /// TASKS_MAX_COMPLEXITY.
        #[arg(
            long,
            value_name = "LEVEL",
            add = ArgValueCandidates::new(crate::complete::complexities),
            add = ValueSet,
            value_parser = ValueSet
        )]
        max_complexity: Option<String>,
        #[command(flatten)]
        without: WithoutArgs,
        #[command(flatten)]
        scope: ScopeArgs,
    },
    /// The first ready task, in the show shape; null when nothing is ready.
    Next {
        /// Hide tasks rated above this level and unassessed tasks; overrides
        /// TASKS_MAX_COMPLEXITY.
        #[arg(
            long,
            value_name = "LEVEL",
            add = ArgValueCandidates::new(crate::complete::complexities),
            add = ValueSet,
            value_parser = ValueSet
        )]
        max_complexity: Option<String>,
        #[command(flatten)]
        without: WithoutArgs,
        /// Pick only among descendants of this task, at any depth: how a session
        /// committed to one lane takes its next step.
        #[arg(long, value_name = "REF", add = ArgValueCompleter::new(crate::complete::scoped))]
        under: Option<String>,
        #[command(flatten)]
        scope: ScopeArgs,
    },
    /// Random open tasks for a curation pass: idea, todo, or blocked; not live-claimed;
    /// not updated within the --older-than age. Rows are list rows.
    Sample {
        /// At most this many, drawn without replacement.
        #[arg(short = 'n', long, value_name = "N", default_value_t = 3)]
        limit: usize,
        /// Exclude tasks updated within this age, `<n>d` or `<n>w` up to 36500 days;
        /// `0d` skips the age check entirely.
        #[arg(
            long,
            value_name = "AGE",
            default_value = "7d",
            value_parser = crate::defer::parse_age
        )]
        older_than: i64,
        /// Fix the draw so a pass can be reproduced.
        #[arg(long, value_name = "N")]
        seed: Option<u64>,
        #[command(flatten)]
        scope: ScopeArgs,
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
        /// Stamp the note with this session's harness provenance, as lifecycle notes are.
        #[arg(long)]
        stamp: bool,
    },
    /// Copy a file into the task's tasks/files/<id>/ and record it in a ledger note.
    #[command(
        after_help = "Examples:\n  tasks attach sci-4f2a9c ~/Pictures/before.png --caption \"the stale row\"\n  grim - | tasks attach sci-4f2a9c - --name after.png\n  tasks attach sci-4f2a9c --clipboard"
    )]
    Attach {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
        /// The file to copy, or `-` for stdin (which needs --name).
        #[arg(required_unless_present = "clipboard", conflicts_with = "clipboard")]
        source: Option<String>,
        /// Read an image from the Wayland clipboard through wl-paste.
        #[arg(long)]
        clipboard: bool,
        /// The stored name: [A-Za-z0-9._-], at most 128 bytes, not starting with '.'.
        #[arg(long)]
        name: Option<String>,
        /// One line saying what the file shows, kept in the ledger note.
        #[arg(long)]
        caption: Option<String>,
    },
    /// Remove one attachment and record why. Git history keeps committed bytes; an
    /// uncommitted file is gone for good.
    Detach {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
        name: String,
        why: String,
    },
    /// Claim a task: status=doing, owner=you.
    Start {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
        /// Take over a claim another live session holds, or acquire past a held need.
        #[arg(long)]
        force: bool,
        /// Explain an audited halt override, need override, or forced takeover.
        #[arg(long)]
        reason: Option<String>,
    },
    /// Set a task down: record the next step, who it waits on, and this session in the
    /// shared store. Status is untouched; `start` resumes it.
    Park {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
        /// The one concrete next step, on one line.
        next_step: String,
        /// Who the task waits on: user or agent.
        #[arg(
            long,
            default_value = "agent",
            value_name = "WHO",
            add = ArgValueCandidates::new(crate::complete::waiting_on),
            add = ValueSet,
            value_parser = ValueSet
        )]
        waiting_on: String,
        /// Why the work stopped: review, decision, approval, environment, dependency,
        /// session, capability, or quiet. Optional; absent means not recorded.
        #[arg(
            long,
            value_name = "WHY",
            add = ArgValueCandidates::new(crate::complete::reason),
            add = ValueSet,
            value_parser = ValueSet
        )]
        reason: Option<String>,
        /// With --reason capability: the rating the work actually needs. Written to the
        /// record and, when waiting on the agent, to the shared store as an escalation.
        #[arg(
            long,
            value_name = "LEVEL",
            add = ArgValueCandidates::new(crate::complete::complexities),
            add = ValueSet,
            value_parser = ValueSet
        )]
        complexity: Option<String>,
        /// With --reason quiet: what a free host means for this work, idle (the desktop
        /// may stay up but nothing else runs; the default) or headless (the ordinary
        /// desktop session is stopped first).
        #[arg(
            long,
            value_name = "COND",
            add = ArgValueCandidates::new(crate::complete::needs),
            add = ValueSet,
            value_parser = ValueSet
        )]
        needs: Option<String>,
        /// With --reason quiet: expected wall-clock minutes once started, 1 to 1440.
        /// Required with that reason; refused with any other.
        #[arg(long, value_name = "N", value_parser = clap::value_parser!(u32).range(1..=1440))]
        minutes: Option<u32>,
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
    /// Put a task out of active work: status=shelved, hidden from the default views,
    /// open for dependencies. The wake condition is required and becomes the note.
    Shelve {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
        /// What would bring the task back.
        wake: String,
    },
    /// Return a shelved task to idea.
    Unshelve {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
    },
    /// Add or remove dependencies.
    Dep {
        #[arg(add = ArgValueCompleter::new(crate::complete::id_directed))]
        id: String,
        /// Depend on these tasks.
        #[arg(
            long = "on",
            value_name = "REF",
            required_unless_present = "rm",
            num_args = 1..,
            add = ArgValueCompleter::new(crate::complete::resolvable)
        )]
        on: Vec<String>,
        /// Stop depending on these tasks; with --on, both apply in one save.
        #[arg(
            long = "rm",
            value_name = "REF",
            num_args = 1..,
            add = ArgValueCompleter::new(crate::complete::dependencies)
        )]
        rm: Vec<String>,
    },
    /// Dependency graph as mermaid or dot.
    Graph {
        /// mermaid or dot.
        #[arg(
            long,
            default_value = "mermaid",
            add = ArgValueCandidates::new(crate::complete::graph_formats),
            add = ValueSet,
            value_parser = ValueSet
        )]
        format: String,
        #[arg(long)]
        all: bool,
    },
    /// Validate every task file. Prints nothing when there is nothing to report.
    Check,
    /// Session context for agents.
    Prime {
        #[command(flatten)]
        scope: ScopeArgs,
        /// Also show the done and dropped counts.
        #[arg(long)]
        closed: bool,
        #[command(flatten)]
        without: WithoutArgs,
    },
    /// Each open lane: its guidance, its state, and the step it could take now.
    Lanes {
        #[command(flatten)]
        without: WithoutArgs,
        /// Hide steps rated above this level and unassessed steps; overrides
        /// TASKS_MAX_COMPLEXITY.
        #[arg(
            long,
            value_name = "LEVEL",
            add = ArgValueCandidates::new(crate::complete::complexities),
            add = ValueSet,
            value_parser = ValueSet
        )]
        max_complexity: Option<String>,
        #[command(flatten)]
        scope: ScopeArgs,
    },
    /// File feedback about a project's tooling into that project.
    Feedback {
        /// The registered project that owns the tooling; its config must have [feedback].
        #[arg(long, add = ArgValueCandidates::new(crate::complete::feedback_owners))]
        project: String,
        summary: String,
        /// friction, gap, idea, or positive.
        #[arg(
            long,
            add = ArgValueCandidates::new(crate::complete::categories),
            add = ValueSet,
            value_parser = ValueSet
        )]
        category: String,
        /// Extra detail, kept verbatim on a new entry; recurrence appends it as a
        /// single-line note, so it must be one line.
        #[arg(short = 'b', long)]
        body: Option<String>,
        /// Append to this open feedback task instead of matching titles.
        #[arg(
            long,
            value_name = "REF",
            conflicts_with = "new",
            add = ArgValueCompleter::new(crate::complete::feedback_recur)
        )]
        recur: Option<String>,
        /// Create a new entry even if a similar one exists.
        #[arg(long)]
        new: bool,
    },
    /// The task hierarchy as nested nodes (open work only unless --all).
    Tree {
        /// One forest per project in scope, so a registry-wide scope and an id conflict.
        #[arg(
            conflicts_with_all = ["all_projects", "group"],
            add = ArgValueCompleter::new(crate::complete::scoped)
        )]
        id: Option<String>,
        #[arg(long)]
        all: bool,
        #[command(flatten)]
        scope: ScopeArgs,
    },
    /// Tag frequencies (open tasks unless --status), per project.
    Tags {
        /// Count over tasks of this status instead (repeatable).
        #[arg(
            long = "status",
            add = ArgValueCandidates::new(crate::complete::statuses),
            add = ValueSet,
            value_parser = ValueSet
        )]
        statuses: Vec<String>,
        #[command(flatten)]
        scope: ScopeArgs,
    },
    /// Work parked waiting for an idle host, as resume briefs.
    Quiet {
        /// At most this many briefs.
        #[arg(short = 'n', long, value_name = "N")]
        limit: Option<usize>,
        /// One registered project instead of all of them.
        #[arg(
            long,
            conflicts_with = "all_projects",
            add = ArgValueCandidates::new(crate::complete::prefixes)
        )]
        project: Option<String>,
        /// The default; accepted for consistency with other read commands.
        #[arg(long)]
        all_projects: bool,
    },
    /// Every claim in the registry's claim stores, with liveness; opens no checkout.
    Claims {
        /// The default and only scope; accepted for consistency with other read commands.
        #[arg(long)]
        all_projects: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum GroupAction {
    /// Create or replace a group; a retired prefix resolves to its live one.
    Set {
        /// Lowercase letters, digits, and -; not a registered or retired prefix.
        name: String,
        #[arg(required = true, add = ArgValueCandidates::new(crate::complete::prefixes))]
        prefixes: Vec<String>,
    },
    /// Delete a group. Its projects stay registered.
    Rm {
        #[arg(add = ArgValueCandidates::new(crate::complete::groups))]
        name: String,
    },
}
