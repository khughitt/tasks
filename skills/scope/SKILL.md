---
name: scope
description: Use when asked to scope captured ideas, review an idea backlog for next actions, re-scope an already scoped task, clear the `*` review queue, or run /scope.
---

# scope

Turn one bounded idea cluster into evidence-backed next actions and handoffs. Invoke deliberately:

    /scope [<id>... | --tag <tag>] [--project <prefix>]

Do not ask the user to choose a cluster. Do not `tasks start` selected ideas: this is a
review pass, and each idea must remain unclaimed until its verdict is written.
Explicit ids may also name scoped tasks to re-scope (see "Re-scope a scoped task").

## 1. Select and fix the root

Explicit ids are the exact batch. Otherwise discover candidates with:

    tasks list --needs-review [--tag <tag>] [--project <prefix>]

It returns the scope queue, the rows pretty tables mark `*`. Each row's `review_reason`
says why it is there: `unreviewed` (no pass has looked, or a finding readmitted it),
`woken` (a dependency closed, or a deferral came due, after the last review),
`stranded` (reviewed, but waiting on nothing recorded and with nothing new), or
`unassessed` (a leaf `todo` or `blocked` task without process). Draw the batch from
`unreviewed` and `woken` ideas first; when none is eligible, from `stranded` ideas; only
then from `unassessed` rows, each a re-scope (see "Re-scope a scoped task"). A goal whose pieces live in other projects
holds them as `depends`, so it appears among those rows as a leaf; rate it as its
closeout, usually `--process direct`.

A reviewed idea stays out of the queue only while it waits on something recorded: an
open dependency, a pending `proposal:`, the user (`awaiting_user`), or a deferral not yet
due. It comes back by itself when that wait ends, or at once when it has none.
Incidental notes (curate, comments) neither readmit it nor change its wait. A todo or
blocked row leaves when a re-scope sets its process. An explicit id bypasses only this
review-state exclusion; it must still be an unclaimed idea at revalidation, or an
unclaimed `todo` or `blocked` task, which makes it a re-scope, and the pass still has the
three-document ceiling.

Fix one root before member reads, and use `tasks -C <root> ...` for every later task
command and that root for every evidence read and document write:

- Unscoped: use the nearest ancestor containing `tasks/.config.toml`. Never resolve a
  registered root for this case; in a worktree it points at the main checkout.
- `--project <prefix>`: resolve a selected id with `tasks --pretty root <id>`, or parse
  `.root` from the default JSON object.

Foreign ids in an unscoped batch, or no local project for an unscoped pass, require a
corrected invocation before any write.

From the eligible rows, choose the largest coherent group sharing a parent, source, tag,
or subsystem; break ties by oldest creation. Take roughly 3–5. If no pair exists, take
the oldest three singletons. A smaller pool yields a smaller pass. An empty pool gets a
short result and no writes.

## 2. Gather evidence

Run `tasks -C <root> show <id>` for each provisional member. Save `task.updated`; also
read its body, links, notes, top-level `claim`, and relationships. Read each source,
parent and relevant `tree`, named code and specs, useful history (`git log -S` or
`--grep`), open duplicate candidates, and any existing brief or attached draft spec.
Keep every read in the fixed checkout.

For a mindful source, confirm the local CLI form, strip only the optional `mindful:`
wrapper, and pass the full typed reference to `mindful --json show`:
`mindful:thought:<uid>` becomes `thought:<uid>`; an already typed reference stays intact.
Keep the task's stored source unchanged. Try this canonical reference before calling
context unavailable. A failed lookup is not evidence that the source is missing or
inaccessible: record command/tool failures separately from unavailable source evidence.
Name genuinely unavailable context as an unknown; never invent the missing finding.

Confirm the cluster from this evidence. Leave unrelated provisional members untouched.
Before authoring, search for both an existing brief and an attached draft spec so a
rerun updates the existing handoff.

Before creating research, search open research across related goals, including goals
owned by ideas excluded from this batch. Reuse an existing task when its question
overlaps; batch membership does not make the same unanswered question new work.

## 3. Choose one verdict per member

| Verdict | Use when | Required result |
|---|---|---|
| `scoped` | Approach and context are established and correctness has a check | Rewrite the body to say why, done, and where to look; set `todo`, priority, size, complexity, and process by the tasks skill rubrics; decompose into children under that idea if needed |
| `briefed` | Decisions remain that a brief can frame | Keep `idea`; cover it in the handoff and add research or design follow-ups only when needed; make it depend on each follow-up that wakes it (§5). A brief that leaves nothing to wait on is not `briefed`: use `question` or `shelved` |
| `question` | Only the user can resolve it and a brief adds no value | Keep status; collect the questions under one `## Open questions` body heading; write the audit note with `--reviewed --awaiting-user` |
| `shelved` | Worth keeping, but not worth reviewing now | Run `tasks -C <root> shelve <id> "<wake condition>"` |
| `drop` | It landed, its premise is gone, or another task covers it | Keep status; propose the drop with the supporting commit or id |

`scoped` is exceptional. Unknowns do not establish priority, make investigation
automatically low complexity, or justify implementation tasks that disguise unresolved
decisions. Preserve the full source material when shortening bodies.

Choose `--process direct` when the task or reviewed plan settles the outcome,
approach, and verification; choose `--process planned` when written design and plan
reviews are needed. Set it explicitly alongside size and complexity, including on
children. Do not derive it from those fields, parentage, or document presence.
Missing process remains unassessed, and assigning it never makes an idea executable.

When a member's work uses a shared resource the project declares in `[needs]` (an idle
host, the owner's judgement), record it with `--need <name>` on the task you scope or
create. Never invent a name: an undeclared one is refused. Propose the vocabulary entry in
the handoff brief instead.

Every successfully processed member receives exactly one audit note. For `briefed`,
`question` and `drop` it sets the flag in the same write:

    tasks -C <root> note <id> --reviewed "scope: <verdict>; <what changed>[; brief: <path>][; proposal: <text>]"

`question` adds `--awaiting-user` to that command. For a woken or stranded idea, gather
the closed follow-ups' outcomes as evidence (§2) before choosing; the new audit note
re-stamps the review, so edges closed earlier stop counting as new. Whoever answers a
question or declines a proposal records it with `tasks note <id> --no-reviewed
"<answer>"`, which readmits the idea with the answer.

`scoped` and `shelved` change the status first, which clears the flag; their audit note
is a plain `tasks note` (`--reviewed` is refused on a non-idea).

Use `proposal:` only for `drop`. A shelf also retains the separate note written by
`shelve`. Never execute `tasks drop` or add `--parallel`.

### Re-scope a scoped task

Every `todo` or `blocked` member, explicit or drawn from the queue, is a re-scope: re-check the premises its body rests
on against the current code, tools and linked work, then narrow it to what is still open.
A `doing` task belongs to its session and is skipped. Gather evidence as in §2, enough to
test the premises, not to do the task. Choose one verdict:

| Verdict | Use when | Required result |
|---|---|---|
| `rescoped` | Premises hold, or the evidence establishes the replacement for a failed one, and work remains | Rewrite the body in its existing shape (why/done/where, or §5's five fields for research, `Ideas it wakes` intact): keep every fact, mark what is now established so the next session does not re-derive it, and narrow what remains; re-rate size, complexity and process; status unchanged; decompose as for `scoped` |
| `confirmed` | Premises hold and nothing narrows | No body edit; set `--process` if it is missing |
| `briefed` or `question` | A premise failed and a decision remains | `tasks -C <root> edit <id> --status idea` first, then the idea rules above apply, including the `--reviewed` audit note |
| `shelved` | As for an idea | As for an idea; `unshelve` returns it as an idea, so the wake condition says it was scoped |
| `drop` | As for an idea | Status unchanged; proposal in a plain note |

When the evidence answers the whole task (common for research), the verdict is still
`rescoped`: record the finding in the body and narrow the remaining work to recording the
result and running its `Ideas it wakes` notes, so the task closes through `done`.

The audit note for every verdict except `briefed` and `question` is a plain
`tasks -C <root> note <id> "scope: <verdict>; <what changed>[; proposal: <text>]"`
(`--reviewed` is refused on a non-idea), naming the evidence (versions, commits) the
premises were checked against. A re-scoped task's priority changes only with the user's
say.

## 4. Write the handoff

For unresolved cluster decisions, write or update one roughly one-page brief at
`docs/notes/YYYY-MM-DD-<topic>-brief.md` with exactly these sections:

1. **Problem** — the ideas' shared outcome in the reader's terms.
2. **Current behaviour and evidence** — observed behavior, paths, and commits.
3. **Constraints** — existing specs, invariants, and open work.
4. **Alternatives** — two or three supported options and the current lean.
5. **Unanswered questions** — each question and who or what can answer it.
6. **Proposed decomposition** — follow-up task ids and the ideas waiting on each.

Use `docs/specs/` with `Status: draft` only when the handoff already presents one
reviewable design; attach that document through `--spec`. Write at most three briefs or
draft specs in one pass. A brief is not attached as a spec.

Reuse a parent shared by all members. Otherwise create one priority-2 `todo` goal whose
source is the brief path, and parent only previously unparented members beneath it.
Members already owned by different goals retain those parents and are associated through
the handoff and audit note. Preserve every existing source; do not rewrite shared goals
incidentally. Research and design follow-ups are children of the cluster goal.

When the cluster is a standalone effort meant to run beside the project's other work,
create that goal with `--lane` and lead its body with the guidance paragraph: why the
effort exists and its first milestone. A cluster inside an existing lane gets an ordinary
child goal, never a lane.

Reuse an existing goal, handoff, and open research task when its question still stands.
If a new goal would have neither member nor follow-up children, do not create it merely
to hold a document; keep the association in member notes.

## 5. Make follow-ups answerable

A research task title names the outcome. Its body has all five fields:

```markdown
Question: Does the existing rename API require loading the profile first?
Where to start: the rename_profile implementation and its panel caller, linked from the brief.
Bound: Trace that call and reproduce one rename of an unloaded profile; no new UI implementation.
Expected result: Record the observed behavior and a recommendation on this task and in the brief.
Ideas it wakes: On completion, run `tasks note <id> --no-reviewed "<finding>"` on each named waiting idea, before this result's branch merges.
```

Replace the example with real paths and waiting ids. Set priority, size, complexity,
and process from the tasks skill rubrics; investigation can be mid or high. File a
high-complexity `Design <topic> from the brief` task only when unresolved questions require design, and make it
depend on prerequisite research. One measurement question does not also need a design
task.

In the same pass that files or reuses a follow-up, record each wait as an edge: run
`tasks -C <root> dep <idea> --on <follow-up>` for every idea its `Ideas it wakes` names.
The edge puts the wait in the record (`show`'s `depends_on`) and keeps the block when
the idea is later promoted to `todo`. When the follow-up closes, the edge readmits the idea by itself; the wake note still carries the finding to it.

## 6. Revalidate, write, and record

Immediately before each member's first write, run `tasks -C <root> show <id>` again.
Compare top-level `claim.live`, `task.status`, and `task.updated` with the first show.
Skip the member without any write or audit note if it changed, has a live claim, or is
no longer in the status it had at the first show (an idea, or a `todo` or `blocked`
task, a re-scope). Documents may name skipped members only as related context; do
not say they were processed. If every member is skipped, create no goal or document.

Write task records only through the CLI and documents through explicit paths under the
fixed root. Preserve the exact successful operations when a later command fails, report
partial work honestly, and never report skipped or failed writes as complete. The check
is not a lock; propagate a CLI race failure. There is no per-member approval checkpoint.

## 7. Summarize and rerun

Report the cluster and why, then one line per selected member with its verdict or skip
reason (a re-scoped member also names what is now established), followed by handoff
paths. Then include only nonempty groups for drops (id and supporting commit/id),
questions (id and text), and shelves (id and wake condition).

When later research completes, its completion must add a finding note with `--no-reviewed` to every waiting
idea named by `Ideas it wakes`, before this result's branch merges, so default selection
can reconsider them. On an explicit rerun, update the existing handoff, goal, and research tasks; do not
replace sources or create duplicates merely because an existing goal was outside the
selected batch. If unchanged evidence warrants no artifact edit, report that plainly.
