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
  command and records an *escalation* in the shared claim store, so every checkout's
  picker honours the new level before the record merges and after the task is resumed.

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
  show shape, and `escalation` (`{level, at, session}` or null, §5) beside it in both.
  Additive; no existing key changes.
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
the escalation's level (§5) when one is recorded for the task and is higher. Escalations
live in the shared claim store outside git, so an escalation made in one worktree governs
picking from every checkout of the project from the moment the store write succeeds,
across resumes and later parks, until a session explicitly reassesses the task or closes
it. An unassessed record with no escalation is unassessed.

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
the record and, when the park waits on the agent, to the store as an escalation.
`--complexity` on `park` without `--reason capability` is rejected. The level rules
depend on who the park waits on:

- `--waiting-on agent` (the default; a stronger session should pick it up):
  `--complexity` is required. The level must be at least the task's **effective rating**
  (§4.1: the record's rating or the existing escalation, whichever is higher), and when
  `TASKS_MAX_COMPLEXITY` is set it must be above that cutoff. Checking against the
  effective rating, not the checkout's record, is what stops a session reading a stale
  `low` from replacing another checkout's `high` escalation with `mid`; lowering is
  reserved to explicit reassessment (§5.1). A level equal to the effective rating is
  allowed so that a rerun after a partial write (§5.2) succeeds, and so that a task
  already at `high` can be escalated as `high` under a lower or absent cutoff. The
  command refuses only when no level above the cutoff exists — the cutoff itself is
  `high` — and then names the `--waiting-on user` route.
- `--waiting-on user` (the envelope cannot be escaped by rerouting; a person must
  decompose, rescope, or reassign): `--complexity` is optional, and when given must be
  at least the effective rating. No escalation is recorded: the task is waiting on a
  person, and `ready` and `next` already omit work waiting on the user.

`park` learns the cutoff only from `TASKS_MAX_COMPLEXITY`. A session that picked with
the flag alone is a person at a terminal, and its escalation is checked against the
record's rating only; the harness form is the variable (§4.2), and the skill says so.

### 5.1 The escalation entry

The claim store gains a third map beside claims and parks: `escalations`, keyed by task
id, each `{level, at, session}`. It is the shared, git-independent statement "this task
needs at least `level`", and it exists because the record's rating lives in one checkout
until merged while the sessions that must respect it read others.

- Written by `park --reason capability --waiting-on agent` only, replacing any earlier
  entry for the id with one at the same or a higher level (§5).
- Untouched by `start`, by any later `park` with another reason, by `note`, and by
  every `edit` that does not name `--complexity`. Resuming an escalated task and parking
  it again for `session` from a checkout whose record still says `low` leaves the
  escalation standing, so a third checkout's picker still hides it.
- Removed by `edit --complexity <level>` and `edit --no-complexity` on that id from any
  checkout — an explicit reassessment is the new truth — with a warning naming the
  cleared level, session, and time so a session reassessing from a stale record sees
  what it overrode. Removed by the transitions to `done` and `dropped`, alongside the
  park entry.
- Never pruned by readers, and never pruned because the record caught up: after the
  merge the entry is redundant and harmless, and its `at` and `session` remain the
  record of an attempt (§5, calibration).
- Carried by `tasks rename` exactly as park entries are
  (docs/specs/2026-09-08-prefix-rename-design.md §5.1 P6, §5.2, §5.3, §5.6): the set the
  rename migrates is parks and escalations together, re-keyed to the new prefix. A
  source store holding only escalations is migrated, not skipped; a target store holding
  only escalations is a destination conflict that preflight refuses; the inventory
  snapshot that recovery restores from holds both maps; and manual recovery restores
  both. That spec's "park entries" becomes "park and escalation entries" wherever the
  store is meant, in the same change.

Pickers compute the effective rating from it (§4.1). `show` and list rows report it as
`escalation`.

### 5.2 Write order and recovery

`park` writes the record before it saves the shared store. For every other reason a
store failure after the record write is a success with a warning, as today: the note is
the trail, and a missing park entry costs a listing. For `--reason capability` waiting
on the agent the store write *is* the guarantee, so the command instead fails: exit
status 1 with a validation-class error,

    the note and rating landed, but the escalation of <id> to <level> was not recorded
    (<error>); rerun the same `tasks park` command

The record then carries the raised rating, and the rerun passes because equal-to-current
is accepted; it writes the escalation and the park entry. No rollback of the rating is
attempted, since a raised rating is a true statement about the task. Until the rerun
succeeds, the raised rating is visible only in the checkout that wrote it: the
cross-checkout guarantee of §4.1 begins with the successful store write, not with the
record write.

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
- `docs/specs/2026-09-08-prefix-rename-design.md`: the store steps and refusals name
  escalations beside parks (§5.1 above), with a pointer here.
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
- Two worktrees of one project, through the lifecycle: escalate in A with
  `park --reason capability --complexity high`; `next --max-complexity mid` and
  `ready --max-complexity mid` from B (and under `--all-projects`) hide the task while B's
  record still says `low`; `start` in B, then `park --reason session` in B; A, B, and a
  third checkout still hide it and `show` reports the escalation; `edit --complexity mid`
  in B clears it with the warning, after which B offers it; `done` clears it too.
- Two checkouts, stale record: A escalates to `high`; B, whose record still says `low`,
  runs `park --reason capability --complexity mid` under `TASKS_MAX_COMPLEXITY=low` and
  is refused naming the effective `high`; `--complexity high` from B succeeds and the
  escalation stays `high`.
- `park --reason capability --waiting-on user` records no escalation; the task is
  omitted by `ready`/`next` as parked on the user.
- `rename`: a source store holding only escalations is migrated and re-keyed, and the
  escalated task is still hidden under a cutoff from the renamed project; a target store
  holding only escalations is refused in preflight; a rename interrupted after the
  inventory is written resumes with the escalations intact, and the manual-recovery
  steps restore them.
- `prime` under `TASKS_MAX_COMPLEXITY` filters the ready and closeout sections and
  nothing else.
- Flag overrides the variable; invalid variable fails even when the flag is given.
- `park --reason capability` waiting on the agent: refuses without `--complexity`;
  refuses a level below the current rating; with `TASKS_MAX_COMPLEXITY=mid` refuses `mid`
  and accepts `high`; a record already `high` accepts `high` under cutoff `mid` or none;
  with `TASKS_MAX_COMPLEXITY=high` refuses and names `--waiting-on user`; on success the
  record and the escalation both carry the level. Waiting on the user: succeeds without
  `--complexity`. `--complexity` without the reason is refused.
- Partial write: with the store made unwritable, the first `park --reason capability`
  exits 1 with the rerun message and the record carries the raised rating; a plain
  `park --reason session` under the same fault still exits 0 with its warning; the rerun
  with the same level succeeds and writes the escalation and the park entry.
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
- **Carrying the level on the park entry.** `start` drops the entry and any later
  `park` replaces it, so the guarantee would end on the first resume from another
  checkout — the normal path. A separate escalation entry survives the lifecycle.
- **Recording the escalating session's cutoff instead of the level.** It would need a
  second comparison rule in every picker; carrying the level reuses the one rule the
  record already needs, and reads the same after the merge.
- **Having `start` copy the escalated level into the resuming checkout's record.** Covers
  the resuming checkout only; a third checkout still reads its own stale record.
- **Requiring a strictly higher level.** Makes the rerun after a partial store write
  impossible and leaves a `high` task with no escalation route at all.
- **Self-assessment before starting.** Sessions are not reliably calibrated about
  unfamiliar work; the rating comes from scoping, and escalation is by observable
  trigger.
