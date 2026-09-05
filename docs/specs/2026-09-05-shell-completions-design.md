# Shell completions: commands, flags, values, and task ids

Status: implemented (2026-09-05)
Task: tasks-ea07fb

## Problem

Nothing completes. Every `tasks` invocation is typed in full, and the two things typed
most often are the two least memorable:

1. **Task ids.** `tasks-4f2a9c` is six random hex digits. The only way to get one is to
   run `list` or `prime`, read it off, and retype or paste it. `tasks show tasks-4f<TAB>`
   is the reported ask.
2. **Hand-parsed value sets.** `--status`, `--size`, `--sort`, `--color`, and
   `feedback --category` are `String` arguments validated after parsing, so a typo is a
   runtime error rather than something the shell could have prevented.

Subcommand and flag completion is the ordinary third of the problem.

## Approach

`clap_complete` offers two unrelated mechanisms, and only one can complete an id.

**`aot`** generates a bash/zsh script from the `clap::Command` at build or install time.
It is stable API, and it structurally cannot complete task ids: the ids live in `tasks/`
and are not known when the script is written.

**`CompleteEnv`** (the crate's `unstable-dynamic` feature) inverts this. The shell script
is a ~20-line stub that, on every TAB, re-invokes the binary as
`TASKS_COMPLETE=<shell> tasks -- <words…>` with `_CLAP_COMPLETE_INDEX` naming the word
under the cursor. The binary builds its `Command`, walks the words, and answers with
candidates for whatever argument the cursor is on. Because the answer is computed per
keystroke by the program itself, an argument can attach an `ArgValueCompleter` that reads
the filesystem — which is how ids become completable.

**Chosen: `CompleteEnv`.** It is the only option that satisfies the task, and it keeps one
source of truth: subcommands, flags, fixed value sets, and ids all come from the same
`Cli` definition, so a new flag is completable the moment it is declared. Hand-written
bash and zsh functions were rejected for the opposite reason — two dialects to maintain
that drift from `cli.rs` on every change.

**The activating variable is `TASKS_COMPLETE`, not `COMPLETE`.** `CompleteEnv::var` sets
it, and the generated stubs export the configured name, so this is one builder call with
no custom protocol. It matters because the default is a bare `COMPLETE`: any such variable
exported for another tool would take over every `tasks` run, and an exported value that
is not a shell name is not a harmless no-op — `completer_for_path` returns an error
listing the supported shells and `complete()` exits on it, so `tasks list` would fail
rather than run. Namespacing removes a whole class of surprise for one line.

**On `unstable-dynamic`.** The feature gate covers the stub-to-binary wire protocol, which
may change between `clap_complete` minor versions. Sourcing the stub from a shell rc
(`source <(TASKS_COMPLETE=zsh tasks)`) regenerates it from the current binary at every
shell start, so a new shell is always self-correcting; writing the stub to a file on disk
would not be, and is documented against. This is not total protection: a shell that was
already running when the binary was upgraded keeps the old function in memory, so the
README says to re-source the stub (or open a new shell) after upgrading `tasks`.

**Cost of the hook.** `CompleteEnv::complete()` runs first in `main` and returns
immediately unless `TASKS_COMPLETE` is set, so an ordinary run pays one `getenv`.

## Wiring

`Cargo.toml` gains `clap_complete = { version = "4.6", features = ["unstable-dynamic"] }`.

`src/main.rs`, before `Cli::parse()` and before anything writes to stdout:

```rust
clap_complete::CompleteEnv::with_factory(cli::Cli::command)
    .var("TASKS_COMPLETE")
    .complete();
```

New module `src/complete.rs` holds every candidate source. `src/cli.rs` references them
through `#[arg(add = …)]`, which is the only change to the argument definitions —
completion never alters parsing.

## Recovering context the completer is not given

`ArgValueCompleter::complete_at` receives the value being completed and nothing else
(`engine/complete.rs`). It does not get the `current_dir` that `CompleteEnv` threads to
the built-in path completers, and it cannot see the other words on the line. Two pieces of
context the candidates depend on therefore have to be recovered by the completer itself:

- **`-C <dir>`**, which selects a different project entirely. Without it,
  `tasks -C <worktree> show tasks-<TAB>` offers the ids of whatever project the shell's
  working directory sits in — the registered main checkout, typically — while the command
  would act on the worktree. Divergent task sets between the two make this visibly wrong.
- **`--project <prefix>`**, which selects the destination of `add`.

Both are available: the completing process's own `std::env::args_os()` is the full
transport argv, `tasks -- tasks -C /path show tasks-`. `complete::line()` drops `argv[0]`
and takes everything after the first `--` — the same split `CompleteEnv::try_complete_`
makes — returning the user's command line, minus the word under the cursor so a half-typed
token is never read as context.

Reading that line requires walking it the way clap does, not grepping it. A flat "first
token that looks like an id" scan is wrong twice over: it reads the title in
`tasks add fam-000001 --parent <TAB>` as a subject and selects `fam` instead of the local
project, and it reads the value in `tasks edit --body fam-000001 tasks-abcdef --parent
<TAB>` as the subject instead of `tasks-abcdef`. So `complete::line()` classifies each
word and returns a `Line` struct:

1. The first non-option word is the **subcommand**. Look it up in `Cli::command()`; an
   unrecognized one ends the walk with no context.
2. After a `--` the user typed, every remaining word is positional.
3. A word starting with `-` is an **option**. `--long=value` and `-Cvalue` carry their own
   value. Otherwise, whether it consumes following words comes from the subcommand's own
   `Arg`: `get_action().takes_values()`, and `get_num_args()` for how many — an option
   declared `num_args = 1..`, as `dep --on` and `dep --rm` are, greedily consumes every
   following non-option word.
4. Anything else is a **positional**, in declaration order.

From that walk, `Line` carries:

- `dir` — the last `-C` value, else `None` (the calling process's current directory serves
  as the fallback). The starting point for every `Project::locate`.
- `project` — the `--project` value, else `None`.
- `subject` — the **first positional of a subcommand whose first positional is an id**:
  `show`, `root`, `tree`, `edit`, `note`, `start`, `done`, `drop`, `block`, `unblock`,
  `dep`. For any other subcommand — `add`, whose first positional is a title — `subject`
  is `None`, and callers fall back rather than guess.
- `all_projects` — whether `--all-projects` is present.

**Ambiguity yields no candidates.** An unrecognized subcommand, a first positional that
does not parse as a `TaskId` where one is expected, a `-C` whose value is the word being
completed: each ends the walk with nothing to offer. Read-only execution keeps a wrong
guess from mutating anything, but that is not the standard — offering an id the command
will reject is a defect in this feature's own contract, so the walk is exact where it can
be and silent where it cannot.

## Candidate scopes

Arguments that take an id do not all accept the same ids. Applying one resolver everywhere
would offer candidates the command then rejects. Six scopes, sharing one scan-and-filter
implementation that differs only in which projects it opens and which tasks it keeps:

A missing local project is never an error in any scope: it simply contributes no local
candidates, leaving whatever the typed prefix reaches. That is what lets `tasks root
fam-<TAB>` and `tasks add --project fam --depends fam-<TAB>` work from outside every
project, as both commands do.

**`Scoped`** — the project at the effective directory, and only that one, unless
`--all-projects` is among the words, in which case every reachable registered project.
Used by `tree <id>` and `list --parent`, which both validate against the scope's own scan
and return `task_not_found` for anything outside it (`tree.rs`, `list.rs`). The
`--all-projects` upgrade matters only for `list`: `tree` declares `<id>` in conflict with
the flag, so a `tree` id is always local.

**`IdDirected`** — local first, then foreign by typed prefix: the project at the effective
directory, unless the text before `-` in the current value is a valid prefix that is
neither that project's nor unregistered-or-unreachable, in which case the project that
prefix names. This is exactly the write-side rule in §6 of `2026-08-29-tasks-design.md`,
including its precedence: a prefix matching the local project completes from *this*
checkout, not the registered root. Used by `show` and every id-taking write — `edit`,
`note`, `start`, `done`, `drop`, `block`, `unblock`, `dep <id>` — and by `root`, which
resolves purely through the registry, so its local half is a convenience and its foreign
half is the whole point.

**`Destination`** — the project a new or edited task will land in, which is not always the
local one: `--project <prefix>` if present, else the project of the subject id if the
subcommand has one (`edit fam-0c3d7e --parent <TAB>` must offer `fam` ids), else the
effective directory's project. Used by `--parent`, because a parent must live in the same
project as its child.

**`Resolvable`** — `Destination`, plus any registered, reachable project reached by a typed
prefix. Used by `--depends` and `dep --on`. The base is the destination and *not* the
effective directory because that is how the command validates: `apply_fields` and
`dep::run` both build `Resolver::new(&ctx.project, …)` on the project being written to. So
from a `fam` worktree, `tasks add --project fam --depends fam-<TAB>` must offer the
registered `fam` root's ids — the worktree's own are exactly the ones the command would
reject.

**`Dependencies`** — the ids already in the subject task's `depends` list, read from the
words. Used by `dep --rm`, which errors with "does not depend on" for anything else, so
the full id set would be actively misleading. Removal does not resolve ids, so a dangling
or unreachable dependency is still removable and stays a candidate; the shared presentation
path must therefore tolerate a candidate whose task cannot be read, emitting the id with no
description rather than dropping it.

**`UpstreamFeedback`** — the project registered as `tasks`, opened through the registry and
never from the local directory, filtered to open tasks tagged `feedback`. Used by
`feedback --recur`, which resolves through `feedback::locate_target` and rejects anything
`is_open_feedback` refuses. Reading the local directory here would suggest, from a `tasks`
worktree, records that do not exist upstream.

## What completes

| Argument | Candidates | Scope |
|---|---|---|
| subcommands, flag names, `--help` | the `Cli` derive | built in |
| `show`/`root` `<id>` | task ids | `IdDirected` |
| `edit`/`note`/`start`/`done`/`drop`/`block`/`unblock`/`dep` `<id>` | task ids | `IdDirected` |
| `tree <id>`, `list --parent` | task ids | `Scoped` |
| `--parent` | task ids | `Destination` |
| `--depends`, `dep --on` | task ids | `Resolvable` |
| `dep --rm` | task ids | `Dependencies` |
| `feedback --recur` | task ids | `UpstreamFeedback` |
| `add --status` | `idea`, `todo` | fixed |
| `edit --status`, `list --status`, `tags --status` | `Status::ALL` | fixed |
| `--size`, `ready --size` | `Size::ALL` | fixed |
| `list --sort` | `priority`, `updated`, `created` | fixed |
| `--color` | `auto`, `always`, `never` | fixed |
| `feedback --category` | `feedback::CATEGORIES` | fixed |
| `add --project`, `unregister <prefix>` | registry keys | registry |

Each fixed set is derived from the constant the parser already validates against —
`Status::ALL`, `Size::ALL`, `CATEGORIES` — so a new status cannot appear in one and not
the other. `add --status` is deliberately narrower than the rest because `add` rejects
anything but `idea` and `todo`.

Not completed: `--tag`, `--spec`, `--plan`, `--step`. Tag and doc-name completion are
worth having but are a second scan with their own scoping questions; they are a follow-up,
not part of this task.

## Presenting ids

Within its scope, a task-id completer filters to ids whose string form starts with the
current value, then:

- **orders** open tasks before closed ones, each by id. The bash and zsh stubs register
  with `-o nosort` / `_describe`, so this order survives to the menu and the ids a person
  can still act on come first;
- **describes** each candidate with `<status>  <title>` as help. Zsh emits `value:help`
  and shows it beside the id; bash's adapter writes values only and drops it. This is what
  makes a menu of hex ids usable in zsh, and costs nothing in bash. A candidate whose task
  cannot be read — the dangling dependencies `Dependencies` must still offer — is emitted
  bare, with no description.

**Failure is silence.** Any error — no project at the effective directory, an unreadable
`tasks/`, a malformed task file, a registry that will not parse — yields an empty candidate
list. Completion runs on every TAB with no error channel: a `stderr` write or a non-zero
exit corrupts the prompt, and the bash stub explicitly discards `COMPREPLY` when the
completer fails. This is a deliberate, documented exception to the repo's fail-early rule,
confined to `src/complete.rs`; nothing in this module is reachable from a command path.

## Installation

Bash, in `~/.bashrc`:

```bash
source <(TASKS_COMPLETE=bash tasks)
```

Zsh, in `~/.zshrc`, **after** completion is initialized — the stub begins `#compdef` and
calls `compdef`/`_describe`, so sourcing it before `compinit` fails with
`command not found: compdef`:

```zsh
autoload -Uz compinit && compinit      # or the framework's own init
source <(TASKS_COMPLETE=zsh tasks)
```

Under a framework that runs `compinit` for you (oh-my-zsh, prezto, a plugin manager), the
same rule applies: the stub goes after that framework's initialization, not before it.

Re-source the stub or start a new shell after upgrading `tasks`; a running shell keeps the
function it loaded at startup. Fish, elvish, and powershell work through the same mechanism
and are listed in the README without being tested here. `TASKS_COMPLETE=` or
`TASKS_COMPLETE=0` disables. No `tasks completions` subcommand: it would be a second,
staler source of truth for the same script.

## Testing

The stub-to-binary protocol is an ordinary process invocation, so `tests/cli.rs` covers it
end to end like everything else — set `TASKS_COMPLETE` and `_CLAP_COMPLETE_INDEX`, pass
`-- tasks <words…>`, and read candidates from stdout. Shell choice is part of the fixture:
**descriptions are asserted under `TASKS_COMPLETE=zsh`**, since bash's adapter emits bare
values.

- ids for a partial id, open-before-closed, and under zsh each carrying status and title;
- `root fam-<TAB>` from outside every project offers `fam`'s ids;
- `IdDirected`: a foreign registered prefix offers that project's ids; a prefix equal to
  the local project's completes from the local checkout, not the registered root — asserted
  with two roots sharing a prefix (`init_forced`) holding **different** tasks;
- `-C <dir>`: with two projects holding divergent task sets, `-C <other>` offers the other
  project's ids from a shell sitting in the first. Covered for `-C <dir>` and `-C<dir>`;
- `Destination`: `add --project fam --parent <TAB>` offers `fam` ids and no local ones;
  `edit fam-… --parent <TAB>` likewise;
- the walk: `add fam-000001 --parent <TAB>` offers *local* ids, because `add`'s first
  positional is a title and not a subject; `edit --body fam-000001 tasks-… --parent <TAB>`
  offers the subject's project, because `--body` consumes its value; `dep <id> --on a b
  --rm <TAB>` still finds the subject past a `num_args = 1..` option; an unrecognized
  subcommand offers nothing;
- `Scoped`: `tree <TAB>` offers no foreign ids even when a foreign prefix is typed;
  `list --parent <TAB>` offers local ids only, and `list --all-projects --parent <TAB>`
  offers every reachable project's;
- `Resolvable`: from a `fam` worktree whose tasks differ from the registered `fam` root,
  `add --project fam --depends fam-<TAB>` offers the registered root's ids, not the
  worktree's; and the same command offers them from outside any project at all;
- `Dependencies`: `dep <id> --rm <TAB>` offers only that task's current dependencies, and
  a dependency whose project is unregistered is still offered, without a description;
- `UpstreamFeedback`: `feedback --recur <TAB>` offers only open `feedback`-tagged tasks
  from the registered `tasks` root, and does not offer a task that exists only in a local
  worktree of it;
- fixed sets: `--status` offers six values on `edit` and two on `add`; `--project` offers
  registry prefixes;
- silence: from outside any project, and with an unreadable `tasks/` directory, a malformed
  task file, and a malformed registry — no candidates, exit 0, empty stderr in each case;
- with no words after `--`, the output is the registration stub naming the binary;
- inertness: with `TASKS_COMPLETE` unset, an **ordinary** argv (`tasks list`, not the
  transport form) runs the command as before. The transport argv is not a valid command
  line, so unsetting the variable on it would prove nothing.

## Non-goals

- Completing `--tag`, `--spec`, `--plan`, `--step`.
- Shipping or installing completion files; the rc-sourced stub is the whole integration.
- Completing ids across every registered project at once. A bare `<TAB>` offers the scope's
  project; a foreign project is reached by typing its prefix, where the scope allows one.
