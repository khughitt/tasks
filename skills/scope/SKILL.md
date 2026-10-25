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

Fix one task-command root before member reads, and use `tasks -C <root> ...` for
every later task command:

- Unscoped: run `tasks where` to discover the project, even without a config in code.
  For `kind: "in-tree"`, use `.project.root`, including a project nested below the
  git top level. For `detached` or `linked` invoked in its code worktree, use that
  worktree's top level (from `git rev-parse --show-toplevel`); otherwise use
  `.project.root`. Keeping the code worktree as the command context preserves
  detached document fallback there.
- `--project <prefix>`: resolve a selected id with `tasks --pretty root <id>`, or parse
  `.root` from the default JSON object; then run `tasks -C <root> where`.

Keep `.project.records` and `.project.checkout` from `where` as separate locations.
Read code and git history in the selected project's `checkout`. For an unscoped pass
in a detached or linked project's code worktree, use the chosen `<root>` instead.
Read working documents under `records`, using `tasks doc` for linked
references; detached projects also resolve retained documents in the code tree.
Write new handoffs under `records`. It is already `<records-root>/<prefix>` for a
detached project; do not append the prefix again.

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
Keep task reads in the fixed root, code evidence in the chosen code checkout, and
document reads at their resolved paths.

For a mindful source, confirm the local CLI form, strip only the optional `mindful:`
wrapper, and pass the full typed reference to `mindful --json show`:
`mindful:thought:<uid>` becomes `thought:<uid>`; an already typed reference stays intact.
Keep the task's stored source unchanged. Try this canonical reference before calling
context unavailable. A failed lookup is not evidence that the source is missing or
inaccessible: record command/tool failures separately from unavailable source evidence.
Name genuinely unavailable context as an unknown; never invent the missing finding.

### Find the originating ask

Follow each member back to the ask that led to it, and record the outcome. Read its
origin by the README's [reading rule](../../README.md#origin-whose-ask-a-task-is) first.
Retrieval has two stages: the filing event always comes first, and an older request,
when the record names one, is found only from the filing event's evidence, never
searched for directly by the record's id or time.

**Stage 1: the filing event, in the filing session.**

1. From `show`, take the occurrence notes and pick the occurrence wanted, usually the
   creation (the first note whose text is `created` and whose `at` equals `created`);
   for feedback, each `feedback from` recurrence in turn.
2. Take that occurrence's filing session from its stamp (`harness_session`), and for a
   Codex worker also `root_session`. With no stamp, the outcome is unavailable.
3. Locate that transcript with the `session-logs` skill: live stores, archived stores
   (`archived_sessions`) and other harness homes on this host. Use hq-c411df's reader
   once obs-d98663 ships.
4. Bind the occurrence to its call. Candidates are the `tasks add` or `tasks feedback`
   calls in that session whose arguments fit the occurrence (title or summary, flags)
   and whose time fits the note's `at`. A `show` or any other mention of the id is not
   a candidate. What a candidate's result shows decides how strongly it binds:
   - **Full result:** JSON output naming the id and an action; `feedback --pretty`'s
     `<action> <id>` is full except for `at`. A `reused` action excludes the candidate;
     a matching action (`created` for a creation, `recurred` for a recurrence) keeps it.
   - **Id-only result:** `add --pretty` prints only the id, and on reuse adds the stderr
     warning "<id> already carries this source and title; reused it, wrote nothing".
     Settle the action from the rest of the call. The reuse warning is in the captured
     result: excluded as a reuse. The call has no `--source`: a creation, since only a
     sourced add can reuse. A sourced call without that warning, where the transcript
     captured stderr: a creation. Otherwise it is unsettled: still a candidate, but
     either a creation or a reuse.
   - **Unreadable result:** a candidate on arguments and time alone.

   The outcome:
   - **matched:** exactly one candidate remains, and its result is full and either
     carries `at` equal to the note's `at` or is the only full-result candidate whose
     time and arguments fit this occurrence.
   - **inferred:** exactly one candidate remains, and it binds without a full result: an
     id-only result whose action the rest of the call settled, or an unreadable result
     whose time and command text match. This covers a created title that differs from
     the command text.
   - **ambiguous:** more than one candidate remains, or the only one left has an
     unsettled action. An id-only sourced add without captured stderr is ambiguous
     between creation and reuse even when it is the session's only candidate; beside
     another call with the same source and title, both are listed. So are two
     recurrences in one session with equal summaries in the same second (in JSON too:
     `at` has whole-second precision), or concurrent subagents under one Claude session.
     List the candidates and any unsettled action; never pick one.
   - **unavailable:** no candidate remains although the transcript was read. Record what
     was searched.
5. Read the request, the action, the tool result and any later correction around the
   bound call, not only the nearest human turn. A human turn can contain pasted agent
   text, so quote the user's own words apart from pasted text, and say when they cannot
   be told apart.

**Stage 2: an older request, when the occurrence names `origin_session`.**

6. From the bound filing call, collect what points into the older session: transcripts
   read, the user's words quoted, the record's body and references. Then open the older
   session and find the request those point to: **matched** when the filing session's
   evidence names a specific location in it (such as a `session-logs` read that printed
   the turn), **inferred** when the request is found by content, **ambiguous** when
   several turns fit. The record's id and the filing time are not anchors here; the
   request predates both.
7. When the older session cannot be read, the older request is unavailable. The filing
   event's outcome stands on its own.

Record one outcome per stage, with its evidence, in the member's audit note (§3):
matched, inferred, or ambiguous with its candidates; unavailable (no stamp; the session
is not in this host's stores, which can mean another host or a pruned store; or the
transcript was read and no candidate remains); or lookup failed (a tool error, recorded
separately from unavailable evidence, as for a source above). Records do not name the
filing host, so a session filed on another host is a stated limitation, not a search.

When the evidence settles whose ask it is and the field does not already say so, plan a
correction: `--origin <user|agent|mixed>`, or `--no-origin` to withdraw a judgment so the
occurrence labels speak again. Evidence that the user did not ask means `agent`; evidence
of a user ask means `user` or `mixed`. Write nothing here: §6 applies it after
revalidation, and the stamped audit note's `origin:` segment cites the evidence. Notes
keep their labels as history.

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

    tasks -C <root> note <id> --reviewed --stamp "scope: <verdict>; <what changed>[; origin: <stage outcomes>][; brief: <path>][; proposal: <text>]"

`question` adds `--awaiting-user` to that command. Write every scope and brief note with
`--stamp`, so a later agent can find the session behind a requirement edit. For a woken or stranded idea, gather
the closed follow-ups' outcomes as evidence (§2) before choosing; the new audit note
re-stamps the review, so edges closed earlier stop counting as new. Whoever answers a
question or declines a proposal records it with `tasks note <id> --no-reviewed
"<answer>"`, which readmits the idea with the answer.

`scoped` and `shelved` change the status first, which clears the flag; their audit note
is a `tasks note --stamp` without `--reviewed` (refused on a non-idea).

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

The audit note for every verdict except `briefed` and `question` is
`tasks -C <root> note <id> --stamp "scope: <verdict>; <what changed>[; proposal: <text>]"`
without `--reviewed` (refused on a non-idea), naming the evidence (versions, commits) the
premises were checked against. A re-scoped task's priority changes only with the user's
say.

## 4. Write the handoff

For unresolved cluster decisions, write or update one roughly one-page brief at
`<records>/docs/notes/YYYY-MM-DD-<topic>-brief.md` with exactly these sections:

1. **Problem** — the ideas' shared outcome in the reader's terms.
2. **Current behaviour and evidence** — observed behavior, paths, and commits.
3. **Constraints** — existing specs, invariants, and open work.
4. **Alternatives** — two or three supported options and the current lean.
5. **Unanswered questions** — each question and who or what can answer it.
6. **Proposed decomposition** — follow-up task ids and the ideas waiting on each.

Use `<records>/docs/specs/` with `Status: draft` only when the handoff already presents one
reviewable design; attach that document through `--spec`. Write at most three briefs or
draft specs in one pass. A brief is not attached as a spec. Add a brief the pass
writes as a reference on each task it scopes, including follow-ups that use it:

    tasks -C <root> edit <id> --ref 'brief (input, required, kind=brief): docs/notes/YYYY-MM-DD-<topic>-brief.md — context and questions for this work'

The stored reference stays `docs/notes/...`, relative to `records`, even when the
file is outside the code checkout. Commit it in the repository holding the records.
Reuse the existing handle on reruns; mark `required` only when the brief must be read
first. In a body the pass rewrites, cite `[ref:brief]` and say why it matters.

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

Revalidation comes once, before all of a member's writes, so its own writes cannot trip
the comparison. An origin correction planned in §2 is one of them:
`tasks -C <root> edit <id> --origin <value>` (or `--no-origin`), in the same edit as the
verdict's other field changes when it has them, and always before the audit note, whose
`origin:` segment names the stage outcomes and the evidence for the correction.

Write task records only through the CLI and handoffs through explicit paths under
`records`. Preserve the exact successful operations when a later command fails, report
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
