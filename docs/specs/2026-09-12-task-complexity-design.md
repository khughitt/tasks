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
  reasoning than it can supply; it must raise the rating in the same command.

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
  show shape. Additive; no existing key changes.
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
  and the ready list behind it. A task a session escalated and parked never comes back
  to a session under the same cutoff.
- `prime`: the ready section only. The roadmap, closeout, parked, and doing sections are
  status views, not pickers, and stay complete.

`list`, `show`, `tree`, `sample`, and `start` are unchanged. `start` is not gated: once
someone names a task by id they have made a decision, and the envelope steers picking,
not obedience.

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

`--reason capability` requires `--complexity` on the same `park` and the new level must
be above the current one (any level when unassessed); otherwise the command fails with a
validation error and writes nothing. This is the only place `park` touches a record field
beyond the park note, and it exists so that an escalation can never leave the task
pickable under the cutoff that just failed on it. `--complexity` on `park` without
`--reason capability` is rejected.

`capability` joins the reason vocabulary of
docs/specs/2026-09-11-park-reason-and-stamps-design.md §4 as a seventh word: the work
needs more reasoning than this session can supply. It is orthogonal to `--waiting-on`
as the others are; an escalation normally waits on the agent, since a stronger session
picks it up. An environment or credential failure is `--reason environment` and must not
raise the rating: the difficulty did not change, the machine did.

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
  cutoff and `TASKS_MAX_COMPLEXITY` for harnesses; the escalation triggers and the
  `park --reason capability --complexity` form; `capability` in the reason list.
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
- `prime` under `TASKS_MAX_COMPLEXITY` filters the ready section and nothing else.
- Flag overrides the variable; invalid variable fails even when the flag is given.
- `park --reason capability` refuses without `--complexity`, refuses a level not above the
  current, succeeds and writes both the park and the rating otherwise; `--complexity`
  without the reason is refused.
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
- **Self-assessment before starting.** Sessions are not reliably calibrated about
  unfamiliar work; the rating comes from scoping, and escalation is by observable
  trigger.
