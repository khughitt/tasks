# Shell completions: commands, flags, values, and task ids

Status: designed (2026-09-05)
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
transport argv, `tasks -- tasks -C /path show tasks-`. `complete::words()` drops `argv[0]`,
takes everything after the first `--` — the same split `CompleteEnv::try_complete_` makes —
and returns the user's command line. From it:

- `complete::effective_dir()` scans for `-C <dir>`, `-C<dir>`, or `-C=<dir>` and returns
  the last one, else the process's current directory. This is the starting point for
  every `Project::locate`.
- `complete::selected_project()` scans for `--project <prefix>` / `--project=<prefix>`.
- `complete::subject_id()` returns the first positional that parses as a `TaskId`, which is
  the task an `edit`/`dep` invocation is acting on.

This is a scan of a flat word list, not a second parser: it recognizes exactly the three
forms clap accepts for these two arguments and gives up otherwise. That is sound here
because a miss costs candidates, never correctness — completion cannot write anything, and
§Failure is silence already makes an empty list the failure mode. The word under the
cursor is excluded from the scan so a half-typed `-C` cannot be read as a directory.

## Candidate scopes

Arguments that take an id do not all accept the same ids. Applying one resolver everywhere
would offer candidates the command then rejects. Five scopes, sharing one scan-and-filter
implementation that differs only in which projects it opens and which tasks it keeps:

**`Local`** — the project at the effective directory, and only that one. Used by
`tree <id>` and `list --parent`, which resolve inside a single scan and return
`task_not_found` for anything else (`tree::run`).

**`IdDirected`** — local first, then foreign by typed prefix: the project at the effective
directory, unless the text before `-` in the current value is a valid prefix that is
neither that project's nor unregistered-or-unreachable, in which case the project that
prefix names. This is exactly the write-side rule in §6 of `2026-08-29-tasks-design.md`,
including its precedence: a prefix matching the local project completes from *this*
checkout, not the registered root. Used by `show`, `root`, and every id-taking write —
`edit`, `note`, `start`, `done`, `drop`, `block`, `unblock`, `dep <id>`.

**`Destination`** — the project a new or edited task will land in, which is not always the
local one: `--project <prefix>` if present, else the project of the subject id if the words
carry one (`edit fam-0c3d7e --parent <TAB>` must offer `fam` ids), else the effective
directory's project. Used by `--parent`, because a parent must live in the same project as
its child.

**`Resolvable`** — the effective directory's project plus any registered, reachable
project, since `Resolver::resolve_task` follows a foreign prefix. Used by `--depends` and
`dep --on`. A bare `<TAB>` offers local ids; typing a foreign prefix reaches that project.

**`Dependencies`** — the ids already in the subject task's `depends` list, read from the
words. Used by `dep --rm`, which errors with "does not depend on" for anything else, so
the full id set would be actively misleading.

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
| `tree <id>`, `list --parent` | task ids | `Local` |
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
  makes a menu of hex ids usable in zsh, and costs nothing in bash.

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
- `IdDirected`: a foreign registered prefix offers that project's ids; a prefix equal to
  the local project's completes from the local checkout, not the registered root — asserted
  with two roots sharing a prefix (`init_forced`) holding **different** tasks;
- `-C <dir>`: with two projects holding divergent task sets, `-C <other>` offers the other
  project's ids from a shell sitting in the first. Covered for `-C <dir>` and `-C<dir>`;
- `Destination`: `add --project fam --parent <TAB>` offers `fam` ids and no local ones;
  `edit fam-… --parent <TAB>` likewise;
- `Local`: `tree <TAB>` offers no foreign ids even when a foreign prefix is typed;
- `Dependencies`: `dep <id> --rm <TAB>` offers only that task's current dependencies;
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
