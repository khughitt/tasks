---
name: scope
description: Use when asked to scope captured ideas, review an idea backlog for next actions, or run /scope.
---

# scope

Turn one bounded idea cluster into evidence-backed next actions and handoffs. Invoke deliberately:

    /scope [<id>... | --tag <tag>] [--project <prefix>]

Do not ask the user to choose a cluster. Do not `tasks start` selected ideas: this is a
review pass, and each idea must remain unclaimed until its verdict is written.

## 1. Select and fix the root

Explicit ids are the exact batch. Otherwise discover candidates with:

    tasks list --status idea [--tag <tag>] [--project <prefix>]

List rows omit notes. Run `show` before deciding eligibility: exclude an idea when its
latest note starts with `scope:`, or when that latest note contains a pending
`proposal:`. A later note readmits it. Existing briefs, goals, or research do not by
themselves exclude an idea whose latest note readmits it. An explicit id bypasses only
this prior-note exclusion; it must still be an unclaimed idea at revalidation, and the
pass still has the three-document ceiling.

Fix one root before member reads, and use `tasks -C <root> ...` for every later task
command and that root for every evidence read and document write:

- Unscoped: use the nearest ancestor containing `tasks/.config.toml`. Never resolve a
  registered root for this case; in a worktree it points at the main checkout.
- `--project <prefix>`: resolve a selected id with `tasks --pretty root <id>`, or parse
  `.root` from the default JSON object.

Foreign ids in an unscoped batch, or no local project for an unscoped pass, require a
corrected invocation before any write.

From eligible ideas, choose the largest coherent group sharing a parent, source, tag,
or subsystem; break ties by oldest creation. Take roughly 3–5. If no pair exists, take
the oldest three singletons. A smaller pool yields a smaller pass. An empty pool gets a
short result and no writes.

## 2. Gather evidence

Run `tasks -C <root> show <id>` for each provisional member. Save `task.updated`; also
read its body, links, notes, top-level `claim`, and relationships. Read each source,
parent and relevant `tree`, named code and specs, useful history (`git log -S` or
`--grep`), open duplicate candidates, and any existing brief or attached draft spec.
Keep every read in the fixed checkout.

For a mindful source, first confirm the local CLI form, then use `mindful --json show`.
Name unavailable context as an unknown; never invent the missing finding.

Confirm the cluster from this evidence. Leave unrelated provisional members untouched.
Before authoring, search for both an existing brief and an attached draft spec so a
rerun updates the existing handoff.

Before creating research, search open research across related goals, including goals
owned by ideas excluded from this batch. Reuse an existing task when its question
overlaps; batch membership does not make the same unanswered question new work.

## 3. Choose one verdict per member

| Verdict | Use when | Required result |
|---|---|---|
| `scoped` | Approach and context are established and correctness has a check | Rewrite the body to say why, done, and where to look; set `todo`, priority, size, and complexity by the existing rubric; decompose into children under that idea if needed |
| `briefed` | Decisions remain that a brief can frame | Keep `idea`; cover it in the handoff and add research or design follow-ups only when needed |
| `question` | Only the user can resolve it and a brief adds no value | Keep status; collect the questions under one `## Open questions` body heading |
| `shelved` | Worth keeping, but not worth reviewing now | Run `tasks -C <root> shelve <id> "<wake condition>"` |
| `drop` | It landed, its premise is gone, or another task covers it | Keep status; propose the drop with the supporting commit or id |

`scoped` is exceptional. Unknowns do not establish priority, make investigation
automatically low complexity, or justify implementation tasks that disguise unresolved
decisions. Preserve the full source material when shortening bodies.

Every successfully processed member receives exactly one audit note:

    scope: <verdict>; <what changed>[; brief: <path>][; proposal: <text>]

Use `proposal:` only for `drop`. A shelf also retains the separate note written by
`shelve`. Never execute `tasks drop` or add `--parallel`.

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
Ideas it wakes: On completion, run tasks note on each named waiting idea with the finding, in the same commit as this result.
```

Replace the example with real paths and waiting ids. Set priority, size, and complexity
from the rubric; investigation can be mid or high. File a high-complexity `Design
<topic> from the brief` task only when unresolved questions require design, and make it
depend on prerequisite research. One measurement question does not also need a design
task.

## 6. Revalidate, write, and record

Immediately before each member's first write, run `tasks -C <root> show <id>` again.
Compare top-level `claim.live`, `task.status`, and `task.updated` with the first show.
Skip the member without any write or audit note if it changed, has a live claim, or is
not an idea. Documents may name skipped members only as related context; do not say they
were processed. If every member is skipped, create no goal or document.

Write task records only through the CLI and documents through explicit paths under the
fixed root. Preserve the exact successful operations when a later command fails, report
partial work honestly, and never report skipped or failed writes as complete. The check
is not a lock; propagate a CLI race failure. There is no per-member approval checkpoint.

## 7. Summarize and rerun

Report the cluster and why, then one line per selected member with its verdict or skip
reason, followed by handoff paths. Then include only nonempty groups for drops (id and
supporting commit/id), questions (id and text), and shelves (id and wake condition).

When later research completes, its completion must add a finding note to every waiting
idea named by `Ideas it wakes`, in the same commit, so default selection can reconsider
them. On an explicit rerun, update the existing handoff, goal, and research tasks; do not
replace sources or create duplicates merely because an existing goal was outside the
selected batch. If unchanged evidence warrants no artifact edit, report that plainly.
