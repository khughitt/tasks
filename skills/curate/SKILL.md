---
name: curate
description: Use when asked to curate, review, tidy, or audit the task corpus (`/curate`). Samples random open tasks and runs one bounded maintenance pass over them; never part of the session protocol.
---

# curate

One pass improves the tasks it draws. It creates no tasks, drops none, and changes no
priority; those are proposals for the human at the end. Invoke deliberately:

    /curate [n] [--project <prefix> | --all-projects]

Default: three tasks from the current project.

## 1. Sample

    tasks sample -n <n> [--project <prefix> | --all-projects]

The pool is already the right one: `idea`, `todo`, or `blocked`; no live claim; not updated
in the last 7 days. Read the warnings: a shortfall names the pool size, and each live-claim
omission names the holder.

## 2. One root per task

`sample --project` and `--all-projects` read registered roots, but `show`, `edit`, and
`note` prefer the current checkout when the id's prefix matches it. From a worktree you
could sample one copy of a task and rewrite another. So, before touching a sampled task,
fix its root and run **every** later command for it as `tasks -C <root> ...`:

- `sample` ran unscoped: the root is the current directory.
- `sample` ran with `--project` or `--all-projects`: the root is the path that
  `tasks --pretty root <id>` prints. The default JSON form is an object; its `root`
  field is the same path. Never pass the JSON to `-C`.

Grep, `git log`, and every other piece of evidence gathering run in that same root.

## 3. Per task

1. **Read.** `tasks -C <root> show <id>`; keep `task.updated`. Read the spec, plan, and
   parent it links to. `tasks -C <root> tree <id>` when it has children.
2. **Evidence.** Grep the code and docs the task names. Does the described thing already
   exist in the tree? If so, find the commit (`git log -S` or `git log --grep`). Search
   open titles and tags for a probable duplicate: `tasks -C <root> list` and read titles.
   For a goal, check that its children cover it.
3. **Verdict**, exactly one:
   - `keep`: clear, current, correctly linked; nothing to change.
   - `refined`: prose or links improved; meaning unchanged.
   - `stale`: the described thing landed, or its premise no longer holds. Proposal: drop,
     with the commit.
   - `duplicate`: another open task covers it. Proposal: drop or merge, naming the other id.
   - `decision`: not actionable without a choice the human owns. Write the questions into
     the record (step 5); the summary asks.
   - `decompose`: a goal is missing children it needs. Proposal: the children, one line
     each.
4. **Revalidate**, immediately before the first write: `tasks -C <root> show <id>` again.
   Skip the task, writing nothing, and report the reason if any of these hold:
   - `task.status` is not `idea`, `todo`, or `blocked` (became active);
   - `claim` is present with `live: true` (claimed; a stale claim does not count);
   - `task.updated` differs from the stamp taken in step 1 (changed).
   This is a check, not a lock. A session that starts the task in the seconds between
   the recheck and your write gets a prose edit and a `curate:` note on a task it holds;
   the note says what happened. Accepted.
5. **Edit**, only through the CLI, only these:
   - `--title`, `--body`: rewrite for clarity and brevity. Keep every fact. State the
     assumptions the text implies. Collect questions you cannot answer under one
     `## Open questions` heading at the end of the body; reuse an existing one. Notes
     are stored apart from the body and survive the rewrite.
   - `--spec`, `--plan <topic> --step "<heading>"`, `--parent`, `--depends`: fix when
     the linked thing exists and the link is missing or wrong.
   - `--size`: set or correct.
   - `--tag`: add a tag the project already uses (`tasks -C <root> tags`) when it
     clearly applies. Never invent one.
   Never: status, priority, `--parallel`, `--source`, `add`, `drop`, or `tasks/*.md`
   by hand.
6. **Note.** Exactly one:

       tasks -C <root> note <id> "curate: <verdict>; <what changed>[; proposal: <text>]"

   The `proposal:` segment is present only for `stale`, `duplicate`, `decision`, and
   `decompose`. The note is the audit trail and moves the task out of the pool for the
   age window.

## Pending proposals

A task whose most recent note is a `curate:` note carrying a `proposal:` segment is not
re-curated. Report it as `pending` with the proposal text and move on. Any later note
clears it: the human records the decision with `tasks note <id> "<decision>"` whether
they acted on the proposal or declined it. A `keep` or `refined` task simply re-enters
the pool after the age window; reviewing it again is the maintenance.

## Bounds

- Zero new tasks per pass. Curation improves what exists; adding is a proposal.
- Prefer shorter. A rewrite that grows the body without adding a fact is wrong.
- Touch only the sampled tasks. A duplicate found in passing is named in the proposal,
  not edited.

## Summary to the human

One line per sampled task: id, verdict (or `skipped: <reason>` / `pending: <proposal>`),
one phrase of what changed. Then the decisions that are the human's, grouped, omitting
empty groups:

- **drops**: id, and the commit or the duplicate id;
- **priority changes**: id, from, to, why;
- **children to add**: the goal, then one line per child;
- **questions**: id, then the open questions a `decision` verdict wrote into the record.

Nothing else. A pass with nothing in those groups says so in one line.

## What a good task reads like

- The title names the outcome, not the activity.
- The body answers why, what done looks like, and where to look. It repeats nothing the
  linked spec already says.
- Assumptions are sentences, not implications. Unknowns are open questions, not hedges.
- A body is as short as those three answers allow.

## Kinds

Task kinds (templates) are derived from passes, not written up front. After a handful of
passes, cluster what was seen; that work is tasks-5b73bf in the tasks project.
