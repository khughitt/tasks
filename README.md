# tasks

A fast, file-based task tracker for software projects, built for coding agents and the
humans supervising them. One markdown file per task, checked into the project; a single
Rust binary as the only writer. JSON output by default.

JSON task records omit unset optional fields and empty task collections (`tags`,
`depends`, `notes`, `depends_on`, `children`). Missing means unset or empty; consumers
must not require those keys. Values such as `false`, `0`, and `body: ""` remain.
Response containers such as `tasks: []` and `warnings: []` remain, as does `next: null`
when nothing is eligible. See
[the output contract](docs/specs/2026-08-29-tasks-design.md#51-output-contract).

Design: [`docs/specs/2026-08-29-tasks-design.md`](docs/specs/2026-08-29-tasks-design.md).
Implementation plan: [`docs/plans/2026-08-29-tasks.md`](docs/plans/2026-08-29-tasks.md).

A terminal front end, [tasks-tui](https://github.com/khughitt/tasks-tui), sits on top of
the CLI: it shells out to `tasks`, reads the JSON, and launches an agent on a task in a
new terminal. The CLI is the whole contract; the TUI never touches `tasks/*.md`.

## For agents

If you have been asked to install `tasks` or adopt it in a project, do these in order.
Each step is idempotent.

1. Install the binary and the skill (once per machine; needs a Rust toolchain):

       git clone https://github.com/khughitt/tasks ~/.local/src/tasks
       cargo install --locked --path ~/.local/src/tasks
       just --justfile ~/.local/src/tasks/justfile install-skills

   `install-skills` symlinks every `skills/*` directory into `~/.claude/skills` and
   `~/.agents/skills` (the latter for other harnesses). Without `just`:
   `ln -sfn ~/.local/src/tasks/skills/tasks ~/.claude/skills/tasks`.

2. Initialise the project (once per repository):

       cd <repo>
       tasks init --prefix <p>          # p: a letter then 1-7 letters/digits, e.g. sci

   If the project keeps specs or plans outside `docs/specs/` and `docs/plans/`, see
   "Adopting in an existing project" below before running `init`.

3. Add this to the project's `AGENTS.md` or `CLAUDE.md`:

       Work is tracked with `tasks` (see ~/.claude/skills/tasks/SKILL.md). Start every
       session with `tasks prime`, `tasks start <id>` before changing code, `tasks done <id>`
       in the same commit as the code, and `tasks check` before every commit.

       Adopt the process and workspace policy in the tasks skill: the task's process
       field, not generic Superpowers triggers, decides whether brainstorming runs.
       Direct executes scoped work without brainstorming or new design/plan documents;
       planned requires user review of a written spec, then a written implementation
       plan, before implementation. Assess missing process explicitly before coding.
       Both code paths use an isolated task worktree, reusing one on resume or creating
       one with git worktree add under .worktrees/; run just setup when defined.
       Planned work creates it before drafting the spec. Explicit user overrides win.

4. Work:

       tasks prime                      # counts, lanes, ready list, who is doing what
       tasks ready                      # what can be worked on now
       tasks tree                       # the goal hierarchy
       tasks start <id>                 # claim it
       tasks note <id> "<one line>"     # when scope or understanding changes
       tasks shelve <id> "<wake condition>" # keep open work out of active views
       tasks unshelve <id>              # bring it back as an idea
       tasks done <id> "<what landed>"  # in the same commit as the code
       tasks check                      # before every commit
       tasks feedback --project tasks "<about the tool>" --category friction   # file friction with this tool

Never edit `tasks/*.md` by hand. `tasks --help` lists every command and `tasks help <command>`
one command's; add `--pretty` to any command, before or after its name, for human-readable
output (`--json` is the default, spelled out). A usage error — an unknown command or option,
a missing argument, a value outside a closed set — exits 2 with the problem and the usage
line on stderr; a command that ran and failed exits 1 with one JSON error object there.

Task ids on the command line accept trailing sentence punctuation: one or more trailing
ASCII periods on any positional id or id-valued flag (`--parent`, `--depends`, `dep
--on/--rm`, `feedback --recur`) are stripped before validation and alias resolution, so an
id copied from the end of a sentence works. Embedded dots, other punctuation, and leading
or trailing whitespace are not accepted; records on disk and every output stay canonical.

`start` also writes a per-project claim outside git, so every worktree sees the session and
its liveness. `ready` and `next` omit live claims with an explanatory warning. When concrete native identity is unavailable, set
`TASKS_SESSION` per agent sharing a terminal or harness process; use
`tasks start --force <id>` for an explicit, recorded takeover. A write refuses with
`stale_copy` when another worktree holds a newer copy of the record, and says whether to
rerun there, merge, or leave it to the session working there.
An open `halt` task in the registered checkout blocks new lower-priority starts across
worktrees. `prime`, `ready`, and `next` show the halt and eligible work. An audited
override is `tasks start <id> --force --reason "<one-line explanation>"`; its free-text
reason is recorded on the halt and target, unlike park's fixed-list `--reason`.

An exclusive need (`exclusive = true` under `[needs]`) is held by the live claim of the
task that needs it, across every registered project on the host. Other sessions' steps
that need it leave `ready`, `next` and `prime` with a warning naming the holder, and `start`
refuses them with `need_held`. `tasks start <id> --force --reason "…"` overrides with a
note on the task, and on the holder when it is in the same project. A park or a dead
session releases the hold.

Lifecycle notes automatically record native harness provenance, independently of
claim identity and liveness. A stamped note has one indented JSON continuation:

```markdown
- 2026-09-17T20:00:00Z (worker): started
  provenance: {"harness_session":"codex:example","harness_session_source":"CODEX_SESSION_ID"}
```

`tasks show` exposes these optional fields directly on the note; both are absent
when unknown. The source names only `CLAUDE_CODE_SESSION_ID`, `CODEX_SESSION_ID`,
or `CODEX_THREAD_ID`, with a matching `claude-code:` or `codex:` key. Codex
selects `CODEX_THREAD_ID` whenever present, including equal root/thread IDs;
otherwise it selects `CODEX_SESSION_ID`. Unequal valid IDs name a worker and its
shared root, and produce worker provenance without changing the serialized pair.
Historical stamps retain their recorded source. Missing/empty native values leave both fields
absent; conflicting or invalid inputs also leave them absent and emit a warning,
without blocking an otherwise valid lifecycle operation. A qualified `TASKS_SESSION`
override that disagrees with the native key is a conflict; a matching override never
replaces the native source. Other claim overrides remain independent. Do not set
`TASKS_SESSION` merely to improve observability: it is a claim override, not a
provenance source. Claim identity reads the same Codex variables on its own: a Codex
claim validates both native values and selects the thread when present, carries
no pid, and lives by the four-hour TTL. Malformed selected Codex input refuses
claim writes with repair or exact held `TASKS_SESSION` recovery guidance. With
only `CODEX_SESSION_ID`, workers cannot be distinguished from their controller.

Codex claims expose optional `root_session` in `show`, `prime` and `claims` and
persist it in the claim store as `codex:<root>`. It names the tree root, never an
immediate parent, and grants no ownership or shared holds. Native Codex sets it
from validated `CODEX_SESSION_ID`; explicit overrides and other harnesses omit it.
A same-owner refresh without root data retains the recorded root; conflicting
known roots refuse before mutation, including same-owner `start --force`. Inspect
`tasks show` and the exports, restore incorrect exports, or intentionally release
with `park` before starting under the new root. Takeovers record the new owner's
root, including absence. Release removes the metadata; notes and parks gain no
root field. Old claims still load, but older writers may discard this field when
rewriting a store; participating sessions need the updated binary for retention.

The generated lifecycle texts are the consumer contract (including status edits):

- `started` on first start; `resumed` on subsequent starts, including owner refresh.
- `parked (waiting on …): <next step>`, using the existing park reason vocabulary.
- `done` or `dropped` on a genuine close transition.
- `completed; next due <YYYY-MM-DD>` instead of `done` for a recurring completion.

These notes persist after claims and parks are released. Close-message notes are also
stamped, but user messages are **not lifecycle markers**. Consumers such as obs derive
transition kinds from generated texts, not simply from the presence of provenance.
There is no serialized marker kind: arbitrary user text can equal a generated text
(for example, a close message of `resumed`). Text matching alone cannot disambiguate
that collision; consumers must retain that uncertainty.
Plain `tasks note`, shelf notes, feedback and takeover commentary remain unstamped.
`tasks note <id> "<text>" --stamp` opts one note in: it carries the same pair under the
same rules (a missing source leaves it off; a conflict warns without values and the note
still lands), and changes nothing else about `note`. Its text is not interpreted, so a
stamped note is no more a lifecycle marker than a close message is; flow's gate notes
use it to name the session at each stage boundary.

Binaries older than the provenance writer reject a stamped file outright, so every host
that reads synced task files needs a current `tasks`. The contract and its rollout are
recorded in `docs/plans/2026-09-17-lifecycle-provenance.md`.

`TASKS_MODEL` per harness process records which model completed each task: a fresh
`done` stamps the record's `model` field from it (and clears the stamp when it is
unset); correct a wrong stamp with `tasks edit --model`/`--no-model`.
`TASKS_AGENT` per harness process records which harness and model filed each task
(`<harness>/<model>`, or the harness alone): `add` and `feedback` stamp the record's
`agent` field from it, `add --agent` overrides it, and `tasks edit --agent`/`--no-agent`
corrects a stamp. The harness owns exporting it: Claude Code is wired through the ops
`claude-provenance` hook (session start and model switch, so the stamp follows
`/model`), Codex exports the harness alone, and other harnesses pass `--agent` when
they know their ids. Design:
`docs/specs/2026-09-13-creation-provenance-design.md`.
`TASKS_MAX_COMPLEXITY` per harness process is the envelope a session picks within:
`ready`, `next`, and `prime` hide tasks rated above it and unassessed tasks, and say how
many. `--max-complexity` on `ready`/`next` overrides it for one call. Design:
`docs/specs/2026-09-12-task-complexity-design.md`.
`TASKS_WITHOUT` (comma-separated need names) is the other half of a session's envelope:
`ready`, `next`, and `prime` hide tasks needing any of them, in the projects that declare
the name, and say how many.
`--without` adds to it for one call and refuses a name no project in scope declares; the
variable never errors, so a host in ordinary use can set `TASKS_WITHOUT=quiet` once for
every project. Needs are declared per project in `[needs]` (see the design's §6). Design:
`docs/specs/2026-10-03-lanes-needs-groups-design.md`.
`park` sets a task down with its next step in the same store, who it waits on, and
optionally why:

| reason        | The work stopped because…                                                        |
|---------------|----------------------------------------------------------------------------------|
| `review`      | an artifact the user must inspect and judge: art sheets, screenshots, a document read |
| `decision`    | a decision only the user can make — scope, taste, priority                       |
| `approval`    | the agent holds a recommendation and wants it confirmed                          |
| `environment` | the checkout or machine cannot run the work — missing deps, a restart, a TTY     |
| `dependency`  | another task or project must land first                                          |
| `session`     | the session ended before the work did — context exhausted, time, crash           |
| `capability`  | the work needs more reasoning than this session can supply; `--complexity` raises the rating |
| `quiet`       | the host is in use; the work is prepared, unattended, and needs only an idle machine — `--minutes <n>` (required) and `--needs idle\|headless` record the recipe |

`start` resumes it, and `prime` lists parked work first. Every record also carries two
stamps written only by status changes: `started`, the first time work began, and
`completed`, the latest completion (cleared by a reopen). Design:
`docs/specs/2026-09-11-park-reason-and-stamps-design.md`.
`tasks quiet` lists quiet parks across every registered project as resume briefs (design:
`docs/specs/2026-09-13-quiet-queue-design.md`); the ordinary pretty tables (`list`,
`prime`, `ready`) mark a quiet park's row with its recipe, such as
`waits on user, quiet; idle, 40 min`. A quiet park's next step lists the run's
phases and any refusal or hang an earlier attempt of the recipe hit. Every attempt then ends
with one note in a fixed form, so the estimate can be compared with what happened:

    run: <actual> min (est <n>, <needs>); <phase> <m>, …; <outcome>[: <cause>]

`<outcome>` is `passed`, `failed`, `refused`, `hung`, or `aborted`. A refused attempt is
timed up to the refusal and gets its note too.

Statuses are `idea`, `todo`, `doing`, `blocked`, `shelved`, `done`, and `dropped`.
`shelved` is open but hidden; use `tasks shelve <id> "<wake condition>"` and
`tasks unshelve <id>` to return it as an idea.

### Relay identity (opt-in)

A host that runs the relay agent registry can key claims by the harness
session itself rather than by the terminal. It is off by default; turn it on in
`~/.config/tasks/config.toml` (or `$XDG_CONFIG_HOME/tasks/config.toml`):

```toml
[identity]
relay = true
```

The file is host-local on purpose. Whether relay runs is a property of a machine, while the
per-project `tasks/.config.toml` syncs between hosts, so the switch does not belong there.

Turn it on only where relay's hooks actually publish its agent registry. Every `tasks` command
run under a harness is in scope once the switch is on, and without the registry every
claim acquisition there refuses. Relay records the handle of the process that is the
session, so headless, nested and background sessions (`claude -p`, `claude --bg`,
`codex exec`) each claim as themselves.

With it on, `tasks` walks its own process ancestry to the nearest harness process —
`claude`, `codex` or `opencode`, or a versioned Claude Code binary under claude/versions/,
which is how a background session runs; Claude Code's own daemon and pty host are not
sessions and are refused — and looks that process up in relay's agent registry, matching on
host, boot id, pid and process start time, and requiring the registry's harness to agree
with the process it found. On a match the claim is keyed by the relay agent id —
`<harness>:<sessionId>`, such as `codex:01J…` — and records that agent's process handle as
the claim's proof. Because the id belongs to the session rather than to the terminal, a
claim survives the command that made it and is recognized again by the same session later.

That proof is also what the session's own commands read back: the owner can `park` or
`done` a claim it already holds even when the registry has gone away, because ownership is
re-derived from what the claim itself recorded. Two limits follow from the same rule.
Parking *releases* the claim, so resuming a parked task is a fresh acquisition and needs
either the registry back or `TASKS_SESSION`. And from 0.2.0, a session nested inside
another harness session is a different session: its nearest boundary is its own harness
process, so it cannot act on the outer session's claims.

Codex hints validate both native exports and select `CODEX_THREAD_ID` when present,
otherwise `CODEX_SESSION_ID`. A matching Codex actor row records native root
association separately. A worker presented only with its registered root row
refuses acquisition: relay worker registration remains unavailable (`relay-f56e30`).
It cannot claim as its controller; other mismatches use a value-free diagnosis.
No registry schema or producer behavior changes here.

Codex proof-only continuation now requires a valid concrete `CODEX_THREAD_ID`
matching the held claim, plus the existing process, host and boot evidence.
A sibling, absent thread, session-only export, or malformed native value cannot
prove ownership. This deliberately removes the former thread-less process-proof
fallback. Exact resolved identity matches still work. A legitimate owner blocked
by missing thread data can inspect `tasks show <id>` and set `TASKS_SESSION` to
that exact held session; set `TASKS_SESSION_PID` only for an actual long-lived
process, never the command PID. Parentage and root association do not authorize
foreign claims. Normal workers release with `done` or `park --waiting-on agent`
before reporting completion; controllers run `start` before review or closure.
Agent-waiting parks are public to all agents. A controller uses forced takeover
only after a worker-specific completion result establishes that worker and its
checks have stopped; silence, a timeout and the shared PID do not establish it.

Relay process matching also requires hooks and task commands to share a PID
namespace. A sandbox exposing the Codex ancestor as PID 1 cannot match relay's
published host PID; this currently refuses acquisition (tracked as tasks-b9cbe3).
Exact legitimate-owner `TASKS_SESSION` recovery remains available. The live
acceptance capture used a separately approved host-PID invocation.

**Upgrading.** tasks 0.2.0 reads relay's registry schema 2 and no other; against an older
registry it refuses and asks you to run `relay reap`. On a host with this switch on, every
`tasks` that can run there must be 0.2.0 or later before relay publishes schema 2: check
`tasks --version` for each `tasks` on `PATH` in each harness's environment. An older `tasks`
refuses fresh claims against the new registry, but it can still let a session nested inside
another continue the outer session's claims, which no registry format can prevent.

Enabling relay never rewrites a claim already held: a claim keeps the identity it was
created with. The level is Linux-only — it needs `/proc` — and it sits *below*
`TASKS_SESSION` in the ladder, which is why every relay identity error ends by telling you
to set `TASKS_SESSION` and `TASKS_SESSION_PID`. That override is the recovery path from all
of them, and distinguishes agents sharing a process when native thread identity is
unavailable. It never supplies native note provenance.

## Install

From a checkout (`--locked` installs the checked dependency versions):

    cargo install --locked --path .

Without a checkout (binary only; the skill still needs the `skills/tasks` directory
from a clone):

    cargo install --git https://github.com/khughitt/tasks

## Use

    cd <repo>
    tasks init --prefix sci          # creates tasks/ and the doc roots; registers the project
    tasks init --prefix sci --force  # re-point the prefix here after moving the repo
    tasks unregister sci             # drop a stale prefix and its aliases; files are untouched
    tasks retire --successor mind6 --reason "superseded"  # declare this project over; unretire undoes it
    tasks rename dot dots            # rename a registered prefix; old ids still resolve
    tasks rename dot dots --explain  # diagnose an interruption without locks or writes
    tasks add "Bank the ledger" -p 1 --size m --complexity low --process direct --tag ledger --agent codex/gpt-6
    tasks add "Emit rows" --parent sci-4f2a9c
    tasks edit <id> --process planned # choose a workflow explicitly
    tasks edit <id> --no-process     # clear it to unassessed
    tasks add "Curation sweep" --every 30d  # days or weeks since each completion
    tasks list --periodic            # recurring tasks, soonest due first
    tasks edit <sweep-id> --every 2w # change the cadence; --no-every clears it and its anchor
    tasks add "Revisit the cache idea" --status idea --defer 60d  # hidden from the pickers until then
    tasks list --deferred            # deferred tasks, soonest first; due ones lead
    tasks edit <id> --no-defer       # clear it; any status change clears it too
    tasks add "Reply to Dana" --source "mail:<42@example.org>"  # where it came from; never interpreted
    tasks add "Reply to Dana" --source "mail:<42@example.org>"  # again: reuses the id, writes nothing
    tasks list --source "mail:<42@example.org>"  # what came from this reference (exact match)
    tasks list -p 0 -p 1 --size s --size xs   # P0 or P1, and small or extra-small
    tasks search "claim identity" --all-projects  # title or body contains the phrase; every list option applies
    tasks ready --complexity none             # ready work nobody has rated yet
    tasks ready                      # what can be worked on now (JSON)
    tasks ready --parallel -n 3      # up to 3 candidates marked safe to dispatch together
    tasks add "Capture lane" --lane -p 1 -b "Why it runs; first milestone"  # an effort beside the others
    tasks edit <id> --no-lane        # clear the lane marker
    tasks lanes                      # each lane: guidance, state, and the step it could take now
    tasks next --under sci-4f2a9c    # the next step inside one lane, at any depth
    tasks list --under sci-4f2a9c    # a goal's whole subtree (--parent is direct children)
    tasks block sci-4f2a9c "host busy"  # pause a lane; tasks unblock resumes it
    tasks add "Capture the trace" --need quiet  # a shared resource from [needs] in tasks/.config.toml
    tasks edit <id> --rm-need quiet  # --need adds; --rm-need/--no-needs remove
    tasks list --need quiet          # tasks that need it (all-of, like --tag)
    tasks ready --without quiet      # hide steps this session cannot meet; also TASKS_WITHOUT
    tasks sample --limit 3           # random open tasks for a curation pass (see skills/curate)
    tasks tree                       # the goal hierarchy
    tasks next                       # parked work waiting on you, else the first ready task
    tasks park <id> "next step" --reason review   # set it down; tasks list --parked to see what is parked
    tasks park <id> "rerun the preflight" --reason quiet --waiting-on user --minutes 50  # needs an idle host
    tasks quiet                      # what could run tonight, across every project; -n 1 for the top
    tasks list --parked --waiting-on user --all-projects  # everything waiting on you, with its next step
    tasks --pretty --headers list     # label each column of a pretty table
    tasks claims                     # every claim in every project's store, live or stale; what a Stop hook reads
    tasks shelve <id> "when the dependency lands" # keep open work out of active views
    tasks unshelve <id>             # return shelved work to idea
    tasks next --all-projects        # the same across every registered project
    tasks prime --project fam        # read another registered project; also list, ready,
                                     #   next, tree, tags, sample. Needs no local project.
    tasks projects                   # the registry: reachable? counts?
    tasks group set vf nodes atoms   # a named set of registered projects (this host only)
    tasks ready --group vf           # read a group; also list, next, prime, tree, tags,
                                     #   sample, lanes, quiet. Needs no local project.
    tasks groups                     # each group, its members, and whether they are reachable
    tasks group rm vf                # delete the group; its projects stay registered
    tasks add "Piece" --project fam  # create in another registered project
    tasks note fam-0c3d7e "…"        # id-taking commands follow the prefix to its project
    tasks show 4f2a9c                # a bare suffix means the current project's task (sci-4f2a9c)
    tasks edit sci-4f2a9c --tag cli --rm-tag triage  # --tag adds; --rm-tag/--no-tags remove
    tasks edit sci-4f2a9c --depends sci-7b1e04       # --depends adds; --no-depends clears
    tasks dep sci-4f2a9c --on sci-7b1e04 --rm sci-0c3d7e  # swap a dependency in one save
    tasks --pretty ready             # same, as a table (or export TASKS_FORMAT=pretty)
    tasks --pretty --color auto ready # color when stdout is a terminal
    tasks start sci-4f2a9c
    tasks start sci-4f2a9c --force --reason "restore the service"  # audited halt or held-need override
    tasks note sci-4f2a9c "spec §4 no longer holds"
    tasks attach sci-4f2a9c ~/Pictures/before.png --caption "the stale row"  # copy into tasks/files/<id>/
    tasks attach sci-4f2a9c --clipboard       # an image from wl-paste; --name to choose the name
    tasks detach sci-4f2a9c before.png "wrong screenshot"  # recorded; uncommitted bytes are gone
    tasks done sci-91be03 "rows emitted"
    tasks done sci-4f2a9c "landed in 1a2b3c"  # open-work rule: closes once its child is closed
    tasks check                      # validate files, links, plan steps, dependencies; silent when clean

Commands targeting an existing id, including `show`, `tree <id>`, `note`, and `start`,
work from outside every project: the prefix selects the registered root. Inside a
project, a matching prefix keeps the current checkout, including a worktree selected
with `-C`. `feedback` still needs a local project for its provenance tag.

A recurring task closes normally: `done` records the completion and anchors its next
cycle. When due, it appears in `ready` with status `done`; use `start` before completing
the next occurrence. Early runs are allowed. `--every` accepts positive whole days or
weeks (for example, `30d` or `2w`), up to 36500 days; goals cannot recur.

Attachments live in `tasks/files/<id>/`, one directory per task, committed with the
record. Each file is capped at 2 MiB unless `tasks/.config.toml` sets
`[attachments] max_bytes`. A task's own `attached: <name> (<n> bytes)` and
`detached: <name>: <why>` notes are the ledger of which files it owns. `show` lists
the files with absolute paths. `check` reports these kinds:

- errors: `attachment_unsafe`, `attachment_orphan`, `attachment_unnoted`, and
  `attachment_missing`;
- warnings: `attachment_invalid` and `attachment_too_large`.

`tasks/files/<id>` and its parent must be real directories, not symlinks.
Binaries older than attachments ignore tasks/files/ entirely. `feedback` never
carries attachments.

An interrupted `attach` leaves a file that `check` reports as `attachment_unnoted`.
Rerun it with the same bytes and `--name <name>` to finish, or `detach` the file. An
interrupted `detach` finishes when rerun.

Run `tasks --help` for the full command list.

The registry maps live prefixes to project roots and retired prefixes to their current
live name. `tasks rename <old> <new>` updates the project's filenames, ids, local
`depends`/`parent` references, config, and registry. References in other projects and in
prose need no edits: retired names keep resolving **for as long as the project stays
registered**. `unregister` removes that project's aliases too; retired names cannot be
reused while registered.

A project that is over declares it in its own `tasks/.config.toml` with `tasks retire
[--successor <prefix>] --reason "<one line>"`, run in its registered checkout. It refuses
while any task is open in any checkout, while a claim is live, or while a linked worktree
cannot be read, and writes a `[retired]` table (`date`, `reason`, optional `successor`).
From then on every write to the project is refused with `retired_project`, from every
checkout, a worktree made before the retirement included; its ids keep resolving and
satisfy other projects' dependencies; `--all-projects` and `--group` reads leave it out,
while `--project` reads and `show` warn; `tasks projects` keeps its row with `retired`;
`group set` refuses it as a member; `init --force` and `rename` refuse it. `tasks
unretire` in the registered checkout removes the table. The project lifecycle runbook in
ops has the whole procedure.

The registry can also declare project groups (`[groups]`), named sets of live prefixes
that `--group <name>` reads together. `rename` carries a member to its new prefix.
`unregister` removes the prefix from every group and deletes, with a warning, any group it
leaves empty. A group name is never a live or retired prefix.

A live id and its retired spelling are one task. `tasks dep <id> --on <x>` when the task
already depends on `x` under any spelling changes nothing, keeps the stored spelling, and
says so in `warnings`; `--rm` with either spelling removes that one dependency. An
`--on`/`--rm` pair therefore cannot rewrite a stored retired prefix: in two calls it drops
the edge, and in one call it is refused because both flags name the same task.

Rename requires clean `tasks/`, no live claims, and at most one git worktree. An interrupted
rename freezes writes to that project; reads remain available. Re-run the same command to
resume, or use `--explain` to observe its recovery verdict without writing, locking, or
checking authorization. Outside git, rename warns that forward recovery is the only
option after source removal. `git checkout .` alone does not undo a rename; see
[manual recovery](docs/specs/2026-09-08-prefix-rename-design.md#56-undo-and-manual-recovery).

Attachment directories move with their records. The inventory records which tasks had
attachments, and recovery refuses R9 (unexpected destination attachments), R10
(attachments missing from both sides), or R11 (source attachments appeared after the
inventory).

An inventory written by a binary older than attachments fails to load, and since every
mutating command checks the host's pending renames, that blocks mutating commands in
every project on the host, not only the one being renamed. Finish that rename with the
older binary, then `git mv tasks/files/<old>-<hex> tasks/files/<new>-<hex>` for each
task directory.

If another host already renamed a synced checkout, run `tasks rename <old> <new> --adopt`
from that checkout on this host **before starting work there**. It changes only this host's
registry and claim state; old ids keep resolving. A live old or new claim blocks adoption:
stop its owning session or wait for it to become stale, then retry. For a rehearsal, set
both `XDG_CONFIG_HOME` and `XDG_STATE_HOME` to scratch directories so no live registry or
park is touched. `--explain --adopt` diagnoses the local state without writes.

### Location history

The host-local registry's optional `[locations.<prefix>]` table records the current
root's symlinked worktree `storage` and `former` roots, each with optional storage and
a UTC `until` date. Paths must be absolute; history is not synced between hosts.

| Command | Location history |
| --- | --- |
| `init` | Records current storage. |
| `init --prefix <p> --force` at the same root | Refreshes current storage. |
| `init --prefix <p> --force` at a new root | Records the former root and storage, then refreshes current storage. |
| `rename <old> <new>` | Carries history to the new prefix and refreshes storage. |
| `rename <old> <new> --adopt` | Merges both prefixes' history, records a displaced root, and refreshes storage. |
| `unregister <p>` | Removes that project's history. |

Before moving a checkout, run `tasks -C <root> init --prefix <p> --force` on every
other host while the old root and storage still exist. Without that record, adoption
can retain the former root but may lack its storage and warn. A dangling link at an
unchanged root preserves recorded storage; after a root change it clears the current
storage record. Former entries remain until unregistering.

For a move made before location history was available, backfill the former root in
that host's `~/.config/tasks/projects.toml`, substituting its absolute paths:

```toml
[locations.tack]
former = [
  { root = "<Dropbox>/ai", storage = "<WORK_ROOT>/ai/.worktrees", until = "2026-09-27" },
]
```

Preserve existing history when adding an entry, then run
`tasks -C <root> init --prefix tack --force` to refresh current storage. An older
`tasks` binary drops `[locations]` whenever it writes the registry; update each host's
binary before recording history.

## Linked projects

A linked project keeps its task records, specs, and plans in an overlay project directory
while its code lives in another git checkout. Run this in the overlay project directory:

    tasks init --prefix <p> --checkout <path>

Its `tasks/.config.toml` carries `[checkout] path`, written relative to the project root
(absolute paths are accepted when read). Commands run in the code checkout or its
worktrees open the linked project; nothing is written into the code tree. Claims and
parks name the code worktree, so resume there. `tasks projects --json` includes an
absolute `checkout` only on linked rows; `check` warns `checkout_missing` when the
checkout is absent on this host. `resolve` matches it with `via: "checkout"`, and the
storage its `.worktrees` link points into, removed worktrees included, with `via: "storage"`,
while that link stands; nothing records it.

`tasks where [PATH] --json` reports which project a directory or file opens. In git,
it stays within that repository; outside git it walks up to find a project. Its three
outcomes are:

- Project found: `{"project":{"prefix":"<p>","root":"<overlay>","checkout":"<code>","linked":true},"holds_records":false,"warnings":[]}`.
  An in-tree project has `linked: false` and `checkout: null`.
- No project: `{"project":null,"holds_records":false,"warnings":[]}`.
- Error: exit 1 with `{"error":{"kind":"<kind>","detail":"<detail>"}}` on stderr;
  success JSON is on stdout with exit 0.

`holds_records` reports whether the repository tracks a regular `tasks/.config.toml`
file in `HEAD`, even if it is deleted locally; an unborn branch holds none. Two
projects declaring the same checkout fail with `checkout_conflict`. Init refuses
writes into a declared code checkout or any of its worktrees.

## Resolving ids and paths

Use `tasks resolve <id|prefix|path>...` or `tasks resolve --stdin` (one input per line)
to map inputs through prefix renames and directory moves. It is read-only and needs no
current project. Paths containing `/` or beginning with `.` or `~` are classified
first, then full ids, then prefixes matching `[a-z][a-z0-9]{1,7}`, then invalid inputs.
Relative paths use the working directory, or `-C` if supplied. Bare hex shorthand is
not expanded: `beef00` is a prefix, while `4f2a9c` is invalid. Id resolution does not
check whether the task file exists; use `tasks show` for that.

```json
{"results": [
  {"input": "ai-1a2b3c", "kind": "id", "status": "resolved",
   "id": "hq-1a2b3c", "prefix": "hq", "alias": "ai", "root": "<Dropbox>/hq"},
  {"input": "zz-123456", "kind": "id", "status": "unregistered", "prefix": "zz"},
  {"input": "/x/y", "kind": "path", "status": "ambiguous",
   "candidates": [{"prefix": "aa", "via": "former_root", "location": "/x"},
                  {"prefix": "bb", "via": "former_root", "location": "/x"}]},
  {"input": "4f2a9c", "kind": "invalid", "status": "invalid",
   "detail": "expected an id <prefix>-<hex6>, a prefix, or a path"}
], "warnings": []}
```

Resolved paths include `via` and `location`. `worktree` is present for a storage match
with a following component, or a git worktree match. `relpath` is relative to
`worktree` when present, otherwise to `location`; it is empty at that directory itself.
`root` always names the current registered root, and inapplicable fields are omitted.

Per-item outcomes never fail a batch. Malformed UTF-8 stdin lines become `invalid`
rows with replacement characters in `input` and decoding details; a stdin read error
adds an invalid diagnostic and stops reading, preserving earlier rows. Unavailable
relative-path bases also produce invalid rows. Only registry problems fail command
execution with the usual error envelope on stderr and exit 1.

## Choosing a process

Set `--process direct|planned` on `add` or `edit`; `edit --no-process` clears it.
Choose it alongside size and complexity when scoping work, including each plan child.
It is never derived from those fields, parentage, or document links.

| Process | Agent workflow |
|---------|----------------|
| `direct` | Execute the scoped task or reviewed plan without brainstorming or new design/plan documents. |
| `planned` | Review a written design spec with the user, then review a written implementation plan, before implementing. |
| Unassessed | Read the task and relevant code, explicitly record the choice and its reason before implementation. |

Both code paths retain appropriate debugging, tests, and review, and use an isolated
task worktree under the adopted policy. A direct task that uncovers an unresolved
design decision or grows beyond its scope needs a note and reassessment to planned.
Ideas still need scoping. Existing document links do not establish approval.

JSON task, summary, and parked rows expose `process` as a string when assessed;
unassessed rows omit it. Pretty summary and parked rows show a process column (`-` when unassessed); show and next
print a `process:` row reading `direct`, `planned`, or `unassessed`. The parked
`phase` remains a link-derived resume hint: a todo without document links can show
`phase: implementing` alongside `process: planned`, which still requires both reviews.

`tasks check` warns `process_missing` only for doing records without a choice,
including goals and plan steps. Unassessed todos need no backfill sweep. The CLI
does not change readiness, ordering, or `start` eligibility based on process, and
does not run skills or create worktrees.

Projects must adopt the process policy in their agent instructions (see the install
snippet above) before it can take precedence over generic brainstorming triggers.
This repo has adopted it. The writing-plans integration is the tasks skill's child
command (`--complexity <level> --process <value>`), not the upstream plan-writing skill.
Design: `docs/specs/2026-09-13-task-process-design.md`.

## Completions

Bash and zsh complete subcommands, flags, `--status`/`--size`/`--sort`/`--color`
values, registered project prefixes, and task ids — `tasks show sci-4f<TAB>`. The id
candidates come from the project the command would actually act on, so `-C`,
`--project`, and a typed foreign prefix all steer them.

Bash, in `~/.bashrc`:

    source <(TASKS_COMPLETE=bash tasks)

Zsh, in `~/.zshrc`, **after** completion is initialized — the stub calls `compdef`, so
sourcing it before `compinit` fails with `command not found: compdef`:

    autoload -Uz compinit && compinit      # or your framework's own init
    source <(TASKS_COMPLETE=zsh tasks)

Under oh-my-zsh, prezto, or a plugin manager, the same rule applies: the stub goes after
that framework's initialization.

Re-source the stub or open a new shell after upgrading `tasks`; a running shell keeps the
function it loaded at startup. Fish, elvish, and powershell use the same mechanism with
their own syntax. `TASKS_COMPLETE=` or `TASKS_COMPLETE=0` disables completion.

Color is off unless you ask for it. `--color auto|always|never`, or `TASKS_COLOR` with the
same three values, styles `--pretty` output only; JSON never carries escape sequences.
`auto` colors a stream only when that stream is a terminal, so putting `TASKS_COLOR=auto`
in a shell rc leaves piped and agent-run output plain. A non-empty `NO_COLOR` turns off
color selected through the environment, and an explicit `--color` overrides it.

Assign a persistent project color with an optional top-level key in `tasks/.config.toml`,
before any table headers:

```toml
prefix = "sci"
color = "#123456"
```

The value must be exactly `#RRGGBB` (either hex case); invalid values fail with a config
error. Change the value to change the color, or remove the key to restore existing styling.
Init/reinitialization and prefix rename preserve the setting. These commands use the same
RGB for the project prefix in `projects` and the task title in `list --all-projects`:

    TASKS_FORMAT=pretty tasks --color=always projects
    TASKS_FORMAT=pretty tasks --color=always list --all-projects

Local `list` uses the project color too. Unassigned or unreachable projects keep their
existing styling; status, priority, dates, tags, and metadata keep their own roles.
Project colors obey the color controls above and never appear in JSON or stored task IDs.

With color on, date columns fade by age: today in the terminal theme's cyan, older dates
toward a dimmed foreground, anything two years or more away the same. Priorities take the
theme's magenta: P0 bold, P1 full, then even steps down to a deeply dimmed foreground at
P4. The colors are read from the terminal itself (foreground, background and palette
slots 5 and 6), and only when stdout is a terminal, since a pager reading the same
terminal would race the query. For piped output, a terminal that does not answer, or ends of your own, set
`TASKS_PALETTE="fg=#rrggbb bg=#rrggbb cyan=#rrggbb magenta=#rrggbb"` (`magenta` may be
left out, which keeps priorities bold at P0 and P1); otherwise dates print plain,
priorities bold at P0 and P1, with one warning saying why. By default the colors come
from the terminal itself. `TASKS_THEME=default` instead uses the palette tasks ships
(`fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=#d75fd7`), which sends no query and is
meant for dark backgrounds; a set `TASKS_PALETTE` still wins over it.
`TASKS_THEME=terminal` is the default behavior, spelled out.

`show` and `next` print their task in a reading layout: an `overview` section with one
aligned `key: value` row per set field, then the park, the description, related tasks,
attached files, and the notes, each under a ruled heading. Prose wraps at the terminal
width (or `COLUMNS`), capped at 100 columns, and stays unwrapped when piped. Fields take
the colors `list` gives them. The task file itself is `tasks/<id>.md`, and JSON output
carries every field.

## Agent skill

`skills/tasks/SKILL.md` teaches agents the session protocol. Install every skill at user
level so they apply to every project, from a checkout of this repo:

    just install-skills

The recipe symlinks each `skills/*` directory into `~/.claude/skills` and
`~/.agents/skills` (the latter for other harnesses) and is idempotent: re-run it after
a pull adds a skill.

or per project, when a project needs to pin its own copy:

    mkdir -p <repo>/.claude/skills && cp -r skills/tasks <repo>/.claude/skills/tasks

`tasks init` warns when neither location has the skill.

`skills/curate/SKILL.md` is the maintenance pass: `/curate` samples open tasks and
refines them within fixed bounds.

`skills/scope/SKILL.md` is the deliberate idea review between capture and maintenance:
`/scope` turns a bounded cluster into supported next actions and handoffs.

## Feedback

A report about a project's tooling goes to the project that owns it:

    tasks feedback --project <owner> "<one line about the tool>" --category <friction|gap|idea|positive> [-b "<detail>"]

The owner must be registered and opt in with a `[feedback]` table (`scope = "<what it
owns>"`) in its `tasks/.config.toml`; any other is refused with the list of those that
accept. The entry lands as an `idea` in the owner's registered checkout, tagged `feedback`,
the category, and `from:<your prefix>`. A repeat of the same one-liner in the same owner
appends a note to the open entry instead of creating a duplicate; `--recur <id>` and
`--new` settle an `ambiguous` result. The command never commits: a person in the owner's
repository reviews each uncommitted file before it is committed, and this repository is
public.

## Adopting in an existing project

1. Keep historical docs in place. By default spec links accept docs/specs/,
   docs/designs/, docs/superpowers/specs/, and docs/superpowers/designs/; plan links
   accept docs/plans/ and docs/superpowers/plans/. If the project keeps them elsewhere,
   say so before `init`:

       mkdir -p tasks && cat > tasks/.config.toml <<'EOF'
       prefix = "sci"
       spec_dirs = ["design", "rfcs"]
       plan_dirs = ["planning"]
       EOF

   A configured list replaces the defaults. The roots are both the validation boundary
   and the search path for bare names, and they are project-level only so `tasks check`
   agrees on every machine.

   The same file may carry the project's tag dictionary, one line per tag with a
   specific meaning:

       [tags]
       testing = "Tests, gates, and CI."
       perf = "Speed or memory."

   `tasks tags` prints the meaning beside each tag, and `tasks check` warns
   `undefined_tag` for an open task carrying a tag the table does not define; a project
   without the table is not held to anything. A new metadata idea starts here as a tag
   plus an entry, and is promoted to a proper field only once it has earned validation
   and a JSON key — then the entry says so and `check` flushes the tag. Design:
   `docs/specs/2026-09-11-tag-dictionary-design.md`.
2. `tasks init --prefix <p>`.
3. Install the skill and mention it in the project's CLAUDE.md / AGENTS.md.
4. Require `tasks prime` at session start and `tasks check` before completion. Add
   `tasks check` to automation once the binary has a pinned install source; never skip it
   conditionally when unavailable.

## Layout

    tasks/.config.toml               prefix = "sci"; optional color / spec_dirs / plan_dirs / [tags]
    tasks/sci-4f2a9c.md              one task
    ~/.config/tasks/projects.toml    per-machine registry: live prefix -> repo path; retired -> live; [groups] name -> prefixes
