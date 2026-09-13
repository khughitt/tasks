# Scope skill validation

Date: 2026-09-13

## Method

The controlled packet used nine fixed `demo` ideas, immutable evidence excerpts, an
unscoped `.worktrees/review` checkout, a registered `demo-main` checkout, and a changed
pre-write snapshot for `demo-a10005`. It tested default selection, a `--project demo`
variation, and an explicit rerun of `demo-a10007` and `demo-a10009`. The approved plan's
[Task 1](../plans/2026-09-13-scope-skill.md#task-1-write-and-validate-the-scope-skill-and-its-discovery-paths)
retains the reproducible inputs and acceptance checks. Exact transcripts are temporary
ignored trial scratch; this note retains the findings and representative quotations.

Three fresh agents ran the packet without the skill, followed by three fresh agents with
the first candidate. The first three green trials disagreed only on cross-goal research
reuse, so the wording received one focused clarification and one focused follow-up trial.
Each wording revision also receives one actual CLI packet in a fresh isolated fixture.

## Baseline: 3 trials

All baseline trials missed the contract in the same areas:

- They excluded `demo-a10009` because a brief and research existed, although its later
  finding note made it eligible again.
- They invented verdicts such as `scope: ready`, `scope: decision`, and
  `scope: already covered`. Some used `proposal:` for unresolved decisions instead of
  reserving it for a supported drop.
- They omitted the required six-section handoff, cluster goal, or answerable research
  tasks. One invented behavior: `Done means edits make the indicator visible and
  returning to the saved state clears it.`

The baselines did consistently preserve different existing parents, avoid rerun
duplication, and skip `demo-a10005` after its status, claim, and timestamp changed. Two
of three kept all unscoped commands under the fixed worktree root; one omitted that
discipline on initial reads and writes.

## Initial green: 3 trials

All three candidates correctly:

- admitted the later-note `demo-a10009`, selected the five-member profile cluster, and
  skipped the changed `demo-a10005` without a write or audit note;
- fixed unscoped work to `.worktrees/review` and `--project demo` work to `demo-main`;
- preserved the different parents and sources of `demo-a10001` and `demo-a10002` while
  parenting only unparented members under a new goal;
- used canonical verdicts and audit notes, kept unresolved alternatives as ideas, wrote
  all six brief sections, and shaped bounded research with all five required fields;
- reused existing artifacts on the explicit rerun of `demo-a10007` and `demo-a10009`.

The only disagreement was initial-pass research reuse. One trial duplicated the reload
question already owned by `demo-c10002`, one reused it, and one filed only the distinct
rename investigation. The candidate now explicitly searches open research across
related goals, including ideas excluded from the selected batch, before creating a task.

One focused fresh trial then created only the distinct unloaded-rename research. It
reused `demo-c10002` and `demo-c10001` on rerun, maintained the fixed roots and existing
parents, and skipped the changed member without writing it. No further wording change
was needed.

## Actual CLI packets

The first wording revision ran once against a real ignored scratch project with isolated
`XDG_CONFIG_HOME` and `XDG_STATE_HOME`. The unscoped pass wrote only its review worktree;
the registered main checkout stayed clean. `tasks root <id>` returned a JSON object whose
`.root` pointed to main, while all selected reads and writes remained fixed to review.
The list rows omitted notes, and eligibility and verdict evidence came from `show`.

The actual records passed the three remaining verdict branches:

- `drop`: the idea stayed `idea`, with one `scope: drop` note and an ancestor commit as
  evidence; no drop command ran;
- `shelved`: the task acquired the native shelf note plus exactly one scope audit note;
- `question`: the idea stayed `idea`, gained one `## Open questions` section, and received
  exactly one scope audit note.

`tasks check` passed and no brief or spec was forced for these three singletons. A first
launcher attempt omitted Cargo's bin directory from its isolated `PATH` and failed before
the CLI ran or wrote anything; the corrected environment completed normally.

The clarified wording received a second fresh CLI packet with the same three verdicts.
The inspected records again had one drop audit with the idea retained, one native shelf
note plus one shelf audit, and one question audit with the question in the body. The main
checkout stayed clean; root JSON pointed to main, list rows lacked notes, all operations
stayed fixed to the review root, and the final check reported zero errors and warnings.
Its first writes warned that a retained sibling review worktree held newer copies, an
expected fixture artifact that did not change the chosen root or final records. One note
rendered an escaped apostrophe; that output polish did not affect the verdict or audit
contract.

## Remaining acceptance

Task 1 completed three baseline trials, three initial green trials, one focused green
trial prompted by their sole disagreement, and two actual CLI packets, one per wording
revision. Task 2 still runs the skill against the real Prism cluster and tests its
explicit rerun; the controlled trials do not substitute for that acceptance.
