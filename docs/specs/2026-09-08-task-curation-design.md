# Task curation: a curate skill and `tasks sample`

Status: designed (2026-09-08)
Task: tasks-c3c0d1; follow-up tasks-5b73bf (task kinds)

## Problem

Tasks are written at capture time, by whoever is in the middle of something else. The
session protocol gets them filed; nothing gets them re-read. Across the registered
projects most open records are ideas, filed in one line from a note, and nobody looks at
one again until it is picked up. By then the context that would have made it clear is
gone. Stale tasks (the described thing already landed) and duplicates sit open because
noticing them is nobody's job.

The corpus needs a maintenance pass that is cheap enough to run often and bounded enough
that it improves records without growing them.

## Decision

Two pieces, one goal.

1. **`tasks sample`**: a read command that draws N tasks uniformly at random from the
   open, unclaimed, not-recently-updated pool of one project or all of them. The pool
   rules live in code, are tested, and return JSON.
2. **`skills/curate/SKILL.md`**: a skill shipped alongside `skills/tasks/SKILL.md` that
   runs one curation pass: sample, gather evidence per task, reach a verdict, apply a
   bounded set of edits, record a note, and hand the human the decisions that are theirs.

The pass **improves what exists**. It creates no tasks, drops none, and changes no
priority. Those are proposals in the summary. Task kinds (templates) are deferred to
tasks-5b73bf so they are derived from passes rather than guessed.

## `tasks sample`

    tasks sample [-n N] [--project <prefix> | --all-projects] [--older-than <days>] [--seed <u64>]

- **Pool.** Tasks whose status is `idea`, `todo`, or `blocked`; not `doing`, `done`, or
  `dropped`. A task with a live claim is excluded. A task whose `updated` is within
  `--older-than` days of now is excluded; the default is 7 and `--older-than 0` admits
  everything open. Goals (tasks with children) stay in the pool; whether a goal is
  decomposed well is a curation question. Ideas stay in; they are most of the corpus and
  the least examined.
- **Selection.** Uniform, without replacement. `-n` defaults to 3. `--seed` fixes the
  draw so a pass can be reproduced and tests are deterministic; without it the seed comes
  from the process's entropy source. When the pool holds fewer than N tasks the command
  returns the whole pool and a warning naming the pool size. An empty pool returns an
  empty list and a warning, exit 0.
- **Scope.** `--project` reads that registered root, `--all-projects` pools every
  reachable project into one list before drawing, and the two conflict. Same semantics
  and same errors as `list`.
- **Output.** `{"tasks": [...], "warnings": [...]}`, each entry a `TaskSummary` in the
  same shape as a `list` row, so any consumer that reads `list` reads `sample`. Order is
  the draw order. `--pretty` prints the same one-line rows `list --pretty` prints.
  Exclusions for live claims are reported in warnings the way `ready` reports them; the
  age and status exclusions are silent, since they are the definition of the pool, and
  the pool size in the shortfall warning is enough to see the effect.
- **Completion.** `sample` joins the subcommand list; its flags complete from clap; no new
  candidate list.
- **Random source.** `fastrand`, already a dependency: `fastrand::Rng::with_seed` when
  `--seed` is given, `fastrand::Rng::new()` otherwise. Selection is a partial
  Fisher–Yates over the pool's index vector, so a seed gives the same result on every
  platform.

## The curate skill

`skills/curate/SKILL.md`, frontmatter `name: curate`, described as a deliberate action,
never part of the session protocol.

**Invocation.** `/curate [n] [--project <prefix> | --all-projects]`, defaulting to three
tasks in the current project. The skill runs `tasks sample` with those arguments.

**Per task.**

1. **Read.** `tasks show <id>`, then the spec, plan, and parent it links to, and
   `tasks tree <id>` when it has children.
2. **Evidence.** Grep the code and docs the task names. Check whether the described thing
   already exists in the tree; if it does, find the commit. Search open titles and tags
   for a probable duplicate. For a goal, check that its children cover it.
3. **Verdict**, exactly one:
   - `keep`: clear, current, correctly linked; nothing to change.
   - `refined`: prose or links improved; the task's meaning is unchanged.
   - `stale`: the described thing landed, or its premise no longer holds. Proposal: drop,
     with the evidence.
   - `duplicate`: another open task covers it. Proposal: drop or merge, naming the other id.
   - `decision`: the task cannot be made actionable without a choice the human owns.
     Questions are written into the record; the summary asks.
   - `decompose`: a goal is missing children it needs. Proposal: the children, each one
     line.
4. **Edits allowed without asking**, all through the CLI:
   - `--title`, `--body`: rewritten for clarity and brevity. Facts are preserved. Implicit
     assumptions are stated. Questions the agent cannot answer are collected under one
     `## Open questions` heading at the end of the body; an existing heading is reused, not
     duplicated. Notes are stored apart from the body and survive a rewrite.
   - `--spec`, `--plan --step`, `--parent`, `--depends`: fixed when the linked thing exists
     and the link is missing or wrong.
   - `--size`: set or corrected.
   - `--tag`: added when a tag the project already uses clearly applies; never invented.
5. **Not touched.** Status, priority, `--parallel`, `--source`, anything in `doing`, and
   `tasks/*.md` by hand. No `add`, no `drop`.
6. **Note.** One `tasks note <id> "curate: <verdict>; <what changed>; <proposals>"`. This is
   the audit trail, and it moves the task out of the next sample's pool.

**Skip rule.** A task whose most recent note starts with `curate:` is not re-curated. It is
reported as skipped and the pass moves on; the human decides its pending proposals or a
later note clears the mark. `--older-than` already makes this rare.

**Summary to the human.** One line per task: id, verdict, one phrase of what changed. Then
the proposals, grouped: drops (with the commit or the duplicate id), priority changes,
children to add (under which goal). Nothing else. A pass with no proposals says so in one
line.

**Bounds.** Zero new tasks per pass. Prefer shorter: a rewrite that grows the body without
adding a fact is wrong. One pass touches only the sampled tasks; a duplicate found in
passing is named in the proposal, not edited.

**Writing guide.** A short section in the skill:

- The title names the outcome, not the activity.
- The body answers why, what done looks like, and where to look. It repeats nothing the
  linked spec already says.
- Assumptions are sentences, not implications. Unknowns are open questions, not hedges.
- A body is as short as those three answers allow.

**Kinds.** A reserved section: one sentence saying kinds are derived from passes, with a
link to tasks-5b73bf.

## Tests

End-to-end in `tests/cli.rs` against the built binary:

- Pool: `doing`, `done`, and `dropped` are never drawn; a task with a live claim is never
  drawn and produces the omission warning; a task updated within the window is never
  drawn; `--older-than 0` admits every open task.
- `-n` larger than the pool returns the pool and a shortfall warning; an empty pool
  returns an empty list, exit 0.
- The same `--seed` draws the same set in the same order; two seeds differ on a large pool.
- `--project` draws only from that project; `--all-projects` draws from a pooled list and
  rows carry each task's own prefix; the two flags conflict.
- `--pretty` prints rows in the `list --pretty` format.

The skill is prose; its check is `tasks check` and a read-through against this spec.

## Documentation

- The design spec's section 5 usage block gains `sample`; the README gains one example.
- `skills/tasks/SKILL.md` gains one line under Recording work pointing at the curate skill.
- `AGENTS.md` layout lists `skills/curate/SKILL.md` next to the tasks skill.

## Not in scope

- Task kinds and templates (tasks-5b73bf).
- Any scheduled or looped invocation. The skill is run by hand until passes show what a
  cadence should be.
- Weighted or stratified sampling. Uniform is unbiased and every task is eventually
  seen; weighting is a later decision with evidence from passes.
- A `kind` tag or field.
