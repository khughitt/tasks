# Task complexity: rating the judgment a task demands so a harness can pick within its envelope

Status: proposed (2026-09-12)
Task: tasks-be447b

## 1. Problem

Work is picked by priority, then size. Neither says how much reasoning a task demands.
`size` is volume: a rename across forty files is `xl` and mechanical; a one-line fix in
the claim liveness path is `xs` and subtle. When the same backlog is worked by sessions
of different capability — frontier models in one harness, cheaper models in another —
the person routing work has to read each task and judge it, every time, because the
record holds nothing to filter on.

A size cutoff would misroute in both directions: hold a capable-enough session back from
large mechanical work and hand small subtle work to a session that cannot verify it.

## 2. Decision

Three additions and one new park reason. The tool stays ignorant of models: it stores a
rating and honours a cutoff; which level a given harness may take lives in that harness's
instructions.

- An optional frontmatter field `complexity`, one of `low`, `mid`, `high`. Absent means
  unassessed — never a default of either end.
- A cutoff, `--max-complexity <level>` on `ready` and `next` and the environment variable
  `TASKS_MAX_COMPLEXITY`, that hides tasks above the level *and* unassessed tasks from
  every picker feed, and says in warnings how many it hid and why.
- A `check` warning for an open plan step with no rating, because a plan step is the unit
  of delegation and its rating is the planner's duty.
- A seventh park reason, `capability`, for a session that found the work needs more
  reasoning than it can supply. When it waits on the agent it sets the rating in the same
  command, and the shared park entry carries that rating so every checkout's picker
  honours it before the record merges.

## 3. The field

### 3.1 Definition

`complexity` is the reasoning and judgment required to complete *and verify* the task
given its current specification, plan, and repository context. It is rated after
preparation, so a `high` investigation can yield `low` implementation tasks without
contradiction, and a task's rating can fall as its spec and plan resolve the hard parts.

| level  | criterion |
|--------|-----------|
| `low`  | The approach is established, the relevant context is identified, and correctness has a clear check. |
| `mid`  | Bounded investigation or implementation choices remain; scope and acceptance criteria are clear. |
| `high` | Substantial discovery, subtle reasoning about interacting behaviour, or an unresolved architectural judgment. |

"Fully specified" alone does not make a task `low`: implementing a precisely specified
concurrent algorithm can still be `high`. Touching many files does not make it `high`.
Complexity routes implementation effort only; it says nothing about how carefully the
result should be reviewed, and that distinction stays in harness instructions.

Three levels, not five: three is what raters reproduce across weeks.

### 3.2 Record and outputs

- Frontmatter key `complexity`, written only when set, like `size`. Parsing rejects any
  other value with a validation error naming the accepted three.
- `Task.complexity: Option<Complexity>`; `Complexity` mirrors `Size` (`ALL`, `parse`,
  `as_str`, `Ord` in the order low < mid < high).
- JSON: `complexity` (string or null) is added to the list row (`TaskSummary`) and the
  show shape, and `park.complexity` (string, present only when the entry carries one) to
  the park info those rows already embed. Additive; no existing key changes.
- `--pretty` list rows gain a complexity column beside size, showing `-` when unassessed.
  Tree and graph labels are unchanged.
- Not a sort key. `ready` keeps priority-then-size order within whatever the cutoff
  leaves.

### 3.3 Setting it

- `add --complexity <level>` and `edit --complexity <level>`; `edit --no-complexity`
  clears it. Shell completion offers the three values.
- The rating is set at scoping time by the session that turns an idea into a todo, next
  to priority and size. Curation fills gaps in the backlog (the curate skill gains
  `--complexity` among its allowed edits, with the rubric). Nothing needs to be rated
  before the feature is useful: rate ready work first.
- Plan steps are rated explicitly by the planner, one `--complexity` per step child. A
  plan is evidence for a lower rating, not a guarantee, so no step inherits `low`.
- No command ever sets or changes the rating implicitly.

## 4. The cutoff

### 4.1 Where it applies

The cutoff applies to every feed a session picks from:

- `ready`: tasks above the level and unassessed tasks are removed before `--size`,
  `--parallel`, and `-n`.
- `next`: both candidate paths — the parked work waiting on the agent that `next` prefers,
  and the ready list behind it.
- `prime`: the ready and closeout sections, each by the task's own rating; a goal whose
  verification is mechanical can be rated `low` at scoping and closed by a restricted
  session, and an unrated goal is never offered to one. The roadmap, parked, and doing
  sections are status views, not pickers, and stay complete.

The level a picker compares is the **effective rating**: the record's `complexity`, or
the park entry's `complexity` (§5) when the entry carries one and is higher. Park entries
live in the shared store outside git, so an escalation made in one worktree governs
picking from every checkout of the project immediately, while the record's own rating
arrives with the merge. An unassessed record with no entry level is unassessed.

`list`, `show`, `tree`, `sample`, and `start` are unchanged. `start` is not gated: once
someone names a task by id they have made a decision, and the envelope steers picking,
not obedience. `list --parked` and `prime`'s parked section stay complete because they
are where a person finds work waiting on them; a session under a cutoff selects only
through `ready` and `next` (§7).

### 4.2 Sources and precedence

- `--max-complexity <level>` on `ready` and `next`.
- `TASKS_MAX_COMPLEXITY=<level>` in the environment, read by `ready`, `next`, and `prime`.
  A harness sets it once in its configuration so the cutoff cannot be forgotten by the
  session running inside it. The flag wins over the variable. An unset or empty variable
  means no cutoff.
- An invalid value in either place is a validation error naming the accepted levels, and
  the variable is validated whenever one of the three commands runs, even when the flag
  overrides it.

### 4.3 Warnings

When a cutoff is in force and hides anything, the command adds one warning per cause,
following the existing `ready` omission style:

    max-complexity mid: 4 above cutoff hidden
    max-complexity mid: 7 unassessed hidden

Nothing is added when nothing was hidden. Under `--all-projects` the counts cover the
whole scope, not one line per project.

Unassessed work is hidden, not defaulted, because a default at either end is a silent
routing decision. The count in the warning is how the gap becomes visible to whoever
rates.

## 5. Escalation

A session under a cutoff that discovers mid-task the rating was too low stops on an
observable trigger, not a feeling:

- the implementation needs a decision the spec or plan leaves unresolved;
- investigation reveals interacting behaviour outside the assessed scope;
- a bounded attempt makes no progress, or there is no way to establish correctness.

It records the evidence in a note and parks with the new reason:

    tasks park <id> "<where it stopped and why>" --reason capability --complexity high

`--reason capability` takes `--complexity <level>` on the same `park` and writes it to
the record and to the park entry (`Park.complexity`, optional, absent on every other
park). `--complexity` on `park` without `--reason capability` is rejected. The level
rules depend on who the park waits on:

- `--waiting-on agent` (the default; a stronger session should pick it up):
  `--complexity` is required. The level must be at least the record's current rating,
  and when `TASKS_MAX_COMPLEXITY` is set it must be above that cutoff. A level equal to
  the current rating is allowed so that a rerun after a partial write (§5.1) succeeds.
  When no level above the cutoff exists — the record is already `high`, or the cutoff is
  `high` — the command refuses and names the `--waiting-on user` route.
- `--waiting-on user` (the envelope cannot be escaped by rerouting; a person must
  decompose, rescope, or reassign): `--complexity` is optional, and when given must be
  at least the current rating.

`park` learns the cutoff only from `TASKS_MAX_COMPLEXITY`. A session that picked with
the flag alone is a person at a terminal, and its escalation is checked against the
record's rating only; the harness form is the variable (§4.2), and the skill says so.

### 5.1 Write order and recovery

`park` writes the record before it saves the shared store, and a store failure after the
record write already tells the caller to rerun `tasks park`. With `--reason capability`
the record then carries the raised rating and the rerun passes because equal-to-current
is accepted; the rerun writes the entry with the same level. No rollback of the rating is
attempted: a raised rating without an entry is a true statement about the task, and the
picker rule of §4.1 is correct with either half present.

`capability` joins the reason vocabulary of
docs/specs/2026-09-11-park-reason-and-stamps-design.md §4 as a seventh word: the work
needs more reasoning than this session can supply. It is the one reason with a
companion field, because it is the one stop whose response is a routing change. An
environment or credential failure is `--reason environment` and must not raise the
rating: the difficulty did not change, the machine did.

Calibration stays in notes. The `model` stamp records only the latest completion, not
who attempted first or who rescued, so the escalation note and reason are the record of
an attempt. Structured attempt tracking is out of scope until notes prove insufficient.

## 6. `check`

One new warning, never an error: an open task with a `step` and no `complexity`.

    <id>: plan step without a complexity rating

Missing complexity on any other task is not a finding, consistent with `size`. The
narrow rule enforces the planner's duty (§3.3) where delegation actually happens, and
starts quiet: no open task in any registered project carries a step today.

## 7. Docs and skills

- `skills/tasks/SKILL.md`: the rubric (§3.1); rating at scoping and per plan step; the
  cutoff and `TASKS_MAX_COMPLEXITY` as the harness form; the escalation triggers and the
  `park --reason capability --complexity` form with its two `--waiting-on` routes;
  `capability` in the reason list. The session protocol gains one rule for a session
  under a cutoff: pick only through `ready` and `next`; do not take work from `prime`'s
  parked or roadmap sections or from `list --parked`, and close goals only when
  `prime`'s closeout offers them. Everything else in the protocol is unchanged.
- `skills/curate/SKILL.md`: `--complexity` among the allowed edits, with the rubric.
- `README.md`: the field in the `add` example and the env var beside `TASKS_MODEL`.
- `docs/specs/2026-09-11-park-reason-and-stamps-design.md`: a one-line pointer from §4
  to this spec for the seventh word, so the vocabulary has one home.
- The harness-side mapping — which level each provider's sessions may take — is written
  in each harness's own instructions, outside this repository.

## 8. Testing

End-to-end in `tests/cli.rs` against the built binary:

- Frontmatter round-trip; `add`/`edit --complexity`; `--no-complexity`; invalid value
  rejected on `add`, `edit`, the flag, and the variable.
- `ready --max-complexity mid` hides `high` and unassessed, keeps `low` and `mid`, and
  emits exactly the warnings whose counts are non-zero; composes with `--size`,
  `--parallel`, `-n`.
- `next` under a cutoff skips a parked-waiting-on-agent task rated above it and falls
  through to the ready list; with nothing eligible returns null with the warnings.
- Two worktrees of one project: `park --reason capability --complexity high` in one,
  then `next --max-complexity mid` and `ready --max-complexity mid` from the other (and
  under `--all-projects`) hide the task while its record there still says `low`; after
  the merge the result is the same.
- `prime` under `TASKS_MAX_COMPLEXITY` filters the ready and closeout sections and
  nothing else.
- Flag overrides the variable; invalid variable fails even when the flag is given.
- `park --reason capability` waiting on the agent: refuses without `--complexity`;
  refuses a level below the current rating; with `TASKS_MAX_COMPLEXITY=mid` refuses `mid`
  and accepts `high`; at the ceiling refuses and names `--waiting-on user`; on success the
  record and the entry both carry the level. Waiting on the user: succeeds without
  `--complexity`. `--complexity` without the reason is refused.
- Partial write: with the store made unwritable after the record write, the first
  `park --reason capability` fails with the rerun message and the record carries the
  raised rating; the rerun with the same level succeeds and writes the entry.
- `check` warns on an open step without a rating and stays silent otherwise.
- Completion offers the three levels for `--complexity` and `--max-complexity`.

## 9. Rejected

- **Reusing `size`.** Volume, not judgment; wrong in both directions (§1).
- **A `routine` tag.** A boolean in a per-project dictionary, not a scale.
- **Five levels.** Not reproducible.
- **Defaulting plan steps to `low`.** Turns "has a plan" into "safe to delegate".
- **Treating unassessed as `low` or `high` under a cutoff.** A silent routing decision
  either way; hidden with a count instead.
- **Gating `start`.** Blocks a person directing a session at a specific task; the
  envelope is for picking.
- **Letting `park --reason capability` raise the rating implicitly.** One level up is
  not always right, and an implicit write to a rated field is the kind of fallback this
  tool refuses elsewhere.
- **Recording the escalating session's cutoff in the park entry instead of the level.**
  It would need a second comparison rule in every picker; carrying the level reuses the
  one rule the record already needs, and reads the same after the merge.
- **Requiring a strictly higher level.** Makes the rerun after a partial store write
  impossible and leaves a `high` task with no escalation route at all.
- **Self-assessment before starting.** Sessions are not reliably calibrated about
  unfamiliar work; the rating comes from scoping, and escalation is by observable
  trigger.
