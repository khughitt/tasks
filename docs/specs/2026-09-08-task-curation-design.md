# Task curation: a curate skill and `tasks sample`

Status: implemented (2026-09-09)
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
  `dropped`. A task with a live claim is excluded. A task whose most recent note starts
  with `curate:` and carries a `proposal:` segment is excluded: its proposal is awaiting
  the human, and re-drawing it would only re-report it. Any later note clears that, so
  the human answers by writing one. A task whose `updated` is within
  `--older-than` days of now is excluded; the default is 7. `--older-than 0` skips the
  age check entirely, so a future-dated record (clock skew) is admitted too. The value is
  bounded at the CLI to 0 through 36500 days (a century); anything else is a clap parse
  error, so the date arithmetic can never overflow. Goals (tasks with children) stay in the pool; whether a goal is
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
  A live-claim exclusion is reported with the message `ready` uses, minus `ready`'s
  takeover hint (a curator never starts a task); a pending-proposal exclusion is
  reported as `<id> pending: <proposal text>`. The age and status exclusions are silent,
  since they are the definition of the pool, and the pool size in the shortfall warning
  is enough to see the effect.
- **Completion.** `sample` joins the subcommand list; its flags complete from clap; no new
  candidate list.
- **Random source.** `fastrand`, already a dependency: `fastrand::Rng::with_seed` when
  `--seed` is given, `fastrand::Rng::new()` otherwise. Selection is a partial
  Fisher–Yates over the pool sorted by id, drawing each index as a fixed-width `u64`
  (`Rng::u64`, never `Rng::usize`, whose generator differs between 32- and 64-bit
  targets), so a seed gives the same result on every platform. A known-answer test pins
  the draw.

## The curate skill

`skills/curate/SKILL.md`, frontmatter `name: curate`, described as a deliberate action,
never part of the session protocol.

**Invocation.** `/curate [n] [--project <prefix> | --all-projects]`, defaulting to three
tasks in the current project. The skill runs `tasks sample` with those arguments.

**One root per task.** `sample --project` and `--all-projects` read registered roots, but
`show`, `edit`, and `note` prefer the current checkout when the id's prefix matches it, so
from a worktree the pass could sample one copy of a task and rewrite another. The skill
therefore resolves a root before touching a sampled task and runs every later command for
it, reads and writes alike, as `tasks -C <root> ...`: when `sample` ran unscoped, the
nearest ancestor of the current directory that contains `tasks/.config.toml` (the project
an unscoped `tasks` command locates, which from a subdirectory is not the current
directory); otherwise the path `tasks --pretty root <id>` prints (the JSON form carries
it in the `root` field). Evidence gathering (grep, git
log) runs in that same root.

**Per task.**

1. **Read.** `tasks show <id>`, then the spec, plan, and parent it links to, and
   `tasks tree <id>` when it has children. Keep the `updated` stamp from this read.
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
4. **Revalidate**, immediately before the first write. `tasks show <id>` again: if the
   status is not one of `idea`, `todo`, or `blocked`, a `claim` is present with
   `live: true`, or `updated` differs from the stamp taken in step 1, the task is
   reported as skipped (with the reason: became active, claimed, or changed) and nothing
   is written. `doing` counts as active here, and a stale claim does not count as
   claimed, matching the pool rules `sample` applies. `edit` and `note` do not consult the claim
   store, so this is a check, not a lock; a session that starts the task between the
   recheck and the write gets a prose edit and a `curate:` note on a task it holds. That
   window is seconds wide, the edit changes no status or link semantics, and the note says
   what happened, so the pass accepts it rather than adding a compare-and-swap flag to
   the CLI. Revisit if a pass ever collides in practice.
5. **Edits allowed without asking**, all through the CLI:
   - `--title`, `--body`: rewritten for clarity and brevity. Facts are preserved. Implicit
     assumptions are stated. Questions the agent cannot answer are collected under one
     `## Open questions` heading at the end of the body; an existing heading is reused, not
     duplicated. Notes are stored apart from the body and survive a rewrite.
   - `--spec`, `--plan --step`, `--parent`, `--depends`: fixed when the linked thing exists
     and the link is missing or wrong.
   - `--size`: set or corrected.
   - `--tag`: added when a tag the project already uses clearly applies; never invented.
6. **Not touched.** Status, priority, `--parallel`, `--source`, anything in `doing`, and
   `tasks/*.md` by hand. No `add`, no `drop`.
7. **Note.** One `tasks note <id> "curate: <verdict>; <what changed>[; proposal: <text>]"`.
   The `proposal:` segment is present only when the pass wants a decision from the human
   (`stale`, `duplicate`, `decision`, `decompose`). The note is the audit trail, and it
   moves the task out of the next sample's pool for the age window.

**Repeat reviews.** The age window governs them. A `keep` or `refined` task re-enters the
pool after `--older-than` days like any other and is reviewed again; that is the
maintenance, not a waste of a draw. The only persistent skip is a pending proposal, and
`sample` enforces it: a task whose most recent note is a `curate:` note carrying a
`proposal:` segment is never drawn, and comes back as a `pending` warning with the
proposal text so the human sees it again. The skill relays those warnings in its summary
and needs no check of its own. Any later note clears it. The human records the decision
with `tasks note <id> "<decision>"` whether they acted on the proposal or declined it, and
that note is the record either way.

**Summary to the human.** One line per task: id, verdict, one phrase of what changed;
skipped and pending tasks appear in the same list with their reason. Then the decisions
that are the human's, grouped:

- **drops**: id, and the commit or the duplicate id;
- **priority changes**: id, from, to, why;
- **children to add**: the goal, then one line per child;
- **questions**: id, then the open questions a `decision` verdict wrote into the record.

Nothing else. A pass with nothing in those groups says so in one line.

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
  drawn and produces the omission warning; a task whose latest note is a `curate:` note
  with a `proposal:` is never drawn and produces the pending warning, while a `curate:`
  note without one or a later note of any kind leaves the task in; a task updated within
  the window is never drawn; `--older-than 0` admits every open task, future-dated ones
  included.
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
