# Quiet queue: work that waits for an idle host

Status: draft (2026-09-13)
Task: tasks-811e02

## 1. Problem

Some steps of a task cannot run while the host is in ordinary use: a GPU capture whose
preflight refuses at any competing load, a benchmark sweep that wants the machine to
itself, a power measurement that needs no graphical session at all. The host is also the
person's daily machine, so these steps arrive at random moments, get refused, and wait for
an evening when the machine is free and someone remembers them.

Today such a step is parked with `--reason environment --waiting-on user`. That records
the stop, and `tasks list --parked --all-projects` lists it, but three things are missing:

- **The signal is not distinct.** `environment` also means "this checkout has no
  binaries" or "a restart is needed", and the parked listing mixes those with dependency
  and decision parks. Nothing says "prepared, unattended, needs only an idle host".
- **There is no bedtime entry point.** No command answers "what could run tonight, from
  where, for how long". The parked listing sorts by park time, prints one line per task,
  and names the checkout only in a warning.
- **Two asks are conflated.** A capture that an agent runs once the person stops using
  the machine is unattended; a baseline that needs the person listening for an hour is
  attended. Only the first belongs in an overnight queue. The vocabulary cannot tell them
  apart.

On 2026-09-13 the parked listing across projects held five such entries in one project
and one attended session in another; two of the five had been forgotten.

## 2. Decision

Build the queue inside the park store rather than as a separate system. The store already
holds the checkout, the session, the next step, and a cross-project read. Three additions:

- An eighth park reason, `quiet`: the work is prepared and its recipe is recorded; it
  waits only for a host free of competing load.
- Two fields on a quiet park, `needs` (what "free" means for this work) and `minutes`
  (how long it takes once started), so the queue can be read before bed without opening
  each task.
- A read command, `tasks quiet`, that lists quiet parks across every reachable project,
  in priority order, each as a short resume brief.

Nothing changes for the other seven reasons. Attended sessions stay under `decision` or
`review`, which already say the person must take part.

Out of scope, deliberately: detecting that the host is idle, launching the resuming agent,
and any timer or notification. The queue plus one command before bed removes the
friction the diagnosis found; automation is a second phase (§8).

## 3. The reason

    tasks park <id> "<next step>" --reason quiet [--needs idle|headless] --minutes <n> [--waiting-on user|agent]

`quiet` joins the vocabulary of `2026-09-11-park-reason-and-stamps-design.md` §4:

| reason  | The work stopped because…                                                                  |
|---------|---------------------------------------------------------------------------------------------|
| `quiet` | the host is in use; the work is prepared and unattended and needs only an idle machine      |

Rules:

- **Vocabulary.** `Reason::Quiet` in `claims.rs`, in `ALL`, `parse`, `as_str`, and
  `complete::reason`. Serialized as `quiet`. A store file written before this change
  loads unchanged.
- **Who.** A quiet park waits on the user by convention: only a person can free the host.
  `--waiting-on` keeps its default of `agent` and the pairing is not policed, matching
  §4's independence rule, but the skill tells agents to pass `--waiting-on user` so
  `ready` and `next` do not offer the work under load. A quiet park waiting on the agent
  is a legitimate choice for a second-phase idle runner (§8) and needs no change here.
- **Not an escalation.** `--reason quiet` never raises a complexity rating and refuses
  `--complexity`, as every reason other than `capability` does today.
- **Distinct from `environment`.** `environment` stays "the checkout or machine cannot
  run the work": missing dependencies, a TTY, a restart. `quiet` is "the machine can run
  it and will, once nothing else is". A preflight refused on CPU, GPU, or load is `quiet`;
  a preflight refused for a missing tool is `environment`.

## 4. The recipe fields

Two optional fields on `Park`, valid only with `--reason quiet`:

| Field     | Flag              | Values                    | Meaning                                                              |
|-----------|-------------------|---------------------------|----------------------------------------------------------------------|
| `needs`   | `--needs <cond>`  | `idle` (default), `headless` | `idle`: the desktop session may stay up but nothing else runs. `headless`: no graphical session; the person logs out or hands the work an isolated display. |
| `minutes` | `--minutes <n>`   | 1 to 1440                 | Expected wall-clock minutes once started, including any warm-up the recipe names. |

Rules:

- **Required with `quiet`.** `--reason quiet` without `--minutes` is a validation error:
  "`--reason quiet` needs `--minutes <n>`". The whole value of the queue is being able to
  decide before bed; an agent that prepared a capture knows its length. `--needs` defaults
  to `idle` when `--reason quiet` is given, so the common case costs one flag.
- **Refused otherwise.** `--needs` or `--minutes` without `--reason quiet` is a validation
  error naming the flag: "`--minutes` on park needs `--reason quiet`", in the shape of the
  `--complexity` rule in `park.rs`.
- **Vocabulary.** A `Needs` enum in `claims.rs` beside `Reason`, with `ALL`, `parse`,
  `as_str`, and `complete::needs`; `serde(rename_all = "lowercase")`. A value outside the
  two is a validation error listing them.
- **Store.** `Park` gains `needs: Option<Needs>` and `minutes: Option<u32>`, both
  `skip_serializing_if = "Option::is_none"` and `default` on read. Re-parking replaces the
  whole entry, as today; a re-park without `--reason quiet` drops both.
- **Note.** The park note keeps its form and gains the recipe when present:
  `parked (waiting on user, quiet; idle, 50 min): <next step>`. The note is the durable
  trail; nothing is added to frontmatter.
- **Rows.** `ParkInfo` gains `needs` and `minutes`, `null` when absent, so `park.needs`
  and `park.minutes` are the JSON path in every view that embeds a park block (`show`,
  `next`, `list`, `ready`, `prime`, `tree`, and the parked rows). Additive; no existing
  key changes. The pretty parked table prints the recipe inside the same parenthetical
  the note uses.
- **The host name.** `Park.host` is the machine name and stays so. The recipe field is
  `needs`, never `host`, to keep the two apart in JSON.

## 5. The queue

    tasks quiet [-n <N>] [--project <prefix> | --all-projects]

Lists quiet parks as resume briefs. Rules:

- **Scope.** Unlike every other read command, the default scope is `--all-projects`: the
  bedtime question is "what could run tonight", not "what could run in this checkout",
  and the command is run from wherever the person happens to be. `--project <prefix>`
  narrows to one project. `--all-projects` is accepted and redundant. The command needs no
  local project.
- **Rows.** The rows of `parked::rows` filtered to `park.reason == quiet`. A row
  resolved from another checkout is included with the same warning the parked listing
  gives; a store-only row (task file unreachable) is included with its snapshot title, as
  §5.3 of the park design allows. Closed tasks whose park survived are excluded, as
  `list --parked` excludes them.
- **Order.** Priority ascending, then park time ascending, then id. A queue is first in,
  first out within a priority; the person takes the top item. Store-only rows have no
  priority and sort last.
- **`-n <N>`.** Cap the list; `-n 1` is "the first item off the queue". Default is all.
- **JSON.** `Output::Quiet(QuietOut)` with `{"tasks": [ParkedRow...], "warnings": []}`,
  the parked listing's shape, so consumers that already read parked rows read this one
  unchanged. The recipe is under `park.needs` and `park.minutes`; the checkout is
  `park.worktree`. No new row type.
- **Pretty.** One brief per row, blank line between:

      material-5b3107  P2  idle      50 min   parked 2026-09-12  Render pass order: what the glass slab refracts
              next: Resume Task 1 after a fresh readiness preflight passes; capture the old-shader additive baseline first.
              in:   /path/to/niri-material/.worktrees/material-5b3107

  The `in:` line is the checkout to open an agent in; the id is what to `tasks start`
  there. No launch command is printed: how an agent is launched belongs to the harness,
  and tying it to this view is the quick-launch idea (tasks-202e1f), not this one.
- **Empty.** No rows prints nothing in pretty mode and `{"tasks":[],"warnings":[]}` in
  JSON; exit status 0. An empty queue is not an error.

## 6. Documentation and protocol

- `skills/tasks/SKILL.md` step 5 gains `quiet` in the reason sentence, with its
  definition, and one further sentence: when a preflight or benchmark refuses on host
  load, park with `--reason quiet --waiting-on user --minutes <n>` (and `--needs headless`
  when the desktop itself is the load), with a next step that names the check to rerun
  before continuing. The step also says what `quiet` is not: a missing tool is
  `environment`; a session the person must attend is `decision`.
- `skills/tasks/SKILL.md` step 2 names `tasks quiet` beside `list --parked` as the
  cross-project view of work waiting for an idle host, so an agent asked "what can run
  tonight" knows where to look.
- The README's park section adds `quiet` to the vocabulary table, describes the two recipe
  flags, and adds `tasks quiet` to the command summary with its all-projects default.
- `AGENTS.md` session protocol gains nothing; the queue is read by the person, not by the
  session protocol.

## 7. Testing

Units in `claims.rs`: `Reason::Quiet` and `Needs` round-trip through the store; a store
file without the new keys loads; a park entry with `needs`/`minutes` and no `reason`
loads (the store does not police the pairing; only `park` does).

Integration cases in `tests/cli.rs` that carry the contract:

- `park --reason quiet --minutes 50` writes the entry with `needs: idle`, prints
  `park.needs` and `park.minutes` in `show`, `list --parked`, and `prime` JSON, and writes
  the note in the extended parenthetical form.
- `park --reason quiet` without `--minutes` fails with the named error; `--minutes 0` and
  `--minutes 1441` fail; `--needs sometimes` fails listing the two values.
- `--minutes 10` or `--needs idle` without `--reason quiet` fails naming the flag; with
  `--reason environment` it fails the same way.
- `--reason quiet --complexity high` fails with the existing `--complexity` rule.
- A re-park of the same task with `--reason review` drops `needs` and `minutes`.
- `start` on a quiet park removes the entry and the recipe with it, as today.
- `tasks quiet` from a directory inside no project lists quiet parks from every reachable
  registered project, in priority then park-time order, and omits parks with any other
  reason; `--project <prefix>` narrows; `-n 1` returns the first row only.
- A quiet park whose checkout is a worktree of the project appears with the "parked in …;
  resume it from that checkout" warning and its `park.worktree` set to that path.
- A quiet park on a task since closed does not appear.
- An empty queue is `{"tasks":[],"warnings":[]}` with exit 0.
- Pretty output prints the brief with the `next:` and `in:` lines.

## 8. Second phase, not this design

- **Idle detection.** A timer on the host that notices the machine has been idle (no
  compute clients, GPU and load under thresholds, no input for some minutes) and either
  notifies the person that the queue is non-empty or launches the top `idle` item. This
  belongs beside the host's other automation (dots or ops), reading `tasks quiet` JSON.
- **Launching the agent.** `tasks quiet` prints the checkout and the id; turning that
  into a running session is tasks-202e1f.
- **Whether `idle` is enough.** A compositor that is never fully static may keep GPU
  utilization above a strict threshold with nothing else running. Which lanes need
  `headless` rather than `idle` is a project question; material-6bd4a3 asks it. The
  queue records the answer per entry and does not decide it.
