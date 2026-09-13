# Scope pass: a `scope` skill and a `shelved` status

Status: implemented, accepted, and merged to `main` (2026-09-13, `5e24819`). The real Prism acceptance is merged to Prism `main` at `6887eae`; see [validation](../notes/2026-09-13-scope-skill-validation.md).
Task: tasks-019c60; pieces tasks-470e8c (`shelved`), tasks-0d50ff (`scope` skill)

## 1. Problem

Ideas arrive one line at a time, through `quick-add` or a `tasks add --status idea` in
the middle of other work. The session protocol says never pick an idea, scope it first,
and says nothing about how. Nothing consumes the pool: `curate` re-reads records but
creates no tasks and changes no status; brainstorming produces a full design, too heavy
to run over a dozen one-liners. So ideas accumulate (most of every project's open
records), the context that would have made each one actionable decays, and someone
picking up a project cannot tell which ideas are close to work, which need a decision,
and which are just kept.

There is also no place for "not now". `dropped` says won't do; `idea` stays in `list`,
in `prime`'s roadmap, and in `sample`'s pool. Priority 4 is still in sight. A thought
worth keeping but not worth looking at has nowhere to go.

## 2. Decision

Two pieces, one goal.

1. **`shelved`**: an open status that is hidden from the default views, entered only
   through `tasks shelve <id> "<wake condition>"`, and left through `tasks unshelve`.
   Open, so it keeps blocking dependents and keeps its goal from closing; hidden, so it
   costs nothing to keep.
2. **`skills/scope/SKILL.md`**: a skill that runs one bounded scoping pass over a
   cluster of related ideas, gathers evidence, and leaves each idea with exactly one of:
   a scoped task, a place in a brief with the research or design tasks needed to settle
   it, a precise question for the user, a shelf note, or a drop proposal.

The pass separates **understood** from **committed**. A brief records what is known and
what remains undecided; only work whose next step is established becomes `todo`. Ideas
whose approach is still open stay ideas, grouped under the goal the brief serves, until
a design review settles them. This is what keeps scoping from flooding `ready` with work
nobody has decided how to do.

The success criterion for a pass is that every reviewed idea has a supported next
action, a precise question for the user, or a recorded reason to be out of active work.
The brief count is a ceiling (at most three per pass), never the measure.

## 3. The `shelved` status

### 3.1 Commands

    tasks shelve <id> "<wake condition>"
    tasks unshelve <id>

`shelve` moves an open task to `shelved` and appends the note
`shelved: <wake condition>`. A goal with open descendants that are not themselves
shelved is refused with a typed error naming those descendants: `ready` reads each
child's own status and never its parent's, so shelving the goal alone would hide the
goal while its children stayed eligible for implementation. Shelve the children first
(or drop them); the goal follows. No cascade is implied. The message is required: a shelf without a wake condition
is a drop with worse bookkeeping. The wake condition names what would bring the task
back ("when profiles have more than one consumer", "after the rack UI lands", "if the
slow-draw investigation blames noise"). `shelve` follows the claim rules `block`
follows today: it goes through the same close path, releases this session's claim, and
refuses with `claimed` when another live session holds the task.

`unshelve` returns a `shelved` task to `idea` and appends `unshelved`. It returns to
`idea` and not to the previous status because the next step for anything that has sat
on the shelf is scoping. The record keeps its priority, size, complexity, and links, so
`edit --status todo` is one command when the work was already scoped.

`edit --status shelved` and an editor save that *changes* a status to `shelved` are
refused with a typed error naming `tasks shelve`: the wake condition is the entry
ticket, and only the command collects it. An editor save that keeps an already-shelved
status is an ordinary edit (title, body, tags) and succeeds. `edit --status` from
`shelved` to any other open status is allowed; it is the reopen path for a person who
knows what they want.

### 3.2 Visibility

Hidden by default from `list`, `prime` (roadmap and ready), `sample`'s pool, `ready`,
and `next`. `list --status shelved` is the explicit view, and `list` with any
`--status` filter shows exactly the statuses named, as it does today.

Counted, separately, wherever open statuses are counted: `prime`'s header line and
`projects`' status counts gain a `shelved` entry. A project with thirty shelved ideas
should say so in one number, not in thirty rows.

Shown wherever hiding it would make a view lie:

- `show` lists dependencies and children with their statuses; a shelved one appears as
  `shelved`.
- `tree` shows a shelved node whenever its parent is shown, recursively: a goal that
  cannot close because a shelved child is open must show that child, and a shelved
  subgoal must show the shelved leaf that keeps it open. Shelved roots (no parent, or a
  hidden parent) are hidden unless `--all`. `prime`'s roadmap shares the walk but hides
  every shelved row; the two views pass their own mode.
- `next` has two feeds, `ready` and the parked candidates; both refuse a shelved task,
  so a park entry that survived a failed store cleanup after `shelve` cannot hand the
  task out. That entry still appears in `prime`'s parked section and `list --parked`,
  with the record's status, because a leftover entry is an anomaly to show, not hide.
- `check` warns when an open task that is not itself shelved depends on a shelved one:
  `<id> depends on shelved <dep>: unshelve it or drop the dependency`. This is the
  blocker exposure for `ready`: a `todo` whose dependency is shelved is not ready, and
  the warning says why, on the command every commit runs.

### 3.3 Lifecycle

`shelved` is open. `Status::is_open` returns true; the transition table's "any open →
any other open" row admits it, and the `done`/`dropped` → `todo` reopen rule is
unchanged (a closed task does not reopen straight onto the shelf). Consequences that
follow from openness, none of them special-cased:

- A dependency on a shelved task is unsatisfied.
- `done` on a goal with a shelved descendant refuses like any open descendant; `drop`
  likewise.
- `open_descendant_count` counts it.
- The hierarchy walks and the forest include it.
- The prefix-rename recovery rules treat it as any open record.

`shelved` is never a work status: `start` on a shelved task refuses with a typed error
naming `unshelve`, so a picker cannot resume it by accident and the shelf is left only
on purpose.

`park` does not apply: a shelved task is not set down mid-work, it is out of work.
`park` on a shelved task refuses with a typed error naming `unshelve`; today `park`
checks only `is_open`, so this is a new guard. `shelve` on a parked task clears the
park entry, as `drop` does and `block` does not: the release intent's `clear_park`
covers `shelved` beside `done` and `dropped`. That same path clears a recorded
escalation, and shelving keeps that consequence: the saved next step and the rating
escalation both existed to drive resumption, and a shelved task is not resumed; it is
unshelved and scoped again. The removal is reported with the `cleared the escalation of
...` warning the release path already has; today that warning is emitted only for an
explicit reassessment (`edit --complexity`), not for `clear_park` alone, so emitting it
on `shelve` is new behaviour, covered in §6.

### 3.4 JSON shapes

- `status` gains the enum value `shelved` in every shape that carries a status.
- `prime` and `projects` gain `shelved` in their count objects, beside the existing open
  counts.
- `shelve` and `unshelve` return the id shape (`{"id", "warnings"}`), as `block` and
  `unblock` do.
- `check` gains the warning text above; no new field.

Completion candidates for `--status` and the status column in `--pretty` rows include it.

## 4. The `scope` skill

    /scope [<id>... | --tag <tag>] [--project <prefix>]

### 4.1 Batch selection

Explicit ids are the batch, exactly, and the only way to revisit a cluster whose
previous pass is still waiting on something. Otherwise the skill reads every `idea` in
the project (`tasks list --status idea`, narrowed by `--tag` when given) and drops from
the pool every idea a previous pass already handled: one whose most recent note starts
with `scope:` (any verdict), and one carrying a pending `proposal:`. An idea re-enters
the pool when a later note lands on it, which is how research findings, an answer from
the user, or an `unshelve` reopen a cluster; the pass that answers a brief's question
writes that note. Without this rule the default pick would return to the same briefed
cluster every run while its research is still open.

The eligible pool is clustered, and the skill picks **one** cluster of roughly three to
five ideas that share a parent, a source, a tag, or plainly the same subsystem. It
states the cluster and why in one line and proceeds; choosing a cluster is not the
user's job. Preference order when several qualify: the one with the most members, then
the oldest. A pool with no cluster of two or more takes the oldest three ideas as
singletons.

The cluster is provisional from titles. Step 4.2 confirms or splits it; a member the
evidence shows unrelated is left as it was, with no note.

### 4.2 Same root, then evidence

Before any read of a member, fix the root and run every later command for it as
`tasks -C <root> ...`, with `curate`'s two cases exactly: an unscoped pass uses the
current project checkout (the nearest ancestor holding `tasks/.config.toml`), and a
`--project` pass uses the path `tasks root <id>` prints. Never `tasks root` for an
unscoped pass: from a worktree it names the main checkout, so the pass would select
the worktree's records and then read and write main's. Keep `task.updated` from the
first `show`.

Per idea: `show`; the `source` (a mindful thought is read with `mindful --json show`);
the parent and `tree` when it has one; grep the code and docs the record names; `git
log -S` / `--grep` for the described thing; the open titles for a duplicate; any
existing brief or spec the cluster already has. What was found is written into the
record or the brief, not narrated in chat.

### 4.3 Verdicts

Exactly one per idea.

| verdict | when | writes |
|---|---|---|
| `scoped` | the approach is established and correctness has a check | `edit --status todo -p --size --complexity`, body rewritten to answer why, what done looks like, where to look; children via `--parent` when it is too big for one task |
| `briefed` | decisions remain that a brief can frame | the idea stays `idea`, associated with the cluster's goal under §4.4 (existing parents preserved), note names the brief; the brief covers it; research or design tasks as needed (§4.5) |
| `question` | not resolvable without the user, and a brief would not help | `## Open questions` in the body; relayed in the summary |
| `shelved` | worth keeping, not worth looking at now | `tasks shelve <id> "<wake condition>"` |
| `drop` | the thing landed, or the premise is gone, or another task covers it | status untouched; the `scope:` note carries `proposal: drop, <commit or other id>`; relayed in the summary |

`scoped` is the exception, not the default. The test is the one the complexity rubric
already states for `low` and `mid`: the approach is established and the context is
identified. A cluster about which the pass still holds two candidate approaches is
`briefed`, and its ideas stay ideas.

Status, priority, size, complexity, `--parent`, `--spec`, `--source`, and body edits are
the allowed writes, plus `add` for the goal and the research or design tasks of §4.5.
Never `drop`, never `--parallel`, never `tasks/*.md` by hand.

Immediately before the first write to a member, `show` it again and skip it (writing
nothing, reporting the reason) if its status is no longer `idea`, it carries a live
claim, or `updated` moved. Same check, same caveat as `curate` step 4.

### 4.4 The brief

One brief per cluster whose decisions remain, at
`docs/notes/YYYY-MM-DD-<topic>-brief.md`, roughly one page:

1. **Problem**: what the ideas are reaching for, in the reader's terms.
2. **Current behaviour and evidence**: what the code and docs do today, with paths and
   commits.
3. **Constraints** already in the tree (specs, invariants, other open work).
4. **Alternatives**: two or three, a paragraph each, and which one the pass leans to.
5. **Unanswered questions**: the ones only the user, a measurement, or a design session
   can answer. Each names who or what answers it.
6. **Proposed decomposition**: the research or design tasks filed (§4.5) and the ideas
   waiting on them, by id.

A brief is a note, not a spec: it goes under `docs/notes/`, not a spec root, so
`--spec` cannot attach it and nothing mistakes it for a reviewed design. When a cluster's
brief actually proposes one reviewable design, it is written as a spec under
`docs/specs/` with `Status: draft` and attached with `--spec`; that is the case a later
brainstorming session finishes rather than starts. The pass may write at most three
briefs or draft specs.

The cluster's **goal** is the parent the members already share, when they share one.
Otherwise the pass files a new `todo` goal (priority 2, body one paragraph, `--source`
the brief's repo-relative path) and parents under it only the members that had **no
parent**. A member that belongs to another goal keeps that parent: reparenting it would
remove an obligation from a goal someone else scoped and could hand that goal to
`closeout` early. Such a member is associated through the brief (its id in §6) and its
`scope:` note, not through the hierarchy. The goal is `todo` because filing research
tasks under it is a commitment to settle the cluster; it is never picked by `ready`
because it has children. The research tasks are its children too.

### 4.5 Research and design tasks

A research task is answerable or it is not filed. Its body has five parts:

- the **question**, one sentence, specific enough that two people would agree when it
  is answered;
- **where to start**: files, specs, commands, prior measurements;
- the **bound**: what is enough (a reproduction, a measurement at N settings, one
  reference implementation read);
- the **expected result**: a recommendation supported by the evidence named, recorded
  as a note on the task and a paragraph in the brief;
- the **ideas it wakes**: the ids waiting on the answer, with the instruction to run
  `tasks note <id> "<one line of the finding>"` on each when the task is done. §4.1
  readmits an idea to the default pool only on a later note; a research task closed
  without these notes leaves its cluster excluded indefinitely, so the notes are part
  of `done`, in the same commit.

Its title names the outcome ("Establish whether niri reads `light-ior` per window"), it
is a `todo` child of the goal with priority, size, and complexity by the rubric.
Investigation is not automatically `low`; a spike whose reading demands judgment is
`mid` or `high`, and the rating says so.

A design task is the one that runs brainstorming against the goal: "Design <topic>
from the brief", `--complexity high`, depending on the research tasks whose answers it
needs. It is filed only when the brief's questions are the design's questions; a cluster
waiting on one measurement does not need it yet.

### 4.6 Reruns

A second pass over a cluster improves the handoff it already has. Detection: a member's
parent has a `source` under `docs/notes/` or a `scope:` note naming a brief. Then the
pass updates that brief in place (findings appended to §2, questions answered or
sharpened, decomposition revised), reuses the goal, reuses open research tasks whose
question stands, and files only what is new. It never creates a second goal or a second
brief for the same cluster, and it retains every source link and prior note.

### 4.7 Notes and summary

One note per member: `scope: <verdict>; <what changed>[; brief: <path>][; proposal:
<text>]`. The note is the audit trail, and it moves the task out of `sample`'s pool for
the age window, so `curate` does not redo the pass. A `proposal:` segment appears only
on `drop`, and it is the one write a `drop` verdict makes.

`tasks sample`'s pending rule recognises `curate:` and `scope:` notes with the same
`proposal:` parse, so a scope proposal is
excluded from the pool and reported as `<id> pending: <proposal>` exactly as a curate
proposal is. That is a CLI change owned by tasks-470e8c (the CLI piece of this goal)
and tested in §6; the skill must not land before it, or its drop proposals would be
re-drawn and re-reported by every curate pass.

Summary to the user: the cluster and why; one line per member with its verdict; the
brief or spec paths for review; then the decisions that are the user's, grouped and
omitting empty groups: **drops** (id and the commit or duplicate id), **questions** (id
and the questions written), **shelved** (id and the wake condition, so a wrong shelf
costs one `unshelve`). Nothing else.

## 5. Documentation and protocol

- `skills/tasks/SKILL.md`: the session protocol's "never pick an `idea`; scope it first"
  gains a pointer to the `scope` skill; the status list and the recording section gain
  `shelved`, `shelve`, and `unshelve`; the "With superpowers" section says a design task
  filed by a pass is where brainstorming attaches.
- `skills/curate/SKILL.md`: the pool description names `scope:` notes beside
  `curate:` ones for the pending rule; its bounds are unchanged.
- `AGENTS.md`: one line under the session protocol.
- `docs/specs/2026-08-29-tasks-design.md`: the status table, the transition rules, and
  the CLI reference gain `shelved`, `shelve`, and `unshelve`; the status header of this
  spec is corrected in the change that lands each piece.
- README: the status list and the two commands.

## 6. Testing

End-to-end in `tests/cli.rs` for the status:

- `shelve` writes the status and the note; without a message it is a usage error and
  nothing is written; on a task claimed by another live session it fails `claimed`.
- `unshelve` returns to `idea` and notes it; on a non-shelved task it fails with a typed
  error.
- `edit --status shelved` and the editor path refuse and name `shelve`; `start` on a
  shelved task refuses and names `unshelve`.
- `list` omits shelved by default and shows it under `--status shelved`; `ready`,
  `next`, `sample`, and `prime`'s roadmap omit it; `prime` and `projects` count it.
- `show` and `tree` display a shelved child of an open goal; `tree` hides a shelved
  root unless `--all`.
- `done` on a goal with a shelved child refuses; a `todo` depending on a shelved task is
  absent from `ready` and `check` warns naming both ids.
- `shelve` on a parked task clears the park entry and a recorded escalation and emits
  the `cleared the escalation` warning (new for this path); `park` on a shelved task
  refuses and names `unshelve`.
- `shelve` on a goal with an unshelved open descendant refuses and names it; succeeds
  once every descendant is shelved or closed.
- Editor path: a save that changes another status to `shelved` refuses; a save that
  edits the body of an already-shelved task succeeds and keeps the status.
- `sample` excludes a task whose latest note is `scope: drop; ...; proposal: ...` and
  reports it pending; a later note readmits it.
- `rename` moves a shelved record like any open one.

The skill is prose; its acceptance test is the first pass run against a real cluster
(the prism profile-editing ideas: b8b589, 920f31, 8a8eac, 49a068, ad2b12, e08ee6 are the
provisional first batch), recorded as a note on tasks-0d50ff with what the summary
looked like and what the skill text got wrong.

## 7. Out of scope

- A wake date. `shelved` wakes on a condition someone recognises; tasks-be6fcc (a
  one-shot defer date) is time-based and composes with it rather than replacing it.
- Restoring the pre-shelf status on `unshelve`. Scoping is the next step regardless.
- A `shelved` filter on `list` beyond `--status`. `--status shelved` is the view.
- Task kinds (tasks-5b73bf). The research-task shape in §4.5 is evidence for that work,
  not a template the CLI validates.
- Clustering in the CLI. The skill reads titles, tags, parents, and sources; a `tasks
  cluster` command waits until passes show the heuristic is stable.
