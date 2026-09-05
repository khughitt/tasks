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
`COMPLETE=<shell> tasks -- <words…>` with `_CLAP_COMPLETE_INDEX` naming the word under the
cursor. The binary builds its `Command`, walks the words, and answers with candidates for
whatever argument the cursor is on. Because the answer is computed per keystroke by the
program itself, an argument can attach an `ArgValueCompleter` that reads the filesystem —
which is how ids become completable.

**Chosen: `CompleteEnv`.** It is the only option that satisfies the task, and it keeps one
source of truth: subcommands, flags, fixed value sets, and ids all come from the same
`Cli` definition, so a new flag is completable the moment it is declared. Hand-written
bash and zsh functions were rejected for the opposite reason — two dialects to maintain
that drift from `cli.rs` on every change.

**On `unstable-dynamic`.** The feature gate covers the stub-to-binary wire protocol, which
may change between `clap_complete` minor versions. The exposure is bounded by how the stub
is installed: `source <(COMPLETE=zsh tasks)` in a shell rc regenerates it from the current
binary at every shell start, so an upgrade cannot leave a stale stub talking to a new
binary. Writing the stub to a file on disk would reintroduce that risk and is documented
against.

**Cost of the hook.** `CompleteEnv::complete()` runs first in `main` and returns
immediately unless `COMPLETE` is set in the environment, so an ordinary run pays one
`getenv`. The contract's edge is that a `COMPLETE` variable already exported for another
tool would make every `tasks` call print a stub instead of running; this is the crate's
chosen protocol and is noted in the README rather than worked around.

## Wiring

`Cargo.toml` gains `clap_complete = { version = "4.6", features = ["unstable-dynamic"] }`.

`src/main.rs`, before `Cli::parse()` and before anything writes to stdout:

```rust
clap_complete::CompleteEnv::with_factory(cli::Cli::command).complete();
```

New module `src/complete.rs` holds every candidate source. `src/cli.rs` references them
through `#[arg(add = …)]`, which is the only change to the argument definitions —
completion never alters parsing.

## What completes

| Argument | Source | Mechanism |
|---|---|---|
| subcommands, flag names, `--help` | the `Cli` derive | built in |
| `show`/`edit`/`note`/`start`/`done`/`drop`/`block`/`unblock`/`dep`/`root`/`tree` `<id>` | `tasks/` scan | `ArgValueCompleter` |
| `--parent`, `--depends`, `dep --on`, `dep --rm`, `list --parent`, `feedback --recur` | `tasks/` scan | `ArgValueCompleter` |
| `add --status` | `idea`, `todo` | `ArgValueCandidates` |
| `edit --status`, `list --status`, `tags --status` | `Status::ALL` | `ArgValueCandidates` |
| `--size`, `ready --size` | `Size::ALL` | `ArgValueCandidates` |
| `list --sort` | `priority`, `updated`, `created` | `ArgValueCandidates` |
| `--color` | `auto`, `always`, `never` | `ArgValueCandidates` |
| `feedback --category` | `feedback::CATEGORIES` | `ArgValueCandidates` |
| `add --project`, `unregister <prefix>` | registry keys | `ArgValueCandidates` |

Each fixed set is derived from the constant the parser already validates against —
`Status::ALL`, `Size::ALL`, `CATEGORIES` — so a new status cannot appear in one and not
the other. `add --status` is deliberately narrower than the rest because `add` rejects
anything but `idea` and `todo`.

Not completed: `--tag`, `--spec`, `--plan`, `--step`. Tag and doc-name completion are
worth having but are a second scan with their own scoping questions; they are a follow-up,
not part of this task.

## Task-id completion

`complete::task_ids(current: &OsStr) -> Vec<CompletionCandidate>`:

1. Non-UTF-8 `current` yields nothing.
2. **Choose the project**, local first. Locate the project containing the current
   directory. If `current` contains `-` and the text before it is a valid prefix that is
   neither that project's nor unregistered-or-unreachable, scan the registered project it
   names; otherwise scan the local one. A prefix matching the local project therefore
   completes from this checkout, not the registered root — the same precedence the
   write-side rule in §6 of `2026-08-29-tasks-design.md` gives `-C` and worktrees. The
   foreign branch is what makes `tasks note fam-<TAB>` work from the hub, the workflow
   cross-project writes just opened.
3. **Filter** to ids whose string form starts with `current`.
4. **Order** open tasks before closed ones, each by id. The bash and zsh stubs register
   with `-o nosort`, so this order survives to the menu, and the ids a person is likely to
   want are the ones they can still act on.
5. **Describe** each candidate with `<status>  <title>` as its help text, which zsh shows
   beside the id and bash ignores. This is what makes a menu of hex ids usable.

**Failure is silence.** Any error — no project here, an unreadable `tasks/`, a malformed
task file, a registry that will not parse — yields an empty candidate list. Completion runs
on every TAB with no error channel: a `stderr` write or a non-zero exit corrupts the
prompt, and the bash stub explicitly discards `COMPREPLY` when the completer fails. This is
a deliberate, documented exception to the repo's fail-early rule, confined to
`src/complete.rs`; nothing in this module is reachable from a command path.

## Installation

Bash and zsh, in a shell rc:

```bash
source <(COMPLETE=bash tasks)   # ~/.bashrc
source <(COMPLETE=zsh tasks)    # ~/.zshrc
```

Fish, elvish, and powershell work through the same mechanism and are listed in the README
without being tested here. `COMPLETE=` or `COMPLETE=0` disables. No `tasks completions`
subcommand: it would be a second, staler source of truth for the same script.

## Testing

The stub-to-binary protocol is an ordinary process invocation, so `tests/cli.rs` covers it
end to end like everything else — set `COMPLETE=bash` and `_CLAP_COMPLETE_INDEX`, pass
`-- tasks <words…>`, and read candidates from stdout:

- ids for a partial id, in open-before-closed order, with status and title as help;
- a foreign registered prefix offers that project's ids, not the local ones;
- from outside any project: no candidates, exit 0, empty stderr;
- an unreadable `tasks/` directory: the same silence, not an error;
- `--status` offers six values on `edit` and two on `add`; `--project` offers registry
  prefixes;
- with no words after `--`, the output is the registration stub naming the binary;
- with `COMPLETE` unset, the same argv runs the ordinary command, proving the hook is inert.

## Non-goals

- Completing `--tag`, `--spec`, `--plan`, `--step`.
- Shipping or installing completion files; the rc-sourced stub is the whole integration.
- Completing ids across every registered project at once. A bare `<TAB>` offers the local
  project only; a foreign project is reached by typing its prefix.
